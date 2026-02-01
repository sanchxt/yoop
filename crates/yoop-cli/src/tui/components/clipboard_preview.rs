//! Clipboard preview component for TUI.
//!
//! Displays a preview of the current clipboard content with type indicator.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::state::ClipboardContentType;
use crate::tui::theme::Theme;

/// Clipboard preview component.
pub struct ClipboardPreview;

impl ClipboardPreview {
    /// Render the clipboard preview.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        content_type: Option<ClipboardContentType>,
        preview: Option<&str>,
        size: Option<usize>,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Current Clipboard ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(3)])
            .split(inner);

        Self::render_type_info(frame, chunks[0], content_type, size, theme);

        Self::render_preview_content(frame, chunks[1], content_type, preview, theme);
    }

    /// Render the content type info line.
    fn render_type_info(
        frame: &mut Frame,
        area: Rect,
        content_type: Option<ClipboardContentType>,
        size: Option<usize>,
        theme: &Theme,
    ) {
        let (type_icon, type_text, type_color) = match content_type {
            Some(ClipboardContentType::Text) => ("T", "Text", theme.info),
            Some(ClipboardContentType::Image) => ("I", "Image", theme.warning),
            None => ("?", "Empty/Unknown", theme.text_muted),
        };

        let size_text = size.map_or_else(|| "Unknown size".to_string(), format_size);

        let spans = vec![
            Span::styled(
                format!("[{}]", type_icon),
                Style::default().fg(type_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Type: ", Style::default().fg(theme.text_muted)),
            Span::styled(type_text, Style::default().fg(theme.text_primary)),
            Span::styled("  |  ", Style::default().fg(theme.border)),
            Span::styled("Size: ", Style::default().fg(theme.text_muted)),
            Span::styled(size_text, Style::default().fg(theme.text_primary)),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    /// Render the preview content.
    fn render_preview_content(
        frame: &mut Frame,
        area: Rect,
        content_type: Option<ClipboardContentType>,
        preview: Option<&str>,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Preview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        match (content_type, preview) {
            (Some(ClipboardContentType::Text), Some(text)) => {
                let display_text = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text.to_string()
                };

                let paragraph = Paragraph::new(display_text)
                    .block(block)
                    .style(Style::default().fg(theme.text_primary))
                    .wrap(Wrap { trim: true });

                frame.render_widget(paragraph, area);
            }
            (Some(ClipboardContentType::Image), _) => {
                let content = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Image Content]",
                        Style::default()
                            .fg(theme.warning)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Image data is available in clipboard",
                        Style::default().fg(theme.text_muted),
                    )),
                ];

                let paragraph = Paragraph::new(content)
                    .block(block)
                    .alignment(Alignment::Left);

                frame.render_widget(paragraph, area);
            }
            _ => {
                let content = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Clipboard is empty",
                        Style::default().fg(theme.text_muted),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Copy something to share",
                        Style::default().fg(theme.text_muted),
                    )),
                ];

                let paragraph = Paragraph::new(content)
                    .block(block)
                    .alignment(Alignment::Center);

                frame.render_widget(paragraph, area);
            }
        }
    }

    /// Render a compact clipboard status for the status bar.
    pub fn render_compact_status(
        frame: &mut Frame,
        area: Rect,
        content_type: Option<ClipboardContentType>,
        size: Option<usize>,
        theme: &Theme,
    ) {
        let (icon, type_str) = match content_type {
            Some(ClipboardContentType::Text) => ("T", "Text"),
            Some(ClipboardContentType::Image) => ("I", "Image"),
            None => ("-", "Empty"),
        };

        let size_str = size.map_or_else(|| "-".to_string(), format_size);

        let text = format!("[{}] {} ({})", icon, type_str, size_str);
        let paragraph = Paragraph::new(text).style(Style::default().fg(theme.text_secondary));

        frame.render_widget(paragraph, area);
    }
}

/// Format size in human-readable form.
#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1536), "1.5 KB");
    }
}
