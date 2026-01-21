//! Compression statistics tracking.
//!
//! This module provides types for tracking compression performance
//! during file transfers.

use serde::{Deserialize, Serialize};

/// Statistics for compression during a transfer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompressionStats {
    /// Total uncompressed bytes (original size)
    pub original_bytes: u64,
    /// Total compressed bytes sent over wire
    pub compressed_bytes: u64,
    /// Number of chunks that were compressed
    pub chunks_compressed: u32,
    /// Number of chunks sent uncompressed
    pub chunks_uncompressed: u32,
}

impl CompressionStats {
    /// Create new empty statistics.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            original_bytes: 0,
            compressed_bytes: 0,
            chunks_compressed: 0,
            chunks_uncompressed: 0,
        }
    }

    /// Calculate the compression ratio (0.0 to 1.0).
    ///
    /// Returns the savings ratio where 0.7 means 70% of data was saved.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        if self.original_bytes == 0 {
            0.0
        } else {
            1.0 - (self.compressed_bytes as f64 / self.original_bytes as f64)
        }
    }

    /// Get bytes saved by compression.
    #[must_use]
    pub fn bytes_saved(&self) -> u64 {
        self.original_bytes.saturating_sub(self.compressed_bytes)
    }

    /// Get a human-readable display of savings.
    ///
    /// Returns a string like "70% saved" or empty string if no significant savings.
    #[must_use]
    pub fn savings_display(&self) -> String {
        let ratio = self.ratio();
        if ratio > 0.01 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percentage = (ratio * 100.0) as u32;
            format!("{percentage}% saved")
        } else {
            String::new()
        }
    }

    /// Add a compressed chunk to the stats.
    pub fn add_compressed(&mut self, original_size: u64, compressed_size: u64) {
        self.original_bytes += original_size;
        self.compressed_bytes += compressed_size;
        self.chunks_compressed += 1;
    }

    /// Add an uncompressed chunk to the stats.
    pub fn add_uncompressed(&mut self, size: u64) {
        self.original_bytes += size;
        self.compressed_bytes += size;
        self.chunks_uncompressed += 1;
    }

    /// Total number of chunks processed.
    #[must_use]
    pub fn total_chunks(&self) -> u32 {
        self.chunks_compressed + self.chunks_uncompressed
    }

    /// Percentage of chunks that were compressed.
    #[must_use]
    pub fn compression_percentage(&self) -> f64 {
        let total = self.total_chunks();
        if total == 0 {
            0.0
        } else {
            (f64::from(self.chunks_compressed) / f64::from(total)) * 100.0
        }
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &Self) {
        self.original_bytes += other.original_bytes;
        self.compressed_bytes += other.compressed_bytes;
        self.chunks_compressed += other.chunks_compressed;
        self.chunks_uncompressed += other.chunks_uncompressed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_stats_new() {
        let stats = CompressionStats::new();
        assert_eq!(stats.original_bytes, 0);
        assert_eq!(stats.compressed_bytes, 0);
        assert_eq!(stats.chunks_compressed, 0);
        assert_eq!(stats.chunks_uncompressed, 0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_compression_ratio() {
        let mut stats = CompressionStats::new();
        stats.original_bytes = 1000;
        stats.compressed_bytes = 300;

        let ratio = stats.ratio();
        assert!((ratio - 0.7).abs() < 0.001);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_compression_ratio_zero_original() {
        let stats = CompressionStats::new();
        assert_eq!(stats.ratio(), 0.0);
    }

    #[test]
    fn test_bytes_saved() {
        let mut stats = CompressionStats::new();
        stats.original_bytes = 1000;
        stats.compressed_bytes = 300;

        assert_eq!(stats.bytes_saved(), 700);
    }

    #[test]
    fn test_savings_display() {
        let mut stats = CompressionStats::new();
        stats.original_bytes = 1000;
        stats.compressed_bytes = 300;

        assert_eq!(stats.savings_display(), "70% saved");
    }

    #[test]
    fn test_savings_display_no_savings() {
        let mut stats = CompressionStats::new();
        stats.original_bytes = 1000;
        stats.compressed_bytes = 995;

        // Less than 1% savings
        assert!(stats.savings_display().is_empty());
    }

    #[test]
    fn test_add_compressed() {
        let mut stats = CompressionStats::new();
        stats.add_compressed(1000, 300);

        assert_eq!(stats.original_bytes, 1000);
        assert_eq!(stats.compressed_bytes, 300);
        assert_eq!(stats.chunks_compressed, 1);
        assert_eq!(stats.chunks_uncompressed, 0);
    }

    #[test]
    fn test_add_uncompressed() {
        let mut stats = CompressionStats::new();
        stats.add_uncompressed(1000);

        assert_eq!(stats.original_bytes, 1000);
        assert_eq!(stats.compressed_bytes, 1000);
        assert_eq!(stats.chunks_compressed, 0);
        assert_eq!(stats.chunks_uncompressed, 1);
    }

    #[test]
    fn test_total_chunks() {
        let mut stats = CompressionStats::new();
        stats.add_compressed(1000, 300);
        stats.add_compressed(1000, 400);
        stats.add_uncompressed(500);

        assert_eq!(stats.total_chunks(), 3);
    }

    #[test]
    fn test_compression_percentage() {
        let mut stats = CompressionStats::new();
        stats.add_compressed(1000, 300);
        stats.add_compressed(1000, 400);
        stats.add_uncompressed(500);
        stats.add_uncompressed(500);

        // 2 out of 4 chunks compressed = 50%
        assert!((stats.compression_percentage() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_merge_stats() {
        let mut stats1 = CompressionStats::new();
        stats1.add_compressed(1000, 300);

        let mut stats2 = CompressionStats::new();
        stats2.add_uncompressed(500);

        stats1.merge(&stats2);

        assert_eq!(stats1.original_bytes, 1500);
        assert_eq!(stats1.compressed_bytes, 800);
        assert_eq!(stats1.chunks_compressed, 1);
        assert_eq!(stats1.chunks_uncompressed, 1);
    }

    #[test]
    fn test_stats_serialization() {
        let mut stats = CompressionStats::new();
        stats.add_compressed(1000, 300);

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: CompressionStats = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.original_bytes, stats.original_bytes);
        assert_eq!(deserialized.compressed_bytes, stats.compressed_bytes);
    }
}
