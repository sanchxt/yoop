//! Session state file handling for CLI/TUI coordination.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current version of the state file format.
const STATE_FILE_VERSION: u32 = 1;

/// Shared state file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateFile {
    /// File format version.
    pub version: u32,
    /// Last update timestamp.
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Active transfer sessions.
    pub sessions: Vec<SessionEntry>,
    /// Active clipboard sync session.
    pub clipboard_sync: Option<ClipboardSyncEntry>,
}

/// A transfer session entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    /// Unique session ID.
    pub id: Uuid,
    /// Session type: "share", "receive", "send", or "sync".
    pub session_type: String,
    /// Share code (if applicable).
    pub code: Option<String>,
    /// Process ID of the CLI command.
    pub pid: u32,
    /// When the session started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the session expires (if applicable).
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Files in the transfer.
    pub files: Vec<FileEntry>,
    /// Connected peer info.
    pub peer: Option<PeerEntry>,
    /// Transfer progress.
    pub progress: ProgressEntry,
}

/// File entry in a transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File name.
    pub name: String,
    /// Total size in bytes.
    pub size: u64,
    /// Bytes transferred.
    pub transferred: u64,
    /// Status: "pending", "transferring", "completed", "failed".
    pub status: String,
}

/// Peer information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEntry {
    /// Peer device name.
    pub name: String,
    /// Peer network address.
    pub address: String,
}

/// Transfer progress information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProgressEntry {
    /// Bytes transferred.
    pub transferred: u64,
    /// Total bytes.
    pub total: u64,
    /// Current speed in bytes per second.
    pub speed_bps: u64,
}

/// Clipboard sync session entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardSyncEntry {
    /// Peer device name.
    pub peer_name: String,
    /// Peer network address.
    pub peer_address: String,
    /// Process ID.
    pub pid: u32,
    /// When the session started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Number of items sent.
    pub items_sent: u64,
    /// Number of items received.
    pub items_received: u64,
}

impl SessionStateFile {
    /// Get the path to the session state file.
    pub fn path() -> PathBuf {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache")
            })
            .join("yoop");
        cache_dir.join("sessions.json")
    }

    /// Load the session state file, returning default if it doesn't exist.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let mut state: Self = serde_json::from_str(&content)?;

        state.cleanup();

        Ok(state)
    }

    /// Load or create a new session state file.
    pub fn load_or_create() -> Self {
        Self::load().unwrap_or_default()
    }

    /// Save the session state file.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add a new session entry.
    pub fn add_session(&mut self, entry: SessionEntry) {
        self.cleanup();
        self.sessions.push(entry);
        self.updated_at = chrono::Utc::now();
        let _ = self.save();
    }

    /// Update an existing session's progress.
    pub fn update_session_progress(&mut self, id: Uuid, progress: ProgressEntry) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
            session.progress = progress;
            self.updated_at = chrono::Utc::now();
            let _ = self.save();
        }
    }

    /// Update a session's peer information.
    pub fn update_session_peer(&mut self, id: Uuid, peer: PeerEntry) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
            session.peer = Some(peer);
            self.updated_at = chrono::Utc::now();
            let _ = self.save();
        }
    }

    /// Update a file's transfer status within a session.
    pub fn update_file_status(
        &mut self,
        session_id: Uuid,
        file_name: &str,
        status: &str,
        transferred: u64,
    ) {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            if let Some(file) = session.files.iter_mut().find(|f| f.name == file_name) {
                file.status = status.to_string();
                file.transferred = transferred;
                self.updated_at = chrono::Utc::now();
                let _ = self.save();
            }
        }
    }

    /// Remove a session by ID.
    pub fn remove_session(&mut self, id: Uuid) {
        self.sessions.retain(|s| s.id != id);
        self.updated_at = chrono::Utc::now();
        let _ = self.save();
    }

    /// Set the clipboard sync session.
    pub fn set_clipboard_sync(&mut self, entry: Option<ClipboardSyncEntry>) {
        self.clipboard_sync = entry;
        self.updated_at = chrono::Utc::now();
        let _ = self.save();
    }

    /// Update clipboard sync stats.
    pub fn update_clipboard_sync_stats(&mut self, items_sent: u64, items_received: u64) {
        if let Some(ref mut sync) = self.clipboard_sync {
            sync.items_sent = items_sent;
            sync.items_received = items_received;
            self.updated_at = chrono::Utc::now();
            let _ = self.save();
        }
    }

    /// Find a session by ID.
    pub fn find_session(&self, id: Uuid) -> Option<&SessionEntry> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Find a session by share code.
    pub fn find_session_by_code(&self, code: &str) -> Option<&SessionEntry> {
        self.sessions
            .iter()
            .find(|s| s.code.as_deref() == Some(code))
    }

    /// Clean up expired and dead sessions.
    pub fn cleanup(&mut self) {
        let now = chrono::Utc::now();

        self.sessions.retain(|s| {
            if let Some(expires) = s.expires_at {
                expires > now
            } else {
                true
            }
        });

        self.sessions.retain(|s| is_process_alive(s.pid));

        if let Some(ref sync) = self.clipboard_sync {
            if !is_process_alive(sync.pid) {
                self.clipboard_sync = None;
            }
        }
    }

    /// Get active share sessions.
    pub fn share_sessions(&self) -> impl Iterator<Item = &SessionEntry> {
        self.sessions.iter().filter(|s| s.session_type == "share")
    }

    /// Get active receive sessions.
    pub fn receive_sessions(&self) -> impl Iterator<Item = &SessionEntry> {
        self.sessions.iter().filter(|s| s.session_type == "receive")
    }

    /// Get active sync sessions.
    pub fn sync_sessions(&self) -> impl Iterator<Item = &SessionEntry> {
        self.sessions.iter().filter(|s| s.session_type == "sync")
    }
}

impl Default for SessionStateFile {
    fn default() -> Self {
        Self {
            version: STATE_FILE_VERSION,
            updated_at: chrono::Utc::now(),
            sessions: Vec::new(),
            clipboard_sync: None,
        }
    }
}

/// Check if a process is alive.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    #[allow(unsafe_code, clippy::cast_possible_wrap)]
    unsafe {
        libc::kill(pid as i32, 0) == 0
    }
}

/// Check if a process is alive (windows).
#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

/// Check if a process is alive (fallback for unsupported platforms).
#[cfg(not(any(unix, windows)))]
fn is_process_alive(_pid: u32) -> bool {
    true
}

/// Send a signal to cancel a transfer process.
#[cfg(unix)]
pub fn signal_cancel(pid: u32) -> bool {
    #[allow(unsafe_code, clippy::cast_possible_wrap)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM) == 0
    }
}

/// Signal cancellation on Windows (not directly supported, use command file).
#[cfg(not(unix))]
pub fn signal_cancel(_pid: u32) -> bool {
    false
}

/// Write a cancel command file for cross-platform cancellation.
pub fn write_cancel_command(pid: u32) -> anyhow::Result<()> {
    let commands_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("yoop")
        .join("commands");

    std::fs::create_dir_all(&commands_dir)?;

    let command_file = commands_dir.join(format!("{}.cmd", pid));
    std::fs::write(command_file, "cancel")?;

    Ok(())
}

/// Check for and consume a cancel command.
pub fn check_cancel_command(pid: u32) -> bool {
    let commands_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("yoop")
        .join("commands");

    let command_file = commands_dir.join(format!("{}.cmd", pid));

    if command_file.exists() {
        let _ = std::fs::remove_file(&command_file);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_file_default() {
        let state = SessionStateFile::default();
        assert_eq!(state.version, STATE_FILE_VERSION);
        assert!(state.sessions.is_empty());
        assert!(state.clipboard_sync.is_none());
    }

    #[test]
    fn test_session_entry_serialization() {
        let entry = SessionEntry {
            id: Uuid::new_v4(),
            session_type: "share".to_string(),
            code: Some("A7K9".to_string()),
            pid: 12345,
            started_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
            files: vec![FileEntry {
                name: "test.txt".to_string(),
                size: 1024,
                transferred: 0,
                status: "pending".to_string(),
            }],
            peer: None,
            progress: ProgressEntry::default(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: SessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_type, "share");
        assert_eq!(parsed.code, Some("A7K9".to_string()));
    }

    #[test]
    fn test_add_and_remove_session() {
        let mut state = SessionStateFile::default();
        let id = Uuid::new_v4();

        state.add_session(SessionEntry {
            id,
            session_type: "share".to_string(),
            code: Some("TEST".to_string()),
            pid: std::process::id(),
            started_at: chrono::Utc::now(),
            expires_at: None,
            files: vec![],
            peer: None,
            progress: ProgressEntry::default(),
        });

        assert_eq!(state.sessions.len(), 1);
        assert!(state.find_session(id).is_some());

        state.remove_session(id);
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_progress_update() {
        let mut state = SessionStateFile::default();
        let id = Uuid::new_v4();

        state.add_session(SessionEntry {
            id,
            session_type: "share".to_string(),
            code: None,
            pid: std::process::id(),
            started_at: chrono::Utc::now(),
            expires_at: None,
            files: vec![],
            peer: None,
            progress: ProgressEntry::default(),
        });

        state.update_session_progress(
            id,
            ProgressEntry {
                transferred: 500,
                total: 1000,
                speed_bps: 100,
            },
        );

        let session = state.find_session(id).unwrap();
        assert_eq!(session.progress.transferred, 500);
        assert_eq!(session.progress.total, 1000);
    }

    #[test]
    fn test_process_alive_current_process() {
        let current_pid = std::process::id();
        assert!(is_process_alive(current_pid));
    }

    #[test]
    fn test_process_alive_nonexistent() {
        let fake_pid = u32::MAX - 1;
        assert!(!is_process_alive(fake_pid));
    }
}
