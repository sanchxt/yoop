//! Integration tests for the update functionality.

#![cfg(feature = "update")]

use tempfile::TempDir;
use yoop_core::config::{PackageManagerKind, UpdateConfig};
use yoop_core::migration::{BackupManager, MigrationManager, MigrationState};
use yoop_core::update::{package_manager::PackageManager, SchemaVersion};

#[test]
fn test_update_config_default() {
    let config = UpdateConfig::default();

    assert!(config.auto_check);
    assert!(config.notify);
    assert_eq!(config.check_interval.as_secs(), 24 * 60 * 60);
    assert!(config.package_manager.is_none());
    assert!(config.last_check.is_none());
}

#[test]
fn test_update_config_with_package_manager() {
    let config = UpdateConfig {
        package_manager: Some(PackageManagerKind::Pnpm),
        ..Default::default()
    };

    let result = PackageManager::detect(&config);
    assert!(result.is_ok());
}

#[test]
fn test_backup_and_migration_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    std::fs::write(data_dir.join("history.json"), r#"{"test": true}"#).unwrap();
    std::fs::write(data_dir.join("trust.json"), "{}").unwrap();

    let manager = BackupManager::new_with_backup_dir(data_dir.clone(), data_dir.join("backups"));

    let backup_id = manager.create_backup("0.1.0").expect("create backup");

    assert!(backup_id.contains("0.1.0"));

    let backups = manager.list_backups().expect("list backups");
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].id, backup_id);

    let backup_path = data_dir.join("backups").join(&backup_id);
    assert!(backup_path.exists(), "Backup directory should exist");
    assert!(
        backup_path.join("history.json").exists(),
        "Backed up history should exist"
    );

    std::fs::write(data_dir.join("history.json"), r#"{"test": false}"#).unwrap();

    let modified = std::fs::read_to_string(data_dir.join("history.json")).unwrap();
    assert_eq!(modified, r#"{"test": false}"#, "File should be modified");

    manager.restore_backup(&backup_id).expect("restore backup");

    let content = std::fs::read_to_string(data_dir.join("history.json")).unwrap();
    assert_eq!(content, r#"{"test": true}"#, "File should be restored");
}

#[test]
fn test_migration_state_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path();

    let mut state = MigrationState::new(SchemaVersion::new(0, 1, 0), SchemaVersion::new(0, 1, 0));

    assert_eq!(state.schema_version, SchemaVersion::new(0, 1, 0));
    assert!(state.history.is_empty());

    state.add_history_entry(yoop_core::migration::MigrationHistoryEntry {
        from_version: SchemaVersion::new(0, 1, 0),
        to_version: SchemaVersion::new(0, 2, 0),
        timestamp: chrono::Utc::now(),
        backup_id: "backup1".to_string(),
        success: true,
        migrations_applied: vec!["migration1".to_string()],
    });

    assert_eq!(state.schema_version, SchemaVersion::new(0, 2, 0));
    assert_eq!(state.history.len(), 1);
    assert_eq!(state.latest_backup(), Some("backup1"));

    state.save(data_dir).expect("save state");

    let loaded = MigrationState::load(data_dir).expect("load state");

    assert_eq!(loaded.schema_version, SchemaVersion::new(0, 2, 0));
    assert_eq!(loaded.history.len(), 1);
}

#[test]
fn test_migration_manager_integration() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    let manager = MigrationManager::new(data_dir);

    let from = SchemaVersion::new(0, 1, 0);
    let to = SchemaVersion::new(0, 2, 0);

    let pending = manager.get_pending(&from, &to);

    #[cfg(feature = "update")]
    {
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].from_version(), SchemaVersion::new(0, 1, 0));
        assert_eq!(pending[0].to_version(), SchemaVersion::new(0, 2, 0));
        assert_eq!(pending[0].description(), "Add [update] config section");
    }

    #[cfg(not(feature = "update"))]
    {
        assert_eq!(pending.len(), 0);
    }
}

#[test]
fn test_backup_cleanup_integration() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    std::fs::write(data_dir.join("history.json"), "{}").unwrap();

    let manager = BackupManager::new_with_backup_dir(data_dir.clone(), data_dir.join("backups"));

    let mut created_ids = Vec::new();
    for i in 0..7 {
        let version = format!("0.1.{i}");
        let backup_id = manager.create_backup(&version).expect("create backup");
        created_ids.push(backup_id);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let backups = manager.list_backups().expect("list backups");

    assert_eq!(
        backups.len(),
        5,
        "Should keep only the 5 most recent backups"
    );

    let backup_ids: Vec<&String> = backups.iter().map(|b| &b.id).collect();

    for expected_id in &created_ids[2..] {
        assert!(
            backup_ids.contains(&expected_id),
            "Should contain backup {expected_id}, but found: {backup_ids:?}"
        );
    }

    for old_id in &created_ids[..2] {
        assert!(
            !backup_ids.contains(&old_id),
            "Should not contain old backup {old_id}"
        );
    }
}

#[test]
fn test_rollback_integration() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    std::fs::write(data_dir.join("history.json"), r#"{"version": 1}"#).unwrap();

    let backup_manager =
        BackupManager::new_with_backup_dir(data_dir.clone(), data_dir.join("backups"));

    let backup_id = backup_manager
        .create_backup("0.1.0")
        .expect("create backup");

    std::fs::write(data_dir.join("history.json"), r#"{"version": 2}"#).unwrap();

    backup_manager
        .restore_backup(&backup_id)
        .expect("restore failed");

    let content = std::fs::read_to_string(data_dir.join("history.json")).unwrap();
    assert_eq!(content, r#"{"version": 1}"#);
}

#[test]
fn test_multiple_migrations_in_sequence() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    let manager = MigrationManager::new(data_dir);

    let v1 = SchemaVersion::new(0, 1, 0);
    let v2 = SchemaVersion::new(0, 2, 0);

    let pending = manager.get_pending(&v1, &v2);

    #[cfg(feature = "update")]
    {
        assert!(!pending.is_empty());

        for migration in &pending {
            assert_eq!(migration.from_version(), v1);
            assert_eq!(migration.to_version(), v2);
        }
    }

    #[cfg(not(feature = "update"))]
    {
        assert_eq!(pending.len(), 0);
    }
}

#[test]
fn test_schema_version_comparison_integration() {
    let versions = [
        SchemaVersion::new(0, 1, 0),
        SchemaVersion::new(0, 2, 0),
        SchemaVersion::new(1, 0, 0),
        SchemaVersion::new(1, 1, 0),
        SchemaVersion::new(2, 0, 0),
    ];

    for i in 0..versions.len() - 1 {
        assert!(versions[i] < versions[i + 1]);
        assert!(versions[i + 1] > versions[i]);
    }

    let v1 = SchemaVersion::new(1, 0, 0);
    let v2 = SchemaVersion::new(1, 0, 0);
    assert_eq!(v1, v2);
}

#[test]
fn test_backup_with_missing_files() {
    let temp_dir = TempDir::new().unwrap();
    let data_dir = temp_dir.path().to_path_buf();

    std::fs::write(data_dir.join("history.json"), "{}").unwrap();

    let manager = BackupManager::new_with_backup_dir(data_dir.clone(), data_dir.join("backups"));

    let result = manager.create_backup("0.1.0");

    assert!(result.is_ok());

    let backup_id = result.unwrap();
    let backups = manager.list_backups().expect("list backups");

    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].id, backup_id);
    assert!(backups[0].files.contains(&"history.json".to_string()));
    assert!(!backups[0].files.contains(&"trust.json".to_string()));
}

#[test]
fn test_package_manager_detection_priority() {
    use std::env;

    let config = UpdateConfig {
        package_manager: Some(PackageManagerKind::Pnpm),
        ..Default::default()
    };
    let result = PackageManager::detect(&config);
    assert!(result.is_ok());

    let config = UpdateConfig::default();
    env::set_var("YOOP_PACKAGE_MANAGER", "yarn");
    let result = PackageManager::detect(&config);
    env::remove_var("YOOP_PACKAGE_MANAGER");
    assert!(result.is_ok());

    env::set_var("npm_config_user_agent", "bun/1.0.0");
    let result = PackageManager::detect(&config);
    env::remove_var("npm_config_user_agent");
    assert!(result.is_ok());
}
