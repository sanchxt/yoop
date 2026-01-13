//! File system watching for directory synchronization.
//!
//! This module provides cross-platform file system watching using the `notify` crate.
//! It handles:
//! - Real-time file system event detection
//! - Event debouncing to coalesce rapid changes
//! - Pattern-based file exclusion
//! - File size filtering
//! - Platform-specific quirks

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use super::{RelativePath, SyncConfig};
use crate::Result;

/// File system event detected by the watcher.
#[derive(Debug, Clone)]
pub struct FileEvent {
    /// Relative path from sync root
    pub path: RelativePath,
    /// Type of event
    pub kind: FileEventKind,
    /// When the event was detected
    pub timestamp: Instant,
}

/// Type of file system event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEventKind {
    /// File or directory was created
    Created,
    /// File content was modified
    Modified,
    /// File or directory was deleted
    Deleted,
}

/// Watches a directory for file system changes.
///
/// The watcher monitors a directory tree for changes and emits debounced events.
/// It respects exclusion patterns and file size limits from the configuration.
///
/// # Example
///
/// ```rust,ignore
/// let config = SyncConfig {
///     sync_root: PathBuf::from("/path/to/sync"),
///     ..Default::default()
/// };
///
/// let mut watcher = FileWatcher::new(config)?;
/// watcher.start()?;
///
/// while let Some(event) = watcher.next_event().await {
///     println!("Change detected: {:?}", event);
/// }
/// ```
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    event_rx: mpsc::UnboundedReceiver<FileEvent>,
    config: Arc<SyncConfig>,
    debouncer: Debouncer,
}

impl FileWatcher {
    /// Create a new file watcher for the given configuration.
    ///
    /// The watcher is created but not started. Call `start()` to begin watching.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The sync root directory doesn't exist
    /// - The exclusion patterns are invalid
    /// - The file system watcher cannot be created
    pub fn new(config: SyncConfig) -> Result<Self> {
        if !config.sync_root.exists() {
            return Err(crate::Error::DirectoryNotFound(
                config.sync_root.display().to_string(),
            ));
        }

        let config = Arc::new(config);
        let config_clone = Arc::clone(&config);

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let watcher = recommended_watcher(move |result: notify::Result<Event>| {
            if let Ok(event) = result {
                if let Err(e) = Self::handle_notify_event(&config_clone, &event, &event_tx) {
                    tracing::warn!("Error handling file event: {}", e);
                }
            }
        })
        .map_err(|e| crate::Error::WatcherError(e.to_string()))?;

        let debouncer = Debouncer::new(config.debounce_ms);

        Ok(Self {
            _watcher: watcher,
            event_rx,
            config,
            debouncer,
        })
    }

    /// Start watching the configured directory.
    ///
    /// This begins monitoring the directory tree for changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the watcher cannot start watching the directory.
    pub fn start(&mut self) -> Result<()> {
        self._watcher
            .watch(&self.config.sync_root, RecursiveMode::Recursive)
            .map_err(|e| crate::Error::WatcherError(e.to_string()))
    }

    /// Stop watching the directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the watcher cannot stop watching the directory.
    pub fn stop(&mut self) -> Result<()> {
        self._watcher
            .unwatch(&self.config.sync_root)
            .map_err(|e| crate::Error::WatcherError(e.to_string()))
    }

    /// Receive the next debounced file event.
    ///
    /// This is an async method that waits for the next event. Events are debounced
    /// according to the configured debounce window.
    ///
    /// Returns `None` if the event channel is closed.
    pub async fn next_event(&mut self) -> Option<FileEvent> {
        loop {
            tokio::select! {
                event = self.event_rx.recv() => {
                    if let Some(event) = event {
                        self.debouncer.add(event);
                    } else {
                        return self.debouncer.flush_all().into_iter().next();
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(self.config.debounce_ms)) => {
                    if let Some(event) = self.debouncer.flush_next() {
                        return Some(event);
                    }
                }
            }
        }
    }

    /// Handle a notify event and convert it to our FileEvent format.
    fn handle_notify_event(
        config: &SyncConfig,
        event: &Event,
        tx: &mpsc::UnboundedSender<FileEvent>,
    ) -> Result<()> {
        let kind = match &event.kind {
            EventKind::Create(_) => FileEventKind::Created,
            EventKind::Modify(_) => FileEventKind::Modified,
            EventKind::Remove(_) => FileEventKind::Deleted,
            _ => return Ok(()), // Ignore other event types
        };

        for path in &event.paths {
            if let Ok(rel_path) = RelativePath::from_absolute(path, &config.sync_root) {
                if should_process_file(config, path, &kind)? {
                    let file_event = FileEvent {
                        path: rel_path,
                        kind: kind.clone(),
                        timestamp: Instant::now(),
                    };

                    let _ = tx.send(file_event);
                }
            }
        }

        Ok(())
    }
}

/// Check if a file should be processed based on exclusion patterns, size limits, and kind.
fn should_process_file(config: &SyncConfig, path: &Path, kind: &FileEventKind) -> Result<bool> {
    // Check exclusion patterns first
    let pattern_matcher = PatternMatcher::new(&config.exclude_patterns)?;
    if pattern_matcher.is_excluded(path) {
        tracing::debug!("Skipping excluded file: {}", path.display());
        return Ok(false);
    }

    // For deletions, no need to check file metadata
    if *kind == FileEventKind::Deleted {
        return Ok(true);
    }

    if !path.exists() {
        return Ok(false);
    }

    let metadata = std::fs::metadata(path)?;

    // Check file size limits
    if metadata.is_file()
        && config.max_file_size > 0
        && metadata.len() > config.max_file_size
    {
        tracing::debug!(
            "Skipping file {} (size {} exceeds max {})",
            path.display(),
            metadata.len(),
            config.max_file_size
        );
        return Ok(false);
    }

    Ok(true)
}

/// Debouncer to coalesce rapid file system events.
///
/// When files are saved, editors often generate multiple events in quick succession
/// (truncate, write, flush, etc.). The debouncer collects these events and emits
/// only the final state after a quiet period.
struct Debouncer {
    pending: HashMap<RelativePath, (FileEvent, Instant)>,
    window_ms: u64,
}

impl Debouncer {
    /// Create a new debouncer with the specified window in milliseconds.
    fn new(window_ms: u64) -> Self {
        Self {
            pending: HashMap::new(),
            window_ms,
        }
    }

    /// Add an event to the debouncer.
    ///
    /// Events for the same path replace previous events. The most recent
    /// event is kept.
    fn add(&mut self, event: FileEvent) {
        self.pending
            .insert(event.path.clone(), (event, Instant::now()));
    }

    /// Flush and return the next ready event.
    ///
    /// Returns events that have been quiet for longer than the debounce window.
    fn flush_next(&mut self) -> Option<FileEvent> {
        let now = Instant::now();
        let window = Duration::from_millis(self.window_ms);

        let ready_path = self
            .pending
            .iter()
            .find(|(_, (_, time))| now.duration_since(*time) >= window)
            .map(|(path, _)| path.clone());

        if let Some(path) = ready_path {
            self.pending.remove(&path).map(|(event, _)| event)
        } else {
            None
        }
    }

    /// Flush all pending events regardless of debounce window.
    ///
    /// Used when shutting down to ensure no events are lost.
    fn flush_all(&mut self) -> Vec<FileEvent> {
        let events: Vec<_> = self
            .pending
            .drain()
            .map(|(_, (event, _))| event)
            .collect();
        events
    }
}

/// Pattern matcher for file exclusions.
struct PatternMatcher {
    set: globset::GlobSet,
}

impl PatternMatcher {
    /// Create a new pattern matcher from glob patterns.
    fn new(patterns: &[String]) -> Result<Self> {
        let mut builder = globset::GlobSetBuilder::new();

        for pattern in patterns {
            let glob = globset::Glob::new(pattern)
                .map_err(|e| crate::Error::InvalidPath(format!("Invalid glob pattern: {e}")))?;
            builder.add(glob);
        }

        let set = builder
            .build()
            .map_err(|e| crate::Error::Internal(format!("Failed to build glob set: {e}")))?;

        Ok(Self { set })
    }

    /// Check if a path matches any exclusion pattern.
    fn is_excluded(&self, path: &Path) -> bool {
        self.set.is_match(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_file_event_kind() {
        let kind1 = FileEventKind::Created;
        let kind2 = FileEventKind::Modified;
        let kind3 = FileEventKind::Deleted;

        assert_eq!(kind1, FileEventKind::Created);
        assert_eq!(kind2, FileEventKind::Modified);
        assert_eq!(kind3, FileEventKind::Deleted);
        assert_ne!(kind1, kind2);
    }

    #[test]
    fn test_file_event_creation() {
        let event = FileEvent {
            path: RelativePath::new("test.txt"),
            kind: FileEventKind::Created,
            timestamp: Instant::now(),
        };

        assert_eq!(event.path.as_str(), "test.txt");
        assert_eq!(event.kind, FileEventKind::Created);
    }

    #[test]
    fn test_debouncer_add_and_flush() {
        let mut debouncer = Debouncer::new(100);

        let event1 = FileEvent {
            path: RelativePath::new("file1.txt"),
            kind: FileEventKind::Created,
            timestamp: Instant::now(),
        };

        let event2 = FileEvent {
            path: RelativePath::new("file2.txt"),
            kind: FileEventKind::Modified,
            timestamp: Instant::now(),
        };

        debouncer.add(event1);
        debouncer.add(event2);

        assert_eq!(debouncer.pending.len(), 2);

        std::thread::sleep(Duration::from_millis(150));

        let flushed1 = debouncer.flush_next();
        assert!(flushed1.is_some());

        let flushed2 = debouncer.flush_next();
        assert!(flushed2.is_some());
    }

    #[test]
    fn test_debouncer_coalesce_same_path() {
        let mut debouncer = Debouncer::new(100);

        let event1 = FileEvent {
            path: RelativePath::new("test.txt"),
            kind: FileEventKind::Created,
            timestamp: Instant::now(),
        };

        let event2 = FileEvent {
            path: RelativePath::new("test.txt"),
            kind: FileEventKind::Modified,
            timestamp: Instant::now(),
        };

        debouncer.add(event1);
        debouncer.add(event2);

        assert_eq!(debouncer.pending.len(), 1);

        let event = debouncer.pending.get(&RelativePath::new("test.txt"));
        assert!(event.is_some());
        assert_eq!(event.unwrap().0.kind, FileEventKind::Modified);
    }

    #[test]
    fn test_debouncer_flush_all() {
        let mut debouncer = Debouncer::new(100);

        for i in 0..5 {
            let event = FileEvent {
                path: RelativePath::new(format!("file{}.txt", i)),
                kind: FileEventKind::Created,
                timestamp: Instant::now(),
            };
            debouncer.add(event);
        }

        let all_events = debouncer.flush_all();
        assert_eq!(all_events.len(), 5);
        assert_eq!(debouncer.pending.len(), 0);
    }

    #[test]
    fn test_pattern_matcher_basic() {
        let patterns = vec![
            "*.tmp".to_string(),
            ".git".to_string(),
            "node_modules".to_string(),
        ];
        let matcher = PatternMatcher::new(&patterns).unwrap();

        assert!(matcher.is_excluded(Path::new("file.tmp")));
        assert!(matcher.is_excluded(Path::new(".git")));
        assert!(matcher.is_excluded(Path::new("node_modules")));
        assert!(!matcher.is_excluded(Path::new("file.txt")));
        assert!(!matcher.is_excluded(Path::new("src")));
    }

    #[test]
    fn test_pattern_matcher_nested_paths() {
        let patterns = vec!["*.log".to_string(), "target/**".to_string()];
        let matcher = PatternMatcher::new(&patterns).unwrap();

        assert!(matcher.is_excluded(Path::new("error.log")));
        assert!(matcher.is_excluded(Path::new("target/debug/app")));
        assert!(!matcher.is_excluded(Path::new("src/main.rs")));
    }

    #[test]
    fn test_pattern_matcher_invalid_pattern() {
        let patterns = vec!["[invalid".to_string()];
        let result = PatternMatcher::new(&patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_watcher_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let result = FileWatcher::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_watcher_nonexistent_directory() {
        let config = SyncConfig {
            sync_root: PathBuf::from("/nonexistent/directory"),
            ..Default::default()
        };

        let result = FileWatcher::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_watcher_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let mut watcher = FileWatcher::new(config).unwrap();
        assert!(watcher.start().is_ok());
        assert!(watcher.stop().is_ok());
    }

    #[tokio::test]
    async fn test_file_watcher_detect_create() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            debounce_ms: 50,
            ..Default::default()
        };

        let mut watcher = FileWatcher::new(config).unwrap();
        watcher.start().unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let file_path = temp_dir.path().join("test.txt");
        fs::File::create(&file_path)
            .unwrap()
            .write_all(b"test content")
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        if let Some(event) = watcher.next_event().await {
            assert_eq!(event.path.as_str(), "test.txt");
            // Platform-specific: some systems emit Modified instead of Created
            assert!(
                event.kind == FileEventKind::Created || event.kind == FileEventKind::Modified
            );
        }
    }

    #[tokio::test]
    async fn test_file_watcher_detect_modify() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::File::create(&file_path)
            .unwrap()
            .write_all(b"initial")
            .unwrap();

        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            debounce_ms: 50,
            ..Default::default()
        };

        let mut watcher = FileWatcher::new(config).unwrap();
        watcher.start().unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        fs::File::create(&file_path)
            .unwrap()
            .write_all(b"modified content")
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        if let Some(event) = watcher.next_event().await {
            assert_eq!(event.path.as_str(), "test.txt");
            assert!(
                event.kind == FileEventKind::Modified || event.kind == FileEventKind::Created
            );
        }
    }

    #[tokio::test]
    async fn test_file_watcher_detect_delete() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::File::create(&file_path)
            .unwrap()
            .write_all(b"content")
            .unwrap();

        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            debounce_ms: 50,
            ..Default::default()
        };

        let mut watcher = FileWatcher::new(config).unwrap();
        watcher.start().unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        fs::remove_file(&file_path).unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        if let Some(event) = watcher.next_event().await {
            assert_eq!(event.path.as_str(), "test.txt");
            assert_eq!(event.kind, FileEventKind::Deleted);
        }
    }

    #[tokio::test]
    async fn test_file_watcher_exclusion_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            exclude_patterns: vec!["*.tmp".to_string()],
            debounce_ms: 50,
            ..Default::default()
        };

        let mut watcher = FileWatcher::new(config).unwrap();
        watcher.start().unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        fs::File::create(temp_dir.path().join("test.txt"))
            .unwrap()
            .write_all(b"content")
            .unwrap();

        fs::File::create(temp_dir.path().join("test.tmp"))
            .unwrap()
            .write_all(b"temp")
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        if let Some(event) = watcher.next_event().await {
            assert_eq!(event.path.as_str(), "test.txt");
        }
    }

    #[tokio::test]
    async fn test_file_watcher_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            max_file_size: 100,
            debounce_ms: 50,
            ..Default::default()
        };

        let mut watcher = FileWatcher::new(config).unwrap();
        watcher.start().unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let small_file = temp_dir.path().join("small.txt");
        fs::File::create(&small_file)
            .unwrap()
            .write_all(b"small content")
            .unwrap();

        let large_file = temp_dir.path().join("large.txt");
        let large_content = vec![b'x'; 200];
        fs::File::create(&large_file)
            .unwrap()
            .write_all(&large_content)
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        if let Some(event) = watcher.next_event().await {
            assert_eq!(event.path.as_str(), "small.txt");
        }
    }

    #[test]
    fn test_should_process_file_deleted() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            max_file_size: 100,
            ..Default::default()
        };

        let result =
            should_process_file(&config, temp_dir.path(), &FileEventKind::Deleted).unwrap();
        assert!(result);
    }

    #[test]
    fn test_should_process_file_size_check() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = vec![b'x'; 200];
        fs::File::create(&file_path)
            .unwrap()
            .write_all(&content)
            .unwrap();

        let config = SyncConfig {
            sync_root: temp_dir.path().to_path_buf(),
            max_file_size: 100,
            ..Default::default()
        };

        let result = should_process_file(&config, &file_path, &FileEventKind::Created).unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_debouncer_timing() {
        let mut debouncer = Debouncer::new(100);

        let event = FileEvent {
            path: RelativePath::new("test.txt"),
            kind: FileEventKind::Modified,
            timestamp: Instant::now(),
        };

        debouncer.add(event);

        let immediate = debouncer.flush_next();
        assert!(immediate.is_none());

        tokio::time::sleep(Duration::from_millis(120)).await;

        let delayed = debouncer.flush_next();
        assert!(delayed.is_some());
    }
}
