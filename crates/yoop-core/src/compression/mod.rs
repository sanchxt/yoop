//! Compression support for file transfers.
//!
//! This module provides transparent compression for file transfers using zstd.
//! Compression is applied per-chunk to enable streaming, resumable transfers.
//!
//! ## Features
//!
//! - **Per-chunk compression**: Each chunk is compressed independently
//! - **Smart detection**: Auto mode skips known incompressible file types
//! - **Statistics tracking**: Track compression ratios and savings
//!
//! ## Example
//!
//! ```rust,ignore
//! use yoop_core::compression::{CompressionConfig, CompressionMode};
//!
//! let config = CompressionConfig {
//!     mode: CompressionMode::Auto,
//!     level: 1,
//!     skip_threshold: 0.95,
//! };
//! ```

mod detector;
mod stats;
mod zstd_impl;

pub use detector::{should_compress_file, CompressionDecision, INCOMPRESSIBLE_EXTENSIONS};
pub use stats::CompressionStats;
pub use zstd_impl::{compress, decompress, should_compress};

use serde::{Deserialize, Serialize};

/// Compression algorithm identifier for wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum CompressionAlgorithm {
    /// No compression
    #[default]
    None = 0,
    /// Zstandard compression
    Zstd = 1,
}

impl CompressionAlgorithm {
    /// Create from a byte value.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::None),
            1 => Some(Self::Zstd),
            _ => None,
        }
    }

    /// Convert to byte value.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        self as u8
    }
}

/// Compression mode setting for transfers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionMode {
    /// Automatically detect compressible files
    #[default]
    Auto,
    /// Always compress
    Always,
    /// Never compress
    Never,
}

/// Compression configuration for transfers.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Compression mode
    pub mode: CompressionMode,
    /// Compression level (1-3 for fast, default 1)
    pub level: u8,
    /// Skip compression if ratio exceeds this threshold (0.95 = 95%)
    pub skip_threshold: f64,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            mode: CompressionMode::Auto,
            level: 1,
            skip_threshold: 0.95,
        }
    }
}

impl CompressionConfig {
    /// Create a new compression config with the given mode.
    #[must_use]
    pub const fn new(mode: CompressionMode) -> Self {
        Self {
            mode,
            level: 1,
            skip_threshold: 0.95,
        }
    }

    /// Set the compression level.
    #[must_use]
    pub const fn with_level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    /// Set the skip threshold.
    #[must_use]
    pub const fn with_skip_threshold(mut self, threshold: f64) -> Self {
        self.skip_threshold = threshold;
        self
    }
}

/// Compression capabilities for protocol negotiation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompressionCapabilities {
    /// Supported compression algorithms
    pub algorithms: Vec<CompressionAlgorithm>,
    /// Preferred compression level
    pub level: u8,
}

impl CompressionCapabilities {
    /// Create capabilities with zstd support.
    #[must_use]
    pub fn with_zstd(level: u8) -> Self {
        Self {
            algorithms: vec![CompressionAlgorithm::Zstd],
            level,
        }
    }

    /// Create empty capabilities (no compression).
    #[must_use]
    pub fn none() -> Self {
        Self {
            algorithms: vec![],
            level: 0,
        }
    }

    /// Check if compression is supported.
    #[must_use]
    pub fn supports_compression(&self) -> bool {
        !self.algorithms.is_empty()
    }

    /// Check if a specific algorithm is supported.
    #[must_use]
    pub fn supports(&self, algo: CompressionAlgorithm) -> bool {
        self.algorithms.contains(&algo)
    }

    /// Negotiate compression with another peer's capabilities.
    ///
    /// Returns the agreed algorithm (or None if no common support).
    #[must_use]
    pub fn negotiate(&self, other: &Self) -> Option<CompressionAlgorithm> {
        if self.supports(CompressionAlgorithm::Zstd) && other.supports(CompressionAlgorithm::Zstd) {
            return Some(CompressionAlgorithm::Zstd);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_algorithm_from_byte() {
        assert_eq!(
            CompressionAlgorithm::from_byte(0),
            Some(CompressionAlgorithm::None)
        );
        assert_eq!(
            CompressionAlgorithm::from_byte(1),
            Some(CompressionAlgorithm::Zstd)
        );
        assert_eq!(CompressionAlgorithm::from_byte(2), None);
    }

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert_eq!(config.mode, CompressionMode::Auto);
        assert_eq!(config.level, 1);
        assert!((config.skip_threshold - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compression_capabilities_negotiate() {
        let caps1 = CompressionCapabilities::with_zstd(1);
        let caps2 = CompressionCapabilities::with_zstd(3);

        assert_eq!(caps1.negotiate(&caps2), Some(CompressionAlgorithm::Zstd));

        let caps3 = CompressionCapabilities::none();
        assert_eq!(caps1.negotiate(&caps3), None);
    }

    #[test]
    fn test_compression_mode_serde() {
        let mode = CompressionMode::Auto;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"auto\"");

        let mode: CompressionMode = serde_json::from_str("\"always\"").unwrap();
        assert_eq!(mode, CompressionMode::Always);
    }
}
