//! File index management for directory synchronization.
//!
//! The file index tracks all files in the synced directory, including their
//! metadata and content hashes. It's used for:
//! - Initial reconciliation between peers
//! - Change detection during live sync
//! - Conflict detection

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use walkdir::WalkDir;
use xxhash_rust::xxh64::xxh64;

use super::{FileKind, RelativePath, SyncConfig, SyncOp};
use crate::Result;

/// Entry in the file index.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path from sync root
    pub path: RelativePath,
    /// Type of file system entry
    pub kind: FileKind,
    /// File size in bytes (0 for directories)
    pub size: u64,
    /// Last modification time
    pub mtime: SystemTime,
    /// xxHash64 of file content (0 for directories)
    pub content_hash: u64,
}

impl FileEntry {
    /// Create entry from file system path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or metadata is unavailable.
    pub fn from_path(root: &Path, rel_path: RelativePath) -> Result<Self> {
        let abs_path = rel_path.to_path(root);
        let metadata = fs::metadata(&abs_path)?;

        let kind = if metadata.is_dir() {
            FileKind::Directory
        } else if metadata.is_symlink() {
            FileKind::Symlink
        } else {
            FileKind::File
        };

        let size = if kind == FileKind::File {
            metadata.len()
        } else {
            0
        };

        let mtime = metadata.modified()?;

        let content_hash = if kind == FileKind::File {
            compute_file_hash(&abs_path)?
        } else {
            0
        };

        Ok(Self {
            path: rel_path,
            kind,
            size,
            mtime,
            content_hash,
        })
    }

    /// Check if content has changed compared to another entry.
    #[must_use]
    pub fn content_changed(&self, other: &Self) -> bool {
        self.content_hash != other.content_hash
    }

    /// Check if this entry is newer than another based on mtime.
    #[must_use]
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.mtime > other.mtime
    }
}

/// Index of all files in the sync directory.
#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    entries: HashMap<RelativePath, FileEntry>,
    root_hash: u64,
}

impl FileIndex {
    /// Build index from a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    pub fn build(root: &Path, config: &SyncConfig) -> Result<Self> {
        let mut entries = HashMap::new();

        let pattern_matcher = build_pattern_matcher(&config.exclude_patterns)?;

        for entry in WalkDir::new(root)
            .follow_links(config.follow_symlinks)
            .into_iter()
            .filter_entry(|e| !pattern_matcher.is_excluded(e.path()))
        {
            let entry = entry.map_err(|e| crate::Error::Io(e.into()))?;
            let abs_path = entry.path();

            if abs_path == root {
                continue;
            }

            let rel_path = RelativePath::from_absolute(abs_path, root)?;

            if let Ok(file_entry) = FileEntry::from_path(root, rel_path.clone()) {
                if config.max_file_size > 0
                    && file_entry.kind == FileKind::File
                    && file_entry.size > config.max_file_size
                {
                    tracing::debug!(
                        "Skipping file {} (size {} exceeds max {})",
                        rel_path.as_str(),
                        file_entry.size,
                        config.max_file_size
                    );
                    continue;
                }

                entries.insert(rel_path, file_entry);
            }
        }

        let root_hash = compute_index_hash(&entries);

        Ok(Self { entries, root_hash })
    }

    /// Create index from entries (used when receiving remote index).
    #[must_use]
    pub fn from_entries(entries: HashMap<RelativePath, FileEntry>) -> Self {
        let root_hash = compute_index_hash(&entries);
        Self { entries, root_hash }
    }

    /// Get entry by path.
    #[must_use]
    pub fn get(&self, path: &RelativePath) -> Option<&FileEntry> {
        self.entries.get(path)
    }

    /// Insert or update entry.
    pub fn insert(&mut self, entry: FileEntry) {
        self.entries.insert(entry.path.clone(), entry);
        self.root_hash = compute_index_hash(&self.entries);
    }

    /// Remove entry by path.
    pub fn remove(&mut self, path: &RelativePath) -> Option<FileEntry> {
        let entry = self.entries.remove(path);
        if entry.is_some() {
            self.root_hash = compute_index_hash(&self.entries);
        }
        entry
    }

    /// Get all entries as an iterator.
    pub fn entries(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries.values()
    }

    /// Compute operations needed to sync with a remote index.
    ///
    /// This performs a three-way comparison:
    /// - Files only in remote: Create
    /// - Files in both with different content: Modify (if remote is newer)
    /// - Files only in local: (remote will handle sending to us)
    #[must_use]
    pub fn diff(&self, remote: &Self) -> Vec<SyncOp> {
        let mut ops = Vec::new();

        for remote_entry in remote.entries() {
            match self.get(&remote_entry.path) {
                None => {
                    ops.push(SyncOp::Create {
                        path: remote_entry.path.clone(),
                        kind: remote_entry.kind,
                        size: remote_entry.size,
                        content_hash: remote_entry.content_hash,
                    });
                }
                Some(local_entry) if local_entry.content_changed(remote_entry) => {
                    if remote_entry.is_newer_than(local_entry) {
                        ops.push(SyncOp::Modify {
                            path: remote_entry.path.clone(),
                            size: remote_entry.size,
                            content_hash: remote_entry.content_hash,
                        });
                    }
                }
                _ => {}
            }
        }

        ops
    }

    /// Get hash of entire index for quick comparison.
    #[must_use]
    pub fn root_hash(&self) -> u64 {
        self.root_hash
    }

    /// Get number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total size of all files.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.entries
            .values()
            .filter(|e| e.kind == FileKind::File)
            .map(|e| e.size)
            .sum()
    }
}

/// Pattern matcher for exclusion rules.
struct PatternMatcher {
    set: globset::GlobSet,
}

impl PatternMatcher {
    /// Check if a path matches any exclusion pattern.
    fn is_excluded(&self, path: &Path) -> bool {
        self.set.is_match(path)
    }
}

/// Build a pattern matcher from exclusion patterns.
fn build_pattern_matcher(patterns: &[String]) -> Result<PatternMatcher> {
    let mut builder = globset::GlobSetBuilder::new();

    for pattern in patterns {
        let glob = globset::Glob::new(pattern)
            .map_err(|e| crate::Error::InvalidPath(format!("Invalid glob pattern: {e}")))?;
        builder.add(glob);
    }

    let set = builder
        .build()
        .map_err(|e| crate::Error::Internal(format!("Failed to build glob set: {e}")))?;

    Ok(PatternMatcher { set })
}

/// Compute xxHash64 of a file's content.
fn compute_file_hash(path: &Path) -> Result<u64> {
    let data = fs::read(path)?;
    Ok(xxh64(&data, 0))
}

/// Compute hash of the entire index.
fn compute_index_hash(entries: &HashMap<RelativePath, FileEntry>) -> u64 {
    let mut paths: Vec<_> = entries.keys().collect();
    paths.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let mut combined = String::new();
    for path in paths {
        if let Some(entry) = entries.get(path) {
            combined.push_str(path.as_str());
            combined.push_str(&entry.content_hash.to_string());
        }
    }

    xxh64(combined.as_bytes(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_file_entry_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"test content").unwrap();

        let rel_path = RelativePath::new("test.txt");
        let entry = FileEntry::from_path(temp_dir.path(), rel_path.clone()).unwrap();

        assert_eq!(entry.path, rel_path);
        assert_eq!(entry.kind, FileKind::File);
        assert_eq!(entry.size, 12);
        assert_ne!(entry.content_hash, 0);
    }

    #[test]
    fn test_file_entry_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("subdir");
        fs::create_dir(&dir_path).unwrap();

        let rel_path = RelativePath::new("subdir");
        let entry = FileEntry::from_path(temp_dir.path(), rel_path.clone()).unwrap();

        assert_eq!(entry.path, rel_path);
        assert_eq!(entry.kind, FileKind::Directory);
        assert_eq!(entry.size, 0);
        assert_eq!(entry.content_hash, 0);
    }

    #[test]
    fn test_file_entry_content_changed() {
        let entry1 = FileEntry {
            path: RelativePath::new("test.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: SystemTime::now(),
            content_hash: 12345,
        };

        let entry2 = FileEntry {
            path: RelativePath::new("test.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: SystemTime::now(),
            content_hash: 67890,
        };

        assert!(entry1.content_changed(&entry2));
        assert!(!entry1.content_changed(&entry1));
    }

    #[test]
    fn test_file_index_build() {
        let temp_dir = TempDir::new().unwrap();

        fs::File::create(temp_dir.path().join("file1.txt"))
            .unwrap()
            .write_all(b"content1")
            .unwrap();

        fs::File::create(temp_dir.path().join("file2.txt"))
            .unwrap()
            .write_all(b"content2")
            .unwrap();

        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let index = FileIndex::build(temp_dir.path(), &config).unwrap();

        assert_eq!(index.len(), 3);
        assert!(index.get(&RelativePath::new("file1.txt")).is_some());
        assert!(index.get(&RelativePath::new("file2.txt")).is_some());
        assert!(index.get(&RelativePath::new("subdir")).is_some());
    }

    #[test]
    fn test_file_index_exclusions() {
        let temp_dir = TempDir::new().unwrap();

        fs::File::create(temp_dir.path().join("file.txt"))
            .unwrap()
            .write_all(b"content")
            .unwrap();

        fs::File::create(temp_dir.path().join("file.tmp"))
            .unwrap()
            .write_all(b"temp")
            .unwrap();

        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            exclude_patterns: vec!["*.tmp".to_string()],
            ..Default::default()
        };

        let index = FileIndex::build(temp_dir.path(), &config).unwrap();

        assert_eq!(index.len(), 1);
        assert!(index.get(&RelativePath::new("file.txt")).is_some());
        assert!(index.get(&RelativePath::new("file.tmp")).is_none());
    }

    #[test]
    fn test_file_index_insert_remove() {
        let mut index = FileIndex::default();

        let entry = FileEntry {
            path: RelativePath::new("test.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: SystemTime::now(),
            content_hash: 12345,
        };

        index.insert(entry);
        assert_eq!(index.len(), 1);
        assert!(index.get(&RelativePath::new("test.txt")).is_some());

        let removed = index.remove(&RelativePath::new("test.txt"));
        assert!(removed.is_some());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_file_index_diff() {
        use std::time::Duration;

        let mut local = FileIndex::default();
        let mut remote = FileIndex::default();

        // Create local entry with an older mtime
        let local_time = SystemTime::now() - Duration::from_secs(10);
        let remote_time = SystemTime::now();

        let entry1 = FileEntry {
            path: RelativePath::new("both.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: local_time,
            content_hash: 12345,
        };

        // Remote entry has newer mtime and different content
        let entry2 = FileEntry {
            path: RelativePath::new("both.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: remote_time,
            content_hash: 67890,
        };

        let entry3 = FileEntry {
            path: RelativePath::new("remote_only.txt"),
            kind: FileKind::File,
            size: 50,
            mtime: remote_time,
            content_hash: 11111,
        };

        local.insert(entry1);
        remote.insert(entry2);
        remote.insert(entry3);

        let ops = local.diff(&remote);

        assert_eq!(ops.len(), 2);

        let has_modify = ops.iter().any(|op| matches!(op, SyncOp::Modify { .. }));
        let has_create = ops.iter().any(|op| matches!(op, SyncOp::Create { .. }));

        assert!(has_modify);
        assert!(has_create);
    }

    #[test]
    fn test_file_index_total_size() {
        let mut index = FileIndex::default();

        index.insert(FileEntry {
            path: RelativePath::new("file1.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: SystemTime::now(),
            content_hash: 12345,
        });

        index.insert(FileEntry {
            path: RelativePath::new("file2.txt"),
            kind: FileKind::File,
            size: 200,
            mtime: SystemTime::now(),
            content_hash: 67890,
        });

        index.insert(FileEntry {
            path: RelativePath::new("dir"),
            kind: FileKind::Directory,
            size: 0,
            mtime: SystemTime::now(),
            content_hash: 0,
        });

        assert_eq!(index.total_size(), 300);
    }

    #[test]
    fn test_root_hash_consistency() {
        let mut index1 = FileIndex::default();
        let mut index2 = FileIndex::default();

        let entry = FileEntry {
            path: RelativePath::new("test.txt"),
            kind: FileKind::File,
            size: 100,
            mtime: SystemTime::now(),
            content_hash: 12345,
        };

        index1.insert(entry.clone());
        index2.insert(entry);

        assert_eq!(index1.root_hash(), index2.root_hash());
    }

    #[test]
    fn test_pattern_matcher() {
        let patterns = vec![
            "*.tmp".to_string(),
            ".git".to_string(),
            "node_modules".to_string(),
        ];
        let matcher = build_pattern_matcher(&patterns).unwrap();

        assert!(matcher.is_excluded(Path::new("file.tmp")));
        assert!(matcher.is_excluded(Path::new(".git")));
        assert!(matcher.is_excluded(Path::new("node_modules")));
        assert!(!matcher.is_excluded(Path::new("file.txt")));
    }

    #[test]
    fn test_compute_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        fs::File::create(&file_path)
            .unwrap()
            .write_all(b"test content")
            .unwrap();

        let hash1 = compute_file_hash(&file_path).unwrap();
        let hash2 = compute_file_hash(&file_path).unwrap();

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, 0);
    }
}
