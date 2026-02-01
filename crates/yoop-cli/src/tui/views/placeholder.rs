//! Placeholder view for unimplemented views.
//!
//! This module provides a simple placeholder that shows the view name
//! and a "coming soon" message. It will be replaced by actual view
//! implementations in later phases.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Render a placeholder view.
pub fn render(frame: &mut Frame, area: Rect, title: &str, description: &str) {
    let theme = Theme::default();

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let muted = Style::default().fg(theme.text_muted);
    let accent_bold = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            description,
            Style::default().fg(theme.text_secondary),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "This view will be implemented in a future phase.",
            muted.add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        create_shortcut_line(
            &["[S] Share", "[R] Receive", "[C] Clipboard"],
            muted,
            accent_bold,
        ),
        create_shortcut_line(
            &["[Y] Sync", "[D] Devices", "[H] History"],
            muted,
            accent_bold,
        ),
        Line::from(""),
        create_shortcut_line(&["[?] Help", "[Q] Quit"], muted, accent_bold),
    ];

    let paragraph = Paragraph::new(content).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}

/// Create a line with keyboard shortcuts.
fn create_shortcut_line(shortcuts: &[&str], muted: Style, accent: Style) -> Line<'static> {
    let mut spans = Vec::new();
    spans.push(Span::styled("Press ", muted));

    for (i, shortcut) in shortcuts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", muted));
        }
        if let Some(bracket_end) = shortcut.find(']') {
            let key = &shortcut[..=bracket_end];
            let label = &shortcut[bracket_end + 1..];
            spans.push(Span::styled(key.to_string(), accent));
            spans.push(Span::styled(label.to_string(), muted));
        } else {
            spans.push(Span::styled(shortcut.to_string(), muted));
        }
    }

    Line::from(spans)
}
