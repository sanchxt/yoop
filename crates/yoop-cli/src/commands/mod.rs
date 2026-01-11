//! CLI command definitions and handlers.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Load configuration with graceful fallback to defaults.
///
/// This function should be used by all commands to load the user's configuration.
/// If the config file doesn't exist or can't be parsed, it falls back to defaults.
pub fn load_config() -> yoop_core::config::Config {
    yoop_core::config::Config::load().unwrap_or_default()
}

/// Spawn a background task to check for updates.
///
/// This function spawns a non-blocking background task that:
/// - Checks if auto_check and notify are enabled in config
/// - Respects the check_interval to avoid excessive API calls
/// - Displays a message to stderr if an update is available
/// - Silently ignores errors and no-update cases
///
/// This should be called by long-running commands (share, receive, clipboard).
#[cfg(feature = "update")]
pub fn spawn_update_check() {
    use yoop_core::config::Config;
    use yoop_core::update::version_check::VersionChecker;

    tokio::spawn(async move {
        let Ok(mut config) = Config::load() else {
            return;
        };

        if !config.update.auto_check || !config.update.notify {
            return;
        }

        let checker = VersionChecker::new();

        match checker.check_with_cache(&mut config).await {
            Ok(Some(status)) if status.update_available => {
                eprintln!();
                eprintln!(
                    "  Update available: {} -> {}",
                    status.current_version, status.latest_version
                );
                eprintln!("  Run 'yoop update' to upgrade.");
                eprintln!();
            }
            _ => {}
        }
    });
}

#[cfg(not(feature = "update"))]
pub fn spawn_update_check() {}

pub mod clipboard;
pub mod completions;
pub mod config;
pub mod diagnose;
pub mod history;
pub mod internal;
pub mod receive;
pub mod scan;
pub mod send;
pub mod share;
pub mod trust;
#[cfg(feature = "update")]
pub mod update;
pub mod web;

/// Yoop - Cross-platform local network file sharing
#[derive(Parser)]
#[command(name = "yoop")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// The command to execute
    #[command(subcommand)]
    pub command: Command,
}

/// Available commands
#[derive(Subcommand)]
pub enum Command {
    /// Share files with other devices
    Share(ShareArgs),

    /// Receive files using a share code
    Receive(ReceiveArgs),

    /// Send files to a trusted device (no code needed)
    Send(SendArgs),

    /// Share and sync clipboard content
    Clipboard(ClipboardArgs),

    /// Scan network for active shares
    Scan(ScanArgs),

    /// Manage trusted devices
    Trust(TrustArgs),

    /// Start web interface
    Web(WebArgs),

    /// Manage configuration
    Config(ConfigArgs),

    /// Run network diagnostics
    Diagnose(DiagnoseArgs),

    /// View transfer history
    History(HistoryArgs),

    /// Generate shell completions
    Completions(CompletionsArgs),

    /// Check for and install updates
    #[cfg(feature = "update")]
    Update(UpdateArgs),

    /// Internal: hold clipboard content (not user-facing, used by spawn)
    #[command(hide = true)]
    InternalClipboardHold(InternalClipboardHoldArgs),
}

/// Arguments for the share command
#[derive(Parser)]
pub struct ShareArgs {
    /// Files and folders to share
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Code expiration time (e.g., 5m, 10m, 30m)
    #[arg(short, long, default_value = "5m")]
    pub expire: String,

    /// Require additional PIN for extra security
    #[arg(short, long)]
    pub pin: bool,

    /// Require manual approval of receiver
    #[arg(long)]
    pub approve: bool,

    /// Allow multiple receivers
    #[arg(long)]
    pub multi: bool,

    /// Custom device name for this session
    #[arg(long)]
    pub name: Option<String>,

    /// Enable compression for transfer
    #[arg(long)]
    pub compress: bool,

    /// Minimal output
    #[arg(short, long)]
    pub quiet: bool,

    /// Detailed logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Non-interactive mode for scripting
    #[arg(long)]
    pub batch: bool,
}

/// Arguments for the receive command
#[derive(Parser)]
pub struct ReceiveArgs {
    /// Share code to connect to
    pub code: String,

    /// Output directory for received files
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Receive to clipboard (for text/images)
    #[arg(long)]
    pub clipboard: bool,

    /// Minimal output
    #[arg(short, long)]
    pub quiet: bool,

    /// Detailed logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Non-interactive mode (auto-accept)
    #[arg(long)]
    pub batch: bool,
}

/// Arguments for the send command (trusted device)
#[derive(Parser)]
pub struct SendArgs {
    /// Name of the trusted device
    pub device: String,

    /// Files and folders to send
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Enable compression
    #[arg(long)]
    pub compress: bool,

    /// Minimal output
    #[arg(short, long)]
    pub quiet: bool,
}

/// Arguments for the scan command
#[derive(Parser)]
pub struct ScanArgs {
    /// Duration to scan (e.g., 5s, 10s)
    #[arg(short, long, default_value = "5s")]
    pub duration: String,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Connect interactively to discovered shares
    #[arg(short, long)]
    pub interactive: bool,
}

/// Arguments for the trust command
#[derive(Parser)]
pub struct TrustArgs {
    /// Trust subcommand
    #[command(subcommand)]
    pub action: TrustAction,
}

/// Trust subcommands
#[derive(Subcommand)]
pub enum TrustAction {
    /// List trusted devices
    List,

    /// Remove a trusted device
    Remove {
        /// Device name or ID
        device: String,
    },

    /// Set trust level for a device
    Set {
        /// Device name or ID
        device: String,

        /// Trust level (full, ask)
        #[arg(long)]
        level: String,
    },
}

/// Arguments for the web command
#[derive(Parser)]
pub struct WebArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Require authentication
    #[arg(long)]
    pub auth: bool,

    /// Bind to localhost only
    #[arg(long)]
    pub localhost_only: bool,
}

/// Arguments for the config command
#[derive(Parser)]
pub struct ConfigArgs {
    /// Config subcommand
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Config subcommands
#[derive(Subcommand)]
pub enum ConfigAction {
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Value to set
        value: String,
    },

    /// Show all configuration
    Show,

    /// List all available configuration keys
    List,

    /// Show the configuration file path
    Path,

    /// Reset to defaults
    Reset,
}

/// Arguments for the diagnose command
#[derive(Parser)]
pub struct DiagnoseArgs {
    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the history command
#[derive(Parser)]
pub struct HistoryArgs {
    /// Show details for a specific transfer
    #[arg(long)]
    pub details: Option<usize>,

    /// Clear history
    #[arg(long)]
    pub clear: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the completions command
#[derive(Parser)]
pub struct CompletionsArgs {
    /// Completions subcommand
    #[command(subcommand)]
    pub action: CompletionsAction,
}

/// Completions subcommands
#[derive(Subcommand, Clone, Copy)]
pub enum CompletionsAction {
    /// Install shell completions (auto-detects shell)
    Install {
        /// Override shell detection
        #[arg(long, value_enum)]
        shell: Option<ShellType>,
    },

    /// Uninstall shell completions
    Uninstall {
        /// Override shell detection
        #[arg(long, value_enum)]
        shell: Option<ShellType>,
    },

    /// Generate completions and print to stdout (for manual installation)
    Generate {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: ShellType,
    },
}

/// Supported shell types for completions
#[derive(Clone, Copy, ValueEnum, Debug)]
pub enum ShellType {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell
    #[value(name = "powershell")]
    PowerShell,
    /// Elvish shell
    Elvish,
}

/// Arguments for the clipboard command
#[derive(Parser)]
pub struct ClipboardArgs {
    /// Clipboard subcommand
    #[command(subcommand)]
    pub action: ClipboardAction,

    /// Minimal output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,
}

/// Clipboard subcommands
#[derive(Subcommand)]
pub enum ClipboardAction {
    /// Share current clipboard content (one-shot)
    Share(ClipboardShareArgs),

    /// Receive clipboard content using a share code
    Receive(ClipboardReceiveArgs),

    /// Start bidirectional clipboard sync session
    Sync(ClipboardSyncArgs),
}

/// Arguments for clipboard share
#[derive(Parser)]
pub struct ClipboardShareArgs {
    /// Code expiration time (e.g., 5m, 10m, 30m)
    #[arg(short, long, default_value = "5m")]
    pub expire: String,
}

/// Arguments for clipboard receive
#[derive(Parser)]
pub struct ClipboardReceiveArgs {
    /// Share code to connect to
    pub code: String,

    /// Non-interactive mode (auto-accept)
    #[arg(long)]
    pub batch: bool,
}

/// Arguments for clipboard sync
#[derive(Parser)]
pub struct ClipboardSyncArgs {
    /// Share code to connect to (omit to host new session)
    pub code: Option<String>,
}

/// Arguments for internal clipboard hold command (not user-facing)
#[derive(Parser)]
pub struct InternalClipboardHoldArgs {
    /// Content type: "image" or "text"
    #[arg(long)]
    pub content_type: String,

    /// Timeout in seconds before the holder exits
    #[arg(long, default_value = "300")]
    pub timeout: u64,
}

/// Arguments for the update command
#[cfg(feature = "update")]
#[derive(Parser)]
pub struct UpdateArgs {
    /// Only check for updates, don't install
    #[arg(long)]
    pub check: bool,

    /// Rollback to previous version (restores backup)
    #[arg(long)]
    pub rollback: bool,

    /// Force update even if already on latest version
    #[arg(long)]
    pub force: bool,

    /// Specify package manager: npm, pnpm, yarn, or bun
    #[arg(long)]
    pub package_manager: Option<String>,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Minimal output
    #[arg(short, long)]
    pub quiet: bool,
}
