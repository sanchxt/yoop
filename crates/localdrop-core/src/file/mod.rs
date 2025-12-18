//! File operations for LocalDrop.
//!
//! This module handles:
//! - File and directory enumeration
//! - Chunking files for transfer
//! - Metadata preservation
//! - Path sanitization
//!
//! ## Metadata Preservation
//!
//! - Relative path structure
//! - File permissions (Unix only)
//! - Timestamps (created, modified)
//! - Symlinks: Option to follow or preserve

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Metadata for a file being transferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Relative path from share root
    pub relative_path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// MIME type
    pub mime_type: Option<String>,
    /// Created timestamp
    pub created: Option<SystemTime>,
    /// Modified timestamp
    pub modified: Option<SystemTime>,
    /// Unix permissions (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<u32>,
    /// Whether this is a symlink
    pub is_symlink: bool,
    /// Symlink target (if is_symlink)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<PathBuf>,
}

impl FileMetadata {
    /// Create metadata from a file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Absolute path to the file
    /// * `base` - Base directory for computing relative path
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn from_path(path: &Path, base: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path)?;
        let relative_path = path.strip_prefix(base).unwrap_or(path).to_path_buf();

        let mime_type = mime_guess::from_path(path).first().map(|m| m.to_string());

        Ok(Self {
            relative_path,
            size: metadata.len(),
            mime_type,
            created: metadata.created().ok(),
            modified: metadata.modified().ok(),
            permissions: None, // TODO: Unix permissions
            is_symlink: metadata.is_symlink(),
            symlink_target: None, // TODO: Read symlink target
        })
    }

    /// Get the file name.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.relative_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }
}

/// A file chunk for transfer.
#[derive(Debug, Clone)]
pub struct FileChunk {
    /// File index in the transfer
    pub file_index: usize,
    /// Chunk index within the file
    pub chunk_index: u64,
    /// Chunk data
    pub data: Vec<u8>,
    /// xxHash64 checksum
    pub checksum: u64,
    /// Whether this is the last chunk
    pub is_last: bool,
}

/// Options for file enumeration.
#[derive(Debug, Clone, Default)]
pub struct EnumerateOptions {
    /// Follow symlinks
    pub follow_symlinks: bool,
    /// Include hidden files
    pub include_hidden: bool,
    /// Maximum depth for directories
    pub max_depth: Option<usize>,
}

/// Enumerate files for sharing.
///
/// # Arguments
///
/// * `paths` - Paths to files and directories to share
/// * `options` - Enumeration options
///
/// # Errors
///
/// Returns an error if enumeration fails.
pub fn enumerate_files(paths: &[PathBuf], options: &EnumerateOptions) -> Result<Vec<FileMetadata>> {
    let mut files = Vec::new();

    for path in paths {
        if path.is_file() {
            let base = path.parent().unwrap_or(path);
            files.push(FileMetadata::from_path(path, base)?);
        } else if path.is_dir() {
            enumerate_directory(path, path, options, &mut files)?;
        }
    }

    Ok(files)
}

fn enumerate_directory(
    dir: &Path,
    base: &Path,
    options: &EnumerateOptions,
    files: &mut Vec<FileMetadata>,
) -> Result<()> {
    let walker = walkdir::WalkDir::new(dir)
        .follow_links(options.follow_symlinks)
        .max_depth(options.max_depth.unwrap_or(usize::MAX));

    for entry in walker.into_iter().filter_map(std::result::Result::ok) {
        let path = entry.path();

        if !options.include_hidden {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
        }

        if path.is_file() {
            files.push(FileMetadata::from_path(path, base)?);
        }
    }

    Ok(())
}

/// Chunker for reading file chunks.
#[derive(Debug)]
pub struct FileChunker {
    /// Chunk size in bytes
    pub chunk_size: usize,
}

impl FileChunker {
    /// Create a new chunker with the given chunk size.
    #[must_use]
    pub const fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }

    /// Read chunks from a file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    /// * `file_index` - Index of the file in the transfer
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub async fn read_chunks(&self, path: &Path, file_index: usize) -> Result<Vec<FileChunk>> {
        use tokio::io::AsyncReadExt;

        use crate::crypto::xxhash64;

        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        let mut chunks = Vec::new();
        let mut chunk_index: u64 = 0;
        let mut bytes_read_total: u64 = 0;

        loop {
            let mut buffer = vec![0u8; self.chunk_size];
            let bytes_read = file.read(&mut buffer).await?;

            if bytes_read == 0 {
                break;
            }

            buffer.truncate(bytes_read);
            bytes_read_total += bytes_read as u64;

            let is_last = bytes_read_total >= file_size;

            let checksum = xxhash64(&buffer);

            chunks.push(FileChunk {
                file_index,
                chunk_index,
                data: buffer,
                checksum,
                is_last,
            });

            chunk_index += 1;
        }

        Ok(chunks)
    }
}

/// Sanitize a path to prevent directory traversal attacks.
///
/// # Arguments
///
/// * `base` - Base directory
/// * `relative` - Relative path from base
///
/// # Returns
///
/// The sanitized absolute path, or None if the path is invalid.
#[must_use]
pub fn sanitize_path(base: &Path, relative: &Path) -> Option<PathBuf> {
    for component in relative.components() {
        if component == std::path::Component::ParentDir {
            return None;
        }
    }

    let full_path = base.join(relative);

    if full_path.starts_with(base) {
        Some(full_path)
    } else {
        None
    }
}

/// Format a file size for display.
#[must_use]
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Writer for receiving file chunks and assembling them.
#[derive(Debug)]
pub struct FileWriter {
    /// Output file path
    pub output_path: PathBuf,
    /// Expected total file size
    pub expected_size: u64,
    /// File handle
    file: Option<tokio::fs::File>,
    /// Bytes written so far
    pub bytes_written: u64,
    /// SHA-256 hasher for final verification
    sha256_hasher: sha2::Sha256,
}

impl FileWriter {
    /// Create a new file writer.
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path to write the file to
    /// * `expected_size` - Expected total file size
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub async fn new(output_path: PathBuf, expected_size: u64) -> Result<Self> {
        use tokio::fs::File;

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let file = File::create(&output_path).await?;

        Ok(Self {
            output_path,
            expected_size,
            file: Some(file),
            bytes_written: 0,
            sha256_hasher: sha2::Sha256::new(),
        })
    }

    /// Write a chunk to the file.
    ///
    /// Verifies the xxHash64 checksum before writing.
    ///
    /// # Errors
    ///
    /// Returns an error if checksum verification fails or write fails.
    pub async fn write_chunk(&mut self, chunk: &FileChunk) -> Result<()> {
        use sha2::Digest;
        use tokio::io::AsyncWriteExt;

        use crate::crypto::xxhash64;
        use crate::error::Error;

        let computed_checksum = xxhash64(&chunk.data);
        if computed_checksum != chunk.checksum {
            return Err(Error::ChecksumMismatch {
                file: self.output_path.display().to_string(),
                chunk: chunk.chunk_index,
            });
        }

        if let Some(ref mut file) = self.file {
            file.write_all(&chunk.data).await?;
        }

        self.sha256_hasher.update(&chunk.data);
        self.bytes_written += chunk.data.len() as u64;

        Ok(())
    }

    /// Finalize the file and return the SHA-256 hash.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be synced.
    pub async fn finalize(mut self) -> Result<[u8; 32]> {
        use sha2::Digest;
        use tokio::io::AsyncWriteExt;

        if let Some(ref mut file) = self.file {
            file.flush().await?;
            file.sync_all().await?;
        }
        self.file = None;

        Ok(self.sha256_hasher.finalize().into())
    }
}

use sha2::Digest;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_sanitize_path_valid() {
        let base = Path::new("/home/user/downloads");
        let relative = Path::new("file.txt");
        let result = sanitize_path(base, relative);
        assert_eq!(result, Some(PathBuf::from("/home/user/downloads/file.txt")));
    }

    #[test]
    fn test_sanitize_path_nested_valid() {
        let base = Path::new("/home/user/downloads");
        let relative = Path::new("subdir/file.txt");
        let result = sanitize_path(base, relative);
        assert_eq!(
            result,
            Some(PathBuf::from("/home/user/downloads/subdir/file.txt"))
        );
    }

    #[test]
    fn test_sanitize_path_traversal_attack() {
        let base = Path::new("/home/user/downloads");
        let relative = Path::new("../../../etc/passwd");
        let result = sanitize_path(base, relative);
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_chunk_small_file() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("small.txt");
        let content = b"Hello, LocalDrop!";
        std::fs::write(&file_path, content).expect("write file");

        let chunker = FileChunker::new(1024 * 1024);
        let chunks = chunker
            .read_chunks(&file_path, 0)
            .await
            .expect("read chunks");

        assert_eq!(chunks.len(), 1, "Small file should produce one chunk");

        let chunk = &chunks[0];
        assert_eq!(chunk.file_index, 0);
        assert_eq!(chunk.chunk_index, 0);
        assert_eq!(chunk.data, content);
        assert!(chunk.is_last, "Single chunk should be marked as last");

        let expected_checksum = crate::crypto::xxhash64(content);
        assert_eq!(chunk.checksum, expected_checksum);
    }

    #[tokio::test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    async fn test_chunk_large_file() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("large.bin");

        let chunk_size = 1024;
        let content: Vec<u8> = (0..2560).map(|i| (i % 256) as u8).collect();
        std::fs::write(&file_path, &content).expect("write file");

        let chunker = FileChunker::new(chunk_size);
        let chunks = chunker
            .read_chunks(&file_path, 0)
            .await
            .expect("read chunks");

        assert_eq!(
            chunks.len(),
            3,
            "2.5KB file with 1KB chunks should produce 3 chunks"
        );

        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].data.len(), 1024);
        assert!(!chunks[0].is_last);

        assert_eq!(chunks[1].chunk_index, 1);
        assert_eq!(chunks[1].data.len(), 1024);
        assert!(!chunks[1].is_last);

        assert_eq!(chunks[2].chunk_index, 2);
        assert_eq!(chunks[2].data.len(), 512);
        assert!(chunks[2].is_last);

        for chunk in &chunks {
            let expected_checksum = crate::crypto::xxhash64(&chunk.data);
            assert_eq!(chunk.checksum, expected_checksum);
        }

        let reassembled: Vec<u8> = chunks.iter().flat_map(|c| c.data.iter().copied()).collect();
        assert_eq!(reassembled, content);
    }

    #[tokio::test]
    async fn test_file_writer_basic() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output_path = temp_dir.path().join("output.txt");
        let content = b"Test content for writer";

        let chunk = FileChunk {
            file_index: 0,
            chunk_index: 0,
            data: content.to_vec(),
            checksum: crate::crypto::xxhash64(content),
            is_last: true,
        };

        let mut writer = FileWriter::new(output_path.clone(), content.len() as u64)
            .await
            .expect("create writer");
        writer.write_chunk(&chunk).await.expect("write chunk");
        let sha256 = writer.finalize().await.expect("finalize");

        let written_content = std::fs::read(&output_path).expect("read file");
        assert_eq!(written_content, content);

        let expected_sha256 = crate::crypto::sha256(content);
        assert_eq!(sha256, expected_sha256);
    }

    #[tokio::test]
    async fn test_file_writer_checksum_mismatch() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output_path = temp_dir.path().join("output.txt");

        let chunk = FileChunk {
            file_index: 0,
            chunk_index: 0,
            data: b"Test content".to_vec(),
            checksum: 12345,
            is_last: true,
        };

        let mut writer = FileWriter::new(output_path, 12)
            .await
            .expect("create writer");

        let result = writer.write_chunk(&chunk).await;
        assert!(result.is_err(), "Should fail on checksum mismatch");
    }

    #[tokio::test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    async fn test_roundtrip_chunking() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let original_path = temp_dir.path().join("original.bin");
        let output_path = temp_dir.path().join("copy.bin");

        let content: Vec<u8> = (0..5632).map(|i| (i % 256) as u8).collect();
        std::fs::write(&original_path, &content).expect("write original");

        let chunk_size = 1024;
        let chunker = FileChunker::new(chunk_size);
        let chunks = chunker
            .read_chunks(&original_path, 0)
            .await
            .expect("read chunks");

        let mut writer = FileWriter::new(output_path.clone(), content.len() as u64)
            .await
            .expect("create writer");

        for chunk in &chunks {
            writer.write_chunk(chunk).await.expect("write chunk");
        }
        let sha256 = writer.finalize().await.expect("finalize");

        let output_content = std::fs::read(&output_path).expect("read output");
        assert_eq!(output_content, content, "Roundtrip should preserve content");

        let expected_sha256 = crate::crypto::sha256(&content);
        assert_eq!(sha256, expected_sha256, "SHA-256 should match");
    }
}
