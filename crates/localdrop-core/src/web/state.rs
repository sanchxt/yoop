//! Application state management for the web server.
//!
//! This module provides shared state that is accessible across all HTTP handlers,
//! including active transfer sessions and progress tracking.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{watch, Mutex, RwLock};

use crate::file::FileMetadata;
use crate::history::HistoryStore;
use crate::transfer::{ReceiveSession, ShareSession, TransferProgress, TransferState};

use super::WebServerConfig;

/// The current mode of operation for the web interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebMode {
    /// No active transfer
    Idle,
    /// Sharing files, waiting for receiver
    Sharing,
    /// Connected to sender, awaiting user decision
    Receiving,
    /// Transfer in progress
    Transferring,
}

impl Default for WebMode {
    fn default() -> Self {
        Self::Idle
    }
}

/// Pending receive session awaiting user decision (accept/decline).
pub struct PendingReceive {
    /// The receive session
    pub session: ReceiveSession,
    /// When this pending receive was created
    pub created_at: Instant,
}

impl std::fmt::Debug for PendingReceive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingReceive")
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

/// Active share session with metadata.
pub struct ActiveShare {
    /// The share session
    pub session: ShareSession,
    /// Files being shared (cached for quick access)
    pub files: Vec<FileMetadata>,
    /// When the share was created
    pub created_at: Instant,
}

impl std::fmt::Debug for ActiveShare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveShare")
            .field("files", &self.files.len())
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

/// Active receive session during transfer.
pub struct ActiveReceive {
    /// The receive session
    pub session: ReceiveSession,
    /// Output directory where files are being saved
    pub output_dir: PathBuf,
    /// When the receive was accepted
    pub accepted_at: Instant,
}

impl std::fmt::Debug for ActiveReceive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveReceive")
            .field("output_dir", &self.output_dir)
            .field("accepted_at", &self.accepted_at)
            .finish_non_exhaustive()
    }
}

/// Information about a completed transfer (for download).
#[derive(Debug, Clone)]
pub struct CompletedReceive {
    /// Files that were received
    pub files: Vec<FileMetadata>,
    /// Output directory containing the files
    pub output_dir: PathBuf,
    /// When the transfer completed
    pub completed_at: Instant,
}

/// Shared application state for all HTTP handlers.
pub struct AppState {
    /// Current operation mode
    pub mode: RwLock<WebMode>,

    /// Current share code (stored separately so it's accessible after session is taken)
    pub share_code: RwLock<Option<String>>,

    /// Active share session (sender side)
    pub active_share: Mutex<Option<ActiveShare>>,

    /// Pending receive awaiting accept/decline
    pub pending_receive: Mutex<Option<PendingReceive>>,

    /// Active receive session (receiver side, during transfer)
    pub active_receive: Mutex<Option<ActiveReceive>>,

    /// Completed receive (for download after transfer)
    pub completed_receive: Mutex<Option<CompletedReceive>>,

    /// Progress broadcast channel sender
    pub progress_tx: watch::Sender<Option<TransferProgress>>,

    /// Progress broadcast channel receiver (clone this for subscribers)
    pub progress_rx: watch::Receiver<Option<TransferProgress>>,

    /// Transfer history store
    pub history: Mutex<HistoryStore>,

    /// Server configuration
    pub config: WebServerConfig,

    /// Temporary directory for uploaded files (share) and received files
    pub temp_dir: PathBuf,

    /// Device name for this instance
    pub device_name: String,
}

impl AppState {
    /// Create a new application state with the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory cannot be created.
    #[must_use]
    pub fn new(config: WebServerConfig) -> Self {
        let (progress_tx, progress_rx) = watch::channel(None);
        let temp_dir = std::env::temp_dir().join("localdrop-web");

        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            tracing::warn!("Failed to create temp directory: {}", e);
        }

        let device_name = hostname::get().map_or_else(
            |_| "LocalDrop-Web".to_string(),
            |h| h.to_string_lossy().into_owned(),
        );

        Self {
            mode: RwLock::new(WebMode::Idle),
            share_code: RwLock::new(None),
            active_share: Mutex::new(None),
            pending_receive: Mutex::new(None),
            active_receive: Mutex::new(None),
            completed_receive: Mutex::new(None),
            progress_tx,
            progress_rx,
            history: Mutex::new(HistoryStore::load().unwrap_or_else(|e| {
                tracing::warn!("Failed to load history store: {}", e);
                HistoryStore::load_from(
                    temp_dir.join("history.json"),
                    crate::config::HistoryConfig::default(),
                )
                .unwrap_or_else(|_| panic!("Failed to create fallback history store"))
            })),
            config,
            temp_dir,
            device_name,
        }
    }

    /// Reset state to idle.
    pub async fn reset_to_idle(&self) {
        *self.mode.write().await = WebMode::Idle;
        *self.share_code.write().await = None;
        *self.active_share.lock().await = None;
        *self.pending_receive.lock().await = None;
        *self.active_receive.lock().await = None;
        let _ = self.progress_tx.send(None);
    }

    /// Set the current share code.
    pub async fn set_share_code(&self, code: String) {
        *self.share_code.write().await = Some(code);
    }

    /// Get the current share code if sharing.
    pub async fn current_share_code(&self) -> Option<String> {
        self.share_code.read().await.clone()
    }

    /// Check if we're currently in an active transfer.
    pub async fn is_transferring(&self) -> bool {
        *self.mode.read().await == WebMode::Transferring
    }

    /// Update progress from a transfer progress update.
    pub fn update_progress(&self, progress: TransferProgress) {
        let _ = self.progress_tx.send(Some(progress));
    }

    /// Mark transfer as complete.
    pub async fn mark_complete(&self) {
        if let Some(ref progress) = *self.progress_rx.borrow() {
            let mut updated = progress.clone();
            updated.state = TransferState::Completed;
            let _ = self.progress_tx.send(Some(updated));
        }
        *self.mode.write().await = WebMode::Idle;
    }

    /// Mark transfer as failed.
    pub async fn mark_failed(&self, error_msg: &str) {
        tracing::error!("Transfer failed: {}", error_msg);
        if let Some(ref progress) = *self.progress_rx.borrow() {
            let mut updated = progress.clone();
            updated.state = TransferState::Failed;
            let _ = self.progress_tx.send(Some(updated));
        }
        *self.mode.write().await = WebMode::Idle;
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("temp_dir", &self.temp_dir)
            .field("device_name", &self.device_name)
            .finish_non_exhaustive()
    }
}

/// Type alias for shared state across handlers.
pub type SharedState = Arc<AppState>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_mode_default() {
        assert_eq!(WebMode::default(), WebMode::Idle);
    }

    #[test]
    fn test_web_mode_serialize() {
        let mode = WebMode::Sharing;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"sharing\"");
    }

    #[test]
    fn test_app_state_creation() {
        let config = WebServerConfig::default();
        let state = AppState::new(config);
        assert!(!state.device_name.is_empty());
    }

    #[tokio::test]
    async fn test_mode_transitions() {
        let state = Arc::new(AppState::new(WebServerConfig::default()));

        assert_eq!(*state.mode.read().await, WebMode::Idle);

        *state.mode.write().await = WebMode::Sharing;
        assert_eq!(*state.mode.read().await, WebMode::Sharing);

        state.reset_to_idle().await;
        assert_eq!(*state.mode.read().await, WebMode::Idle);
    }

    #[tokio::test]
    async fn test_progress_channel() {
        let state = Arc::new(AppState::new(WebServerConfig::default()));

        let mut rx = state.progress_rx.clone();

        assert!(rx.borrow().is_none());

        let progress = TransferProgress::new(2, 1000);
        state.update_progress(progress);

        rx.changed().await.unwrap();

        assert!(rx.borrow().is_some());
    }
}
