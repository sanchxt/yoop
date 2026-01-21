//! Conflict detection and resolution for directory synchronization.
//!
//! When both devices modify the same file between syncs, a conflict occurs.
//! This module provides mechanisms to detect and resolve such conflicts.

use std::time::{Duration, SystemTime};

use super::{FileKind, RelativePath};

/// A conflict between local and remote versions of a file.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// Path where the conflict occurred
    pub path: RelativePath,
    /// Local file's last modification time
    pub local_mtime: SystemTime,
    /// Local file's content hash
    pub local_hash: u64,
    /// Remote file's last modification time
    pub remote_mtime: SystemTime,
    /// Remote file's content hash
    pub remote_hash: u64,
    /// File kind
    pub kind: FileKind,
}

impl Conflict {
    /// Create a new conflict.
    #[must_use]
    pub fn new(
        path: RelativePath,
        local_mtime: SystemTime,
        local_hash: u64,
        remote_mtime: SystemTime,
        remote_hash: u64,
        kind: FileKind,
    ) -> Self {
        Self {
            path,
            local_mtime,
            local_hash,
            remote_mtime,
            remote_hash,
            kind,
        }
    }

    /// Check if this is truly a conflict (both modified, ambiguous timing).
    ///
    /// A conflict exists if:
    /// 1. Content differs (different hashes)
    /// 2. Modification times are within the ambiguity window
    #[must_use]
    pub fn is_ambiguous(&self, window_secs: u64) -> bool {
        if self.local_hash == self.remote_hash {
            return false;
        }

        let time_diff = self
            .local_mtime
            .duration_since(self.remote_mtime)
            .or_else(|_| self.remote_mtime.duration_since(self.local_mtime))
            .unwrap_or_default();

        time_diff < Duration::from_secs(window_secs)
    }

    /// Get which version is newer based on modification time.
    #[must_use]
    pub fn newer_version(&self) -> ConflictVersion {
        if self.remote_mtime > self.local_mtime {
            ConflictVersion::Remote
        } else {
            ConflictVersion::Local
        }
    }

    /// Get the time difference between versions in seconds.
    #[must_use]
    pub fn time_diff_secs(&self) -> u64 {
        self.local_mtime
            .duration_since(self.remote_mtime)
            .or_else(|_| self.remote_mtime.duration_since(self.local_mtime))
            .unwrap_or_default()
            .as_secs()
    }
}

/// Which version of a file in a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictVersion {
    /// Local version
    Local,
    /// Remote version
    Remote,
}

/// Resolution strategy for a conflict.
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    /// Accept the remote version (overwrite local)
    AcceptRemote,
    /// Keep the local version (don't sync)
    KeepLocal,
    /// Rename local file, then accept remote
    RenameThenAcceptRemote {
        /// Path to rename local file to
        rename_to: RelativePath,
    },
}

impl ConflictResolution {
    /// Generate a conflicted filename based on the original path and timestamp.
    ///
    /// Example: `file.txt` becomes `file.conflict.1234567890.txt`
    #[must_use]
    pub fn generate_conflict_name(path: &RelativePath, mtime: SystemTime) -> RelativePath {
        let path_str = path.as_str();
        let timestamp = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        path_str.rfind('.').map_or_else(
            || RelativePath::new(format!("{path_str}.conflict.{timestamp}")),
            |dot_idx| {
                let (name, ext) = path_str.split_at(dot_idx);
                RelativePath::new(format!("{name}.conflict.{timestamp}{ext}"))
            },
        )
    }
}

/// Conflict resolution strategy selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStrategy {
    /// Always accept the most recently modified version
    LastWriteWins,
    /// Keep both versions by renaming the local file
    KeepBoth,
    /// Always keep local version
    PreferLocal,
    /// Always accept remote version
    PreferRemote,
}

impl ResolutionStrategy {
    /// Resolve a conflict using this strategy.
    #[must_use]
    pub fn resolve(&self, conflict: &Conflict) -> ConflictResolution {
        match self {
            Self::LastWriteWins => resolve_last_write_wins(conflict),
            Self::KeepBoth => resolve_keep_both(conflict),
            Self::PreferLocal => ConflictResolution::KeepLocal,
            Self::PreferRemote => ConflictResolution::AcceptRemote,
        }
    }
}

impl Default for ResolutionStrategy {
    fn default() -> Self {
        Self::LastWriteWins
    }
}

/// Resolve conflict using last-write-wins strategy.
///
/// The file with the most recent modification time is accepted.
#[must_use]
pub fn resolve_last_write_wins(conflict: &Conflict) -> ConflictResolution {
    match conflict.newer_version() {
        ConflictVersion::Remote => ConflictResolution::AcceptRemote,
        ConflictVersion::Local => ConflictResolution::KeepLocal,
    }
}

/// Resolve conflict using keep-both strategy.
///
/// The local file is renamed with a conflict marker, then the remote version is accepted.
#[must_use]
pub fn resolve_keep_both(conflict: &Conflict) -> ConflictResolution {
    let rename_to =
        ConflictResolution::generate_conflict_name(&conflict.path, conflict.local_mtime);
    ConflictResolution::RenameThenAcceptRemote { rename_to }
}

/// Conflict detector for sync operations.
#[derive(Debug)]
pub struct ConflictDetector {
    strategy: ResolutionStrategy,
    ambiguity_window_secs: u64,
}

impl ConflictDetector {
    /// Create a new conflict detector with the given strategy.
    #[must_use]
    pub fn new(strategy: ResolutionStrategy) -> Self {
        Self {
            strategy,
            ambiguity_window_secs: 2,
        }
    }

    /// Set the ambiguity window for conflict detection.
    ///
    /// Files modified within this window are considered potentially conflicting.
    #[must_use]
    pub fn with_ambiguity_window(mut self, secs: u64) -> Self {
        self.ambiguity_window_secs = secs;
        self
    }

    /// Detect if a conflict exists between local and remote versions.
    ///
    /// Returns `Some(Conflict)` if both versions have been modified and timing is ambiguous.
    #[must_use]
    pub fn detect(
        &self,
        path: RelativePath,
        local_mtime: SystemTime,
        local_hash: u64,
        remote_mtime: SystemTime,
        remote_hash: u64,
        kind: FileKind,
    ) -> Option<Conflict> {
        if local_hash == remote_hash {
            return None;
        }

        let conflict = Conflict::new(
            path,
            local_mtime,
            local_hash,
            remote_mtime,
            remote_hash,
            kind,
        );

        if conflict.is_ambiguous(self.ambiguity_window_secs) {
            Some(conflict)
        } else {
            None
        }
    }

    /// Resolve a conflict using the configured strategy.
    #[must_use]
    pub fn resolve(&self, conflict: &Conflict) -> ConflictResolution {
        self.strategy.resolve(conflict)
    }

    /// Get the configured resolution strategy.
    #[must_use]
    pub fn strategy(&self) -> ResolutionStrategy {
        self.strategy
    }
}

impl Default for ConflictDetector {
    fn default() -> Self {
        Self::new(ResolutionStrategy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_conflict_is_ambiguous() {
        let now = SystemTime::now();
        let recent = now - Duration::from_secs(1);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            recent,
            67890,
            FileKind::File,
        );

        assert!(conflict.is_ambiguous(2));
        assert!(!conflict.is_ambiguous(0));
    }

    #[test]
    fn test_conflict_is_not_ambiguous_same_hash() {
        let now = SystemTime::now();
        let recent = now - Duration::from_secs(1);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            recent,
            12345,
            FileKind::File,
        );

        assert!(!conflict.is_ambiguous(10));
    }

    #[test]
    fn test_conflict_newer_version() {
        let now = SystemTime::now();
        let older = now - Duration::from_secs(10);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            older,
            67890,
            FileKind::File,
        );

        assert_eq!(conflict.newer_version(), ConflictVersion::Local);

        let conflict2 = Conflict::new(
            RelativePath::new("test.txt"),
            older,
            12345,
            now,
            67890,
            FileKind::File,
        );

        assert_eq!(conflict2.newer_version(), ConflictVersion::Remote);
    }

    #[test]
    fn test_conflict_time_diff() {
        let now = SystemTime::now();
        let older = now - Duration::from_secs(5);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            older,
            67890,
            FileKind::File,
        );

        assert_eq!(conflict.time_diff_secs(), 5);
    }

    #[test]
    fn test_generate_conflict_name_with_extension() {
        let path = RelativePath::new("document.txt");
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_234_567_890);

        let conflict_name = ConflictResolution::generate_conflict_name(&path, mtime);
        assert_eq!(conflict_name.as_str(), "document.conflict.1234567890.txt");
    }

    #[test]
    fn test_generate_conflict_name_without_extension() {
        let path = RelativePath::new("README");
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(9_876_543_210);

        let conflict_name = ConflictResolution::generate_conflict_name(&path, mtime);
        assert_eq!(conflict_name.as_str(), "README.conflict.9876543210");
    }

    #[test]
    fn test_generate_conflict_name_multiple_dots() {
        let path = RelativePath::new("archive.tar.gz");
        let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_111_111_111);

        let conflict_name = ConflictResolution::generate_conflict_name(&path, mtime);
        assert_eq!(conflict_name.as_str(), "archive.tar.conflict.1111111111.gz");
    }

    #[test]
    fn test_resolve_last_write_wins_remote_newer() {
        let now = SystemTime::now();
        let older = now - Duration::from_secs(10);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            older,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = resolve_last_write_wins(&conflict);
        assert!(matches!(resolution, ConflictResolution::AcceptRemote));
    }

    #[test]
    fn test_resolve_last_write_wins_local_newer() {
        let now = SystemTime::now();
        let older = now - Duration::from_secs(10);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            older,
            67890,
            FileKind::File,
        );

        let resolution = resolve_last_write_wins(&conflict);
        assert!(matches!(resolution, ConflictResolution::KeepLocal));
    }

    #[test]
    fn test_resolve_keep_both() {
        let now = SystemTime::now();
        let older = now - Duration::from_secs(10);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            older,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = resolve_keep_both(&conflict);
        assert!(matches!(
            resolution,
            ConflictResolution::RenameThenAcceptRemote { .. }
        ));

        if let ConflictResolution::RenameThenAcceptRemote { rename_to } = resolution {
            assert!(rename_to.as_str().contains("conflict"));
            assert!(std::path::Path::new(rename_to.as_str())
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("txt")));
        }
    }

    #[test]
    fn test_resolution_strategy_last_write_wins() {
        let strategy = ResolutionStrategy::LastWriteWins;
        let now = SystemTime::now();
        let older = now - Duration::from_secs(10);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            older,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = strategy.resolve(&conflict);
        assert!(matches!(resolution, ConflictResolution::AcceptRemote));
    }

    #[test]
    fn test_resolution_strategy_keep_both() {
        let strategy = ResolutionStrategy::KeepBoth;
        let now = SystemTime::now();

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = strategy.resolve(&conflict);
        assert!(matches!(
            resolution,
            ConflictResolution::RenameThenAcceptRemote { .. }
        ));
    }

    #[test]
    fn test_resolution_strategy_prefer_local() {
        let strategy = ResolutionStrategy::PreferLocal;
        let now = SystemTime::now();

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = strategy.resolve(&conflict);
        assert!(matches!(resolution, ConflictResolution::KeepLocal));
    }

    #[test]
    fn test_resolution_strategy_prefer_remote() {
        let strategy = ResolutionStrategy::PreferRemote;
        let now = SystemTime::now();

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            now,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = strategy.resolve(&conflict);
        assert!(matches!(resolution, ConflictResolution::AcceptRemote));
    }

    #[test]
    fn test_conflict_detector_detect_conflict() {
        let detector = ConflictDetector::default();
        let now = SystemTime::now();
        let recent = now - Duration::from_secs(1);

        let conflict = detector.detect(
            RelativePath::new("test.txt"),
            now,
            12345,
            recent,
            67890,
            FileKind::File,
        );

        assert!(conflict.is_some());
    }

    #[test]
    fn test_conflict_detector_no_conflict_same_hash() {
        let detector = ConflictDetector::default();
        let now = SystemTime::now();
        let recent = now - Duration::from_secs(1);

        let conflict = detector.detect(
            RelativePath::new("test.txt"),
            now,
            12345,
            recent,
            12345,
            FileKind::File,
        );

        assert!(conflict.is_none());
    }

    #[test]
    fn test_conflict_detector_no_conflict_clear_winner() {
        let detector = ConflictDetector::default();
        let now = SystemTime::now();
        let much_older = now - Duration::from_secs(100);

        let conflict = detector.detect(
            RelativePath::new("test.txt"),
            now,
            12345,
            much_older,
            67890,
            FileKind::File,
        );

        assert!(conflict.is_none());
    }

    #[test]
    fn test_conflict_detector_with_custom_window() {
        let detector = ConflictDetector::default().with_ambiguity_window(10);
        let now = SystemTime::now();
        let recent = now - Duration::from_secs(5);

        let conflict = detector.detect(
            RelativePath::new("test.txt"),
            now,
            12345,
            recent,
            67890,
            FileKind::File,
        );

        assert!(conflict.is_some());
    }

    #[test]
    fn test_conflict_detector_resolve() {
        let detector = ConflictDetector::new(ResolutionStrategy::LastWriteWins);
        let now = SystemTime::now();
        let older = now - Duration::from_secs(10);

        let conflict = Conflict::new(
            RelativePath::new("test.txt"),
            older,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let resolution = detector.resolve(&conflict);
        assert!(matches!(resolution, ConflictResolution::AcceptRemote));
    }

    #[test]
    fn test_conflict_detector_strategy() {
        let detector = ConflictDetector::new(ResolutionStrategy::KeepBoth);
        assert_eq!(detector.strategy(), ResolutionStrategy::KeepBoth);
    }

    #[test]
    fn test_resolution_strategy_default() {
        let strategy = ResolutionStrategy::default();
        assert_eq!(strategy, ResolutionStrategy::LastWriteWins);
    }
}
