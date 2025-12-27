//! Share code generation and validation.
//!
//! This module handles the generation and validation of 4-character share codes
//! used for device discovery.
//!
//! ## Code Format
//!
//! Codes use a 32-character alphabet that excludes ambiguous characters:
//! - Valid characters: `2-9`, `A-H`, `J-K`, `M`, `N`, `P-Z`
//! - Excluded: `0`, `1`, `I`, `L`, `O` (easily confused)
//!
//! This gives 32^4 = 1,048,576 unique codes.
//!
//! ## Example
//!
//! ```rust,ignore
//! use yoop_core::code::{ShareCode, CodeGenerator};
//!
//! let generator = CodeGenerator::new();
//! let code = generator.generate()?;
//! println!("Generated code: {}", code);
//!
//! let code = ShareCode::parse("A7K9")?;
//! ```

use crate::error::{Error, Result};

/// The character set used for code generation.
/// Excludes ambiguous characters: 0, 1, I, L, O
pub const CODE_CHARSET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// Length of a share code
pub const CODE_LENGTH: usize = 4;

/// A validated share code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShareCode {
    code: String,
}

impl ShareCode {
    /// Parse and validate a share code from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the code is invalid (wrong length or invalid characters).
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = input.trim().to_uppercase();

        if normalized.len() != CODE_LENGTH {
            return Err(Error::InvalidCodeFormat(format!(
                "code must be {} characters, got {}",
                CODE_LENGTH,
                normalized.len()
            )));
        }

        for c in normalized.chars() {
            if !CODE_CHARSET.contains(&(c as u8)) {
                return Err(Error::InvalidCodeFormat(format!(
                    "invalid character '{c}' in code"
                )));
            }
        }

        Ok(Self { code: normalized })
    }

    /// Returns the code as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.code
    }
}

impl std::fmt::Display for ShareCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}

/// Generator for share codes.
#[derive(Debug, Default)]
pub struct CodeGenerator {
    // TODO: track recently generated codes to avoid collisions
}

impl CodeGenerator {
    /// Create a new code generator.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// Generate a new random share code.
    ///
    /// # Errors
    ///
    /// Returns an error if code generation fails.
    pub fn generate(&self) -> Result<ShareCode> {
        use rand::Rng;

        let mut rng = rand::thread_rng();
        let code: String = (0..CODE_LENGTH)
            .map(|_| {
                let idx = rng.gen_range(0..CODE_CHARSET.len());
                CODE_CHARSET[idx] as char
            })
            .collect();

        ShareCode::parse(&code)
    }
}
