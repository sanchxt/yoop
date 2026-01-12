//! Version tracking and migration state management.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::update::SchemaVersion;

/// State tracking for migrations including version history and backup information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationState {
    /// State format version for future compatibility.
    pub version: u32,
    /// Current schema version after migrations.
    pub schema_version: SchemaVersion,
    /// Application version that created this state.
    pub app_version: SchemaVersion,
    /// Timestamp of the last migration execution.
    pub last_migration: Option<DateTime<Utc>>,
    /// History of all migration executions.
    #[serde(default)]
    pub history: Vec<MigrationHistoryEntry>,
}

/// Record of a single migration execution including success status and backup information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationHistoryEntry {
    /// Schema version before the migration.
    pub from_version: SchemaVersion,
    /// Schema version after the migration.
    pub to_version: SchemaVersion,
    /// When the migration was executed.
    pub timestamp: DateTime<Utc>,
    /// ID of the backup created before migration.
    pub backup_id: String,
    /// Whether the migration completed successfully.
    pub success: bool,
    /// List of migration IDs that were applied.
    pub migrations_applied: Vec<String>,
}

impl MigrationState {
    /// Create a new migration state with the given schema and app versions.
    #[must_use]
    pub fn new(schema_version: SchemaVersion, app_version: SchemaVersion) -> Self {
        Self {
            version: 1,
            schema_version,
            app_version,
            last_migration: None,
            history: Vec::new(),
        }
    }

    /// Load migration state from disk or create a new one if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the state file exists but cannot be read or parsed.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = Self::state_path(data_dir);

        if !path.exists() {
            let current_version = SchemaVersion::parse(crate::VERSION)?;
            return Ok(Self::new(current_version.clone(), current_version));
        }

        let content = std::fs::read_to_string(&path).map_err(|e| {
            crate::error::Error::ConfigError(format!("failed to read migration state: {e}"))
        })?;

        serde_json::from_str(&content).map_err(|e| {
            crate::error::Error::ConfigError(format!("failed to parse migration state: {e}"))
        })
    }

    /// Save migration state to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the state directory cannot be created or the file cannot be written.
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let path = Self::state_path(data_dir);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::Error::ConfigError(format!(
                    "failed to create migration state directory: {e}"
                ))
            })?;
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| {
            crate::error::Error::Internal(format!("failed to serialize migration state: {e}"))
        })?;

        std::fs::write(&path, content).map_err(|e| {
            crate::error::Error::ConfigError(format!("failed to write migration state: {e}"))
        })
    }

    /// Get the path where migration state is stored.
    #[must_use]
    pub fn state_path(data_dir: &Path) -> PathBuf {
        data_dir.join("migration_state.json")
    }

    /// Add a migration history entry and update state accordingly.
    pub fn add_history_entry(&mut self, entry: MigrationHistoryEntry) {
        self.last_migration = Some(entry.timestamp);
        self.schema_version = entry.to_version.clone();
        self.history.push(entry);
    }

    /// Get the backup ID of the most recent successful migration.
    #[must_use]
    pub fn latest_backup(&self) -> Option<&str> {
        self.history
            .iter()
            .rev()
            .find(|entry| entry.success)
            .map(|entry| entry.backup_id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_migration_state_new() {
        let v1 = SchemaVersion::new(0, 1, 0);
        let v2 = SchemaVersion::new(0, 2, 0);
        let state = MigrationState::new(v1.clone(), v2.clone());

        assert_eq!(state.version, 1);
        assert_eq!(state.schema_version, v1);
        assert_eq!(state.app_version, v2);
        assert!(state.last_migration.is_none());
        assert!(state.history.is_empty());
    }

    #[test]
    fn test_migration_state_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();

        let mut state =
            MigrationState::new(SchemaVersion::new(0, 1, 0), SchemaVersion::new(0, 1, 0));

        state.add_history_entry(MigrationHistoryEntry {
            from_version: SchemaVersion::new(0, 1, 0),
            to_version: SchemaVersion::new(0, 2, 0),
            timestamp: Utc::now(),
            backup_id: "test_backup".to_string(),
            success: true,
            migrations_applied: vec!["migration1".to_string()],
        });

        state.save(data_dir).expect("save failed");

        let loaded = MigrationState::load(data_dir).expect("load failed");

        assert_eq!(loaded.version, state.version);
        assert_eq!(loaded.schema_version, state.schema_version);
        assert_eq!(loaded.history.len(), 1);
        assert_eq!(loaded.history[0].backup_id, "test_backup");
    }

    #[test]
    fn test_latest_backup() {
        let mut state =
            MigrationState::new(SchemaVersion::new(0, 1, 0), SchemaVersion::new(0, 1, 0));

        assert!(state.latest_backup().is_none());

        state.add_history_entry(MigrationHistoryEntry {
            from_version: SchemaVersion::new(0, 1, 0),
            to_version: SchemaVersion::new(0, 2, 0),
            timestamp: Utc::now(),
            backup_id: "backup1".to_string(),
            success: true,
            migrations_applied: vec![],
        });

        assert_eq!(state.latest_backup(), Some("backup1"));

        state.add_history_entry(MigrationHistoryEntry {
            from_version: SchemaVersion::new(0, 2, 0),
            to_version: SchemaVersion::new(0, 3, 0),
            timestamp: Utc::now(),
            backup_id: "backup2".to_string(),
            success: true,
            migrations_applied: vec![],
        });

        assert_eq!(state.latest_backup(), Some("backup2"));
    }
}
