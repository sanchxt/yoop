//! Migration framework for schema and data transformations across versions.
//!
//! This module provides a framework for handling database migrations, schema changes,
//! and data transformations when updating between different versions of Yoop.

pub mod backup;
pub mod version;

#[cfg(feature = "update")]
pub mod migrations;

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::update::SchemaVersion;

pub use backup::{BackupId, BackupInfo, BackupManager};
pub use version::{MigrationHistoryEntry, MigrationState};

/// Get the application data directory.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "yoop", "Yoop").map(|dirs| dirs.data_dir().to_path_buf())
}

/// Run any pending migrations if the app version is newer than the schema version.
///
/// This function should be called at application startup (e.g., during config loading)
/// to ensure data files are migrated to the current version regardless of how the
/// user updated (npm install, yoop update, etc.).
///
/// The function operates silently - it only returns an error if migration fails.
/// Successful migrations and "no migration needed" cases return `Ok(())`.
///
/// # Errors
///
/// Returns an error if migrations fail. On failure, attempts to restore from backup.
pub fn migrate_if_needed() -> Result<()> {
    let data_dir = match data_dir() {
        Some(dir) => dir,
        None => return Ok(()),
    };

    let app_version = SchemaVersion::parse(crate::VERSION)?;
    let state = MigrationState::load(&data_dir)?;

    if state.schema_version >= app_version {
        return Ok(());
    }

    let manager = MigrationManager::new(data_dir);
    manager.run(&state.schema_version, &app_version, true)?;

    Ok(())
}

/// Trait for implementing database/schema migrations between versions.
#[allow(clippy::wrong_self_convention)]
pub trait Migration: Send + Sync {
    /// Get the version this migration starts from.
    fn from_version(&self) -> SchemaVersion;

    /// Get the version this migration upgrades to.
    fn to_version(&self) -> SchemaVersion;

    /// Apply the migration forward.
    ///
    /// # Errors
    ///
    /// Returns an error if the migration cannot be applied.
    fn up(&self, data_dir: &Path) -> Result<()>;

    /// Rollback the migration.
    ///
    /// # Errors
    ///
    /// Returns an error if the rollback cannot be performed.
    fn down(&self, data_dir: &Path) -> Result<()>;

    /// Get a human-readable description of what this migration does.
    fn description(&self) -> &'static str;

    /// Generate a unique identifier for this migration.
    #[must_use]
    fn id(&self) -> String {
        format!(
            "{}_{}_to_{}_{}",
            self.from_version().major,
            self.from_version().minor,
            self.to_version().major,
            self.to_version().minor
        )
    }
}

/// Orchestrates migration execution with backup/restore capabilities.
pub struct MigrationManager {
    /// Registered migrations available for execution.
    migrations: Vec<Box<dyn Migration>>,
    /// Backup manager for creating and restoring backups.
    backup_manager: BackupManager,
    /// Application data directory.
    data_dir: std::path::PathBuf,
}

impl MigrationManager {
    /// Create a new migration manager for the given data directory.
    #[must_use]
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        let migrations = Self::register_migrations();
        let backup_manager = BackupManager::new(data_dir.clone());

        Self {
            migrations,
            backup_manager,
            data_dir,
        }
    }

    #[cfg(feature = "update")]
    fn register_migrations() -> Vec<Box<dyn Migration>> {
        vec![Box::new(migrations::v0_1_to_v0_2::V0_1ToV0_2)]
    }

    #[cfg(not(feature = "update"))]
    fn register_migrations() -> Vec<Box<dyn Migration>> {
        vec![]
    }

    /// Get the list of migrations needed to upgrade from one version to another.
    #[must_use]
    pub fn get_pending(&self, from: &SchemaVersion, to: &SchemaVersion) -> Vec<&dyn Migration> {
        if from >= to {
            return vec![];
        }

        let mut pending = Vec::new();
        let mut current = from.clone();

        while current < *to {
            if let Some(migration) = self
                .migrations
                .iter()
                .find(|m| m.from_version() == current && m.to_version() <= *to)
            {
                pending.push(migration.as_ref());
                current = migration.to_version();
            } else {
                break;
            }
        }

        pending
    }

    /// Run all pending migrations from one version to another.
    ///
    /// # Errors
    ///
    /// Returns an error if backup creation fails or any migration fails. On failure, attempts to restore from backup.
    pub fn run(&self, from: &SchemaVersion, to: &SchemaVersion, create_backup: bool) -> Result<()> {
        let pending = self.get_pending(from, to);

        if pending.is_empty() {
            return Ok(());
        }

        let backup_id = if create_backup {
            Some(self.backup_manager.create_backup(&from.to_string())?)
        } else {
            None
        };

        let mut applied = Vec::new();
        let mut state = MigrationState::load(&self.data_dir)?;

        for migration in &pending {
            migration.up(&self.data_dir).map_err(|e| {
                if let Some(backup_id) = &backup_id {
                    let _ = self.backup_manager.restore_backup(backup_id);
                }
                crate::error::Error::Internal(format!("migration {} failed: {e}", migration.id()))
            })?;

            applied.push(migration.id());
        }

        state.add_history_entry(MigrationHistoryEntry {
            from_version: from.clone(),
            to_version: to.clone(),
            timestamp: chrono::Utc::now(),
            backup_id: backup_id.unwrap_or_else(|| "none".to_string()),
            success: true,
            migrations_applied: applied,
        });

        state.save(&self.data_dir)?;

        Ok(())
    }

    /// Rollback to a previous state using a backup.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup cannot be restored or the migration state cannot be updated.
    pub fn rollback(&self, backup_id: &str) -> Result<()> {
        self.backup_manager.restore_backup(&backup_id.to_string())?;

        let mut state = MigrationState::load(&self.data_dir)?;

        if let Some(entry) = state.history.iter().find(|e| e.backup_id == backup_id) {
            state.schema_version = entry.from_version.clone();
            state.save(&self.data_dir)?;
        }

        Ok(())
    }

    /// List all available backups.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup manifest cannot be read.
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        self.backup_manager.list_backups()
    }

    /// Get information about the most recent backup.
    ///
    /// # Errors
    ///
    /// Returns an error if the backup manifest cannot be read.
    pub fn latest_backup(&self) -> Result<Option<BackupInfo>> {
        self.backup_manager.latest_backup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestMigration {
        from: SchemaVersion,
        to: SchemaVersion,
    }

    impl Migration for TestMigration {
        fn from_version(&self) -> SchemaVersion {
            self.from.clone()
        }

        fn to_version(&self) -> SchemaVersion {
            self.to.clone()
        }

        fn up(&self, _data_dir: &Path) -> Result<()> {
            Ok(())
        }

        fn down(&self, _data_dir: &Path) -> Result<()> {
            Ok(())
        }

        fn description(&self) -> &'static str {
            "Test migration"
        }
    }

    #[test]
    fn test_migration_manager_get_pending() {
        let temp_dir = TempDir::new().unwrap();

        let mut manager = MigrationManager::new(temp_dir.path().to_path_buf());

        manager.migrations = vec![
            Box::new(TestMigration {
                from: SchemaVersion::new(0, 1, 0),
                to: SchemaVersion::new(0, 2, 0),
            }),
            Box::new(TestMigration {
                from: SchemaVersion::new(0, 2, 0),
                to: SchemaVersion::new(0, 3, 0),
            }),
        ];

        let from = SchemaVersion::new(0, 1, 0);
        let to = SchemaVersion::new(0, 3, 0);
        let pending = manager.get_pending(&from, &to);

        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].from_version(), SchemaVersion::new(0, 1, 0));
        assert_eq!(pending[1].from_version(), SchemaVersion::new(0, 2, 0));
    }

    #[test]
    fn test_migration_manager_no_pending() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MigrationManager::new(temp_dir.path().to_path_buf());

        let from = SchemaVersion::new(0, 2, 0);
        let to = SchemaVersion::new(0, 1, 0);
        let pending = manager.get_pending(&from, &to);

        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_migration_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();

        let mut state =
            MigrationState::new(SchemaVersion::new(0, 1, 0), SchemaVersion::new(0, 1, 0));

        state.add_history_entry(MigrationHistoryEntry {
            from_version: SchemaVersion::new(0, 1, 0),
            to_version: SchemaVersion::new(0, 2, 0),
            timestamp: chrono::Utc::now(),
            backup_id: "test_backup".to_string(),
            success: true,
            migrations_applied: vec!["test_migration".to_string()],
        });

        state.save(data_dir).expect("save state");

        let loaded = MigrationState::load(data_dir).expect("load state");

        assert_eq!(loaded.schema_version, SchemaVersion::new(0, 2, 0));
        assert_eq!(loaded.history.len(), 1);
    }

    #[test]
    fn test_migration_manager_with_gap() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = MigrationManager::new(temp_dir.path().to_path_buf());

        manager.migrations = vec![
            Box::new(TestMigration {
                from: SchemaVersion::new(0, 1, 0),
                to: SchemaVersion::new(0, 2, 0),
            }),
            Box::new(TestMigration {
                from: SchemaVersion::new(0, 3, 0),
                to: SchemaVersion::new(0, 4, 0),
            }),
        ];

        let from = SchemaVersion::new(0, 1, 0);
        let to = SchemaVersion::new(0, 4, 0);
        let pending = manager.get_pending(&from, &to);

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].to_version(), SchemaVersion::new(0, 2, 0));
    }

    #[test]
    fn test_migration_manager_same_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MigrationManager::new(temp_dir.path().to_path_buf());

        let version = SchemaVersion::new(0, 1, 0);
        let pending = manager.get_pending(&version, &version);

        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_migration_manager_downgrade() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MigrationManager::new(temp_dir.path().to_path_buf());

        let from = SchemaVersion::new(0, 2, 0);
        let to = SchemaVersion::new(0, 1, 0);
        let pending = manager.get_pending(&from, &to);

        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_migration_id_generation() {
        let migration = TestMigration {
            from: SchemaVersion::new(0, 1, 0),
            to: SchemaVersion::new(0, 2, 0),
        };

        assert_eq!(migration.id(), "0_1_to_0_2");
    }
}
