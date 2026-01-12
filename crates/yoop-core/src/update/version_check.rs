//! Version checking against npm registry.

use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::Config;
use crate::error::{Error, Result};

const REGISTRY_URL: &str = "https://registry.npmjs.org/yoop/latest";
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Status of an update check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// Current installed version
    pub current_version: Version,
    /// Latest available version
    pub latest_version: Version,
    /// Whether an update is available
    pub update_available: bool,
    /// URL to the release page
    pub release_url: String,
}

/// Version checker for querying npm registry.
pub struct VersionChecker {
    client: Client,
    registry_url: String,
}

impl VersionChecker {
    /// Create a new version checker instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            registry_url: REGISTRY_URL.to_string(),
        }
    }

    /// Check for updates by querying the npm registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the version cannot be parsed, network request fails, or registry response is invalid.
    pub async fn check(&self) -> Result<UpdateStatus> {
        let current = Version::parse(crate::VERSION)
            .map_err(|e| Error::Internal(format!("failed to parse current version: {e}")))?;

        let resp: NpmPackageInfo = self
            .client
            .get(&self.registry_url)
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| Error::Internal(format!("failed to check for updates: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Internal(format!("failed to parse registry response: {e}")))?;

        let latest = Version::parse(&resp.version)
            .map_err(|e| Error::Internal(format!("failed to parse latest version: {e}")))?;

        let update_available = latest > current;

        Ok(UpdateStatus {
            current_version: current,
            latest_version: latest.clone(),
            update_available,
            release_url: format!("https://github.com/sanchxt/yoop/releases/tag/v{latest}"),
        })
    }

    /// Check for updates with caching based on check interval.
    ///
    /// # Errors
    ///
    /// Returns an error if system time cannot be retrieved or update check fails.
    pub async fn check_with_cache(&self, config: &mut Config) -> Result<Option<UpdateStatus>> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::Internal(format!("system time error: {e}")))?
            .as_secs();

        if let Some(last_check) = config.update.last_check {
            let elapsed = now.saturating_sub(last_check);
            if elapsed < config.update.check_interval.as_secs() {
                return Ok(None);
            }
        }

        let status = self.check().await?;

        config.update.last_check = Some(now);
        config.save()?;

        Ok(Some(status))
    }
}

impl Default for VersionChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct NpmPackageInfo {
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("0.1.3").unwrap();
        let v2 = Version::parse("0.2.0").unwrap();
        assert!(v2 > v1);

        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.0.0").unwrap();
        assert_eq!(v1, v2);
    }
}
