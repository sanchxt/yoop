//! Sync reconciliation engine.
//!
//! This module handles the reconciliation logic for directory synchronization,
//! including comparing file indices and generating sync operations.

use std::collections::HashMap;

use super::conflict::{Conflict, ConflictDetector, ConflictResolution, ResolutionStrategy};
use super::index::FileIndex;
use super::{FileKind, SyncOp};

/// Engine for reconciling file indices and generating sync operations.
#[derive(Debug)]
pub struct SyncEngine {
    conflict_detector: ConflictDetector,
}

impl SyncEngine {
    /// Create a new sync engine with the given conflict resolution strategy.
    #[must_use]
    pub fn new(strategy: ResolutionStrategy) -> Self {
        Self {
            conflict_detector: ConflictDetector::new(strategy),
        }
    }

    /// Reconcile local and remote file indices.
    ///
    /// Returns a tuple of:
    /// - Operations to apply locally (receive from remote)
    /// - Operations to send to remote
    /// - Conflicts detected
    pub fn reconcile(
        &self,
        local: &FileIndex,
        remote: &FileIndex,
    ) -> (Vec<SyncOp>, Vec<SyncOp>, Vec<Conflict>) {
        let mut local_ops = Vec::new();
        let mut remote_ops = Vec::new();
        let mut conflicts = Vec::new();

        let mut processed_paths = HashMap::new();

        for remote_entry in remote.entries() {
            processed_paths.insert(remote_entry.path.clone(), true);

            match local.get(&remote_entry.path) {
                None => {
                    local_ops.push(SyncOp::Create {
                        path: remote_entry.path.clone(),
                        kind: remote_entry.kind,
                        size: remote_entry.size,
                        content_hash: remote_entry.content_hash,
                    });
                }
                Some(local_entry) if local_entry.content_changed(remote_entry) => {
                    if let Some(conflict) = self.conflict_detector.detect(
                        remote_entry.path.clone(),
                        local_entry.mtime,
                        local_entry.content_hash,
                        remote_entry.mtime,
                        remote_entry.content_hash,
                        remote_entry.kind,
                    ) {
                        conflicts.push(conflict);
                    } else if remote_entry.is_newer_than(local_entry) {
                        local_ops.push(SyncOp::Modify {
                            path: remote_entry.path.clone(),
                            size: remote_entry.size,
                            content_hash: remote_entry.content_hash,
                        });
                    } else {
                        remote_ops.push(SyncOp::Modify {
                            path: local_entry.path.clone(),
                            size: local_entry.size,
                            content_hash: local_entry.content_hash,
                        });
                    }
                }
                _ => {}
            }
        }

        for local_entry in local.entries() {
            if processed_paths.contains_key(&local_entry.path) {
                continue;
            }

            remote_ops.push(SyncOp::Create {
                path: local_entry.path.clone(),
                kind: local_entry.kind,
                size: local_entry.size,
                content_hash: local_entry.content_hash,
            });
        }

        (local_ops, remote_ops, conflicts)
    }

    /// Apply conflict resolution and update operations.
    ///
    /// This modifies the operation lists based on conflict resolutions.
    pub fn apply_conflict_resolutions(
        &self,
        conflicts: &[Conflict],
        local_ops: &mut Vec<SyncOp>,
        _remote_ops: &mut Vec<SyncOp>,
    ) -> Vec<ConflictResolution> {
        let mut resolutions = Vec::new();

        for conflict in conflicts {
            let resolution = self.conflict_detector.resolve(conflict);

            match &resolution {
                ConflictResolution::AcceptRemote => {
                    local_ops.push(SyncOp::Modify {
                        path: conflict.path.clone(),
                        size: 0,
                        content_hash: conflict.remote_hash,
                    });
                }
                ConflictResolution::KeepLocal => {}
                ConflictResolution::RenameThenAcceptRemote { rename_to } => {
                    local_ops.push(SyncOp::Rename {
                        from: conflict.path.clone(),
                        to: rename_to.clone(),
                        kind: conflict.kind,
                    });
                    local_ops.push(SyncOp::Modify {
                        path: conflict.path.clone(),
                        size: 0,
                        content_hash: conflict.remote_hash,
                    });
                }
            }

            resolutions.push(resolution);
        }

        resolutions
    }

    /// Get the conflict detector.
    #[must_use]
    pub fn conflict_detector(&self) -> &ConflictDetector {
        &self.conflict_detector
    }
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self::new(ResolutionStrategy::default())
    }
}

/// Plan operations for efficient sync.
///
/// Groups operations by type and priority for optimal execution order.
#[derive(Debug, Default)]
pub struct SyncPlan {
    /// Directory creations (must happen before file creates)
    pub dir_creates: Vec<SyncOp>,
    /// File creates and modifies
    pub file_ops: Vec<SyncOp>,
    /// Renames
    pub renames: Vec<SyncOp>,
    /// Deletions (must happen after file ops)
    pub deletes: Vec<SyncOp>,
}

impl SyncPlan {
    /// Create a sync plan from a list of operations.
    #[must_use]
    pub fn from_ops(ops: Vec<SyncOp>) -> Self {
        let mut plan = Self::default();

        for op in ops {
            match op {
                SyncOp::Create {
                    kind: FileKind::Directory,
                    ..
                } => {
                    plan.dir_creates.push(op);
                }
                SyncOp::Create { .. } | SyncOp::Modify { .. } => {
                    plan.file_ops.push(op);
                }
                SyncOp::Rename { .. } => {
                    plan.renames.push(op);
                }
                SyncOp::Delete { .. } => {
                    plan.deletes.push(op);
                }
            }
        }

        plan
    }

    /// Get all operations in execution order.
    #[must_use]
    pub fn into_ordered_ops(self) -> Vec<SyncOp> {
        let mut ops = Vec::new();
        ops.extend(self.dir_creates);
        ops.extend(self.renames);
        ops.extend(self.file_ops);
        ops.extend(self.deletes);
        ops
    }

    /// Get total number of operations.
    #[must_use]
    pub fn total_ops(&self) -> usize {
        self.dir_creates.len() + self.file_ops.len() + self.renames.len() + self.deletes.len()
    }

    /// Check if plan is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total_ops() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::index::FileEntry;
    use crate::sync::RelativePath;
    use std::time::SystemTime;

    fn make_entry(path: &str, hash: u64, mtime: SystemTime) -> FileEntry {
        FileEntry {
            path: RelativePath::new(path),
            kind: FileKind::File,
            size: 100,
            mtime,
            content_hash: hash,
        }
    }

    #[test]
    fn test_reconcile_empty_indices() {
        let engine = SyncEngine::default();
        let local = FileIndex::default();
        let remote = FileIndex::default();

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert!(local_ops.is_empty());
        assert!(remote_ops.is_empty());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_reconcile_remote_only_file() {
        let engine = SyncEngine::default();
        let local = FileIndex::default();
        let mut remote = FileIndex::default();

        let entry = make_entry("file.txt", 12345, SystemTime::now());
        remote.insert(entry);

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert_eq!(local_ops.len(), 1);
        assert!(matches!(local_ops[0], SyncOp::Create { .. }));
        assert!(remote_ops.is_empty());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_reconcile_local_only_file() {
        let engine = SyncEngine::default();
        let mut local = FileIndex::default();
        let remote = FileIndex::default();

        let entry = make_entry("file.txt", 12345, SystemTime::now());
        local.insert(entry);

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert!(local_ops.is_empty());
        assert_eq!(remote_ops.len(), 1);
        assert!(matches!(remote_ops[0], SyncOp::Create { .. }));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_reconcile_same_files() {
        let engine = SyncEngine::default();
        let mut local = FileIndex::default();
        let mut remote = FileIndex::default();

        let now = SystemTime::now();
        let entry1 = make_entry("file.txt", 12345, now);
        let entry2 = make_entry("file.txt", 12345, now);

        local.insert(entry1);
        remote.insert(entry2);

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert!(local_ops.is_empty());
        assert!(remote_ops.is_empty());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_reconcile_remote_newer() {
        let engine = SyncEngine::default();
        let mut local = FileIndex::default();
        let mut remote = FileIndex::default();

        let now = SystemTime::now();
        let older = now - std::time::Duration::from_secs(10);

        let local_entry = make_entry("file.txt", 12345, older);
        let remote_entry = make_entry("file.txt", 67890, now);

        local.insert(local_entry);
        remote.insert(remote_entry);

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert_eq!(local_ops.len(), 1);
        assert!(matches!(local_ops[0], SyncOp::Modify { .. }));
        assert!(remote_ops.is_empty());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_reconcile_local_newer() {
        let engine = SyncEngine::default();
        let mut local = FileIndex::default();
        let mut remote = FileIndex::default();

        let now = SystemTime::now();
        let older = now - std::time::Duration::from_secs(10);

        let local_entry = make_entry("file.txt", 67890, now);
        let remote_entry = make_entry("file.txt", 12345, older);

        local.insert(local_entry);
        remote.insert(remote_entry);

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert!(local_ops.is_empty());
        assert_eq!(remote_ops.len(), 1);
        assert!(matches!(remote_ops[0], SyncOp::Modify { .. }));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_reconcile_conflict_detected() {
        let engine = SyncEngine::default();
        let mut local = FileIndex::default();
        let mut remote = FileIndex::default();

        let now = SystemTime::now();
        let recent = now - std::time::Duration::from_secs(1);

        let local_entry = make_entry("file.txt", 12345, now);
        let remote_entry = make_entry("file.txt", 67890, recent);

        local.insert(local_entry);
        remote.insert(remote_entry);

        let (local_ops, remote_ops, conflicts) = engine.reconcile(&local, &remote);

        assert_eq!(conflicts.len(), 1);
        assert!(local_ops.is_empty() || remote_ops.is_empty());
    }

    #[test]
    fn test_apply_conflict_resolutions_accept_remote() {
        let engine = SyncEngine::new(ResolutionStrategy::PreferRemote);
        let now = SystemTime::now();

        let conflict = Conflict::new(
            RelativePath::new("file.txt"),
            now,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let mut local_ops = Vec::new();
        let mut remote_ops = Vec::new();

        let resolutions = engine.apply_conflict_resolutions(&[conflict], &mut local_ops, &mut remote_ops);

        assert_eq!(resolutions.len(), 1);
        assert!(matches!(
            resolutions[0],
            ConflictResolution::AcceptRemote
        ));
        assert_eq!(local_ops.len(), 1);
    }

    #[test]
    fn test_apply_conflict_resolutions_keep_local() {
        let engine = SyncEngine::new(ResolutionStrategy::PreferLocal);
        let now = SystemTime::now();

        let conflict = Conflict::new(
            RelativePath::new("file.txt"),
            now,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let mut local_ops = Vec::new();
        let mut remote_ops = Vec::new();

        let resolutions = engine.apply_conflict_resolutions(&[conflict], &mut local_ops, &mut remote_ops);

        assert_eq!(resolutions.len(), 1);
        assert!(matches!(resolutions[0], ConflictResolution::KeepLocal));
        assert!(local_ops.is_empty());
    }

    #[test]
    fn test_apply_conflict_resolutions_keep_both() {
        let engine = SyncEngine::new(ResolutionStrategy::KeepBoth);
        let now = SystemTime::now();

        let conflict = Conflict::new(
            RelativePath::new("file.txt"),
            now,
            12345,
            now,
            67890,
            FileKind::File,
        );

        let mut local_ops = Vec::new();
        let mut remote_ops = Vec::new();

        let resolutions = engine.apply_conflict_resolutions(&[conflict], &mut local_ops, &mut remote_ops);

        assert_eq!(resolutions.len(), 1);
        assert!(matches!(
            resolutions[0],
            ConflictResolution::RenameThenAcceptRemote { .. }
        ));
        assert_eq!(local_ops.len(), 2);
        assert!(matches!(local_ops[0], SyncOp::Rename { .. }));
        assert!(matches!(local_ops[1], SyncOp::Modify { .. }));
    }

    #[test]
    fn test_sync_plan_from_ops() {
        let ops = vec![
            SyncOp::Create {
                path: RelativePath::new("dir"),
                kind: FileKind::Directory,
                size: 0,
                content_hash: 0,
            },
            SyncOp::Create {
                path: RelativePath::new("file.txt"),
                kind: FileKind::File,
                size: 100,
                content_hash: 12345,
            },
            SyncOp::Modify {
                path: RelativePath::new("other.txt"),
                size: 200,
                content_hash: 67890,
            },
            SyncOp::Rename {
                from: RelativePath::new("old.txt"),
                to: RelativePath::new("new.txt"),
                kind: FileKind::File,
            },
            SyncOp::Delete {
                path: RelativePath::new("deleted.txt"),
                kind: FileKind::File,
            },
        ];

        let plan = SyncPlan::from_ops(ops);

        assert_eq!(plan.dir_creates.len(), 1);
        assert_eq!(plan.file_ops.len(), 2);
        assert_eq!(plan.renames.len(), 1);
        assert_eq!(plan.deletes.len(), 1);
        assert_eq!(plan.total_ops(), 5);
        assert!(!plan.is_empty());
    }

    #[test]
    fn test_sync_plan_into_ordered_ops() {
        let ops = vec![
            SyncOp::Delete {
                path: RelativePath::new("deleted.txt"),
                kind: FileKind::File,
            },
            SyncOp::Modify {
                path: RelativePath::new("file.txt"),
                size: 100,
                content_hash: 12345,
            },
            SyncOp::Create {
                path: RelativePath::new("dir"),
                kind: FileKind::Directory,
                size: 0,
                content_hash: 0,
            },
            SyncOp::Rename {
                from: RelativePath::new("old.txt"),
                to: RelativePath::new("new.txt"),
                kind: FileKind::File,
            },
        ];

        let plan = SyncPlan::from_ops(ops);
        let ordered = plan.into_ordered_ops();

        assert_eq!(ordered.len(), 4);
        assert!(matches!(ordered[0], SyncOp::Create { kind: FileKind::Directory, .. }));
        assert!(matches!(ordered[1], SyncOp::Rename { .. }));
        assert!(matches!(ordered[2], SyncOp::Modify { .. }));
        assert!(matches!(ordered[3], SyncOp::Delete { .. }));
    }

    #[test]
    fn test_sync_plan_empty() {
        let plan = SyncPlan::default();
        assert!(plan.is_empty());
        assert_eq!(plan.total_ops(), 0);
    }

    #[test]
    fn test_engine_conflict_detector() {
        let engine = SyncEngine::new(ResolutionStrategy::KeepBoth);
        let detector = engine.conflict_detector();
        assert_eq!(detector.strategy(), ResolutionStrategy::KeepBoth);
    }
}
