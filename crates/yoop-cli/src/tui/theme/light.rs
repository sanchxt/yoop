//! Light theme definition.

use ratatui::style::Color;

use super::Theme;

/// Create the light theme.
pub fn theme() -> Theme {
    Theme {
        name: "light".to_string(),

        background: Color::Reset,
        foreground: Color::Black,
        accent: Color::Blue,

        success: Color::Rgb(0, 128, 0),
        warning: Color::Rgb(200, 150, 0),
        error: Color::Rgb(200, 0, 0),
        info: Color::Rgb(0, 100, 200),

        border: Color::Gray,
        border_focused: Color::Blue,
        selection: Color::Rgb(200, 200, 230),
        highlight: Color::Rgb(220, 220, 250),

        text_primary: Color::Black,
        text_secondary: Color::DarkGray,
        text_muted: Color::Gray,

        progress_bar: Color::Blue,
        progress_bar_bg: Color::Rgb(200, 200, 200),
    }
}
