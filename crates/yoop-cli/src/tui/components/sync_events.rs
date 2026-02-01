//! Sync events component for TUI.
//!
//! Displays real-time sync events in a scrollable list.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// A sync event for display.
#[derive(Debug, Clone)]
pub struct SyncEventDisplay {
    /// Timestamp of the event
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event type
    pub event_type: SyncEventType,
    /// Associated file path (if any)
    pub path: Option<String>,
    /// Additional message
    pub message: Option<String>,
}

/// Type of sync event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncEventType {
    /// Connected to peer
    Connected,
    /// Index exchanged
    IndexExchanged,
    /// File being sent
    FileSending,
    /// File sent successfully
    FileSent,
    /// File being received
    FileReceiving,
    /// File received successfully
    FileReceived,
    /// File deleted
    FileDeleted,
    /// Conflict detected
    Conflict,
    /// Error occurred
    Error,
}

impl SyncEventType {
    /// Get the icon for this event type.
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Connected => "✓",
            Self::IndexExchanged => "↔",
            Self::FileSending => "→",
            Self::FileSent => "✓",
            Self::FileReceiving => "←",
            Self::FileReceived => "✓",
            Self::FileDeleted => "🗑",
            Self::Conflict => "⚠",
            Self::Error => "✗",
        }
    }

    /// Get the label for this event type.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::IndexExchanged => "Index",
            Self::FileSending => "Sending",
            Self::FileSent => "Sent",
            Self::FileReceiving => "Receiving",
            Self::FileReceived => "Received",
            Self::FileDeleted => "Deleted",
            Self::Conflict => "Conflict",
            Self::Error => "Error",
        }
    }
}

/// Sync events list component.
pub struct SyncEventsList {
    /// List state for scrolling
    list_state: ListState,
    /// Auto-scroll to bottom
    auto_scroll: bool,
}

impl SyncEventsList {
    /// Create a new sync events list.
    pub fn new() -> Self {
        Self {
            list_state: ListState::default(),
            auto_scroll: true,
        }
    }

    /// Render the sync events list.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        events: &[SyncEventDisplay],
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(format!(" Sync Events ({}) ", events.len()))
            .borders(Borders::ALL)
            .border_style(border_style);

        if events.is_empty() {
            let paragraph = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                "No sync events yet",
                Style::default().fg(theme.text_muted),
            )))
            .block(block);

            frame.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = events
            .iter()
            .map(|event| {
                let icon_color = match event.event_type {
                    SyncEventType::Connected
                    | SyncEventType::FileSent
                    | SyncEventType::FileReceived => theme.success,
                    SyncEventType::FileSending | SyncEventType::FileReceiving => theme.info,
                    SyncEventType::IndexExchanged => theme.accent,
                    SyncEventType::FileDeleted => theme.warning,
                    SyncEventType::Conflict => theme.warning,
                    SyncEventType::Error => theme.error,
                };

                let time = event.timestamp.format("%H:%M:%S");
                let icon = event.event_type.icon();

                let mut spans = vec![
                    Span::styled(format!("{} ", time), Style::default().fg(theme.text_muted)),
                    Span::styled(
                        format!("{} ", icon),
                        Style::default().fg(icon_color).add_modifier(Modifier::BOLD),
                    ),
                ];

                if let Some(ref path) = event.path {
                    let short_path = shorten_path(path, 30);
                    spans.push(Span::styled(
                        short_path,
                        Style::default().fg(theme.text_primary),
                    ));
                }

                if let Some(ref msg) = event.message {
                    if event.path.is_some() {
                        spans.push(Span::styled(" - ", Style::default().fg(theme.text_muted)));
                    }
                    spans.push(Span::styled(
                        msg.as_str(),
                        Style::default().fg(theme.text_secondary),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        if self.auto_scroll && !events.is_empty() {
            self.list_state.select(Some(events.len() - 1));
        }

        let list = List::new(items).block(block).highlight_style(
            Style::default()
                .bg(theme.selection)
                .add_modifier(Modifier::BOLD),
        );

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Scroll up in the list.
    pub fn scroll_up(&mut self, events_count: usize) {
        self.auto_scroll = false;
        if events_count == 0 {
            return;
        }

        let current = self.list_state.selected().unwrap_or(0);
        if current > 0 {
            self.list_state.select(Some(current - 1));
        }
    }

    /// Scroll down in the list.
    pub fn scroll_down(&mut self, events_count: usize) {
        if events_count == 0 {
            return;
        }

        let current = self.list_state.selected().unwrap_or(0);
        if current < events_count - 1 {
            self.list_state.select(Some(current + 1));
        }

        if current + 1 >= events_count - 1 {
            self.auto_scroll = true;
        }
    }

    /// Jump to the bottom and enable auto-scroll.
    pub fn scroll_to_bottom(&mut self, events_count: usize) {
        self.auto_scroll = true;
        if events_count > 0 {
            self.list_state.select(Some(events_count - 1));
        }
    }

    /// Enable or disable auto-scroll.
    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.auto_scroll = enabled;
    }

    /// Check if auto-scroll is enabled.
    #[cfg(test)]
    pub fn is_auto_scroll(&self) -> bool {
        self.auto_scroll
    }
}

impl Default for SyncEventsList {
    fn default() -> Self {
        Self::new()
    }
}

/// Sync stats display component.
pub struct SyncStatsDisplay;

impl SyncStatsDisplay {
    /// Render sync stats.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        files_sent: u64,
        files_received: u64,
        bytes_sent: u64,
        bytes_received: u64,
        conflicts: u64,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Stats ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let content = vec![
            Line::from(vec![
                Span::styled("↑ Sent: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    format!("{} files ({})", files_sent, format_size(bytes_sent)),
                    Style::default().fg(theme.success),
                ),
            ]),
            Line::from(vec![
                Span::styled("↓ Received: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    format!("{} files ({})", files_received, format_size(bytes_received)),
                    Style::default().fg(theme.info),
                ),
            ]),
            if conflicts > 0 {
                Line::from(vec![
                    Span::styled("⚠ Conflicts: ", Style::default().fg(theme.text_muted)),
                    Span::styled(conflicts.to_string(), Style::default().fg(theme.warning)),
                ])
            } else {
                Line::from("")
            },
        ];

        let paragraph = ratatui::widgets::Paragraph::new(content).block(block);
        frame.render_widget(paragraph, area);
    }
}

/// Format size in human-readable form.
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

/// Shorten a path for display.
fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }

    if let Some(filename) = std::path::Path::new(path).file_name() {
        let name = filename.to_string_lossy();
        if name.len() <= max_len {
            return name.to_string();
        }
        return format!("{}...", &name[..max_len.saturating_sub(3)]);
    }

    format!("...{}", &path[path.len() - max_len + 3..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_shorten_path() {
        assert_eq!(shorten_path("short.txt", 20), "short.txt");
        assert_eq!(shorten_path("/very/long/path/to/file.txt", 15), "file.txt");
        assert_eq!(shorten_path("verylongfilename.txt", 10), "verylon...");
    }
}
