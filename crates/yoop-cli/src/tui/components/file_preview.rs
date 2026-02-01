//! File preview component for TUI.
//!
//! Displays a list of incoming files with metadata during receive.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Information about an incoming file.
#[derive(Debug, Clone)]
pub struct IncomingFile {
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// MIME type (if known)
    pub mime_type: Option<String>,
    /// Is this a directory
    pub is_directory: bool,
    /// Preview text (for text files)
    pub preview_text: Option<String>,
    /// Image dimensions (for images)
    pub dimensions: Option<(u32, u32)>,
    /// File count (for archives/directories)
    pub file_count: Option<usize>,
}

impl IncomingFile {
    /// Create from yoop_core FileMetadata.
    pub fn from_metadata(meta: &yoop_core::file::FileMetadata) -> Self {
        let preview_text = meta.preview.as_ref().and_then(|p| {
            if matches!(p.preview_type, yoop_core::preview::PreviewType::Text) {
                Some(p.data.clone())
            } else {
                None
            }
        });

        let dimensions = meta
            .preview
            .as_ref()
            .and_then(|p| p.metadata.as_ref().and_then(|m| m.dimensions));

        let file_count = meta
            .preview
            .as_ref()
            .and_then(|p| p.metadata.as_ref().and_then(|m| m.file_count));

        Self {
            name: meta.file_name().to_string(),
            size: meta.size,
            mime_type: meta.mime_type.clone(),
            is_directory: meta.is_directory,
            preview_text,
            dimensions,
            file_count,
        }
    }

    /// Get an icon for this file type.
    pub fn icon(&self) -> &'static str {
        if self.is_directory {
            "[dir]"
        } else if let Some(ref mime) = self.mime_type {
            if mime.starts_with("image/") {
                "[img]"
            } else if mime.starts_with("video/") {
                "[vid]"
            } else if mime.starts_with("audio/") {
                "[aud]"
            } else if mime.starts_with("text/") {
                "[txt]"
            } else if mime.contains("zip") || mime.contains("tar") || mime.contains("archive") {
                "[zip]"
            } else {
                "[file]"
            }
        } else {
            "[file]"
        }
    }

    /// Get additional info string.
    pub fn extra_info(&self) -> String {
        let mut parts = Vec::new();

        if let Some((w, h)) = self.dimensions {
            parts.push(format!("{}x{}", w, h));
        }

        if let Some(count) = self.file_count {
            parts.push(format!("{} files", count));
        }

        if let Some(ref preview) = self.preview_text {
            let snippet: String = preview
                .chars()
                .take(30)
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect();
            if !snippet.is_empty() {
                parts.push(format!("\"{}...\"", snippet.trim()));
            }
        }

        parts.join(" | ")
    }
}

/// File preview component for displaying incoming files.
pub struct FilePreview;

impl FilePreview {
    /// Render the file preview list.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        files: &[IncomingFile],
        sender_name: &str,
        theme: &Theme,
    ) {
        let total_size: u64 = files.iter().map(|f| f.size).sum();

        let block = Block::default()
            .title(format!(
                " Incoming: {} items ({}) from {} ",
                files.len(),
                format_size(total_size),
                sender_name
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.info));

        if files.is_empty() {
            let paragraph = Paragraph::new(Line::from(Span::styled(
                "No files in transfer",
                Style::default().fg(theme.text_muted),
            )))
            .block(block);

            frame.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = files
            .iter()
            .map(|file| {
                let icon = file.icon();
                let size_str = format_size(file.size);
                let extra = file.extra_info();

                let mut spans = vec![
                    Span::styled(icon, Style::default().fg(theme.accent)),
                    Span::raw(" "),
                    Span::styled(&file.name, Style::default().fg(theme.text_primary)),
                    Span::raw("  "),
                    Span::styled(size_str, Style::default().fg(theme.text_secondary)),
                ];

                if !extra.is_empty() {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(extra, Style::default().fg(theme.text_muted)));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).block(block);

        frame.render_widget(list, area);
    }

    /// Render a compact version showing only counts.
    pub fn render_compact(
        frame: &mut Frame,
        area: Rect,
        files: &[IncomingFile],
        sender_name: &str,
        theme: &Theme,
    ) {
        let total_size: u64 = files.iter().map(|f| f.size).sum();

        let block = Block::default()
            .title(" Incoming ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.info));

        let text = format!(
            "{} items ({}) from {}",
            files.len(),
            format_size(total_size),
            sender_name
        );

        let paragraph = Paragraph::new(Line::from(Span::styled(
            text,
            Style::default().fg(theme.text_primary),
        )))
        .block(block);

        frame.render_widget(paragraph, area);
    }

    /// Render the accept/decline prompt.
    pub fn render_accept_prompt(frame: &mut Frame, area: Rect, theme: &Theme) {
        let spans = vec![
            Span::styled(
                "[A]",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("ccept  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "[D]",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("ecline  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Cancel", Style::default().fg(theme.text_muted)),
        ];

        let paragraph =
            Paragraph::new(Line::from(spans)).alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(paragraph, area);
    }
}

/// Format file size for display.
#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
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
    fn test_incoming_file_icon() {
        let file = IncomingFile {
            name: "test.jpg".to_string(),
            size: 1024,
            mime_type: Some("image/jpeg".to_string()),
            is_directory: false,
            preview_text: None,
            dimensions: Some((800, 600)),
            file_count: None,
        };

        assert_eq!(file.icon(), "[img]");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }
}
