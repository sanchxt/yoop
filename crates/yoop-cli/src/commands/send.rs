//! Send command implementation (trusted devices).
//!
//! Sends files directly to a trusted device without requiring a share code.
//! Authentication is done via Ed25519 signatures.

use std::io::{self, Write};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::watch;
use uuid::Uuid;

use yoop_core::config::{CompressionMode, TrustLevel};
use yoop_core::crypto::DeviceIdentity;
use yoop_core::file::format_size;
use yoop_core::history::{
    HistoryFileEntry, HistoryStore, TransferDirection, TransferHistoryEntry,
    TransferState as HistoryState,
};
use yoop_core::transfer::{TransferConfig, TransferProgress, TransferState, TrustedSendSession};
use yoop_core::trust::TrustStore;

use super::SendArgs;

/// Run the send command.
#[allow(clippy::too_many_lines)]
pub async fn run(args: SendArgs) -> Result<()> {
    let global_config = super::load_config();

    let trust_store = TrustStore::load().context("Failed to load trust store")?;

    let trusted_device = trust_store
        .find_by_name(&args.device)
        .ok_or_else(|| anyhow::anyhow!("Device '{}' not found in trust store", args.device))?
        .clone();

    let identity = DeviceIdentity::load_or_generate().context("Failed to load device identity")?;

    let compression = if args.no_compress {
        CompressionMode::Never
    } else if args.compress {
        CompressionMode::Always
    } else {
        global_config.transfer.compression
    };
    let compression_level = args
        .compression_level
        .unwrap_or(global_config.transfer.compression_level);

    let config = TransferConfig {
        compression,
        compression_level,
        chunk_size: global_config.transfer.chunk_size,
        parallel_streams: global_config.transfer.parallel_chunks,
        verify_checksums: global_config.transfer.verify_checksum,
        discovery_port: global_config.network.port,
        ..Default::default()
    };

    let mut session =
        TrustedSendSession::new(trusted_device.clone(), identity, &args.paths, config)
            .await
            .context("Failed to create send session")?;

    if !args.quiet {
        println!();
        println!("Yoop v{}", yoop_core::VERSION);
        println!("{}", "-".repeat(37));
        println!();
    }

    let files = session.files().to_vec();
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    display_send_info(&files, total_size, &trusted_device.device_name, &args);

    let discovered_addr = find_device(&mut session, &trusted_device, &args).await?;

    if trusted_device.trust_level == TrustLevel::AskEachTime && !args.quiet {
        print!(
            "  Send {} files ({}) to {}? [Y/n] ",
            files.len(),
            format_size(total_size),
            trusted_device.device_name
        );
        let _ = io::stdout().flush();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if !input.is_empty() && input != "y" && input != "yes" {
            println!();
            println!("  Transfer cancelled.");
            return Ok(());
        }
        println!();
    }

    let progress_rx = session.progress();
    let start_time = Instant::now();

    let progress_handle = if args.quiet {
        None
    } else {
        Some(tokio::spawn(display_progress(progress_rx)))
    };

    let result = session.send().await;

    if let Some(handle) = progress_handle {
        let _ = handle.await;
    }

    let elapsed = start_time.elapsed();

    handle_transfer_result(
        result,
        &trusted_device.device_name,
        trusted_device.device_id,
        discovered_addr,
        &files,
        total_size,
        elapsed.as_secs(),
        &args,
    )
}

/// Display send information (files and device).
fn display_send_info(
    files: &[yoop_core::file::FileMetadata],
    total_size: u64,
    device_name: &str,
    args: &SendArgs,
) {
    if args.quiet {
        return;
    }

    println!(
        "  Sending {} items ({}) to {}",
        files.len(),
        format_size(total_size),
        device_name
    );
    println!();
    for file in files {
        println!("  {} {}", file_icon(file), file.file_name());
    }
    println!();
}

/// Find the target device using stored address first, then discovery as fallback.
///
/// Connection priority:
/// 1. Try stored address if available (with short timeout TCP probe)
/// 2. Fall back to network discovery if stored address is unreachable
async fn find_device(
    session: &mut TrustedSendSession,
    trusted_device: &yoop_core::trust::TrustedDevice,
    args: &SendArgs,
) -> Result<SocketAddr> {
    let stored_addr = trusted_device
        .address()
        .map(|(ip, port)| SocketAddr::new(ip, port));

    if let Some(addr) = stored_addr {
        if !args.quiet {
            print!("  Trying stored address {}...", addr);
            let _ = io::stdout().flush();
        }

        match tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(addr))
            .await
        {
            Ok(Ok(stream)) => {
                drop(stream);
                if !args.quiet {
                    println!(" reachable");
                    println!();
                }
                session.set_direct_address(addr);
                return Ok(addr);
            }
            Ok(Err(e)) => {
                tracing::debug!("Stored address {} unreachable: {}", addr, e);
                if !args.quiet {
                    println!(" unreachable");
                    print!(
                        "  Searching for {} on the network...",
                        trusted_device.device_name
                    );
                    let _ = io::stdout().flush();
                }
            }
            Err(_) => {
                tracing::debug!("Stored address {} connection timed out", addr);
                if !args.quiet {
                    println!(" timed out");
                    print!(
                        "  Searching for {} on the network...",
                        trusted_device.device_name
                    );
                    let _ = io::stdout().flush();
                }
            }
        }
    } else if !args.quiet {
        print!("  Searching for {}...", trusted_device.device_name);
        let _ = io::stdout().flush();
    }

    let discovery_timeout = Duration::from_secs(30);
    let start_discovery = Instant::now();

    let discovered = loop {
        match tokio::time::timeout(Duration::from_secs(5), session.discover()).await {
            Ok(Ok(device)) => break device,
            Ok(Err(e)) => {
                if start_discovery.elapsed() > discovery_timeout {
                    if !args.quiet {
                        println!();
                    }
                    return Err(anyhow::anyhow!(
                        "Could not find '{}' on the network: {}",
                        trusted_device.device_name,
                        e
                    ));
                }
            }
            Err(_) => {
                if start_discovery.elapsed() > discovery_timeout {
                    if !args.quiet {
                        println!();
                    }
                    return Err(anyhow::anyhow!(
                        "Timed out searching for '{}'",
                        trusted_device.device_name
                    ));
                }
            }
        }
    };

    let discovered_addr = discovered.source;

    if !args.quiet {
        println!(" found at {}", discovered_addr);
        println!();
    }

    Ok(discovered_addr)
}

/// Handle the result of the transfer and update history.
#[allow(clippy::too_many_arguments)]
fn handle_transfer_result(
    result: yoop_core::error::Result<()>,
    device_name: &str,
    device_id: Uuid,
    discovered_addr: std::net::SocketAddr,
    files: &[yoop_core::file::FileMetadata],
    total_size: u64,
    duration_secs: u64,
    args: &SendArgs,
) -> Result<()> {
    match result {
        Ok(()) => {
            record_history(
                device_name,
                files,
                total_size,
                duration_secs,
                HistoryState::Completed,
                None,
            );

            if let Ok(mut store) = TrustStore::load() {
                if store.find_by_id(&device_id).is_some() {
                    let _ = store.update_address(
                        &device_id,
                        discovered_addr.ip(),
                        discovered_addr.port(),
                    );
                }
            }

            if !args.quiet {
                println!();
                println!("  Transfer complete!");
                println!();
            }
            Ok(())
        }
        Err(e) => {
            record_history(
                device_name,
                files,
                total_size,
                duration_secs,
                HistoryState::Failed,
                Some(e.to_string()),
            );

            if !args.quiet {
                eprintln!();
                eprintln!("  Transfer failed: {}", e);
                eprintln!();
            }
            Err(e.into())
        }
    }
}

/// Record the transfer to history.
fn record_history(
    device_name: &str,
    files: &[yoop_core::file::FileMetadata],
    total_bytes: u64,
    duration_secs: u64,
    state: HistoryState,
    error: Option<String>,
) {
    let history_files: Vec<HistoryFileEntry> = files
        .iter()
        .map(|f| HistoryFileEntry {
            name: f.file_name().to_string(),
            size: f.size,
            success: state == HistoryState::Completed,
        })
        .collect();

    let mut entry = TransferHistoryEntry::new(
        TransferDirection::Sent,
        device_name.to_string(),
        format!("trusted:{}", device_name),
    )
    .with_files(history_files)
    .with_stats(total_bytes, duration_secs)
    .with_state(state);

    if let Some(err_msg) = error {
        entry = entry.with_error(err_msg);
    }

    if let Ok(mut store) = HistoryStore::load() {
        if let Err(e) = store.add(entry) {
            tracing::warn!("Failed to record history: {}", e);
        }
    }
}

fn file_icon(file: &yoop_core::file::FileMetadata) -> &'static str {
    if file.is_symlink {
        "->"
    } else if let Some(ref mime) = file.mime_type {
        if mime.starts_with("image/") {
            "[img]"
        } else if mime.starts_with("video/") {
            "[vid]"
        } else if mime.starts_with("audio/") {
            "[aud]"
        } else if mime.starts_with("text/") {
            "[txt]"
        } else {
            "[file]"
        }
    } else {
        "[file]"
    }
}

async fn display_progress(mut rx: watch::Receiver<TransferProgress>) {
    let mut last_state = TransferState::Preparing;

    loop {
        let timeout = tokio::time::timeout(Duration::from_secs(1), rx.changed()).await;

        let progress = rx.borrow().clone();

        if progress.state != last_state {
            last_state = progress.state;

            match progress.state {
                TransferState::Connected => {
                    println!("  Connected to receiver!");
                }
                TransferState::Transferring => {
                    println!("  Starting transfer...");
                }
                TransferState::Completed => {
                    break;
                }
                TransferState::Cancelled => {
                    println!("  Transfer cancelled.");
                    break;
                }
                TransferState::Failed => {
                    println!("  Transfer failed.");
                    break;
                }
                TransferState::Preparing | TransferState::Waiting => {}
            }
        }

        if progress.state == TransferState::Transferring {
            let pct = progress.percentage();
            let speed = format_size(progress.speed_bps);
            let eta = progress
                .eta
                .map_or_else(|| "--".to_string(), |d| format!("{}s", d.as_secs()));

            print!(
                "\r  [{:>6.2}%] {} - {}/s - ETA: {}    ",
                pct, progress.current_file_name, speed, eta
            );
            let _ = io::stdout().flush();
        }

        if timeout.is_err() {
            continue;
        }
        if timeout.unwrap().is_err() {
            break;
        }
    }

    println!();
}
