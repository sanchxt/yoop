//! Directory synchronization module.
//!
//! This module provides real-time bidirectional directory synchronization
//! between two connected devices. Files, directories, and their modifications
//! are automatically synced across devices.
//!
//! ## Features
//!
//! - Real-time file system watching
//! - Bidirectional synchronization
//! - Conflict resolution (last-write-wins)
//! - Exclusion pattern support (.gitignore-style)
//! - Content-based change detection (xxHash64)
//!
//! ## Example
//!
//! ```rust,ignore
//! use yoop_core::sync::{SyncSession, SyncConfig};
//!
//! // Host a sync session
//! let config = SyncConfig::default();
//! let (code, session) = SyncSession::host(config, transfer_config).await?;
//! println!("Share code: {}", code);
//!
//! // On another device, connect
//! let session = SyncSession::connect("A7K9", config, transfer_config).await?;
//! session.run(|event| println!("{:?}", event)).await?;
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod conflict;
pub mod engine;
pub mod index;
pub mod session;
pub mod watcher;

pub use conflict::{
    Conflict, ConflictDetector, ConflictResolution, ConflictVersion, ResolutionStrategy,
};
pub use engine::{SyncEngine, SyncPlan};
pub use session::{SyncEvent, SyncSession};

/// Configuration for a sync session.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Directory to sync
    pub sync_root: PathBuf,

    /// Patterns to exclude (gitignore-style)
    pub exclude_patterns: Vec<String>,

    /// Whether to follow symbolic links
    pub follow_symlinks: bool,

    /// Whether to sync deletions
    pub sync_deletions: bool,

    /// Debounce window for file events (ms)
    pub debounce_ms: u64,

    /// Maximum file size to sync (0 = unlimited)
    pub max_file_size: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_root: PathBuf::new(),
            exclude_patterns: vec![
                ".git".into(),
                ".DS_Store".into(),
                "Thumbs.db".into(),
                "*.swp".into(),
                "*.tmp".into(),
            ],
            follow_symlinks: false,
            sync_deletions: true,
            debounce_ms: 100,
            max_file_size: 0,
        }
    }
}

/// Kind of file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum FileKind {
    /// Regular file
    File = 0,
    /// Directory
    Directory = 1,
    /// Symbolic link
    Symlink = 2,
}

/// A relative path within the sync root.
///
/// Paths are normalized to use forward slashes regardless of platform.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelativePath(String);

impl RelativePath {
    /// Create a new relative path, normalizing path separators.
    #[must_use]
    pub fn new(path: impl AsRef<str>) -> Self {
        Self(path.as_ref().replace('\\', "/"))
    }

    /// Get the path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to an absolute path given the sync root.
    #[must_use]
    pub fn to_path(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }

    /// Create from a path relative to the sync root.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not relative to the root.
    pub fn from_absolute(absolute: &Path, root: &Path) -> crate::Result<Self> {
        let rel = absolute
            .strip_prefix(root)
            .map_err(|_| crate::Error::InvalidPath(absolute.display().to_string()))?;

        Ok(Self::new(rel.to_str().ok_or_else(|| {
            crate::Error::InvalidPath(absolute.display().to_string())
        })?))
    }
}

/// A sync operation to be applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncOp {
    /// File or directory created
    Create {
        /// Relative path
        path: RelativePath,
        /// File kind
        kind: FileKind,
        /// File size (0 for directories)
        size: u64,
        /// Content hash (0 for directories)
        content_hash: u64,
    },
    /// File content modified
    Modify {
        /// Relative path
        path: RelativePath,
        /// New file size
        size: u64,
        /// New content hash
        content_hash: u64,
    },
    /// File or directory deleted
    Delete {
        /// Relative path
        path: RelativePath,
        /// File kind
        kind: FileKind,
    },
    /// File or directory renamed/moved
    Rename {
        /// Original path
        from: RelativePath,
        /// New path
        to: RelativePath,
        /// File kind
        kind: FileKind,
    },
}

impl SyncOp {
    /// Get the target path of this operation.
    #[must_use]
    pub fn path(&self) -> &RelativePath {
        match self {
            Self::Create { path, .. } | Self::Modify { path, .. } | Self::Delete { path, .. } => {
                path
            }
            Self::Rename { to, .. } => to,
        }
    }

    /// Get the operation type as a string.
    #[must_use]
    pub fn operation_type(&self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Modify { .. } => "modify",
            Self::Delete { .. } => "delete",
            Self::Rename { .. } => "rename",
        }
    }
}

/// Statistics from a sync session.
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Total session duration
    pub duration: std::time::Duration,
    /// Number of files sent
    pub files_sent: u64,
    /// Number of files received
    pub files_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of conflicts encountered
    pub conflicts: u64,
    /// Number of errors
    pub errors: u64,
}

impl SyncStats {
    /// Create a new empty stats instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get total number of operations.
    #[must_use]
    pub fn total_operations(&self) -> u64 {
        self.files_sent + self.files_received
    }

    /// Get total bytes transferred.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relative_path_normalization() {
        let path1 = RelativePath::new("foo/bar/baz.txt");
        let path2 = RelativePath::new("foo\\bar\\baz.txt");

        assert_eq!(path1.as_str(), "foo/bar/baz.txt");
        assert_eq!(path2.as_str(), "foo/bar/baz.txt");
        assert_eq!(path1, path2);
    }

    #[test]
    fn test_relative_path_to_path() {
        let rel = RelativePath::new("foo/bar.txt");
        let root = PathBuf::from("/home/user/sync");
        let abs = rel.to_path(&root);

        assert_eq!(abs, PathBuf::from("/home/user/sync/foo/bar.txt"));
    }

    #[test]
    fn test_relative_path_from_absolute() {
        let root = PathBuf::from("/home/user/sync");
        let abs = PathBuf::from("/home/user/sync/foo/bar.txt");

        let rel = RelativePath::from_absolute(&abs, &root).unwrap();
        assert_eq!(rel.as_str(), "foo/bar.txt");
    }

    #[test]
    fn test_relative_path_from_absolute_error() {
        let root = PathBuf::from("/home/user/sync");
        let abs = PathBuf::from("/other/path/file.txt");

        let result = RelativePath::from_absolute(&abs, &root);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_config_default() {
        let config = SyncConfig::default();

        assert_eq!(config.debounce_ms, 100);
        assert!(config.sync_deletions);
        assert!(!config.follow_symlinks);
        assert_eq!(config.max_file_size, 0);
        assert!(config.exclude_patterns.contains(&".git".to_string()));
    }

    #[test]
    fn test_sync_op_path() {
        let path = RelativePath::new("test.txt");

        let create_op = SyncOp::Create {
            path: path.clone(),
            kind: FileKind::File,
            size: 100,
            content_hash: 12345,
        };
        assert_eq!(create_op.path(), &path);

        let modify_op = SyncOp::Modify {
            path: path.clone(),
            size: 200,
            content_hash: 67890,
        };
        assert_eq!(modify_op.path(), &path);

        let delete_op = SyncOp::Delete {
            path: path.clone(),
            kind: FileKind::File,
        };
        assert_eq!(delete_op.path(), &path);

        let rename_op = SyncOp::Rename {
            from: RelativePath::new("old.txt"),
            to: path.clone(),
            kind: FileKind::File,
        };
        assert_eq!(rename_op.path(), &path);
    }

    #[test]
    fn test_sync_op_operation_type() {
        let path = RelativePath::new("test.txt");

        let create_op = SyncOp::Create {
            path: path.clone(),
            kind: FileKind::File,
            size: 100,
            content_hash: 12345,
        };
        assert_eq!(create_op.operation_type(), "create");

        let modify_op = SyncOp::Modify {
            path: path.clone(),
            size: 200,
            content_hash: 67890,
        };
        assert_eq!(modify_op.operation_type(), "modify");

        let delete_op = SyncOp::Delete {
            path: path.clone(),
            kind: FileKind::File,
        };
        assert_eq!(delete_op.operation_type(), "delete");

        let rename_op = SyncOp::Rename {
            from: RelativePath::new("old.txt"),
            to: path,
            kind: FileKind::File,
        };
        assert_eq!(rename_op.operation_type(), "rename");
    }

    #[test]
    fn test_file_kind_serialization() {
        let file = FileKind::File;
        let dir = FileKind::Directory;
        let link = FileKind::Symlink;

        assert_eq!(file as u8, 0);
        assert_eq!(dir as u8, 1);
        assert_eq!(link as u8, 2);
    }

    #[test]
    fn test_sync_stats_default() {
        let stats = SyncStats::new();

        assert_eq!(stats.files_sent, 0);
        assert_eq!(stats.files_received, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.conflicts, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.total_bytes(), 0);
    }

    #[test]
    fn test_sync_stats_totals() {
        let mut stats = SyncStats::new();
        stats.files_sent = 10;
        stats.files_received = 5;
        stats.bytes_sent = 1024;
        stats.bytes_received = 512;

        assert_eq!(stats.total_operations(), 15);
        assert_eq!(stats.total_bytes(), 1536);
    }
}
