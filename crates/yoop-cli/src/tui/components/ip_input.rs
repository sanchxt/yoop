//! IP input component for TUI.
//!
//! Provides a segmented input field for direct IPv4 address connections
//! with 4 octet boxes and an optional port field.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::state::IpInputState;
use crate::tui::theme::Theme;

/// IP input component for direct peer connections.
/// Renders a segmented IPv4 input with 4 octet boxes + port box.
pub struct IpInput;

impl IpInput {
    /// Render the segmented IP input field.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        ip_input: &IpInputState,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Direct IP Connection ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 2 {
            return;
        }

        let segments_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let hint_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);

        Self::render_segments(frame, segments_area, ip_input, focused, theme);
        Self::render_hint(frame, hint_area, ip_input, focused, theme);
    }

    /// Render the IP address segments (4 octets + port).
    fn render_segments(
        frame: &mut Frame,
        area: Rect,
        ip_input: &IpInputState,
        focused: bool,
        theme: &Theme,
    ) {
        let total_width = area.width;
        let octet_width = 5_u16;
        let dot_width = 1_u16;
        let colon_width = 1_u16;
        let port_width = 7_u16;

        let content_width = (octet_width * 4) + (dot_width * 3) + colon_width + port_width;

        let start_x = area.x + (total_width.saturating_sub(content_width)) / 2;

        let mut x = start_x;

        for i in 0..4 {
            let is_current = focused && ip_input.cursor_position == i;
            let octet_area = Rect::new(x, area.y, octet_width, 1);
            Self::render_octet_box(frame, octet_area, &ip_input.octets[i], is_current, theme);
            x += octet_width;

            if i < 3 {
                let dot_area = Rect::new(x, area.y, dot_width, 1);
                Self::render_separator(frame, dot_area, ".", theme);
                x += dot_width;
            }
        }

        let colon_area = Rect::new(x, area.y, colon_width, 1);
        Self::render_separator(frame, colon_area, ":", theme);
        x += colon_width;

        let is_port_current = focused && ip_input.cursor_position == 4;
        let port_area = Rect::new(x, area.y, port_width, 1);
        Self::render_port_box(frame, port_area, &ip_input.port, is_port_current, theme);
    }

    /// Render a single octet input box.
    fn render_octet_box(
        frame: &mut Frame,
        area: Rect,
        value: &str,
        is_current: bool,
        theme: &Theme,
    ) {
        let is_valid = IpInputState::is_valid_octet(value);

        let style = Self::segment_style(value, is_valid, is_current, theme);

        let display = if is_current {
            if value.is_empty() {
                "_".to_string()
            } else {
                format!("{}_", value)
            }
        } else if value.is_empty() {
            "___".to_string()
        } else {
            format!("{:>3}", value)
        };

        let paragraph = Paragraph::new(Span::styled(display, style)).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    /// Render the port input box.
    fn render_port_box(
        frame: &mut Frame,
        area: Rect,
        value: &str,
        is_current: bool,
        theme: &Theme,
    ) {
        let is_valid = IpInputState::is_valid_port(value);

        let style = Self::segment_style(value, is_valid, is_current, theme);

        let display = if is_current {
            if value.is_empty() {
                "_".to_string()
            } else {
                format!("{}_", value)
            }
        } else if value.is_empty() {
            "_____".to_string()
        } else {
            format!("{:>5}", value)
        };

        let paragraph = Paragraph::new(Span::styled(display, style)).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    /// Get the style for a segment based on its state.
    fn segment_style(value: &str, is_valid: bool, is_current: bool, theme: &Theme) -> Style {
        if value.is_empty() {
            if is_current {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_muted)
            }
        } else if is_valid {
            if is_current {
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.success)
            }
        } else if is_current {
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.warning)
        }
    }

    /// Render a separator (dot or colon).
    fn render_separator(frame: &mut Frame, area: Rect, sep: &str, theme: &Theme) {
        let paragraph = Paragraph::new(Span::styled(
            sep,
            Style::default()
                .fg(theme.text_muted)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    /// Render the input hint below the segments.
    fn render_hint(
        frame: &mut Frame,
        area: Rect,
        ip_input: &IpInputState,
        focused: bool,
        theme: &Theme,
    ) {
        let hint = if !focused {
            "Tab to focus, then enter IP address"
        } else if ip_input.is_empty() {
            "Enter IPv4: digits, . or : to advance, Tab to port"
        } else if ip_input.is_complete() && ip_input.is_valid() {
            "Press Enter to connect"
        } else {
            "Tab/→ next segment, Backspace to go back"
        };

        let style = Style::default().fg(theme.text_muted);
        let paragraph = Paragraph::new(Span::styled(hint, style)).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    /// Render a compact version of the IP input.
    pub fn render_compact(
        frame: &mut Frame,
        area: Rect,
        ip_input: &IpInputState,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" IP ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let display = if ip_input.is_empty() {
            if focused {
                "_._._._:_".to_string()
            } else {
                "IP:PORT".to_string()
            }
        } else {
            let addr = ip_input.to_address_string();
            if focused {
                format!("{}_", addr)
            } else {
                addr
            }
        };

        let style = if ip_input.is_empty() {
            Style::default().fg(theme.text_muted)
        } else if ip_input.is_complete() && ip_input.is_valid() {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.warning)
        };

        let paragraph =
            Paragraph::new(Line::from(Span::styled(display, style))).alignment(Alignment::Center);

        frame.render_widget(paragraph, inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_style_empty_focused() {
        let theme = Theme::default();
        let style = IpInput::segment_style("", true, true, &theme);
        assert_eq!(style.fg, Some(theme.accent));
    }

    #[test]
    fn test_segment_style_empty_not_focused() {
        let theme = Theme::default();
        let style = IpInput::segment_style("", true, false, &theme);
        assert_eq!(style.fg, Some(theme.text_muted));
    }

    #[test]
    fn test_segment_style_valid_value() {
        let theme = Theme::default();
        let style = IpInput::segment_style("192", true, false, &theme);
        assert_eq!(style.fg, Some(theme.success));
    }

    #[test]
    fn test_segment_style_invalid_value() {
        let theme = Theme::default();
        let style = IpInput::segment_style("999", false, false, &theme);
        assert_eq!(style.fg, Some(theme.warning));
    }
}
