//! Receive command implementation.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::watch;

use localdrop_core::file::format_size;
use localdrop_core::transfer::{ReceiveSession, TransferConfig, TransferProgress, TransferState};

use super::ReceiveArgs;

/// Run the receive command.
#[allow(clippy::too_many_lines)]
pub async fn run(args: ReceiveArgs) -> Result<()> {
    let code = localdrop_core::code::ShareCode::parse(&args.code)?;

    if !args.quiet && !args.json {
        println!();
        println!("LocalDrop v{}", localdrop_core::VERSION);
        println!("{}", "-".repeat(37));
        println!();
        println!("  Searching for code {}...", code.as_str());
        println!();
    }

    if args.json {
        let output = serde_json::json!({
            "status": "searching",
            "code": code.as_str(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    let output_dir = args.output.unwrap_or_else(|| PathBuf::from("."));

    let config = TransferConfig::default();

    let mut session = ReceiveSession::connect(&code, output_dir.clone(), config).await?;

    let (sender_addr, sender_name) = session.sender();
    let files = session.files();
    let total_files = files.len();
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    if args.json {
        let output = serde_json::json!({
            "status": "connected",
            "sender": {
                "name": sender_name,
                "address": sender_addr.to_string(),
            },
            "files": files.iter().map(|f| serde_json::json!({
                "name": f.file_name(),
                "size": f.size,
                "path": f.relative_path.display().to_string(),
            })).collect::<Vec<_>>(),
            "total_size": total_size,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !args.quiet {
        println!("  Found sender: {} ({})", sender_name, sender_addr);
        println!();
        println!(
            "  Receiving {} items ({}) to {}",
            total_files,
            format_size(total_size),
            output_dir.display()
        );
        println!();
        for file in files {
            println!("  {} {}", file_icon(file), file.file_name());
        }
        println!();
    }

    let accepted = if !args.batch && !args.json && !args.quiet {
        print!("  Accept transfer? [Y/n] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
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

    match result {
        Ok(()) => {
            if !args.quiet && !args.json {
                println!();
                println!("  Transfer complete!");
                println!();
                println!("  Files saved to: {}", output_dir.display());
                println!();
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
            if !args.quiet && !args.json {
                eprintln!();
                eprintln!("  Transfer failed: {}", e);
                eprintln!();
            }
            Err(e.into())
        }
    }
}

fn file_icon(file: &localdrop_core::file::FileMetadata) -> &'static str {
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
