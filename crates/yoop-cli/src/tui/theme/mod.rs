//! TUI theme system.
//!
//! Provides configurable color themes for the TUI interface.

use ratatui::style::Color;

mod dark;
mod light;

/// Theme configuration for the TUI.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name
    pub name: String,

    // primary
    /// Background color (Reset uses terminal default)
    pub background: Color,
    /// Main foreground/text color
    pub foreground: Color,
    /// Accent color for highlights and focus
    pub accent: Color,

    // semantic
    /// Success indicators
    pub success: Color,
    /// Warning indicators
    pub warning: Color,
    /// Error indicators
    pub error: Color,
    /// Information indicators
    pub info: Color,

    // UI element
    /// Border color (unfocused)
    pub border: Color,
    /// Border color (focused)
    pub border_focused: Color,
    /// Selection background
    pub selection: Color,
    /// Highlight background
    pub highlight: Color,

    // text
    /// Primary text color
    pub text_primary: Color,
    /// Secondary text color
    pub text_secondary: Color,
    /// Muted/disabled text color
    pub text_muted: Color,

    // component specific
    /// Progress bar fill color
    pub progress_bar: Color,
    /// Progress bar background color
    pub progress_bar_bg: Color,
}

impl Theme {
    /// Create the dark theme.
    pub fn dark() -> Self {
        dark::theme()
    }

    /// Create the light theme.
    pub fn light() -> Self {
        light::theme()
    }

    /// Create a theme by name.
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "light" => Self::light(),
            _ => Self::dark(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_name() {
        let dark = Theme::from_name("dark");
        assert_eq!(dark.name, "dark");

        let light = Theme::from_name("light");
        assert_eq!(light.name, "light");

        let unknown = Theme::from_name("unknown");
        assert_eq!(unknown.name, "dark");
    }

    #[test]
    fn test_default_is_dark() {
        let default = Theme::default();
        assert_eq!(default.name, "dark");
    }
}
