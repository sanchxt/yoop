//! History view for TUI.
//!
//! Provides the interface for viewing transfer history.

use std::path::PathBuf;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::state::{AppState, HistoryFocus};
use crate::tui::theme::Theme;

/// Transfer history entry for display.
#[derive(Debug, Clone)]
pub struct TuiHistoryEntry {
    /// Unique identifier
    pub id: uuid::Uuid,
    /// Unix timestamp
    pub timestamp: u64,
    /// Formatted timestamp string
    pub timestamp_str: String,
    /// Direction (Sent/Received)
    pub direction: String,
    /// Is this a send (vs receive)
    pub is_sent: bool,
    /// Device name
    pub device_name: String,
    /// Number of files
    pub file_count: usize,
    /// File names
    pub file_names: Vec<String>,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Formatted size string
    pub size_str: String,
    /// Transfer state (Completed/Failed/Cancelled)
    pub state: String,
    /// Is this a completed transfer
    pub is_completed: bool,
    /// Is this a failed transfer
    pub is_failed: bool,
    /// Duration in seconds
    pub duration_secs: u64,
    /// Speed in bytes per second
    pub speed_bps: Option<u64>,
    /// Output directory (for received files)
    pub output_dir: Option<PathBuf>,
    /// Error message (if failed)
    pub error_message: Option<String>,
    /// Share code used
    pub share_code: String,
}

impl TuiHistoryEntry {
    /// Format bytes as human-readable string.
    #[allow(clippy::cast_precision_loss)]
    fn format_bytes(bytes: u64) -> String {
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

    /// Format speed as human-readable string.
    pub fn speed_str(&self) -> String {
        self.speed_bps.map_or_else(
            || "-".to_string(),
            |bps| format!("{}/s", Self::format_bytes(bps)),
        )
    }

    /// Format duration as human-readable string.
    pub fn duration_str(&self) -> String {
        let secs = self.duration_secs;
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            let mins = secs / 60;
            let remaining_secs = secs % 60;
            format!("{}m {}s", mins, remaining_secs)
        } else {
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            format!("{}h {}m", hours, mins)
        }
    }
}

/// History view component.
pub struct HistoryView {
    /// List state for history selection
    list_state: ListState,
    /// Cached list of history entries
    pub entries: Vec<TuiHistoryEntry>,
}

impl HistoryView {
    /// Create a new history view.
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            entries: Vec::new(),
        }
    }

    /// Render the history view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        if self.entries.is_empty() {
            self.render_empty_state(frame, area, theme);
            return;
        }

        if state.history.confirm_clear {
            self.render_clear_confirmation(frame, area, theme);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        self.render_history_list(frame, chunks[0], state, theme);

        self.render_entry_details(frame, chunks[1], state, theme);
    }

    /// Render empty state when no history exists.
    #[allow(clippy::unused_self)]
    fn render_empty_state(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .title(" Transfer History ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "No transfer history",
                Style::default()
                    .fg(theme.text_muted)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Completed transfers will appear here.",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [S] for Share or [R] for Receive to start.",
                Style::default().fg(theme.text_secondary),
            )),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Render the history list.
    fn render_history_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let focused = state.history.focus == HistoryFocus::HistoryList;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" History ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let mut items: Vec<ListItem> = Vec::new();
        let mut current_date: Option<String> = None;

        for entry in &self.entries {
            let entry_date = Self::timestamp_to_date(entry.timestamp);

            if current_date.as_ref() != Some(&entry_date) {
                if current_date.is_some() {
                    items.push(ListItem::new(Line::from("")));
                }
                items.push(ListItem::new(Line::from(Span::styled(
                    entry_date.clone(),
                    Style::default()
                        .fg(theme.text_secondary)
                        .add_modifier(Modifier::BOLD),
                ))));
                current_date = Some(entry_date);
            }

            let direction_icon = if entry.is_sent { "↑" } else { "↓" };
            let status_icon = if entry.is_completed {
                "✓"
            } else if entry.is_failed {
                "✗"
            } else {
                "○"
            };

            let status_color = if entry.is_completed {
                theme.success
            } else if entry.is_failed {
                theme.error
            } else {
                theme.warning
            };

            let text = format!(
                "  {} {} {}  {}  {}",
                entry.timestamp_str.split(' ').next_back().unwrap_or(""),
                direction_icon,
                entry.device_name,
                entry.size_str,
                status_icon
            );

            items.push(ListItem::new(Line::from(vec![
                Span::styled(text, Style::default().fg(theme.text_primary)),
                Span::styled(
                    format!(" {}", entry.state),
                    Style::default().fg(status_color),
                ),
            ])));
        }

        self.list_state.select(Some(state.history.selected_index));

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(theme.selection)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Render entry details panel.
    fn render_entry_details(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let focused = state.history.focus == HistoryFocus::Details;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Details ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let entry = self.get_selected_entry(state);

        if let Some(entry) = entry {
            self.render_entry_info(frame, inner, entry, theme);
        } else {
            let text = Paragraph::new(Span::styled(
                "No entry selected",
                Style::default().fg(theme.text_muted),
            ))
            .alignment(Alignment::Center);
            frame.render_widget(text, inner);
        }
    }

    /// Render detailed info for a history entry.
    #[allow(clippy::unused_self, clippy::too_many_lines)]
    fn render_entry_info(
        &self,
        frame: &mut Frame,
        area: Rect,
        entry: &TuiHistoryEntry,
        theme: &Theme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(4),
            ])
            .split(area);

        let (dir_text, dir_color) = if entry.is_sent {
            ("Sent", theme.info)
        } else {
            ("Received", theme.accent)
        };
        let (status_text, status_color) = if entry.is_completed {
            ("Completed", theme.success)
        } else if entry.is_failed {
            ("Failed", theme.error)
        } else {
            ("Cancelled", theme.warning)
        };

        let direction_status = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(dir_text, Style::default().fg(dir_color)),
                Span::styled(" • ", Style::default().fg(theme.text_muted)),
                Span::styled(status_text, Style::default().fg(status_color)),
            ]),
            Line::from(Span::styled(
                &entry.timestamp_str,
                Style::default().fg(theme.text_secondary),
            )),
        ]);
        frame.render_widget(direction_status, chunks[0]);

        let device = Paragraph::new(vec![
            Line::from(Span::styled(
                "Device",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(
                &entry.device_name,
                Style::default().fg(theme.text_primary),
            )),
        ]);
        frame.render_widget(device, chunks[1]);

        let code = Paragraph::new(vec![
            Line::from(Span::styled("Code", Style::default().fg(theme.text_muted))),
            Line::from(Span::styled(
                &entry.share_code,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
        ]);
        frame.render_widget(code, chunks[2]);

        let size_duration = Paragraph::new(vec![Line::from(vec![
            Span::styled("Size: ", Style::default().fg(theme.text_muted)),
            Span::styled(&entry.size_str, Style::default().fg(theme.text_primary)),
            Span::styled("  Duration: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                entry.duration_str(),
                Style::default().fg(theme.text_primary),
            ),
        ])]);
        frame.render_widget(size_duration, chunks[3]);

        let speed = Paragraph::new(vec![
            Line::from(Span::styled("Speed", Style::default().fg(theme.text_muted))),
            Line::from(Span::styled(
                entry.speed_str(),
                Style::default().fg(theme.text_secondary),
            )),
        ]);
        frame.render_widget(speed, chunks[4]);

        let files_block = Block::default()
            .title(format!(" Files ({}) ", entry.file_count))
            .borders(Borders::TOP);

        let files_inner = files_block.inner(chunks[5]);
        frame.render_widget(files_block, chunks[5]);

        let file_lines: Vec<Line> = entry
            .file_names
            .iter()
            .take(5)
            .map(|name| {
                Line::from(Span::styled(
                    name,
                    Style::default().fg(theme.text_secondary),
                ))
            })
            .collect();

        let mut file_content = file_lines;
        if entry.file_count > 5 {
            file_content.push(Line::from(Span::styled(
                format!("  ... and {} more", entry.file_count - 5),
                Style::default().fg(theme.text_muted),
            )));
        }

        let files = Paragraph::new(file_content);
        frame.render_widget(files, files_inner);

        let mut action_lines = vec![Line::from("")];

        if entry.is_failed {
            action_lines.push(Line::from(Span::styled(
                "[R] Retry  [O] Open folder  [X] Clear history",
                Style::default().fg(theme.text_muted),
            )));
        } else {
            action_lines.push(Line::from(Span::styled(
                "[O] Open folder  [X] Clear history",
                Style::default().fg(theme.text_muted),
            )));
        }

        if let Some(ref error) = entry.error_message {
            action_lines.insert(
                0,
                Line::from(Span::styled(
                    format!("Error: {}", error),
                    Style::default().fg(theme.error),
                )),
            );
        }

        let actions = Paragraph::new(action_lines);
        frame.render_widget(actions, chunks[6]);
    }

    /// Render clear history confirmation dialog.
    #[allow(clippy::unused_self)]
    fn render_clear_confirmation(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .title(" Clear History ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.warning));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Clear all transfer history?",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("This will remove {} entries.", self.entries.len()),
                Style::default().fg(theme.text_secondary),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Y] ", Style::default().fg(theme.error)),
                Span::styled("Yes, clear all", Style::default().fg(theme.text_primary)),
                Span::styled("    ", Style::default()),
                Span::styled("[N] ", Style::default().fg(theme.success)),
                Span::styled("No, cancel", Style::default().fg(theme.text_primary)),
            ]),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Get the currently selected entry.
    pub fn get_selected_entry(&self, state: &AppState) -> Option<&TuiHistoryEntry> {
        if state.history.selected_index < self.entries.len() {
            self.entries.get(state.history.selected_index)
        } else {
            None
        }
    }

    /// Load history from the history store.
    pub fn load_history(&mut self) {
        self.entries.clear();

        if let Ok(store) = yoop_core::history::HistoryStore::load() {
            for entry in store.list(None) {
                let is_sent = entry.direction == yoop_core::history::TransferDirection::Sent;
                let is_completed = entry.state == yoop_core::history::TransferState::Completed;
                let is_failed = entry.state == yoop_core::history::TransferState::Failed;

                let file_names: Vec<String> = entry.files.iter().map(|f| f.name.clone()).collect();

                self.entries.push(TuiHistoryEntry {
                    id: entry.id,
                    timestamp: entry.timestamp,
                    timestamp_str: entry.formatted_timestamp(),
                    direction: entry.direction.to_string(),
                    is_sent,
                    device_name: entry.device_name.clone(),
                    file_count: entry.files.len(),
                    file_names,
                    total_bytes: entry.total_bytes,
                    size_str: TuiHistoryEntry::format_bytes(entry.total_bytes),
                    state: entry.state.to_string(),
                    is_completed,
                    is_failed,
                    duration_secs: entry.duration_secs,
                    speed_bps: entry.speed_bps,
                    output_dir: entry.output_dir.clone(),
                    error_message: entry.error_message.clone(),
                    share_code: entry.share_code.clone(),
                });
            }
        }
    }

    /// Convert timestamp to date string.
    fn timestamp_to_date(timestamp: u64) -> String {
        use chrono::{DateTime, Local, Utc};

        let today = Local::now().date_naive();
        let timestamp_i64 = i64::try_from(timestamp).unwrap_or(i64::MAX);

        if let Some(dt) = DateTime::<Utc>::from_timestamp(timestamp_i64, 0) {
            let date = dt.with_timezone(&Local).date_naive();

            if date == today {
                "Today".to_string()
            } else if date == today.pred_opt().unwrap_or(today) {
                "Yesterday".to_string()
            } else {
                date.format("%B %d, %Y").to_string()
            }
        } else {
            "Unknown".to_string()
        }
    }

    /// Cycle focus to next element.
    pub fn focus_next(&mut self, state: &mut super::super::state::HistoryState) {
        state.focus = match state.focus {
            HistoryFocus::HistoryList => HistoryFocus::Details,
            HistoryFocus::Details => HistoryFocus::HistoryList,
        };
    }

    /// Clear all history entries.
    pub fn clear_history(&mut self) -> bool {
        if let Ok(mut store) = yoop_core::history::HistoryStore::load() {
            if store.clear().is_ok() {
                self.entries.clear();
                return true;
            }
        }
        false
    }
}

impl Default for HistoryView {
    fn default() -> Self {
        Self::new()
    }
}
