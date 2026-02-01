//! Session state coordination between CLI and TUI.
//!
//! This module handles shared state between CLI commands and the TUI,
//! allowing the TUI to monitor transfers started from CLI and vice versa.
//!
//! # Architecture
//!
//! - `SessionStateFile`: Shared JSON file at `~/.cache/yoop/sessions.json`
//! - `StateWatcher`: Monitors the state file for changes
//! - CLI writes session state after each operation
//! - TUI reads and watches for updates

pub mod state_file;
pub mod watcher;

pub use state_file::{
    ClipboardSyncEntry, FileEntry, PeerEntry, ProgressEntry, SessionEntry, SessionStateFile,
};
pub use watcher::{StateWatcher, StateWatcherHandle};
