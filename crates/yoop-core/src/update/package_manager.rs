//! Package manager detection and execution.

use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::config::UpdateConfig;
use crate::error::{Error, Result};

/// Supported package managers for installing Yoop updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    /// Node Package Manager (npm).
    Npm,
    /// Performant npm (pnpm).
    Pnpm,
    /// Yarn package manager.
    Yarn,
    /// Bun JavaScript runtime and package manager.
    Bun,
}

impl PackageManager {
    /// Detect the best package manager to use based on configuration and environment.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured package manager is not found in PATH.
    pub fn detect(config: &UpdateConfig) -> Result<Self> {
        if let Some(pm_kind) = config.package_manager {
            let pm: Self = pm_kind.into();
            if !pm.is_available() {
                return Err(Error::Internal(format!(
                    "configured package manager '{pm}' not found in PATH"
                )));
            }
            return Ok(pm);
        }

        if let Some(pm) = Self::detect_from_environment() {
            return Ok(pm);
        }

        Self::detect_from_availability()
    }

    fn detect_from_environment() -> Option<Self> {
        if let Ok(agent) = std::env::var("npm_config_user_agent") {
            if agent.contains("pnpm") {
                return Some(Self::Pnpm);
            }
            if agent.contains("yarn") {
                return Some(Self::Yarn);
            }
            if agent.contains("bun") {
                return Some(Self::Bun);
            }
            if agent.contains("npm") {
                return Some(Self::Npm);
            }
        }

        if let Ok(pm) = std::env::var("YOOP_PACKAGE_MANAGER") {
            match pm.to_lowercase().as_str() {
                "npm" => return Some(Self::Npm),
                "pnpm" => return Some(Self::Pnpm),
                "yarn" => return Some(Self::Yarn),
                "bun" => return Some(Self::Bun),
                _ => {}
            }
        }

        None
    }

    fn detect_from_availability() -> Result<Self> {
        for pm in [Self::Pnpm, Self::Bun, Self::Yarn, Self::Npm] {
            if pm.is_available() {
                return Ok(pm);
            }
        }

        Err(Error::Internal(
            "no package manager found in PATH (npm, pnpm, yarn, or bun)".to_string(),
        ))
    }

    /// Check if this package manager is available in PATH.
    #[must_use]
    pub fn is_available(self) -> bool {
        Command::new(self.command_name())
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Get the command name for this package manager.
    #[must_use]
    pub fn command_name(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }

    /// Build the command to update/install a package globally.
    #[must_use]
    pub fn update_command(self, package: &str, version: Option<&str>) -> Vec<String> {
        let pkg = version.map_or_else(|| package.to_string(), |v| format!("{package}@{v}"));

        match self {
            Self::Npm => vec![
                "npm".to_string(),
                "install".to_string(),
                "-g".to_string(),
                pkg,
            ],
            Self::Pnpm => vec!["pnpm".to_string(), "add".to_string(), "-g".to_string(), pkg],
            Self::Yarn => vec![
                "yarn".to_string(),
                "global".to_string(),
                "add".to_string(),
                pkg,
            ],
            Self::Bun => vec!["bun".to_string(), "add".to_string(), "-g".to_string(), pkg],
        }
    }

    /// Build the command to install a specific version of a package globally.
    #[must_use]
    pub fn install_command(self, package: &str, version: &str) -> Vec<String> {
        self.update_command(package, Some(version))
    }
}

impl std::fmt::Display for PackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.command_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_manager_command_name() {
        assert_eq!(PackageManager::Npm.command_name(), "npm");
        assert_eq!(PackageManager::Pnpm.command_name(), "pnpm");
        assert_eq!(PackageManager::Yarn.command_name(), "yarn");
        assert_eq!(PackageManager::Bun.command_name(), "bun");
    }

    #[test]
    fn test_update_command() {
        let npm = PackageManager::Npm;
        assert_eq!(
            npm.update_command("yoop", None),
            vec!["npm", "install", "-g", "yoop"]
        );
        assert_eq!(
            npm.update_command("yoop", Some("0.2.0")),
            vec!["npm", "install", "-g", "yoop@0.2.0"]
        );

        let pnpm = PackageManager::Pnpm;
        assert_eq!(
            pnpm.update_command("yoop", None),
            vec!["pnpm", "add", "-g", "yoop"]
        );

        let yarn = PackageManager::Yarn;
        assert_eq!(
            yarn.update_command("yoop", None),
            vec!["yarn", "global", "add", "yoop"]
        );

        let bun = PackageManager::Bun;
        assert_eq!(
            bun.update_command("yoop", None),
            vec!["bun", "add", "-g", "yoop"]
        );
    }

    #[test]
    fn test_install_command() {
        let npm = PackageManager::Npm;
        assert_eq!(
            npm.install_command("yoop", "0.1.3"),
            vec!["npm", "install", "-g", "yoop@0.1.3"]
        );

        let pnpm = PackageManager::Pnpm;
        assert_eq!(
            pnpm.install_command("yoop", "0.1.3"),
            vec!["pnpm", "add", "-g", "yoop@0.1.3"]
        );

        let yarn = PackageManager::Yarn;
        assert_eq!(
            yarn.install_command("yoop", "0.1.3"),
            vec!["yarn", "global", "add", "yoop@0.1.3"]
        );

        let bun = PackageManager::Bun;
        assert_eq!(
            bun.install_command("yoop", "0.1.3"),
            vec!["bun", "add", "-g", "yoop@0.1.3"]
        );
    }

    #[test]
    fn test_package_manager_display() {
        assert_eq!(PackageManager::Npm.to_string(), "npm");
        assert_eq!(PackageManager::Pnpm.to_string(), "pnpm");
        assert_eq!(PackageManager::Yarn.to_string(), "yarn");
        assert_eq!(PackageManager::Bun.to_string(), "bun");
    }

    #[test]
    fn test_detect_with_env_var() {
        use std::env;

        let config = UpdateConfig::default();

        env::set_var("YOOP_PACKAGE_MANAGER", "pnpm");
        let result = PackageManager::detect(&config);
        env::remove_var("YOOP_PACKAGE_MANAGER");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PackageManager::Pnpm);
    }

    #[test]
    fn test_detect_with_config() {
        use crate::config::PackageManagerKind;

        let config = UpdateConfig {
            package_manager: Some(PackageManagerKind::Npm),
            ..Default::default()
        };

        let result = PackageManager::detect(&config);

        if PackageManager::Npm.is_available() {
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), PackageManager::Npm);
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_detect_from_user_agent() {
        use std::env;

        let config = UpdateConfig::default();

        env::set_var("npm_config_user_agent", "yarn/1.22.19 npm/? node/v18.0.0");
        let result = PackageManager::detect(&config);
        env::remove_var("npm_config_user_agent");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PackageManager::Yarn);
    }

    #[test]
    fn test_package_manager_equality() {
        assert_eq!(PackageManager::Npm, PackageManager::Npm);
        assert_ne!(PackageManager::Npm, PackageManager::Pnpm);
        assert_ne!(PackageManager::Yarn, PackageManager::Bun);
    }
}
