//! Sync command handler for bidirectional directory synchronization.

use std::path::PathBuf;

use yoop_core::sync::{SyncConfig, SyncEvent, SyncSession};
use yoop_core::transfer::TransferConfig;

use super::load_config;

/// Arguments for the sync command
#[derive(clap::Parser)]
pub struct SyncArgs {
    /// Directory to synchronize
    pub directory: PathBuf,

    /// Share code to connect to (omit to host)
    pub code: Option<String>,

    /// Patterns to exclude (can be specified multiple times)
    #[arg(short = 'x', long = "exclude", action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Path to ignore file (like .gitignore)
    #[arg(long)]
    pub ignore_file: Option<PathBuf>,

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,

    /// Don't sync file deletions
    #[arg(long)]
    pub no_delete: bool,

    /// Maximum file size to sync (e.g., 100MB)
    #[arg(long)]
    pub max_size: Option<String>,

    /// Minimal output
    #[arg(short, long)]
    pub quiet: bool,

    /// Detailed logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Run the sync command
pub async fn run(args: SyncArgs) -> anyhow::Result<()> {
    let sync_root = args.directory.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Cannot access directory '{}': {}",
            args.directory.display(),
            e
        )
    })?;

    if !sync_root.is_dir() {
        anyhow::bail!("Not a directory: {}", args.directory.display());
    }

    let mut config = SyncConfig {
        sync_root: sync_root.clone(),
        follow_symlinks: args.follow_symlinks,
        sync_deletions: !args.no_delete,
        ..Default::default()
    };

    for pattern in &args.exclude {
        config.exclude_patterns.push(pattern.clone());
    }

    if let Some(ignore_file) = &args.ignore_file {
        let patterns = load_ignore_file(ignore_file)?;
        config.exclude_patterns.extend(patterns);
    }

    if let Some(max_size) = &args.max_size {
        config.max_file_size = parse_size(max_size)?;
    }

    let global_config = load_config();
    let transfer_config = TransferConfig {
        chunk_size: global_config.transfer.chunk_size,
        parallel_streams: global_config.transfer.parallel_chunks,
        verify_checksums: global_config.transfer.verify_checksum,
        discovery_port: global_config.network.port,
        ..Default::default()
    };

    if let Some(ref code) = args.code {
        run_client(code, config, transfer_config, &args).await
    } else {
        run_host(config, transfer_config, &args).await
    }
}

async fn run_host(
    config: SyncConfig,
    transfer_config: TransferConfig,
    args: &SyncArgs,
) -> anyhow::Result<()> {
    if !args.quiet {
        println!("\nYoop v{}", env!("CARGO_PKG_VERSION"));
        println!("─────────────────────────────────────");
        println!();
        println!(
            "  Starting sync session for: {}",
            config.sync_root.display()
        );
        println!();
    }

    let (code, mut session) = SyncSession::host(config.clone(), transfer_config).await?;

    if !args.quiet {
        println!("  ┌─────────────────────────────────────┐");
        println!("  │                                     │");
        println!("  │       Code:  {}               │", code);
        println!("  │                                     │");
        println!("  └─────────────────────────────────────┘");
        println!();
        println!("  Waiting for peer to connect...");
        println!();
    }

    let quiet = args.quiet;
    let json = args.json;
    let stats = session
        .run(move |event| {
            print_event(&event, quiet, json);
        })
        .await?;

    if !args.quiet {
        println!();
        print_stats(&stats);
    }

    Ok(())
}

async fn run_client(
    code: &str,
    config: SyncConfig,
    transfer_config: TransferConfig,
    args: &SyncArgs,
) -> anyhow::Result<()> {
    if !args.quiet {
        println!("\nYoop v{}", env!("CARGO_PKG_VERSION"));
        println!("─────────────────────────────────────");
        println!();
        println!("  Connecting to sync session: {}", code);
        println!("  Syncing directory: {}", config.sync_root.display());
        println!();
    }

    let mut session = SyncSession::connect(code, config, transfer_config).await?;

    if !args.quiet {
        println!("  ✓ Connected to: {}", session.peer_name());
        println!("  Sync active (Ctrl+C to stop)");
        println!();
    }

    let quiet = args.quiet;
    let json = args.json;
    let stats = session
        .run(move |event| {
            print_event(&event, quiet, json);
        })
        .await?;

    if !args.quiet {
        println!();
        print_stats(&stats);
    }

    Ok(())
}

fn print_event(event: &SyncEvent, quiet: bool, json: bool) {
    if quiet {
        return;
    }

    if json {
        if let Ok(json_str) = serde_json::to_string(&event) {
            println!("{}", json_str);
        }
        return;
    }

    match event {
        SyncEvent::Connected { peer_name } => {
            println!("  ✓ Connected: {}", peer_name);
        }
        SyncEvent::IndexExchanged {
            local_files,
            remote_files,
        } => {
            println!(
                "  ✓ Index exchanged ({} local, {} remote files)",
                local_files, remote_files
            );
        }
        SyncEvent::ReconcileStart { ops_count } => {
            if *ops_count > 0 {
                println!("  ✓ Reconciling {} differences...", ops_count);
            } else {
                println!("  ✓ Already in sync");
            }
        }
        SyncEvent::FileSending { path, size } => {
            println!("  → Sending: {} ({} bytes)", path, format_size(*size));
        }
        SyncEvent::FileSent { path } => {
            if let Some(filename) = std::path::Path::new(path).file_name() {
                println!("  ✓ Sent: {}", filename.to_string_lossy());
            }
        }
        SyncEvent::FileReceiving { path, size } => {
            println!("  ← Receiving: {} ({} bytes)", path, format_size(*size));
        }
        SyncEvent::FileReceived { path } => {
            if let Some(filename) = std::path::Path::new(path).file_name() {
                println!("  ✓ Received: {}", filename.to_string_lossy());
            }
        }
        SyncEvent::FileDeleted { path } => {
            println!("  🗑  Deleted: {}", path);
        }
        SyncEvent::Conflict { path, resolution } => {
            println!("  ⚠  Conflict on {}: {}", path, resolution);
        }
        SyncEvent::Error { message } => {
            eprintln!("  ✗ Error: {}", message);
        }
        SyncEvent::Stats { .. } => {}
    }
}

fn print_stats(stats: &yoop_core::sync::SyncStats) {
    println!("  Session ended. Stats:");
    println!("    Duration: {}s", stats.duration.as_secs());
    println!(
        "    Sent: {} files ({} bytes)",
        stats.files_sent,
        format_size(stats.bytes_sent)
    );
    println!(
        "    Received: {} files ({} bytes)",
        stats.files_received,
        format_size(stats.bytes_received)
    );
    if stats.conflicts > 0 {
        println!("    Conflicts: {}", stats.conflicts);
    }
    if stats.errors > 0 {
        println!("    Errors: {}", stats.errors);
    }
}

#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn load_ignore_file(path: &PathBuf) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let patterns: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect();
    Ok(patterns)
}

fn parse_size(size_str: &str) -> anyhow::Result<u64> {
    let size_str = size_str.trim().to_uppercase();
    let (num_part, unit_part) = if size_str.ends_with("GB") {
        (&size_str[..size_str.len() - 2], 1024 * 1024 * 1024)
    } else if size_str.ends_with("MB") {
        (&size_str[..size_str.len() - 2], 1024 * 1024)
    } else if size_str.ends_with("KB") {
        (&size_str[..size_str.len() - 2], 1024)
    } else if size_str.ends_with('B') {
        (&size_str[..size_str.len() - 1], 1)
    } else {
        (size_str.as_str(), 1)
    };

    let num: u64 = num_part.trim().parse()?;
    Ok(num * unit_part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("100B").unwrap(), 100);
        assert_eq!(parse_size("10KB").unwrap(), 10 * 1024);
        assert_eq!(parse_size("10MB").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("10kb").unwrap(), 10 * 1024);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }
}
