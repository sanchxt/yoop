//! Zstd compression implementation.
//!
//! This module wraps the zstd library for compressing and decompressing
//! chunk data during file transfers.

use std::io::Cursor;

use crate::error::{Error, Result};

/// Compress data using zstd.
///
/// # Arguments
///
/// * `data` - The data to compress
/// * `level` - Compression level (1-22, lower = faster)
///
/// # Errors
///
/// Returns an error if compression fails.
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let cursor = Cursor::new(data);
    zstd::stream::encode_all(cursor, level)
        .map_err(|e| Error::Compression(format!("zstd compress failed: {e}")))
}

/// Decompress zstd data.
///
/// # Arguments
///
/// * `data` - The compressed data
///
/// # Errors
///
/// Returns an error if decompression fails.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(data);
    zstd::stream::decode_all(cursor)
        .map_err(|e| Error::Compression(format!("zstd decompress failed: {e}")))
}

/// Check if compression is worthwhile for this data.
///
/// Does a quick compression test and returns `true` if the compressed
/// size is below the given threshold ratio.
///
/// # Arguments
///
/// * `data` - The data to test
/// * `threshold` - Maximum ratio to consider compression worthwhile (e.g., 0.95)
///
/// # Returns
///
/// `true` if compression would be beneficial, `false` otherwise.
#[must_use]
pub fn should_compress(data: &[u8], threshold: f64) -> bool {
    // Don't bother compressing tiny chunks
    if data.len() < 1024 {
        return false;
    }

    // Quick compression test at level 1 (fastest)
    let Ok(compressed) = compress(data, 1) else {
        return false;
    };

    let ratio = compressed.len() as f64 / data.len() as f64;
    ratio < threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"Hello, this is test data that should compress well. ".repeat(100);
        let compressed = compress(&original, 1).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compress_empty() {
        let data = b"";
        let compressed = compress(data, 1).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_compression_ratio_text() {
        let text = b"Repetitive text that compresses well. ".repeat(1000);
        let compressed = compress(&text, 1).unwrap();
        let ratio = compressed.len() as f64 / text.len() as f64;
        assert!(ratio < 0.5, "Text should compress to <50% of original");
    }

    #[test]
    fn test_should_compress_tiny_data() {
        let tiny = b"hi";
        assert!(!should_compress(tiny, 0.95));
    }

    #[test]
    fn test_should_compress_compressible() {
        let data = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".repeat(100);
        assert!(should_compress(&data, 0.95));
    }

    #[test]
    fn test_should_compress_random() {
        // Pseudo-random data (deterministic for testing)
        let mut data = Vec::with_capacity(10000);
        let mut state: u64 = 12345;
        for _ in 0..10000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((state >> 16) as u8);
        }
        // Random data typically doesn't compress well below 95%
        assert!(!should_compress(&data, 0.90));
    }

    #[test]
    fn test_decompress_invalid_data() {
        let invalid = b"this is not valid zstd data";
        let result = decompress(invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_compress_different_levels() {
        let data = b"Test data for different compression levels".repeat(100);

        let level1 = compress(&data, 1).unwrap();
        let level3 = compress(&data, 3).unwrap();

        // Both should decompress correctly
        assert_eq!(decompress(&level1).unwrap(), data.as_slice());
        assert_eq!(decompress(&level3).unwrap(), data.as_slice());

        // Higher level might compress better (or same)
        assert!(level3.len() <= level1.len() + 10); // Allow some variance
    }
}
