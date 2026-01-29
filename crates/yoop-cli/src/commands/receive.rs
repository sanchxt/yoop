//! Receive command implementation.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;
use uuid::Uuid;

use yoop_core::config::TrustLevel;
use yoop_core::connection::parse_host_address;
use yoop_core::file::{format_size, FileMetadata};
use yoop_core::history::{
    HistoryFileEntry, HistoryStore, TransferDirection, TransferHistoryEntry,
    TransferState as HistoryState,
};
use yoop_core::preview::PreviewType;
use yoop_core::transfer::{ReceiveSession, TransferConfig, TransferProgress, TransferState};
use yoop_core::trust::{TrustStore, TrustedDevice};

use super::ReceiveArgs;

/// Run the receive command.
#[allow(clippy::too_many_lines)]
pub async fn run(args: ReceiveArgs) -> Result<()> {
    let global_config = super::load_config();

    super::spawn_update_check();

    let (code_str, direct_addr) = resolve_connection_params(&args)?;
    let code = yoop_core::code::ShareCode::parse(&code_str)?;

    if !args.quiet && !args.json {
        println!();
        println!("Yoop v{}", yoop_core::VERSION);
        println!("{}", "-".repeat(37));
        println!();
        if args.device.is_some() {
            println!("  Connecting to trusted device...");
        } else {
            println!("  Searching for code {}...", code.as_str());
        }
        println!();
    }

    if args.json {
        let output = serde_json::json!({
            "status": "searching",
            "code": code.as_str(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    let output_dir = args
        .output
        .or_else(|| global_config.general.default_output.clone())
        .unwrap_or_else(|| PathBuf::from("."));

    let config = TransferConfig {
        chunk_size: global_config.transfer.chunk_size,
        parallel_streams: global_config.transfer.parallel_chunks,
        verify_checksums: global_config.transfer.verify_checksum,
        discovery_port: global_config.network.port,
        ..Default::default()
    };

    let mut session =
        ReceiveSession::connect_with_options(&code, output_dir.clone(), direct_addr, config)
            .await?;

    let (sender_addr, sender_name) = session.sender();
    let sender_name = sender_name.to_string();
    let sender_addr = *sender_addr;
    let sender_device_id = session.sender_device_id();
    let sender_public_key = session.sender_public_key().map(String::from);
    let files = session.files().to_vec();
    let total_files = files.len();
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    if args.json {
        let output = serde_json::json!({
            "status": "connected",
            "sender": {
                "name": &sender_name,
                "address": sender_addr.to_string(),
            },
            "files": files.iter().map(|f| {
                let mut file_json = serde_json::json!({
                    "name": f.file_name(),
                    "size": f.size,
                    "path": f.relative_path.display().to_string(),
                });
                if let Some(ref preview) = f.preview {
                    file_json["preview"] = serde_json::json!({
                        "type": format!("{:?}", preview.preview_type).to_lowercase(),
                        "mime_type": &preview.mime_type,
                    });
                    if let Some(ref meta) = preview.metadata {
                        if let Some((w, h)) = meta.dimensions {
                            file_json["preview"]["dimensions"] = serde_json::json!([w, h]);
                        }
                        if let Some(count) = meta.file_count {
                            file_json["preview"]["file_count"] = serde_json::json!(count);
                        }
                    }
                }
                file_json
            }).collect::<Vec<_>>(),
            "total_size": total_size,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !args.quiet {
        println!("  Found sender: {} ({})", &sender_name, sender_addr);
        println!();
        println!(
            "  Receiving {} items ({}) to {}",
            total_files,
            format_size(total_size),
            output_dir.display()
        );
        println!();
        for file in &files {
            let preview_info = format_preview_info(file);
            if preview_info.is_empty() {
                println!("  {} {}", file_icon(file), file.file_name());
            } else {
                println!(
                    "  {} {} {}",
                    file_icon(file),
                    file.file_name(),
                    preview_info
                );
            }
        }
        println!();
    }

    let accepted = if !args.batch && !args.json && !args.quiet {
        session.start_keep_alive()?;

        print!("  Accept transfer? [Y/n] ");
        io::stdout().flush()?;

        let mut input = String::new();
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        reader.read_line(&mut input).await?;
        let input = input.trim().to_lowercase();

        input.is_empty() || input == "y" || input == "yes"
    } else {
        true
    };

    if !accepted {
        session.decline().await;
        if !args.quiet && !args.json {
            println!();
            println!("  Transfer declined.");
            println!();
        }
        if args.json {
            let output = serde_json::json!({
                "status": "declined",
                "code": code.as_str(),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        return Ok(());
    }

    let progress_rx = session.progress();
    let start_time = Instant::now();

    let quiet = args.quiet;
    let json = args.json;
    let progress_handle = if !quiet && !json {
        Some(tokio::spawn(display_progress(progress_rx)))
    } else {
        None
    };

    let result = session.accept().await;

    if let Some(handle) = progress_handle {
        let _ = handle.await;
    }

    let elapsed = start_time.elapsed();

    match result {
        Ok(()) => {
            record_history(
                code.as_str(),
                &sender_name,
                &files,
                total_size,
                elapsed.as_secs(),
                &output_dir,
                HistoryState::Completed,
                None,
            );

            if !args.quiet && !args.json {
                println!();
                println!("  Transfer complete!");
                println!();
                println!("  Files saved to: {}", output_dir.display());
                println!();

                if !args.batch && global_config.trust.auto_prompt {
                    prompt_trust_device(
                        &sender_name,
                        sender_device_id,
                        sender_public_key.as_deref(),
                    )
                    .await;
                }
            }
            if args.json {
                let output = serde_json::json!({
                    "status": "complete",
                    "code": code.as_str(),
                    "total_received": total_size,
                    "output_dir": output_dir.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            Ok(())
        }
        Err(e) => {
            record_history(
                code.as_str(),
                &sender_name,
                &files,
                total_size,
                elapsed.as_secs(),
                &output_dir,
                HistoryState::Failed,
                Some(e.to_string()),
            );

            if !args.quiet && !args.json {
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
    sender_name: &str,
    files: &[yoop_core::file::FileMetadata],
    total_bytes: u64,
    duration_secs: u64,
    output_dir: &std::path::Path,
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
        TransferDirection::Received,
        sender_name.to_string(),
        code.to_string(),
    )
    .with_files(history_files)
    .with_stats(total_bytes, duration_secs)
    .with_state(state)
    .with_output_dir(output_dir.to_path_buf());

    if let Some(err_msg) = error {
        entry = entry.with_error(err_msg);
    }

    if let Ok(mut store) = HistoryStore::load() {
        if let Err(e) = store.add(entry) {
            tracing::warn!("Failed to record history: {}", e);
        }
    }
}

fn file_icon(file: &FileMetadata) -> &'static str {
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

fn format_preview_info(file: &FileMetadata) -> String {
    match &file.preview {
        Some(preview) => match preview.preview_type {
            PreviewType::Thumbnail => {
                if let Some(ref meta) = preview.metadata {
                    if let Some((w, h)) = meta.dimensions {
                        return format!("({}x{})", w, h);
                    }
                }
                String::new()
            }
            PreviewType::Text => {
                let snippet: String = preview
                    .data
                    .chars()
                    .take(40)
                    .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                    .collect();
                if snippet.len() < preview.data.len() {
                    format!("\"{}...\"", snippet.trim())
                } else if !snippet.is_empty() {
                    format!("\"{}\"", snippet.trim())
                } else {
                    String::new()
                }
            }
            PreviewType::ArchiveListing => {
                if let Some(ref meta) = preview.metadata {
                    if let Some(count) = meta.file_count {
                        return format!("({} files)", count);
                    }
                }
                String::new()
            }
            PreviewType::Icon | PreviewType::None => String::new(),
        },
        None => String::new(),
    }
}

async fn display_progress(mut rx: watch::Receiver<TransferProgress>) {
    let mut last_state = TransferState::Preparing;

    loop {
        if rx.changed().await.is_err() {
            break;
        }

        let progress = rx.borrow().clone();

        if progress.state != last_state {
            last_state = progress.state;

            match progress.state {
                TransferState::Transferring => {
                    println!("  Starting download...");
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
                TransferState::Preparing | TransferState::Waiting | TransferState::Connected => {}
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
    }

    println!();
}

/// Prompt the user to trust the sender device after a successful transfer.
async fn prompt_trust_device(
    sender_name: &str,
    sender_device_id: Option<Uuid>,
    sender_public_key: Option<&str>,
) {
    let (Some(device_id), Some(public_key)) = (sender_device_id, sender_public_key) else {
        return;
    };

    if let Ok(trust_store) = TrustStore::load() {
        if trust_store.find_by_id(&device_id).is_some() {
            return;
        }
    }

    print!("  Trust \"{}\" for future transfers? [y/N] ", sender_name);
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

    let device = TrustedDevice::new(device_id, sender_name.to_string(), public_key.to_string())
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

/// Resolve connection parameters from command args.
///
/// Returns the code string and optional direct address based on --code, --host, or --device flags.
fn resolve_connection_params(
    args: &super::ReceiveArgs,
) -> Result<(String, Option<std::net::SocketAddr>)> {
    if let Some(ref device_name) = args.device {
        let trust_store = TrustStore::load()?;
        let device = trust_store
            .find_by_name(device_name)
            .ok_or_else(|| anyhow::anyhow!("Device '{}' not found in trusted devices. Run 'yoop trust list' to see trusted devices.", device_name))?;

        let addr = device.address().ok_or_else(|| {
            anyhow::anyhow!(
                "Device '{}' has no stored address. Connect with code first to save the address.",
                device_name
            )
        })?;

        anyhow::bail!(
            "Device '{}' found at {}:{}, but codeless trusted connections are not yet implemented.\n\
            For now, please use: yoop receive --host {}:{} <CODE>\n\
            where <CODE> is the share code displayed on the peer device.",
            device_name, addr.0, addr.1, addr.0, addr.1
        );
    }

    let code = args
        .code
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Either a share code or --device must be provided"))?
        .clone();

    let direct_addr = if let Some(ref host) = args.host {
        Some(parse_host_address(host)?)
    } else {
        None
    };

    Ok((code, direct_addr))
}
