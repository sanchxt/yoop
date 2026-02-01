//! Share command implementation.

use std::io::{self, Write};
use std::time::Instant;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;
use uuid::Uuid;

use yoop_core::config::{CompressionMode, TrustLevel};
use yoop_core::file::format_size;
use yoop_core::history::{
    HistoryFileEntry, HistoryStore, TransferDirection, TransferHistoryEntry,
    TransferState as HistoryState,
};
use yoop_core::transfer::{ShareSession, TransferConfig, TransferProgress, TransferState};
use yoop_core::trust::{TrustStore, TrustedDevice};

use super::ShareArgs;
use crate::tui::session::{FileEntry, ProgressEntry, SessionEntry, SessionStateFile};
use crate::ui::{format_remaining, parse_duration, CodeBox};

/// Run the share command.
pub async fn run(args: ShareArgs) -> Result<()> {
    let global_config = super::load_config();

    super::spawn_update_check();

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

    let mut session = ShareSession::new(&args.paths, config).await?;

    if !args.quiet {
        println!();
        println!("Yoop v{}", yoop_core::VERSION);
        println!("{}", "-".repeat(37));
        println!();
    }

    let files = session.files().to_vec();
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    let code = session.code().to_string();

    display_share_info(&files, total_size, &code, &args, &global_config)?;

    let progress_rx = session.progress();
    let expire_duration =
        parse_duration(&args.expire).or(Some(global_config.general.default_expire));
    let start_time = Instant::now();

    let session_id = Uuid::new_v4();
    let expires_at = expire_duration
        .map(|d| chrono::Utc::now() + chrono::Duration::from_std(d).unwrap_or_default());
    let session_entry = SessionEntry {
        id: session_id,
        session_type: "share".to_string(),
        code: Some(code.clone()),
        pid: std::process::id(),
        started_at: chrono::Utc::now(),
        expires_at,
        files: files
            .iter()
            .map(|f| FileEntry {
                name: f.file_name().to_string(),
                size: f.size,
                transferred: 0,
                status: "pending".to_string(),
            })
            .collect(),
        peer: None,
        progress: ProgressEntry::default(),
    };

    let mut state_file = SessionStateFile::load_or_create();
    state_file.add_session(session_entry);

    let progress_handle = if !args.quiet && !args.json {
        Some(tokio::spawn(display_progress(
            progress_rx.clone(),
            expire_duration,
            start_time,
            session_id,
        )))
    } else {
        Some(tokio::spawn(update_session_state(
            progress_rx.clone(),
            session_id,
        )))
    };

    let result = session.wait().await;

    let mut state_file = SessionStateFile::load_or_create();
    state_file.remove_session(session_id);

    if let Some(handle) = progress_handle {
        let _ = handle.await;
    }

    let elapsed = start_time.elapsed();

    let receiver_name = session.receiver_name().map(String::from);
    let receiver_device_id = session.receiver_device_id();
    let receiver_public_key = session.receiver_public_key().map(String::from);

    handle_transfer_result(
        result,
        &code,
        &files,
        total_size,
        elapsed.as_secs(),
        &args,
        receiver_name.as_deref(),
        receiver_device_id,
        receiver_public_key.as_deref(),
        global_config.trust.auto_prompt,
    )
    .await
}

/// Display share information (files and code).
fn display_share_info(
    files: &[yoop_core::file::FileMetadata],
    total_size: u64,
    code: &str,
    args: &ShareArgs,
    global_config: &yoop_core::config::Config,
) -> Result<()> {
    let total_files = files.len();

    if !args.quiet {
        println!(
            "  Sharing {} items ({})",
            total_files,
            format_size(total_size)
        );
        println!();
        for file in files {
            println!("  {} {}", file_icon(file), file.file_name());
        }
        println!();
    }

    if args.json {
        let output = serde_json::json!({
            "code": code,
            "files": files.iter().map(|f| serde_json::json!({
                "name": f.file_name(),
                "size": f.size,
                "path": f.relative_path.display().to_string(),
            })).collect::<Vec<_>>(),
            "total_size": total_size,
            "expire": args.expire,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !args.quiet {
        CodeBox::new(code)
            .with_expire(&args.expire)
            .with_qr(global_config.ui.show_qr)
            .display();
        println!();
    }

    Ok(())
}

/// Handle the result of the transfer and update history.
#[allow(clippy::too_many_arguments)]
async fn handle_transfer_result(
    result: yoop_core::error::Result<()>,
    code: &str,
    files: &[yoop_core::file::FileMetadata],
    total_size: u64,
    duration_secs: u64,
    args: &ShareArgs,
    receiver_name: Option<&str>,
    receiver_device_id: Option<Uuid>,
    receiver_public_key: Option<&str>,
    trust_auto_prompt: bool,
) -> Result<()> {
    match result {
        Ok(()) => {
            record_history(
                code,
                files,
                total_size,
                duration_secs,
                HistoryState::Completed,
                None,
                receiver_name,
                receiver_device_id,
            );

            if !args.quiet {
                println!();
                println!("  Transfer complete!");
                println!();

                if trust_auto_prompt {
                    if let Some(name) = receiver_name {
                        prompt_trust_device(name, receiver_device_id, receiver_public_key).await;
                    }
                }
            }
            if args.json {
                let output = serde_json::json!({
                    "status": "complete",
                    "code": code,
                    "total_transferred": total_size,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            Ok(())
        }
        Err(e) => {
            record_history(
                code,
                files,
                total_size,
                duration_secs,
                HistoryState::Failed,
                Some(e.to_string()),
                receiver_name,
                receiver_device_id,
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
#[allow(clippy::too_many_arguments)]
fn record_history(
    code: &str,
    files: &[yoop_core::file::FileMetadata],
    total_bytes: u64,
    duration_secs: u64,
    state: HistoryState,
    error: Option<String>,
    receiver_name: Option<&str>,
    receiver_device_id: Option<Uuid>,
) {
    let device_name = receiver_name.unwrap_or("Unknown").to_string();

    let history_files: Vec<HistoryFileEntry> = files
        .iter()
        .map(|f| HistoryFileEntry {
            name: f.file_name().to_string(),
            size: f.size,
            success: state == HistoryState::Completed,
        })
        .collect();

    let mut entry =
        TransferHistoryEntry::new(TransferDirection::Sent, device_name, code.to_string())
            .with_files(history_files)
            .with_stats(total_bytes, duration_secs)
            .with_state(state);

    if let Some(device_id) = receiver_device_id {
        entry = entry.with_device_id(device_id);
    }

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

async fn display_progress(
    mut rx: watch::Receiver<TransferProgress>,
    expire_duration: Option<std::time::Duration>,
    start_time: Instant,
    session_id: Uuid,
) {
    let mut last_state = TransferState::Preparing;
    let mut waiting_printed = false;
    let mut last_update = Instant::now();

    loop {
        let timeout = tokio::time::timeout(std::time::Duration::from_secs(1), rx.changed()).await;

        let progress = rx.borrow().clone();

        if last_update.elapsed() >= std::time::Duration::from_millis(500) {
            update_session_state_once(&progress, session_id);
            last_update = Instant::now();
        }

        if progress.state != last_state {
            if waiting_printed {
                println!();
                waiting_printed = false;
            }
            last_state = progress.state;

            match progress.state {
                TransferState::Connected => {
                    println!("  Receiver connected!");
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

        if progress.state == TransferState::Waiting {
            if let Some(expire) = expire_duration {
                let elapsed = start_time.elapsed();
                let remaining = expire.saturating_sub(elapsed);
                print!(
                    "\r  Waiting for receiver... ({} remaining)   ",
                    format_remaining(remaining)
                );
                let _ = io::stdout().flush();
                waiting_printed = true;
            } else if !waiting_printed {
                print!("\r  Waiting for receiver...   ");
                let _ = io::stdout().flush();
                waiting_printed = true;
            }
        } else if progress.state == TransferState::Transferring {
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

/// Update session state in the background (for quiet mode).
async fn update_session_state(mut rx: watch::Receiver<TransferProgress>, session_id: Uuid) {
    let mut last_update = Instant::now();

    loop {
        let timeout = tokio::time::timeout(std::time::Duration::from_secs(1), rx.changed()).await;
        let progress = rx.borrow().clone();

        if last_update.elapsed() >= std::time::Duration::from_millis(500) {
            update_session_state_once(&progress, session_id);
            last_update = Instant::now();
        }

        match progress.state {
            TransferState::Completed | TransferState::Cancelled | TransferState::Failed => break,
            _ => {}
        }

        if timeout.is_err() {
            continue;
        }
        if timeout.unwrap().is_err() {
            break;
        }
    }
}

/// Update session state file once with current progress.
fn update_session_state_once(progress: &TransferProgress, session_id: Uuid) {
    let mut state_file = SessionStateFile::load_or_create();
    state_file.update_session_progress(
        session_id,
        ProgressEntry {
            transferred: progress.total_bytes_transferred,
            total: progress.total_bytes,
            speed_bps: progress.speed_bps,
        },
    );
}

/// Prompt the user to trust the receiver device after a successful transfer.
async fn prompt_trust_device(
    receiver_name: &str,
    receiver_device_id: Option<Uuid>,
    receiver_public_key: Option<&str>,
) {
    let (Some(device_id), Some(public_key)) = (receiver_device_id, receiver_public_key) else {
        return;
    };

    if let Ok(trust_store) = TrustStore::load() {
        if trust_store.find_by_id(&device_id).is_some() {
            return;
        }
    }

    print!("  Trust \"{}\" for future transfers? [y/N] ", receiver_name);
    if io::stdout().flush().is_err() {
        return;
    }

    let mut input = String::new();
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    if reader.read_line(&mut input).await.is_err() {
        return;
    }
    let input = input.trim().to_lowercase();

    if input != "y" && input != "yes" {
        return;
    }

    println!();
    println!("  Trust level:");
    println!("    (1) Full - auto-accept transfers");
    println!("    (2) Ask each time - confirm before sending");
    print!("  Choose [1]: ");
    if io::stdout().flush().is_err() {
        return;
    }

    let mut level_input = String::new();
    if reader.read_line(&mut level_input).await.is_err() {
        return;
    }
    let level_input = level_input.trim();

    let trust_level = if level_input == "2" {
        TrustLevel::AskEachTime
    } else {
        TrustLevel::Full
    };

    let device = TrustedDevice::new(device_id, receiver_name.to_string(), public_key.to_string())
        .with_trust_level(trust_level);

    match TrustStore::load() {
        Ok(mut store) => {
            if let Err(e) = store.add(device) {
                eprintln!("  Failed to save trust: {}", e);
            } else {
                println!();
                println!("  Device trusted.");
                println!();
            }
        }
        Err(e) => {
            eprintln!("  Failed to load trust store: {}", e);
        }
    }
}
