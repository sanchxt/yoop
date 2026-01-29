//! Clipboard sharing command implementation.

use std::io::{self, Write};
use std::net::SocketAddr;
use std::time::Instant;

use anyhow::{bail, Result};
use tokio::io::{AsyncBufReadExt, BufReader};

use yoop_core::clipboard::{
    diagnose_clipboard, ClipboardReceiveSession, ClipboardShareSession, ClipboardSyncSession,
    SyncHostSession, SyncSessionRunner,
};
use yoop_core::config::Config;
use yoop_core::connection::parse_host_address;
use yoop_core::transfer::TransferConfig;
use yoop_core::trust::TrustStore;

use super::{ClipboardAction, ClipboardArgs};
use crate::ui::{format_remaining, CodeBox};

/// Create a TransferConfig using global config values.
fn create_transfer_config(global_config: &Config) -> TransferConfig {
    TransferConfig {
        chunk_size: global_config.transfer.chunk_size,
        parallel_streams: global_config.transfer.parallel_chunks,
        verify_checksums: global_config.transfer.verify_checksum,
        discovery_port: global_config.network.port,
        ..Default::default()
    }
}

/// Run the clipboard command.
pub async fn run(args: ClipboardArgs) -> Result<()> {
    super::spawn_update_check();

    match args.action {
        ClipboardAction::Share(share_args) => run_share(share_args, args.quiet, args.json).await,
        ClipboardAction::Receive(recv_args) => run_receive(recv_args, args.quiet, args.json).await,
        ClipboardAction::Sync(sync_args) => run_sync(sync_args, args.quiet, args.json).await,
    }
}

/// Run clipboard share (one-shot).
#[allow(clippy::unused_async)]
async fn run_share(_args: super::ClipboardShareArgs, quiet: bool, json: bool) -> Result<()> {
    let global_config = super::load_config();

    if !quiet && !json {
        println!();
        println!("Yoop Clipboard Share");
        println!("{}", "-".repeat(37));
        println!();
    }

    let config = create_transfer_config(&global_config);

    let session = match ClipboardShareSession::new(config).await {
        Ok(s) => s,
        Err(e) => {
            let error_str = format!("{}", e);
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": &error_str,
                    "diagnostics": diagnose_clipboard(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Error: {}", e);
                print_clipboard_troubleshooting(&error_str);
            }
            bail!("{}", e);
        }
    };

    let code = session.code().to_string();
    let content_preview = session.content().preview(50);
    let content_size = session.content().format_size();

    if json {
        let output = serde_json::json!({
            "status": "waiting",
            "code": code,
            "content": {
                "preview": content_preview,
                "size": content_size,
                "type": format!("{:?}", session.content().content_type()),
            },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !quiet {
        println!("  Sharing clipboard: {}", content_preview);
        println!("  Size: {}", content_size);
        println!();
        CodeBox::new(&code).display();
        println!();
        println!("  Waiting for receiver...");
        println!();
    }

    let result = session.wait().await;

    match result {
        Ok(()) => {
            if json {
                let output = serde_json::json!({
                    "status": "complete",
                    "code": code,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                println!("  Clipboard sent successfully!");
                println!();
            }
            Ok(())
        }
        Err(e) => {
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Error: {}", e);
            }
            Err(e.into())
        }
    }
}

/// Run clipboard receive (one-shot).
#[allow(clippy::too_many_lines)]
async fn run_receive(args: super::ClipboardReceiveArgs, quiet: bool, json: bool) -> Result<()> {
    let global_config = super::load_config();
    let config = create_transfer_config(&global_config);

    if let Some(ref device_name) = args.device {
        return run_receive_trusted(device_name, config, &args, quiet, json).await;
    }

    let (code_str, direct_addr) = resolve_clipboard_receive_params(&args)?;

    if !quiet && !json {
        println!();
        println!("Yoop Clipboard Receive");
        println!("{}", "-".repeat(37));
        println!();
        println!("  Searching for code {}...", code_str);
        println!();
    }

    if json {
        let output = serde_json::json!({
            "status": "searching",
            "code": &code_str,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    let mut session =
        match ClipboardReceiveSession::connect_with_options(&code_str, direct_addr, config).await {
            Ok(s) => s,
            Err(e) => {
                if json {
                    let output = serde_json::json!({
                        "status": "error",
                        "error": format!("{}", e),
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else if !quiet {
                    eprintln!("  Error: {}", e);
                }
                bail!("{}", e);
            }
        };

    let (sender_addr, sender_name) = session.sender();
    let metadata = session.metadata();
    let preview = metadata.as_ref().map_or_else(
        || "unknown".to_string(),
        |m| format!("{:?}, {} bytes", m.content_type, m.size),
    );

    if json {
        let output = serde_json::json!({
            "status": "connected",
            "sender": {
                "name": sender_name,
                "address": sender_addr.to_string(),
            },
            "content": {
                "type": metadata.as_ref().map(|m| format!("{:?}", m.content_type)),
                "size": metadata.as_ref().map(|m| m.size),
            },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !quiet {
        println!("  Found sender: {} ({})", sender_name, sender_addr);
        println!("  Content: {}", preview);
        println!();
    }

    let accepted = if !args.batch && !json && !quiet {
        session.start_keep_alive()?;

        print!("  Accept clipboard content? [Y/n] ");
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
        if !quiet && !json {
            println!();
            println!("  Transfer declined.");
            println!();
        }
        if json {
            let output = serde_json::json!({
                "status": "declined",
                "code": &args.code,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        return Ok(());
    }

    let result = session.accept_to_clipboard().await;

    match result {
        Ok(()) => {
            if json {
                let output = serde_json::json!({
                    "status": "complete",
                    "code": &args.code,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                println!();
                println!("  Clipboard received and copied!");
                println!();
            }
            Ok(())
        }
        Err(e) => {
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Error: {}", e);
            }
            Err(e.into())
        }
    }
}

/// Run clipboard receive with trusted device connection.
#[allow(clippy::too_many_lines)]
async fn run_receive_trusted(
    device_name: &str,
    config: TransferConfig,
    args: &super::ClipboardReceiveArgs,
    quiet: bool,
    json: bool,
) -> Result<()> {
    let trust_store = TrustStore::load()?;
    let device = trust_store
        .find_by_name(device_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Device '{}' not found in trusted devices. Run 'yoop trust list' to see trusted devices.",
                device_name
            )
        })?
        .clone();

    if !quiet && !json {
        println!();
        println!("Yoop Clipboard Receive");
        println!("{}", "-".repeat(37));
        println!();
        println!("  Connecting to trusted device '{}'...", device_name);
        println!();
    }

    if json {
        let output = serde_json::json!({
            "status": "connecting",
            "device": device_name,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    let mut session = match ClipboardReceiveSession::connect_trusted(&device, config).await {
        Ok(s) => s,
        Err(e) => {
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Error: {}", e);
            }
            bail!("{}", e);
        }
    };

    let (sender_addr, sender_name) = session.sender();
    let metadata = session.metadata();
    let preview = metadata.as_ref().map_or_else(
        || "unknown".to_string(),
        |m| format!("{:?}, {} bytes", m.content_type, m.size),
    );

    if json {
        let output = serde_json::json!({
            "status": "connected",
            "sender": {
                "name": sender_name,
                "address": sender_addr.to_string(),
            },
            "content": {
                "type": metadata.as_ref().map(|m| format!("{:?}", m.content_type)),
                "size": metadata.as_ref().map(|m| m.size),
            },
            "trusted": true,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !quiet {
        println!(
            "  Connected to: {} ({}) [trusted]",
            sender_name, sender_addr
        );
        println!("  Content: {}", preview);
        println!();
    }

    let accepted = if !args.batch && !json && !quiet {
        session.start_keep_alive()?;

        print!("  Accept clipboard content? [Y/n] ");
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
        if !quiet && !json {
            println!();
            println!("  Transfer declined.");
            println!();
        }
        if json {
            let output = serde_json::json!({
                "status": "declined",
                "device": device_name,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        return Ok(());
    }

    let result = session.accept_to_clipboard().await;

    match result {
        Ok(()) => {
            if json {
                let output = serde_json::json!({
                    "status": "complete",
                    "device": device_name,
                    "trusted": true,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                println!();
                println!("  Clipboard received and copied!");
                println!();
            }
            Ok(())
        }
        Err(e) => {
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Error: {}", e);
            }
            Err(e.into())
        }
    }
}

/// Run clipboard sync (bidirectional live sync).
async fn run_sync(args: super::ClipboardSyncArgs, quiet: bool, json: bool) -> Result<()> {
    let global_config = super::load_config();
    let config = create_transfer_config(&global_config);

    if !quiet && !json {
        println!();
        println!("Yoop Clipboard Sync");
        println!("{}", "-".repeat(37));
        println!();
    }

    if let Some(ref device_name) = args.device {
        return run_sync_trusted(device_name, config, quiet, json).await;
    }

    if let Some((code_str, direct_addr)) = resolve_clipboard_sync_params(&args)? {
        if !quiet && !json {
            println!("  Connecting to sync session {}...", code_str);
            println!();
        }

        let (session, runner) = match ClipboardSyncSession::connect_with_options(
            &code_str,
            direct_addr,
            config,
        )
        .await
        {
            Ok(s) => s,
            Err(e) => {
                if json {
                    let output = serde_json::json!({
                        "status": "error",
                        "error": format!("{}", e),
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else if !quiet {
                    eprintln!("  Error: {}", e);
                }
                bail!("{}", e);
            }
        };

        run_sync_session(session, runner, quiet, json).await
    } else {
        let host_session = match ClipboardSyncSession::host(config).await {
            Ok(result) => result,
            Err(e) => {
                if json {
                    let output = serde_json::json!({
                        "status": "error",
                        "error": format!("{}", e),
                    });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else if !quiet {
                    eprintln!("  Error: {}", e);
                }
                bail!("{}", e);
            }
        };

        let code = host_session.code().to_string();

        if json {
            let output = serde_json::json!({
                "status": "waiting",
                "code": &code,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if !quiet {
            CodeBox::new(&code).display();
            println!();
        }

        let trust_store = TrustStore::load().ok();
        let (session, runner) =
            wait_for_peer_with_display(host_session, trust_store.as_ref(), quiet, json).await?;

        run_sync_session(session, runner, quiet, json).await
    }
}

/// Run clipboard sync with trusted device connection.
async fn run_sync_trusted(
    device_name: &str,
    config: TransferConfig,
    quiet: bool,
    json: bool,
) -> Result<()> {
    let trust_store = TrustStore::load()?;
    let device = trust_store
        .find_by_name(device_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Device '{}' not found in trusted devices. Run 'yoop trust list' to see trusted devices.",
                device_name
            )
        })?
        .clone();

    if !quiet && !json {
        println!("  Connecting to trusted device '{}'...", device_name);
        println!();
    }

    if json {
        let output = serde_json::json!({
            "status": "connecting",
            "device": device_name,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    let (session, runner) = match ClipboardSyncSession::connect_trusted(&device, config).await {
        Ok(s) => s,
        Err(e) => {
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Error: {}", e);
            }
            bail!("{}", e);
        }
    };

    if !quiet && !json {
        println!(
            "  Connected to trusted device '{}' [trusted]",
            session.peer_name()
        );
        println!();
    }

    run_sync_session(session, runner, quiet, json).await
}

async fn wait_for_peer_with_display(
    host_session: SyncHostSession,
    trust_store: Option<&TrustStore>,
    quiet: bool,
    json: bool,
) -> Result<(ClipboardSyncSession, SyncSessionRunner)> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let stop = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    let display_task = if !quiet && !json {
        let stop_clone = Arc::clone(&stop);
        Some(tokio::spawn(async move {
            while !stop_clone.load(Ordering::Relaxed) {
                let elapsed = start_time.elapsed();
                print!(
                    "\r  Waiting for peer to connect... ({} elapsed)   ",
                    format_remaining(elapsed)
                );
                let _ = io::stdout().flush();
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }))
    } else {
        None
    };

    let result = host_session.wait_for_peer_with_trust(trust_store).await;

    stop.store(true, Ordering::Relaxed);
    if let Some(task) = display_task {
        task.abort();
        let _ = task.await;
        if !quiet && !json {
            print!("\r{}\r", " ".repeat(60));
            let _ = io::stdout().flush();
        }
    }

    result.map_err(Into::into)
}

/// Run the sync session loop.
async fn run_sync_session(
    session: ClipboardSyncSession,
    runner: SyncSessionRunner,
    quiet: bool,
    json: bool,
) -> Result<()> {
    use yoop_core::clipboard::SyncEvent;

    if !quiet && !json {
        println!("  Sync active! Clipboard changes will be shared.");
        println!("  Connected to: {}", session.peer_name());
        println!();
    }

    let result = runner.run().await;

    match result {
        Ok((stats, mut event_rx)) => {
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    SyncEvent::Sent { content_type, size } => {
                        if !quiet && !json {
                            println!("  -> Sent {:?} ({} bytes)", content_type, size);
                        }
                    }
                    SyncEvent::Received { content_type, size } => {
                        if !quiet && !json {
                            println!("  <- Received {:?} ({} bytes)", content_type, size);
                        }
                    }
                }
            }

            if json {
                let output = serde_json::json!({
                    "status": "complete",
                    "stats": {
                        "duration_secs": stats.duration.as_secs(),
                        "items_sent": stats.items_sent,
                        "items_received": stats.items_received,
                        "bytes_sent": stats.bytes_sent,
                        "bytes_received": stats.bytes_received,
                    },
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                println!();
                println!("  Sync session ended.");
                println!(
                    "  Sent: {} items ({} bytes)",
                    stats.items_sent, stats.bytes_sent
                );
                println!(
                    "  Received: {} items ({} bytes)",
                    stats.items_received, stats.bytes_received
                );
                println!();
            }

            session.shutdown();

            Ok(())
        }
        Err(e) => {
            if json {
                let output = serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if !quiet {
                eprintln!("  Sync error: {}", e);
            }
            Err(e.into())
        }
    }
}

/// Print platform-specific troubleshooting hints for clipboard errors.
fn print_clipboard_troubleshooting(error: &str) {
    let is_empty = error.contains("clipboard is empty");
    let is_access_error = error.contains("Cannot access clipboard");

    if !is_empty && !is_access_error {
        return;
    }

    eprintln!();
    eprintln!("  Troubleshooting:");

    #[cfg(target_os = "linux")]
    {
        if is_empty {
            eprintln!("  - Make sure you've copied something to the clipboard first");
        }
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            eprintln!("  - Running on Wayland - clipboard access should work");
            if is_access_error {
                eprintln!("  - Check if your compositor supports wlr-data-control protocol");
            }
        } else if std::env::var("DISPLAY").is_ok() {
            eprintln!("  - Running on X11 - clipboard access should work");
        } else {
            eprintln!("  - No display server detected (DISPLAY/WAYLAND_DISPLAY not set)");
            eprintln!("  - Run this command from a graphical terminal session");
        }
        eprintln!("  - Run with RUST_LOG=debug for detailed diagnostics");
    }

    #[cfg(target_os = "macos")]
    {
        eprintln!("  - Make sure you've copied something (Cmd+C) first");
        eprintln!("  - Check System Preferences > Privacy & Security for clipboard access");
    }

    #[cfg(target_os = "windows")]
    {
        eprintln!("  - Make sure you've copied something (Ctrl+C) first");
        eprintln!("  - Try closing other applications that might be locking the clipboard");
    }

    eprintln!();
}

/// Resolve connection parameters for clipboard receive (code-based only).
/// The --device case is handled separately in run_receive.
fn resolve_clipboard_receive_params(
    args: &super::ClipboardReceiveArgs,
) -> Result<(String, Option<SocketAddr>)> {
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

/// Resolve connection parameters for clipboard sync (code-based only).
/// The --device case is handled separately in run_sync.
fn resolve_clipboard_sync_params(
    args: &super::ClipboardSyncArgs,
) -> Result<Option<(String, Option<SocketAddr>)>> {
    if args.host.is_some() && args.code.is_none() {
        bail!("--host requires a share code. Use: yoop clipboard sync --host <IP> <CODE>");
    }

    if let Some(ref code_str) = args.code {
        let direct_addr = if let Some(ref host) = args.host {
            Some(parse_host_address(host)?)
        } else {
            None
        };
        return Ok(Some((code_str.clone(), direct_addr)));
    }

    Ok(None)
}
