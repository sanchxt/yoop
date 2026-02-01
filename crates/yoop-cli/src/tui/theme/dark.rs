//! Dark theme definition.

use ratatui::style::Color;

use super::Theme;

/// Create the dark theme.
pub fn theme() -> Theme {
    Theme {
        name: "dark".to_string(),

        background: Color::Reset,
        foreground: Color::White,
        accent: Color::Cyan,

        success: Color::Green,
        warning: Color::Yellow,
        error: Color::Red,
        info: Color::Blue,

        border: Color::DarkGray,
        border_focused: Color::Cyan,
        selection: Color::Rgb(50, 50, 80),
        highlight: Color::Rgb(70, 70, 100),

        text_primary: Color::White,
        text_secondary: Color::Gray,
        text_muted: Color::DarkGray,

        progress_bar: Color::Cyan,
        progress_bar_bg: Color::DarkGray,
    }
}
