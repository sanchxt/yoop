//! Error types for Yoop.
//!
//! This module provides a unified error type for all Yoop operations,
//! with specific error variants for different failure modes.

use std::io;
use std::net::SocketAddr;

use thiserror::Error;

/// A specialized `Result` type for Yoop operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The main error type for Yoop.
#[derive(Error, Debug)]
pub enum Error {
    /// No network connection detected (E001)
    #[error("no network connection detected")]
    NoNetwork,

    /// Unable to broadcast on network (E002)
    #[error("unable to broadcast on network: {0}")]
    BroadcastFailed(String),

    /// Code not found on network (E003)
    #[error("code '{0}' not found on network")]
    CodeNotFound(String),

    /// Code has expired (E004)
    #[error("code has expired")]
    CodeExpired,

    /// Code collision detected during generation
    #[error("code collision detected, unable to generate unique code")]
    CodeCollision,

    /// Connection lost during transfer (E005)
    #[error("connection lost during transfer to {0}")]
    ConnectionLost(SocketAddr),

    /// Checksum mismatch detected (E006)
    #[error("checksum mismatch for chunk {chunk} of file '{file}'")]
    ChecksumMismatch {
        /// The file being transferred
        file: String,
        /// The chunk number that failed
        chunk: u64,
    },

    /// Transfer was cancelled
    #[error("transfer cancelled")]
    TransferCancelled,

    /// Transfer rejected by receiver
    #[error("transfer rejected by receiver")]
    TransferRejected,

    /// Resume state doesn't match current transfer
    #[error("resume mismatch: {0}")]
    ResumeMismatch(String),

    /// Resume request was rejected by sender
    #[error("resume rejected: {0}")]
    ResumeRejected(String),

    /// Cannot read file: permission denied (E007)
    #[error("cannot read file '{0}': permission denied")]
    PermissionDenied(String),

    /// Insufficient disk space (E008)
    #[error("insufficient disk space: need {needed} bytes, have {available} bytes")]
    InsufficientSpace {
        /// Bytes needed
        needed: u64,
        /// Bytes available
        available: u64,
    },

    /// File not found
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// Invalid path
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Too many failed attempts (E009)
    #[error("too many failed attempts, locked for {0} seconds")]
    RateLimited(u64),

    /// Connection rejected by sender (E010)
    #[error("connection rejected by sender")]
    ConnectionRejected,

    /// Invalid code format
    #[error("invalid code format: {0}")]
    InvalidCodeFormat(String),

    /// TLS handshake failed
    #[error("TLS handshake failed: {0}")]
    TlsError(String),

    /// Signature verification failed
    #[error("signature verification failed")]
    SignatureInvalid,

    /// Invalid protocol message
    #[error("invalid protocol message: {0}")]
    ProtocolError(String),

    /// Unsupported protocol version
    #[error("unsupported protocol version: {major}.{minor}")]
    UnsupportedVersion {
        /// Major version
        major: u8,
        /// Minor version
        minor: u8,
    },

    /// Unexpected message type
    #[error("unexpected message type: expected {expected}, got {actual}")]
    UnexpectedMessage {
        /// Expected message type
        expected: String,
        /// Actual message type received
        actual: String,
    },

    /// Configuration file error
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Invalid configuration value
    #[error("invalid configuration value for '{key}': {reason}")]
    InvalidConfig {
        /// Configuration key
        key: String,
        /// Reason for invalidity
        reason: String,
    },

    /// Device not trusted
    #[error("device '{0}' is not trusted")]
    DeviceNotTrusted(String),

    /// Trust database error
    #[error("trust database error: {0}")]
    TrustDbError(String),

    /// Preview generation failed
    #[error("failed to generate preview for '{file}': {reason}")]
    PreviewFailed {
        /// File path
        file: String,
        /// Reason for failure
        reason: String,
    },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Internal error (should not happen)
    #[error("internal error: {0}")]
    Internal(String),

    /// Operation timeout
    #[error("operation timed out after {0} seconds")]
    Timeout(u64),

    /// Keep-alive failed (connection may be dead)
    #[error("keep-alive failed: no response after {0} seconds")]
    KeepAliveFailed(u64),

    /// Clipboard access failed
    #[error("clipboard error: {0}")]
    ClipboardError(String),

    /// Clipboard is empty
    #[error("clipboard is empty")]
    ClipboardEmpty,

    /// Unsupported clipboard content type
    #[error("unsupported clipboard content type: {0}")]
    UnsupportedClipboardType(String),

    /// Migration failed
    #[error("migration failed from {from} to {to}: {reason}")]
    MigrationFailed {
        /// Version migrating from
        from: String,
        /// Version migrating to
        to: String,
        /// Reason for failure
        reason: String,
    },

    /// Backup operation failed
    #[error("backup failed: {0}")]
    BackupFailed(String),

    /// Rollback operation failed
    #[error("rollback failed: {0}")]
    RollbackFailed(String),

    /// No backup available
    #[error("no backup available for rollback")]
    NoBackupAvailable,

    /// Update check failed
    #[error("failed to check for updates: {0}")]
    UpdateCheckFailed(String),

    /// Already on latest version
    #[error("already on the latest version ({0})")]
    AlreadyLatest(String),

    /// Package manager not found
    #[error("package manager '{0}' not found in PATH")]
    PackageManagerNotFound(String),

    /// Update command execution failed
    #[error("update command failed: {0}")]
    UpdateCommandFailed(String),

    /// File system watcher error
    #[error("file watcher error: {0}")]
    WatcherError(String),

    /// Sync session error
    #[error("sync error: {0}")]
    SyncError(String),

    /// File conflict detected during sync
    #[error("sync conflict on '{path}': local and remote both modified")]
    SyncConflict {
        /// Path where conflict occurred
        path: String,
    },

    /// Directory not found
    #[error("directory not found: {0}")]
    DirectoryNotFound(String),

    /// Sync operation failed
    #[error("sync operation failed: {0}")]
    SyncOperationFailed(String),
}

impl Error {
    /// Returns the error code associated with this error, if any.
    ///
    /// Error codes follow the pattern EXXX where XXX is a 3-digit number.
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::NoNetwork => Some("E001"),
            Self::BroadcastFailed(_) => Some("E002"),
            Self::CodeNotFound(_) => Some("E003"),
            Self::CodeExpired => Some("E004"),
            Self::ConnectionLost(_) => Some("E005"),
            Self::ChecksumMismatch { .. } => Some("E006"),
            Self::PermissionDenied(_) => Some("E007"),
            Self::InsufficientSpace { .. } => Some("E008"),
            Self::RateLimited(_) => Some("E009"),
            Self::ConnectionRejected => Some("E010"),
            _ => None,
        }
    }

    /// Returns whether this error is recoverable (can be retried).
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionLost(_)
                | Self::ChecksumMismatch { .. }
                | Self::RateLimited(_)
                | Self::Timeout(_)
                | Self::KeepAliveFailed(_)
        )
    }

    /// Returns a helpful suggestion for resolving the error, if applicable.
    #[must_use]
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::PackageManagerNotFound(_) => Some(
                "Install Node.js (includes npm) from https://nodejs.org\n\
                 Or install pnpm: npm install -g pnpm\n\
                 Or install yarn: npm install -g yarn\n\
                 Or install bun: curl -fsSL https://bun.sh/install | bash",
            ),
            Self::UpdateCheckFailed(_) => Some(
                "Check your internet connection and try again.\n\
                 You can also manually check: https://www.npmjs.com/package/yoop",
            ),
            Self::MigrationFailed { .. } => Some(
                "Your data has been backed up. Try:\n\
                   yoop update --rollback",
            ),
            Self::UpdateCommandFailed(_) => Some(
                "Try running the update manually:\n\
                   npm install -g yoop\n\
                 Or check permissions (may need sudo on some systems)",
            ),
            Self::NoBackupAvailable => Some(
                "No backup found to rollback to. You may need to reinstall manually:\n\
                   npm install -g yoop",
            ),
            Self::RollbackFailed(_) => Some(
                "Failed to rollback. You may need to manually reinstall:\n\
                   npm install -g yoop",
            ),
            _ => None,
        }
    }
}
