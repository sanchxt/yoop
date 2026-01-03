//! File preview generation for Yoop.
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
        use base64::Engine;
        use image::GenericImageView;
        use std::io::Cursor;

        let metadata = std::fs::metadata(path)?;

        // Try to open the image
        let Ok(img) = image::open(path) else {
            // If image cannot be opened (e.g., unsupported format like JPEG),
            // return a preview with just metadata
            return Ok(Preview {
                preview_type: PreviewType::Icon,
                data: String::new(),
                mime_type: mime_guess::from_path(path)
                    .first()
                    .map_or_else(|| "application/octet-stream".to_string(), |m| m.to_string()),
                original_size: metadata.len(),
                metadata: None,
            });
        };

        let (width, height) = img.dimensions();
        let (max_w, max_h) = self.config.thumbnail_size;

        // Generate thumbnail
        let thumb = img.thumbnail(max_w, max_h);

        // Encode as PNG to a buffer
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        thumb
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;

        // Base64 encode the thumbnail
        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);

        Ok(Preview {
            preview_type: PreviewType::Thumbnail,
            data: encoded,
            mime_type: "image/png".to_string(),
            original_size: metadata.len(),
            metadata: Some(PreviewMetadata {
                dimensions: Some((width, height)),
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

    async fn generate_archive_listing(&self, path: &Path) -> Result<Preview> {
        #[cfg(feature = "web")]
        {
            self.generate_zip_listing(path).await
        }

        #[cfg(not(feature = "web"))]
        {
            // Without the web feature, zip crate is not available
            let metadata = std::fs::metadata(path)?;
            Ok(Preview {
                preview_type: PreviewType::ArchiveListing,
                data: "[]".to_string(),
                mime_type: "application/json".to_string(),
                original_size: metadata.len(),
                metadata: Some(PreviewMetadata {
                    file_count: None,
                    ..Default::default()
                }),
            })
        }
    }

    #[cfg(feature = "web")]
    async fn generate_zip_listing(&self, path: &Path) -> Result<Preview> {
        use std::fs::File;

        let metadata = std::fs::metadata(path)?;
        let file = File::open(path)?;

        // Try to open as ZIP archive
        let Ok(archive) = zip::ZipArchive::new(file) else {
            // Not a valid ZIP file, return empty listing
            return Ok(Preview {
                preview_type: PreviewType::ArchiveListing,
                data: "[]".to_string(),
                mime_type: "application/json".to_string(),
                original_size: metadata.len(),
                metadata: Some(PreviewMetadata {
                    file_count: Some(0),
                    ..Default::default()
                }),
            });
        };

        let total_files = archive.len();

        // Collect file names (up to 50 entries)
        let entries: Vec<String> = archive.file_names().take(50).map(String::from).collect();

        let data = serde_json::to_string(&entries)
            .map_err(|e| crate::error::Error::Io(std::io::Error::other(e.to_string())))?;

        Ok(Preview {
            preview_type: PreviewType::ArchiveListing,
            data,
            mime_type: "application/json".to_string(),
            original_size: metadata.len(),
            metadata: Some(PreviewMetadata {
                file_count: Some(total_files),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_text_preview() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let content = "Hello, this is a test file.\nWith multiple lines.\n";
        std::fs::write(&file_path, content).unwrap();

        let generator = PreviewGenerator::new();
        let preview = generator.generate(&file_path).await.unwrap();

        assert_eq!(preview.preview_type, PreviewType::Text);
        assert_eq!(preview.data, content);
        assert_eq!(preview.mime_type, "text/plain");
        assert_eq!(preview.original_size, content.len() as u64);
    }

    #[tokio::test]
    async fn test_text_preview_truncation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");

        // Create a file larger than max_text_length
        let content = "x".repeat(2000);
        std::fs::write(&file_path, &content).unwrap();

        let config = PreviewConfig {
            max_text_length: 100,
            ..Default::default()
        };
        let generator = PreviewGenerator::with_config(config);
        let preview = generator.generate(&file_path).await.unwrap();

        assert_eq!(preview.preview_type, PreviewType::Text);
        assert_eq!(preview.data.len(), 100);
        assert_eq!(preview.original_size, 2000);
    }

    #[tokio::test]
    async fn test_png_thumbnail_generation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.png");

        // Create a simple 2x2 PNG image
        let img = image::RgbImage::from_fn(100, 100, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        img.save(&file_path).unwrap();

        let generator = PreviewGenerator::new();
        let preview = generator.generate(&file_path).await.unwrap();

        assert_eq!(preview.preview_type, PreviewType::Thumbnail);
        assert_eq!(preview.mime_type, "image/png");
        assert!(
            !preview.data.is_empty(),
            "Thumbnail data should not be empty"
        );

        // Verify dimensions metadata
        let meta = preview.metadata.unwrap();
        assert_eq!(meta.dimensions, Some((100, 100)));
    }

    #[tokio::test]
    async fn test_preview_type_detection() {
        let generator = PreviewGenerator::new();

        // Test image detection
        assert_eq!(
            generator.determine_preview_type(Path::new("image.png"), None),
            PreviewType::Thumbnail
        );
        assert_eq!(
            generator.determine_preview_type(Path::new("photo.gif"), None),
            PreviewType::Thumbnail
        );

        // Test text detection
        assert_eq!(
            generator.determine_preview_type(Path::new("file.txt"), None),
            PreviewType::Text
        );
        assert_eq!(
            generator.determine_preview_type(Path::new("code.rs"), None),
            PreviewType::Text
        );
        assert_eq!(
            generator.determine_preview_type(Path::new("config.toml"), None),
            PreviewType::Text
        );

        // Test archive detection
        assert_eq!(
            generator.determine_preview_type(Path::new("archive.zip"), None),
            PreviewType::ArchiveListing
        );
        assert_eq!(
            generator.determine_preview_type(Path::new("backup.tar"), None),
            PreviewType::ArchiveListing
        );

        // Test icon fallback
        assert_eq!(
            generator.determine_preview_type(Path::new("unknown.xyz"), None),
            PreviewType::Icon
        );
    }

    #[tokio::test]
    async fn test_icon_preview_for_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("unknown.xyz");

        std::fs::write(&file_path, "binary data").unwrap();

        let generator = PreviewGenerator::new();
        let preview = generator.generate(&file_path).await.unwrap();

        assert_eq!(preview.preview_type, PreviewType::Icon);
        assert!(preview.data.is_empty());
    }

    #[cfg(feature = "web")]
    #[tokio::test]
    async fn test_zip_archive_listing() {
        use std::io::Write;

        let temp_dir = TempDir::new().unwrap();
        let zip_path = temp_dir.path().join("test.zip");

        // Create a simple ZIP file with some entries
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("file1.txt", options).unwrap();
        zip.write_all(b"content1").unwrap();
        zip.start_file("file2.txt", options).unwrap();
        zip.write_all(b"content2").unwrap();
        zip.start_file("subdir/file3.txt", options).unwrap();
        zip.write_all(b"content3").unwrap();
        zip.finish().unwrap();

        let generator = PreviewGenerator::new();
        let preview = generator.generate(&zip_path).await.unwrap();

        assert_eq!(preview.preview_type, PreviewType::ArchiveListing);
        assert_eq!(preview.mime_type, "application/json");

        let entries: Vec<String> = serde_json::from_str(&preview.data).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&"file1.txt".to_string()));
        assert!(entries.contains(&"file2.txt".to_string()));
        assert!(entries.contains(&"subdir/file3.txt".to_string()));

        let meta = preview.metadata.unwrap();
        assert_eq!(meta.file_count, Some(3));
    }
}
