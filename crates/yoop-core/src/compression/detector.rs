//! Smart compression detection.
//!
//! This module provides logic to determine whether a file should be compressed
//! based on its extension (known incompressible types) or content analysis.

use std::path::Path;

use super::CompressionMode;

/// List of file extensions that are known to be incompressible.
///
/// These are already compressed or don't benefit from compression.
pub const INCOMPRESSIBLE_EXTENSIONS: &[&str] = &[
    // images
    "jpg",
    "jpeg",
    "png",
    "gif",
    "webp",
    "heic",
    "heif",
    "avif",
    "ico",
    "bmp",
    "tiff",
    "tif",
    // videos
    "mp4",
    "mkv",
    "webm",
    "avi",
    "mov",
    "m4v",
    "wmv",
    "flv",
    "mpeg",
    "mpg",
    "3gp",
    // audios
    "mp3",
    "aac",
    "ogg",
    "flac",
    "m4a",
    "opus",
    "wma",
    "wav",
    "aiff",
    // archives
    "zip",
    "gz",
    "bz2",
    "xz",
    "7z",
    "rar",
    "zst",
    "lz4",
    "lzma",
    "tar.gz",
    "tar.bz2",
    "tar.xz",
    "tgz",
    "tbz2",
    "txz",
    // docs
    "pdf",
    "docx",
    "xlsx",
    "pptx",
    "epub",
    "odt",
    "ods",
    "odp",
    // fonts
    "woff",
    "woff2",
    "eot",
    // game/3d assets
    "unity3d",
    "unitypackage",
    // disk images
    "dmg",
    "iso",
];

/// Decision on whether to compress a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionDecision {
    /// Definitely compress all chunks
    Compress,
    /// Definitely don't compress
    Skip,
    /// Test first chunk to decide
    TestFirstChunk,
}

/// Determine if a file should be compressed based on extension and mode.
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `mode` - Compression mode setting
///
/// # Returns
///
/// A decision on whether to compress the file.
#[must_use]
pub fn should_compress_file(path: &Path, mode: CompressionMode) -> CompressionDecision {
    match mode {
        CompressionMode::Never => CompressionDecision::Skip,
        CompressionMode::Always => CompressionDecision::Compress,
        CompressionMode::Auto => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase);

            match ext {
                Some(ref e) if is_incompressible_extension(e) => CompressionDecision::Skip,
                _ => CompressionDecision::TestFirstChunk,
            }
        }
    }
}

/// Check if an extension is in the incompressible list.
fn is_incompressible_extension(ext: &str) -> bool {
    INCOMPRESSIBLE_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_compress_never_mode() {
        assert_eq!(
            should_compress_file(Path::new("data.json"), CompressionMode::Never),
            CompressionDecision::Skip
        );
    }

    #[test]
    fn test_should_compress_always_mode() {
        assert_eq!(
            should_compress_file(Path::new("photo.jpg"), CompressionMode::Always),
            CompressionDecision::Compress
        );
    }

    #[test]
    fn test_should_compress_auto_incompressible() {
        let incompressible_files = [
            "photo.jpg",
            "video.mp4",
            "song.mp3",
            "archive.zip",
            "document.pdf",
            "image.PNG", // Test case insensitivity
        ];

        for file in incompressible_files {
            assert_eq!(
                should_compress_file(Path::new(file), CompressionMode::Auto),
                CompressionDecision::Skip,
                "File {file} should be skipped"
            );
        }
    }

    #[test]
    fn test_should_compress_auto_compressible() {
        let compressible_files = [
            "data.json",
            "config.toml",
            "script.js",
            "style.css",
            "readme.txt",
            "code.rs",
            "log.log",
        ];

        for file in compressible_files {
            assert_eq!(
                should_compress_file(Path::new(file), CompressionMode::Auto),
                CompressionDecision::TestFirstChunk,
                "File {file} should test first chunk"
            );
        }
    }

    #[test]
    fn test_should_compress_no_extension() {
        assert_eq!(
            should_compress_file(Path::new("Makefile"), CompressionMode::Auto),
            CompressionDecision::TestFirstChunk
        );
    }

    #[test]
    fn test_incompressible_extensions_list() {
        // Verify common formats are covered
        assert!(is_incompressible_extension("jpg"));
        assert!(is_incompressible_extension("mp4"));
        assert!(is_incompressible_extension("zip"));
        assert!(is_incompressible_extension("pdf"));

        // Verify text formats are NOT in the list
        assert!(!is_incompressible_extension("txt"));
        assert!(!is_incompressible_extension("json"));
        assert!(!is_incompressible_extension("xml"));
    }
}
