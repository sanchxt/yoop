//! Backup and restore functionality for migrations.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::update::SchemaVersion;

/// Unique identifier for a backup.
pub type BackupId = String;

/// Information about a backup including version, timestamp, and files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Unique identifier for this backup.
    pub id: BackupId,
    /// Application version at the time of backup.
    pub app_version: SchemaVersion,
    /// Schema version at the time of backup.
    pub schema_version: SchemaVersion,
    /// When the backup was created.
    pub timestamp: DateTime<Utc>,
    /// List of files included in the backup.
    pub files: Vec<String>,
    /// Total size of the backup in bytes.
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    version: u32,
    backups: Vec<BackupInfo>,
    max_backups: usize,
}

impl Default for BackupManifest {
    fn default() -> Self {
        Self {
            version: 1,
            backups: Vec::new(),
            max_backups: 5,
        }
    }
}

/// Manages backup creation, restoration, and cleanup for safe migrations.
pub struct BackupManager {
    /// Directory where backups are stored.
    backup_dir: PathBuf,
    /// Application data directory to backup.
    data_dir: PathBuf,
    /// Maximum number of backups to retain.
    max_backups: usize,
}

impl BackupManager {
    const FILES_TO_BACKUP: &'static [&'static str] = &[
        "config.toml",
        "history.json",
        "trust.json",
        "migration_state.json",
    ];

    /// Create a new backup manager for the given data directory.
    #[must_use]
    pub fn new(data_dir: PathBuf) -> Self {
        let backup_dir = Self::get_backup_dir().unwrap_or_else(|| data_dir.join("backups"));

        Self {
            backup_dir,
            data_dir,
            max_backups: 5,
        }
    }

    /// Create a backup manager with custom backup directory (for testing).
    #[must_use]
    #[doc(hidden)]
    pub fn new_with_backup_dir(data_dir: PathBuf, backup_dir: PathBuf) -> Self {
        Self {
            backup_dir,
            data_dir,
            max_backups: 5,
        }
    }

    fn get_backup_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "yoop", "Yoop")
            .map(|dirs| dirs.data_dir().join("backups"))
    }

    /// Create a backup of all critical application files.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup directory cannot be created or files cannot be copied.
    pub fn create_backup(&self, version: &str) -> Result<BackupId> {
        let timestamp = Utc::now();
        let backup_id = format!(
            "{}_{}_{}",
            version,
            timestamp.format("%Y%m%d"),
            timestamp.format("%H%M%S")
        );

        let backup_path = self.backup_dir.join(&backup_id);
        fs::create_dir_all(&backup_path).map_err(|e| {
            crate::error::Error::Internal(format!("failed to create backup directory: {e}"))
        })?;

        let mut backed_up_files = Vec::new();
        let mut total_size = 0u64;

        for file_name in Self::FILES_TO_BACKUP {
            let src = if *file_name == "config.toml" && !cfg!(test) {
                crate::config::Config::config_path()
            } else {
                self.data_dir.join(file_name)
            };

            if src.exists() {
                let dest = backup_path.join(file_name);
                fs::copy(&src, &dest).map_err(|e| {
                    crate::error::Error::Internal(format!("failed to backup {file_name}: {e}"))
                })?;

                backed_up_files.push((*file_name).to_string());
                total_size += fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            }
        }

        let current_version = SchemaVersion::parse(crate::VERSION)?;
        let backup_info = BackupInfo {
            id: backup_id.clone(),
            app_version: current_version.clone(),
            schema_version: current_version,
            timestamp,
            files: backed_up_files,
            size_bytes: total_size,
        };

        self.add_to_manifest(backup_info)?;
        self.cleanup()?;

        Ok(backup_id)
    }

    /// Restore files from a backup by its ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup doesn't exist or files cannot be restored.
    pub fn restore_backup(&self, backup_id: &BackupId) -> Result<()> {
        let backup_path = self.backup_dir.join(backup_id);

        if !backup_path.exists() {
            return Err(crate::error::Error::Internal(format!(
                "backup not found: {backup_id}"
            )));
        }

        for file_name in Self::FILES_TO_BACKUP {
            let src = backup_path.join(file_name);

            if src.exists() {
                let dest = if *file_name == "config.toml" && !cfg!(test) {
                    crate::config::Config::config_path()
                } else {
                    self.data_dir.join(file_name)
                };

                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        crate::error::Error::Internal(format!(
                            "failed to create directory for restore: {e}"
                        ))
                    })?;
                }

                fs::copy(&src, &dest).map_err(|e| {
                    crate::error::Error::Internal(format!("failed to restore {file_name}: {e}"))
                })?;
            }
        }

        Ok(())
    }

    /// List all available backups.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup manifest cannot be read.
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let manifest = self.load_manifest()?;
        Ok(manifest.backups)
    }

    /// Get information about the most recent backup.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup manifest cannot be read.
    pub fn latest_backup(&self) -> Result<Option<BackupInfo>> {
        let manifest = self.load_manifest()?;
        Ok(manifest.backups.into_iter().next_back())
    }

    /// Remove old backups beyond the maximum retention limit.
    ///
    /// # Errors
    ///
    /// Returns an error if backups cannot be deleted or the manifest cannot be updated.
    pub fn cleanup(&self) -> Result<usize> {
        let mut manifest = self.load_manifest()?;

        if manifest.backups.len() <= self.max_backups {
            return Ok(0);
        }

        manifest
            .backups
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let to_remove = manifest.backups.len() - self.max_backups;
        let removed_backups: Vec<_> = manifest.backups.drain(..to_remove).collect();

        for backup in &removed_backups {
            let backup_path = self.backup_dir.join(&backup.id);
            if backup_path.exists() {
                fs::remove_dir_all(&backup_path).map_err(|e| {
                    crate::error::Error::Internal(format!(
                        "failed to remove old backup {}: {e}",
                        backup.id
                    ))
                })?;
            }
        }

        self.save_manifest(&manifest)?;

        Ok(removed_backups.len())
    }

    fn manifest_path(&self) -> PathBuf {
        self.backup_dir.join("manifest.json")
    }

    /// Load the backup manifest or create a default one if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file exists but cannot be read or parsed.
    fn load_manifest(&self) -> Result<BackupManifest> {
        let path = self.manifest_path();

        if !path.exists() {
            return Ok(BackupManifest::default());
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            crate::error::Error::Internal(format!("failed to read backup manifest: {e}"))
        })?;

        serde_json::from_str(&content).map_err(|e| {
            crate::error::Error::Internal(format!("failed to parse backup manifest: {e}"))
        })
    }

    /// Save the backup manifest to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest directory cannot be created or the file cannot be written.
    fn save_manifest(&self, manifest: &BackupManifest) -> Result<()> {
        let path = self.manifest_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::Error::Internal(format!("failed to create backup directory: {e}"))
            })?;
        }

        let content = serde_json::to_string_pretty(manifest).map_err(|e| {
            crate::error::Error::Internal(format!("failed to serialize backup manifest: {e}"))
        })?;

        fs::write(&path, content).map_err(|e| {
            crate::error::Error::Internal(format!("failed to write backup manifest: {e}"))
        })
    }

    /// Add a backup entry to the manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be loaded or saved.
    fn add_to_manifest(&self, backup_info: BackupInfo) -> Result<()> {
        let mut manifest = self.load_manifest()?;
        manifest.backups.push(backup_info);
        self.save_manifest(&manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let manager = BackupManager::new(data_dir.clone());

        assert!(manager.backup_dir.ends_with("backups"));
        assert_eq!(manager.data_dir, data_dir);
        assert_eq!(manager.max_backups, 5);
    }

    #[test]
    fn test_create_and_list_backups() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        fs::write(data_dir.join("config.toml"), "test config").unwrap();
        fs::write(data_dir.join("history.json"), "{}").unwrap();

        let manager = BackupManager {
            backup_dir: data_dir.join("backups"),
            data_dir: data_dir.clone(),
            max_backups: 5,
        };

        let backup_id = manager.create_backup("0.1.0").expect("create backup");

        assert!(backup_id.contains("0.1.0"));

        let backups = manager.list_backups().expect("list backups");
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].id, backup_id);
    }

    #[test]
    fn test_restore_backup() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        let original_config = "device_name = \"test\"";
        fs::write(data_dir.join("config.toml"), original_config).unwrap();

        let manager = BackupManager {
            backup_dir: data_dir.join("backups"),
            data_dir: data_dir.clone(),
            max_backups: 5,
        };

        let backup_id = manager.create_backup("0.1.0").expect("create backup");

        fs::write(data_dir.join("config.toml"), "device_name = \"modified\"").unwrap();

        manager.restore_backup(&backup_id).expect("restore backup");

        let restored = fs::read_to_string(data_dir.join("config.toml")).unwrap();
        assert_eq!(restored, original_config);
    }

    #[test]
    fn test_cleanup_old_backups() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        fs::write(data_dir.join("config.toml"), "test").unwrap();

        let manager = BackupManager {
            backup_dir: data_dir.join("backups"),
            data_dir: data_dir.clone(),
            max_backups: 2,
        };

        let _b1 = manager.create_backup("0.1.0").expect("backup 1");
        let _b2 = manager.create_backup("0.1.1").expect("backup 2");
        let _b3 = manager.create_backup("0.1.2").expect("backup 3");

        let backups = manager.list_backups().expect("list");
        assert_eq!(backups.len(), 2);
    }
}
