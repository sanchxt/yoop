//! File watcher for session state updates.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc;

/// Event indicating the session state file has changed.
#[derive(Debug, Clone)]
pub enum StateEvent {
    /// The file was modified.
    Modified,
    /// The file was deleted.
    Deleted,
    /// An error occurred while watching.
    Error(String),
}

/// Watches the session state file for changes.
pub struct StateWatcher {
    /// Path to the state file.
    path: PathBuf,
    /// Channel to send events.
    tx: mpsc::Sender<StateEvent>,
    /// Last known modification time.
    last_modified: Option<SystemTime>,
    /// Poll interval.
    poll_interval: Duration,
}

impl StateWatcher {
    /// Create a new state watcher.
    pub fn new(poll_interval: Duration) -> (Self, mpsc::Receiver<StateEvent>) {
        let (tx, rx) = mpsc::channel(16);
        let path = super::SessionStateFile::path();
        let last_modified = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());

        (
            Self {
                path,
                tx,
                last_modified,
                poll_interval,
            },
            rx,
        )
    }

    /// Create a watcher with default 500ms poll interval.
    pub fn default_interval() -> (Self, mpsc::Receiver<StateEvent>) {
        Self::new(Duration::from_millis(500))
    }

    /// Start watching the state file.
    ///
    /// This spawns a background task that polls the file for changes.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Run the watcher loop.
    async fn run(mut self) {
        loop {
            tokio::time::sleep(self.poll_interval).await;

            match self.check_for_changes() {
                Ok(Some(event)) => {
                    if self.tx.send(event).await.is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = self.tx.send(StateEvent::Error(e.to_string())).await;
                }
            }
        }
    }

    /// Check if the file has changed.
    fn check_for_changes(&mut self) -> anyhow::Result<Option<StateEvent>> {
        match std::fs::metadata(&self.path) {
            Ok(metadata) => {
                let modified = metadata.modified()?;

                if self.last_modified == Some(modified) {
                    Ok(None)
                } else {
                    self.last_modified = Some(modified);
                    Ok(Some(StateEvent::Modified))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.last_modified.is_some() {
                    self.last_modified = None;
                    Ok(Some(StateEvent::Deleted))
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// A handle to the state watcher that can be used to control it.
pub struct StateWatcherHandle {
    /// Receiver for state events.
    rx: mpsc::Receiver<StateEvent>,
    /// Join handle for the watcher task.
    _handle: tokio::task::JoinHandle<()>,
}

impl StateWatcherHandle {
    /// Start a new state watcher and return a handle.
    pub fn start() -> Self {
        let (watcher, rx) = StateWatcher::default_interval();
        let handle = watcher.start();
        Self {
            rx,
            _handle: handle,
        }
    }

    /// Start a watcher with custom poll interval.
    pub fn start_with_interval(poll_interval: Duration) -> Self {
        let (watcher, rx) = StateWatcher::new(poll_interval);
        let handle = watcher.start();
        Self {
            rx,
            _handle: handle,
        }
    }

    /// Try to receive a state event without blocking.
    pub fn try_recv(&mut self) -> Option<StateEvent> {
        self.rx.try_recv().ok()
    }

    /// Receive a state event, waiting if necessary.
    pub async fn recv(&mut self) -> Option<StateEvent> {
        self.rx.recv().await
    }
}

/// Helper to watch a custom path (for testing).
pub struct CustomPathWatcher {
    path: PathBuf,
    last_modified: Option<SystemTime>,
}

impl CustomPathWatcher {
    /// Create a watcher for a specific path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let last_modified = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        Self {
            path,
            last_modified,
        }
    }

    /// Check if the file has changed since last check.
    pub fn has_changed(&mut self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(metadata) => {
                if let Ok(modified) = metadata.modified() {
                    if self.last_modified != Some(modified) {
                        self.last_modified = Some(modified);
                        return true;
                    }
                }
            }
            Err(_) => {
                if self.last_modified.is_some() {
                    self.last_modified = None;
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_custom_path_watcher() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "initial content").unwrap();

        let mut watcher = CustomPathWatcher::new(temp_file.path());

        assert!(!watcher.has_changed());

        std::thread::sleep(Duration::from_millis(100));
        writeln!(temp_file, "modified content").unwrap();
        temp_file.flush().unwrap();

        assert!(watcher.has_changed());

        assert!(!watcher.has_changed());
    }

    #[test]
    fn test_custom_watcher_deleted_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let mut watcher = CustomPathWatcher::new(&path);
        assert!(!watcher.has_changed());

        drop(temp_file);

        assert!(watcher.has_changed());

        assert!(!watcher.has_changed());
    }
}
