//! Update functionality for Yoop.
//!
//! This module provides functionality for checking and installing updates.

#[cfg(feature = "update")]
pub mod package_manager;
#[cfg(feature = "update")]
pub mod version_check;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

use crate::error::Result;

/// Semantic version number for schema versioning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaVersion {
    /// Major version number.
    pub major: u32,
    /// Minor version number.
    pub minor: u32,
    /// Patch version number.
    pub patch: u32,
}

impl SchemaVersion {
    /// Create a new schema version.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string in the format "major.minor.patch" or "vmajor.minor.patch".
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not in the correct format or contains invalid numbers.
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.trim().trim_start_matches('v').split('.').collect();

        if parts.len() != 3 {
            return Err(crate::error::Error::Internal(format!(
                "invalid version format: {s}"
            )));
        }

        let major = parts[0]
            .parse()
            .map_err(|e| crate::error::Error::Internal(format!("invalid major version: {e}")))?;
        let minor = parts[1]
            .parse()
            .map_err(|e| crate::error::Error::Internal(format!("invalid minor version: {e}")))?;
        let patch = parts[2]
            .parse()
            .map_err(|e| crate::error::Error::Internal(format!("invalid patch version: {e}")))?;

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for SchemaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SchemaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                other => other,
            },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version_parse() {
        let v = SchemaVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);

        let v = SchemaVersion::parse("v0.1.3").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_schema_version_display() {
        let v = SchemaVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_schema_version_ord() {
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(2, 0, 0);
        assert!(v1 < v2);

        let v1 = SchemaVersion::new(1, 1, 0);
        let v2 = SchemaVersion::new(1, 2, 0);
        assert!(v1 < v2);

        let v1 = SchemaVersion::new(1, 0, 1);
        let v2 = SchemaVersion::new(1, 0, 2);
        assert!(v1 < v2);

        let v1 = SchemaVersion::new(1, 2, 3);
        let v2 = SchemaVersion::new(1, 2, 3);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_schema_version_parse_with_v_prefix() {
        let v = SchemaVersion::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_schema_version_parse_invalid_format() {
        assert!(SchemaVersion::parse("1.2").is_err());
        assert!(SchemaVersion::parse("1").is_err());
        assert!(SchemaVersion::parse("1.2.3.4").is_err());
        assert!(SchemaVersion::parse("abc.def.ghi").is_err());
        assert!(SchemaVersion::parse("").is_err());
    }

    #[test]
    fn test_schema_version_parse_edge_cases() {
        let v = SchemaVersion::parse("0.0.0").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);

        let v = SchemaVersion::parse("999.999.999").unwrap();
        assert_eq!(v.major, 999);
        assert_eq!(v.minor, 999);
        assert_eq!(v.patch, 999);
    }

    #[test]
    fn test_schema_version_ord_edge_cases() {
        let v1 = SchemaVersion::new(0, 0, 0);
        let v2 = SchemaVersion::new(0, 0, 1);
        assert!(v1 < v2);

        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(1, 0, 0);
        assert!(v1 <= v2);
        assert!(v1 >= v2);

        let v1 = SchemaVersion::new(2, 0, 0);
        let v2 = SchemaVersion::new(1, 999, 999);
        assert!(v1 > v2);
    }

    #[test]
    fn test_schema_version_clone_and_equality() {
        let v1 = SchemaVersion::new(1, 2, 3);
        let v2 = &v1;

        assert_eq!(v1, *v2);
        assert!(v1 >= *v2);
        assert!(v1 <= *v2);
    }

    #[test]
    fn test_schema_version_roundtrip() {
        let original = SchemaVersion::new(1, 2, 3);
        let string = original.to_string();
        let parsed = SchemaVersion::parse(&string).unwrap();

        assert_eq!(original, parsed);
    }
}
