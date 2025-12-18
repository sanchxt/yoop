//! File preview generation for LocalDrop.
//!
//! This module generates previews for various file types:
//!
//! | File Type | Preview Method | Max Size |
//! |-----------|----------------|----------|
//! | Images | Thumbnail (256x256) | 50KB |
//! | Videos | First frame thumbnail | 50KB |
//! | PDF | First page thumbnail | 100KB |
//! | Text/Code | First 1KB of content | 1KB |
//! | Documents | File icon + metadata | 1KB |
//! | Archives | File listing (first 50) | 10KB |
//! | Other | File icon + size/type | 100B |

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Type of preview generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewType {
    /// Image thumbnail
    Thumbnail,
    /// Text snippet
    Text,
    /// Archive file listing
    ArchiveListing,
    /// Generic icon with metadata
    Icon,
    /// No preview available
    None,
}

/// A file preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preview {
    /// Type of preview
    pub preview_type: PreviewType,
    /// Preview data (base64 encoded for images)
    pub data: String,
    /// MIME type of preview data
    pub mime_type: String,
    /// Size of original file
    pub original_size: u64,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PreviewMetadata>,
}

/// Additional preview metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreviewMetadata {
    /// Image dimensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<(u32, u32)>,
    /// Number of files in archive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
    /// Number of pages in document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
    /// Duration for media files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
}

/// Configuration for preview generation.
#[derive(Debug, Clone)]
pub struct PreviewConfig {
    /// Maximum thumbnail size in bytes
    pub max_thumbnail_size: usize,
    /// Maximum text preview length
    pub max_text_length: usize,
    /// Thumbnail dimensions
    pub thumbnail_size: (u32, u32),
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            max_thumbnail_size: 50 * 1024,
            max_text_length: 1024,
            thumbnail_size: (256, 256),
        }
    }
}

/// Preview generator.
#[derive(Debug)]
pub struct PreviewGenerator {
    config: PreviewConfig,
}

impl PreviewGenerator {
    /// Create a new preview generator with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(PreviewConfig::default())
    }

    /// Create a new preview generator with custom config.
    #[must_use]
    pub const fn with_config(config: PreviewConfig) -> Self {
        Self { config }
    }

    /// Generate a preview for a file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    ///
    /// # Errors
    ///
    /// Returns an error if the preview cannot be generated.
    pub async fn generate(&self, path: &Path) -> Result<Preview> {
        let metadata = std::fs::metadata(path)?;
        let mime = mime_guess::from_path(path).first();

        let preview_type = self.determine_preview_type(path, mime.as_ref());

        match preview_type {
            PreviewType::Thumbnail => self.generate_thumbnail(path).await,
            PreviewType::Text => self.generate_text_preview(path).await,
            PreviewType::ArchiveListing => self.generate_archive_listing(path).await,
            PreviewType::Icon | PreviewType::None => Ok(Preview {
                preview_type,
                data: String::new(),
                mime_type: mime.map(|m| m.to_string()).unwrap_or_default(),
                original_size: metadata.len(),
                metadata: None,
            }),
        }
    }

    fn determine_preview_type(&self, path: &Path, mime: Option<&mime_guess::Mime>) -> PreviewType {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);

        if let Some(mime) = mime {
            match mime.type_().as_str() {
                "image" => return PreviewType::Thumbnail,
                "text" => return PreviewType::Text,
                _ => {}
            }
        }

        if let Some(ext) = extension {
            match ext.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" => {
                    return PreviewType::Thumbnail;
                }
                "txt" | "md" | "json" | "xml" | "csv" | "log" | "rs" | "py" | "js" | "ts"
                | "go" | "java" | "c" | "cpp" | "h" | "toml" | "yaml" | "yml" => {
                    return PreviewType::Text;
                }
                "zip" | "tar" | "gz" | "7z" | "rar" => {
                    return PreviewType::ArchiveListing;
                }
                _ => {}
            }
        }

        PreviewType::Icon
    }

    async fn generate_thumbnail(&self, path: &Path) -> Result<Preview> {
        // TODO: Implement image thumbnail generation using `image` crate
        let metadata = std::fs::metadata(path)?;

        Ok(Preview {
            preview_type: PreviewType::Thumbnail,
            data: String::new(), // TODO: Base64 encoded thumbnail
            mime_type: "image/jpeg".to_string(),
            original_size: metadata.len(),
            metadata: Some(PreviewMetadata {
                dimensions: None, // TODO: Read from image
                ..Default::default()
            }),
        })
    }

    async fn generate_text_preview(&self, path: &Path) -> Result<Preview> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut buffer = vec![0u8; self.config.max_text_length];
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read);

        let text = String::from_utf8_lossy(&buffer).to_string();
        let metadata = std::fs::metadata(path)?;

        Ok(Preview {
            preview_type: PreviewType::Text,
            data: text,
            mime_type: "text/plain".to_string(),
            original_size: metadata.len(),
            metadata: None,
        })
    }

    async fn generate_archive_listing(&self, _path: &Path) -> Result<Preview> {
        // TODO: Implement archive listing
        Ok(Preview {
            preview_type: PreviewType::ArchiveListing,
            data: String::new(),
            mime_type: "application/json".to_string(),
            original_size: 0,
            metadata: Some(PreviewMetadata {
                file_count: None, // TODO: Count files in archive
                ..Default::default()
            }),
        })
    }
}

impl Default for PreviewGenerator {
    fn default() -> Self {
        Self::new()
    }
}
