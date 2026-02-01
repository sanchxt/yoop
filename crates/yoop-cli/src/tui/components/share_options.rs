//! Share options component for TUI.
//!
//! Displays and manages share configuration options.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::{ShareOptionFocus, ShareOptions};
use crate::tui::theme::Theme;

/// Share options component.
pub struct ShareOptionsWidget;

impl ShareOptionsWidget {
    /// Render the share options widget.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        options: &ShareOptions,
        focus: Option<ShareOptionFocus>,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Options ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
            ])
            .split(inner);

        Self::render_expire_option(
            frame,
            chunks[0],
            &options.expire,
            focus == Some(ShareOptionFocus::Expire),
            theme,
        );
        Self::render_toggle_option(
            frame,
            chunks[1],
            "PIN (soon)",
            options.require_pin,
            focus == Some(ShareOptionFocus::Pin),
            theme,
        );
        Self::render_toggle_option(
            frame,
            chunks[2],
            "Approve (soon)",
            options.require_approval,
            focus == Some(ShareOptionFocus::Approval),
            theme,
        );
        Self::render_toggle_option(
            frame,
            chunks[3],
            "Compress",
            options.compress,
            focus == Some(ShareOptionFocus::Compress),
            theme,
        );
    }

    /// Render the expire option.
    fn render_expire_option(
        frame: &mut Frame,
        area: Rect,
        value: &str,
        focused: bool,
        theme: &Theme,
    ) {
        let label_style = if focused {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let value_style = if focused {
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().fg(theme.text_secondary)
        };

        let spans = vec![
            Span::styled("Expire: ", label_style),
            Span::styled(format!("[{}]", value), value_style),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    /// Render a toggle option (checkbox).
    fn render_toggle_option(
        frame: &mut Frame,
        area: Rect,
        label: &str,
        enabled: bool,
        focused: bool,
        theme: &Theme,
    ) {
        let checkbox = if enabled { "[x]" } else { "[ ]" };

        let label_style = if focused {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let checkbox_style = if enabled {
            Style::default().fg(theme.success)
        } else if focused {
            Style::default().fg(theme.text_primary)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let spans = vec![
            Span::styled(format!("{}: ", label), label_style),
            Span::styled(checkbox, checkbox_style),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    /// Render a compact version of options (single line).
    pub fn render_compact(frame: &mut Frame, area: Rect, options: &ShareOptions, theme: &Theme) {
        let expire = format!("[{}]", options.expire);
        let pin = if options.require_pin { "[x]" } else { "[ ]" };
        let approval = if options.require_approval {
            "[x]"
        } else {
            "[ ]"
        };
        let compress = if options.compress { "[x]" } else { "[ ]" };

        let spans = vec![
            Span::styled("Expire: ", Style::default().fg(theme.text_muted)),
            Span::styled(expire, Style::default().fg(theme.text_secondary)),
            Span::styled("  PIN*: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                pin,
                Style::default().fg(if options.require_pin {
                    theme.success
                } else {
                    theme.text_muted
                }),
            ),
            Span::styled("  Approve*: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                approval,
                Style::default().fg(if options.require_approval {
                    theme.success
                } else {
                    theme.text_muted
                }),
            ),
            Span::styled("  Compress: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                compress,
                Style::default().fg(if options.compress {
                    theme.success
                } else {
                    theme.text_muted
                }),
            ),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }
}

/// Available expiration options.
pub const EXPIRE_OPTIONS: &[&str] = &["1m", "5m", "10m", "30m", "1h", "2h", "12h", "24h"];

/// Get the next expiration option.
pub fn next_expire_option(current: &str) -> String {
    let idx = EXPIRE_OPTIONS
        .iter()
        .position(|&s| s == current)
        .unwrap_or(0);
    let next_idx = (idx + 1) % EXPIRE_OPTIONS.len();
    EXPIRE_OPTIONS[next_idx].to_string()
}

/// Get the previous expiration option.
pub fn prev_expire_option(current: &str) -> String {
    let idx = EXPIRE_OPTIONS
        .iter()
        .position(|&s| s == current)
        .unwrap_or(0);
    let prev_idx = if idx == 0 {
        EXPIRE_OPTIONS.len() - 1
    } else {
        idx - 1
    };
    EXPIRE_OPTIONS[prev_idx].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_focus_cycle() {
        let focus = ShareOptionFocus::Expire;
        assert_eq!(focus.next(), ShareOptionFocus::Compress);
        assert_eq!(focus.next().next(), ShareOptionFocus::Expire);
        assert_eq!(ShareOptionFocus::Compress.prev(), ShareOptionFocus::Expire);
        assert_eq!(ShareOptionFocus::Expire.prev(), ShareOptionFocus::Compress);
    }

    #[test]
    fn test_expire_options() {
        assert_eq!(next_expire_option("5m"), "10m");
        assert_eq!(next_expire_option("24h"), "1m");
        assert_eq!(prev_expire_option("5m"), "1m");
        assert_eq!(prev_expire_option("1m"), "24h");
    }
}
