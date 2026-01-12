//! Migration from version 0.1.x to 0.2.x.
//!
//! This migration adds the `[update]` section to the configuration file.

use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::migration::Migration;
use crate::update::SchemaVersion;

/// Migration from v0.1.x to v0.2.x adding `[update]` configuration section.
#[allow(non_camel_case_types)]
pub struct V0_1ToV0_2;

impl Migration for V0_1ToV0_2 {
    fn from_version(&self) -> SchemaVersion {
        SchemaVersion::new(0, 1, 0)
    }

    fn to_version(&self) -> SchemaVersion {
        SchemaVersion::new(0, 2, 0)
    }

    fn description(&self) -> &'static str {
        "Add [update] config section"
    }

    fn up(&self, data_dir: &Path) -> Result<()> {
        let config_path = if cfg!(test) {
            data_dir.join("config.toml")
        } else {
            Config::config_path()
        };

        if !config_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| crate::error::Error::ConfigError(format!("failed to read config: {e}")))?;

        let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| {
            crate::error::Error::ConfigError(format!("failed to parse config: {e}"))
        })?;

        if doc.get("update").is_none() {
            let mut update_table = toml_edit::Table::new();
            update_table.insert("auto_check", toml_edit::value(false));
            update_table.insert("check_interval", toml_edit::value("86400s"));
            update_table.insert("notify", toml_edit::value(true));

            doc.insert("update", toml_edit::Item::Table(update_table));

            fs::write(&config_path, doc.to_string()).map_err(|e| {
                crate::error::Error::ConfigError(format!("failed to write config: {e}"))
            })?;
        }

        Ok(())
    }

    fn down(&self, data_dir: &Path) -> Result<()> {
        let config_path = if cfg!(test) {
            data_dir.join("config.toml")
        } else {
            Config::config_path()
        };

        if !config_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| crate::error::Error::ConfigError(format!("failed to read config: {e}")))?;

        let mut doc = content.parse::<toml_edit::DocumentMut>().map_err(|e| {
            crate::error::Error::ConfigError(format!("failed to parse config: {e}"))
        })?;

        if doc.get("update").is_some() {
            doc.remove("update");

            fs::write(&config_path, doc.to_string()).map_err(|e| {
                crate::error::Error::ConfigError(format!("failed to write config: {e}"))
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_v0_1_to_v0_2_up() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let initial_config = r#"
[general]
device_name = "Test Device"

[network]
port = 52525
"#;

        fs::write(&config_path, initial_config).unwrap();

        let migration = V0_1ToV0_2;

        let result = migration.up(temp_dir.path());
        assert!(result.is_ok());

        let updated = fs::read_to_string(&config_path).unwrap();
        assert!(updated.contains("[update]"));
        assert!(updated.contains("auto_check"));
        assert!(updated.contains("check_interval"));
        assert!(updated.contains("notify"));
    }

    #[test]
    fn test_v0_1_to_v0_2_down() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config_with_update = r#"
[general]
device_name = "Test Device"

[update]
auto_check = false
check_interval = "86400s"
notify = true
"#;

        fs::write(&config_path, config_with_update).unwrap();

        let migration = V0_1ToV0_2;

        let result = migration.down(temp_dir.path());
        assert!(result.is_ok());

        let updated = fs::read_to_string(&config_path).unwrap();
        assert!(!updated.contains("[update]"));
    }

    #[test]
    fn test_v0_1_to_v0_2_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config_with_update = r#"
[general]
device_name = "Test Device"

[update]
auto_check = true
check_interval = "3600s"
notify = false
"#;

        fs::write(&config_path, config_with_update).unwrap();

        let migration = V0_1ToV0_2;

        let result = migration.up(temp_dir.path());
        assert!(result.is_ok());

        let updated = fs::read_to_string(&config_path).unwrap();
        assert!(updated.contains("auto_check = true"));
        assert!(updated.contains("3600s"));
    }

    #[test]
    fn test_migration_metadata() {
        let migration = V0_1ToV0_2;

        assert_eq!(migration.from_version(), SchemaVersion::new(0, 1, 0));
        assert_eq!(migration.to_version(), SchemaVersion::new(0, 2, 0));
        assert_eq!(migration.description(), "Add [update] config section");
        assert_eq!(migration.id(), "0_1_to_0_2");
    }
}
