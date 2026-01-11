//! QR code generation for Yoop share codes.
//!
//! This module generates QR codes containing deep links for mobile scanning.
//!
//! ## Features
//!
//! - ASCII art QR for terminal display
//! - SVG QR for web interface
//! - Configurable URL scheme for deep links
//!
//! ## Example
//!
//! ```rust,ignore
//! use yoop_core::qr;
//!
//! let ascii = qr::generate_ascii("A7K9")?;
//! println!("{}", ascii);
//!
//! let svg = qr::generate_svg("A7K9")?;
//! ```

use base64::Engine;
use qrcode::render::{svg, unicode};
use qrcode::{EcLevel, QrCode};

use crate::error::{Error, Result};

/// Configuration for QR code generation.
#[derive(Debug, Clone)]
pub struct QrConfig {
    /// URL scheme for deep links (default: "yoop")
    pub scheme: String,
    /// Error correction level (default: Medium)
    pub error_correction: EcLevel,
}

impl Default for QrConfig {
    fn default() -> Self {
        Self {
            scheme: "yoop".to_string(),
            error_correction: EcLevel::M,
        }
    }
}

/// Create a deep link URL from a share code.
///
/// # Arguments
///
/// * `code` - The share code (e.g., "A7K9")
/// * `config` - QR configuration
///
/// # Returns
///
/// A deep link URL (e.g., "yoop://A7K9")
///
/// # Example
///
/// ```
/// use yoop_core::qr::{create_deep_link, QrConfig};
///
/// let link = create_deep_link("A7K9", &QrConfig::default());
/// assert_eq!(link, "yoop://A7K9");
/// ```
#[must_use]
pub fn create_deep_link(code: &str, config: &QrConfig) -> String {
    format!("{}://{}", config.scheme, code.to_uppercase())
}

/// Generate ASCII art QR code for terminal display.
///
/// Uses Unicode block characters for compact display in terminals.
///
/// # Arguments
///
/// * `code` - The share code (e.g., "A7K9")
///
/// # Errors
///
/// Returns an error if QR code generation fails.
///
/// # Example
///
/// ```
/// use yoop_core::qr::generate_ascii;
///
/// let qr = generate_ascii("A7K9").unwrap();
/// println!("{}", qr);
/// ```
pub fn generate_ascii(code: &str) -> Result<String> {
    let deep_link = create_deep_link(code, &QrConfig::default());

    let qr_code = QrCode::with_error_correction_level(&deep_link, EcLevel::M)
        .map_err(|e| Error::Internal(format!("Failed to generate QR code: {e}")))?;

    let rendered = qr_code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();

    Ok(rendered)
}

/// Generate SVG QR code for web interface.
///
/// Returns an SVG string that can be embedded in HTML.
///
/// # Arguments
///
/// * `code` - The share code (e.g., "A7K9")
///
/// # Errors
///
/// Returns an error if QR code generation fails.
///
/// # Example
///
/// ```
/// use yoop_core::qr::generate_svg;
///
/// let svg = generate_svg("A7K9").unwrap();
/// assert!(svg.contains("<svg"));
/// assert!(svg.contains("</svg>"));
/// ```
pub fn generate_svg(code: &str) -> Result<String> {
    let deep_link = create_deep_link(code, &QrConfig::default());

    let qr_code = QrCode::with_error_correction_level(&deep_link, EcLevel::M)
        .map_err(|e| Error::Internal(format!("Failed to generate QR code: {e}")))?;

    let svg_string = qr_code
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    Ok(svg_string)
}

/// Generate base64-encoded PNG QR code.
///
/// Useful for embedding in web pages as data URLs.
///
/// # Arguments
///
/// * `code` - The share code (e.g., "A7K9")
/// * `size` - Size of the QR code in pixels
///
/// # Errors
///
/// Returns an error if QR code generation or encoding fails.
///
/// # Example
///
/// ```
/// use yoop_core::qr::generate_png_base64;
///
/// let png_data = generate_png_base64("A7K9", 256).unwrap();
/// assert!(!png_data.is_empty());
/// ```
pub fn generate_png_base64(code: &str, size: u32) -> Result<String> {
    use image::Luma;

    let deep_link = create_deep_link(code, &QrConfig::default());

    let qr_code = QrCode::with_error_correction_level(&deep_link, EcLevel::M)
        .map_err(|e| Error::Internal(format!("Failed to generate QR code: {e}")))?;

    let image = qr_code.render::<Luma<u8>>().build();

    let scaled = image::imageops::resize(&image, size, size, image::imageops::FilterType::Nearest);

    let mut png_bytes = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(&scaled, size, size, image::ExtendedColorType::L8)
            .map_err(|e| Error::Internal(format!("Failed to encode PNG: {e}")))?;
    }

    Ok(base64::prelude::BASE64_STANDARD.encode(&png_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_deep_link_default() {
        let link = create_deep_link("A7K9", &QrConfig::default());
        assert_eq!(link, "yoop://A7K9");
    }

    #[test]
    fn test_create_deep_link_uppercase() {
        let link = create_deep_link("a7k9", &QrConfig::default());
        assert_eq!(link, "yoop://A7K9");
    }

    #[test]
    fn test_create_deep_link_custom_scheme() {
        let config = QrConfig {
            scheme: "localdrop".to_string(),
            ..Default::default()
        };
        let link = create_deep_link("A7K9", &config);
        assert_eq!(link, "localdrop://A7K9");
    }

    #[test]
    fn test_generate_ascii_not_empty() {
        let qr = generate_ascii("A7K9").unwrap();
        assert!(!qr.is_empty());
        assert!(qr.contains('█') || qr.contains('▀') || qr.contains('▄'));
    }

    #[test]
    fn test_generate_ascii_multiline() {
        let qr = generate_ascii("A7K9").unwrap();
        assert!(qr.lines().count() > 5);
    }

    #[test]
    fn test_generate_svg_valid_xml() {
        let svg = generate_svg("A7K9").unwrap();
        assert!(
            svg.starts_with("<?xml") || svg.starts_with("<svg"),
            "SVG should start with XML declaration or svg tag"
        );
        assert!(svg.contains("</svg>"), "SVG should have closing tag");
        assert!(
            svg.contains("xmlns") || svg.contains("viewBox"),
            "SVG should have xmlns or viewBox attribute"
        );
    }

    #[test]
    fn test_generate_svg_has_dimensions() {
        let svg = generate_svg("A7K9").unwrap();
        assert!(svg.contains("width") && svg.contains("height"));
    }

    #[test]
    fn test_qr_code_generation_succeeds() {
        let result = QrCode::new("yoop://A7K9");
        assert!(result.is_ok());

        let qr = result.unwrap();
        assert!(matches!(qr.version(), qrcode::Version::Normal(_)));
    }

    #[test]
    fn test_generate_png_base64_not_empty() {
        let png = generate_png_base64("A7K9", 256).unwrap();
        assert!(!png.is_empty());

        let decoded = base64::prelude::BASE64_STANDARD.decode(&png).unwrap();
        assert!(decoded.len() > 100);
    }

    #[test]
    fn test_different_codes_produce_different_qrs() {
        let qr1 = generate_ascii("A7K9").unwrap();
        let qr2 = generate_ascii("B8M3").unwrap();
        assert_ne!(qr1, qr2);
    }

    #[test]
    fn test_config_error_correction_levels() {
        let config_low = QrConfig {
            scheme: "yoop".to_string(),
            error_correction: EcLevel::L,
        };
        let config_high = QrConfig {
            scheme: "yoop".to_string(),
            error_correction: EcLevel::H,
        };

        assert_eq!(config_low.error_correction, EcLevel::L);
        assert_eq!(config_high.error_correction, EcLevel::H);
    }
}
