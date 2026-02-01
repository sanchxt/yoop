//! Clipboard view for TUI.
//!
//! Provides the interface for clipboard sharing, receiving, and syncing.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::components::{ClipboardPreview, CodeInput, DeviceInfo, SpinnerStyle};
use crate::tui::state::{AppState, ClipboardFocus, ClipboardOperation};
use crate::tui::theme::Theme;

/// Clipboard view component.
pub struct ClipboardView {
    /// Cached trusted devices
    pub devices: Vec<DeviceInfo>,
}

#[allow(clippy::unused_self, clippy::too_many_lines)]
impl ClipboardView {
    /// Create a new clipboard view.
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Load trusted devices from the trust store.
    pub fn load_devices(&mut self) {
        self.devices = match yoop_core::trust::TrustStore::load() {
            Ok(store) => store
                .list()
                .iter()
                .map(DeviceInfo::from_trusted_device)
                .collect(),
            Err(_) => Vec::new(),
        };
    }

    /// Render the clipboard view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        if self.devices.is_empty() {
            self.load_devices();
        }

        if state.clipboard_sync.is_some() {
            Self::render_sync_active(frame, area, state, theme);
            return;
        }

        if let Some(op) = state.clipboard.operation_in_progress {
            Self::render_operation_in_progress(frame, area, op, state, theme);
            return;
        }

        self.render_main_view(frame, area, state, theme);
    }

    /// Render the main clipboard view.
    fn render_main_view(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),
                Constraint::Length(5),
                Constraint::Length(7),
                Constraint::Length(3),
            ])
            .split(area);

        ClipboardPreview::render(
            frame,
            chunks[0],
            state.clipboard.content_type,
            state.clipboard.preview.as_deref(),
            state.clipboard.content_size,
            state.clipboard.focus == ClipboardFocus::Preview,
            theme,
        );

        self.render_actions(frame, chunks[1], state, theme);

        self.render_code_input(frame, chunks[2], state, theme);

        self.render_hints(frame, chunks[3], state, theme);
    }

    /// Render the actions panel.
    fn render_actions(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let border_style = if state.clipboard.focus == ClipboardFocus::Actions {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let action_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let has_content = state.clipboard.content_type.is_some();
        let share_style = if has_content {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let share_line = Line::from(vec![
            Span::styled("[S]", share_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                " Share clipboard (one-shot)",
                if has_content {
                    Style::default().fg(theme.text_primary)
                } else {
                    Style::default().fg(theme.text_muted)
                },
            ),
        ]);
        frame.render_widget(Paragraph::new(share_line), action_chunks[0]);

        let receive_line = Line::from(vec![
            Span::styled(
                "[R]",
                Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " Receive clipboard",
                Style::default().fg(theme.text_primary),
            ),
        ]);
        frame.render_widget(Paragraph::new(receive_line), action_chunks[1]);

        let sync_line = Line::from(vec![
            Span::styled(
                "[Y]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " Start/Join sync session",
                Style::default().fg(theme.text_primary),
            ),
        ]);
        frame.render_widget(Paragraph::new(sync_line), action_chunks[2]);
    }

    /// Render code input section.
    fn render_code_input(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let focused = state.clipboard.focus == ClipboardFocus::SyncStatus;

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        CodeInput::render(
            frame,
            chunks[0],
            &state.clipboard.code_input,
            focused,
            theme,
        );

        let device_block = Block::default()
            .title(" Quick Connect ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let inner = device_block.inner(chunks[1]);
        frame.render_widget(device_block, chunks[1]);

        let device_text = if self.devices.is_empty() {
            "No trusted devices"
        } else {
            "Use [D] to select device"
        };

        let v_center = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        let paragraph = Paragraph::new(Line::from(Span::styled(
            device_text,
            Style::default().fg(theme.text_muted),
        )))
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, v_center[1]);
    }

    /// Render hints bar.
    fn render_hints(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        if let Some(ref status_msg) = state.clipboard.status_message {
            let is_error = status_msg.starts_with("Failed");
            let color = if is_error { theme.error } else { theme.info };

            let paragraph = Paragraph::new(Line::from(Span::styled(
                status_msg.as_str(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )))
            .block(block)
            .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
            return;
        }

        let code_input_focused = state.clipboard.focus == ClipboardFocus::SyncStatus;

        let spans = if code_input_focused {
            let mut hints = vec![
                Span::styled(
                    "[A-Z0-9]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Type code  ", Style::default().fg(theme.text_muted)),
            ];

            if state.clipboard.code_input.len() >= 4 {
                hints.extend([
                    Span::styled(
                        "[Enter]",
                        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" Receive  ", Style::default().fg(theme.text_muted)),
                ]);
            } else if state.clipboard.code_input.is_empty() {
                hints.extend([
                    Span::styled(
                        "[Enter]",
                        Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" Host sync  ", Style::default().fg(theme.text_muted)),
                ]);
            }

            hints.extend([
                Span::styled(
                    "[Tab]",
                    Style::default()
                        .fg(theme.text_secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Next", Style::default().fg(theme.text_muted)),
            ]);

            hints
        } else {
            let mut hints = vec![
                Span::styled(
                    "[Tab]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Navigate  ", Style::default().fg(theme.text_muted)),
            ];

            if state.clipboard.content_type.is_some() {
                hints.extend([
                    Span::styled(
                        "[S]",
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("hare  ", Style::default().fg(theme.text_muted)),
                ]);
            }

            hints.extend([
                Span::styled(
                    "[Y]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Sync  ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    "[R]",
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::styled("eceive  ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    "[F]",
                    Style::default()
                        .fg(theme.text_secondary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Refresh", Style::default().fg(theme.text_muted)),
            ]);

            hints
        };

        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render operation in progress state.
    fn render_operation_in_progress(
        frame: &mut Frame,
        area: Rect,
        operation: ClipboardOperation,
        state: &AppState,
        theme: &Theme,
    ) {
        let (title, message, color) = match operation {
            ClipboardOperation::Sharing => (
                "Sharing Clipboard",
                "Waiting for receiver...",
                theme.success,
            ),
            ClipboardOperation::Receiving => ("Receiving Clipboard", "Searching...", theme.info),
            ClipboardOperation::StartingSync => ("Starting Sync", "Connecting...", theme.accent),
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Length(10),
                Constraint::Percentage(25),
                Constraint::Min(0),
            ])
            .split(area);

        let block = Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color));

        let spinner_char = state.spinner.current_frame(SpinnerStyle::Braille);
        let base_msg = state.clipboard.status_message.as_deref().unwrap_or(message);
        let status_msg = format!("{} {}", spinner_char, base_msg);

        let content = vec![
            Line::from(""),
            Line::from(Span::styled(
                status_msg,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [Esc] to cancel",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(""),
        ];

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, chunks[1]);
    }

    /// Render active sync session.
    fn render_sync_active(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        let sync_session = state.clipboard_sync.as_ref().unwrap();
        let block = Block::default()
            .title(" Clipboard Sync Active ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.success));

        let header_content = vec![
            Line::from(vec![
                Span::styled("Connected to: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    &sync_session.peer_name,
                    Style::default()
                        .fg(theme.text_primary)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Address: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    &sync_session.peer_address,
                    Style::default().fg(theme.text_secondary),
                ),
            ]),
        ];

        let header = Paragraph::new(header_content).block(block);
        frame.render_widget(header, chunks[0]);

        let stats_block = Block::default()
            .title(" Sync Stats ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let duration = sync_session
            .started_at
            .signed_duration_since(chrono::Utc::now())
            .abs();
        let duration_str = format_duration(duration);

        let stats_content = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("↑ Sent: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    format!("{} items", sync_session.items_sent),
                    Style::default().fg(theme.success),
                ),
            ]),
            Line::from(vec![
                Span::styled("↓ Received: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    format!("{} items", sync_session.items_received),
                    Style::default().fg(theme.info),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Session duration: ", Style::default().fg(theme.text_muted)),
                Span::styled(duration_str, Style::default().fg(theme.text_primary)),
            ]),
        ];

        let stats_paragraph = Paragraph::new(stats_content)
            .block(stats_block)
            .alignment(Alignment::Center);
        frame.render_widget(stats_paragraph, chunks[1]);

        let action_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let action_line = Line::from(vec![
            Span::styled(
                "[D]",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("isconnect  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "[P]",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("ause", Style::default().fg(theme.text_muted)),
        ]);

        let actions = Paragraph::new(action_line)
            .block(action_block)
            .alignment(Alignment::Center);
        frame.render_widget(actions, chunks[2]);
    }

    /// Cycle focus to next element.
    pub fn focus_next(&mut self, state: &mut crate::tui::state::ClipboardState) {
        state.focus = match state.focus {
            ClipboardFocus::Preview => ClipboardFocus::Actions,
            ClipboardFocus::Actions => ClipboardFocus::SyncStatus,
            ClipboardFocus::SyncStatus => ClipboardFocus::Preview,
        };
    }

    /// Cycle focus to previous element.
    pub fn focus_prev(&mut self, state: &mut crate::tui::state::ClipboardState) {
        state.focus = match state.focus {
            ClipboardFocus::Preview => ClipboardFocus::SyncStatus,
            ClipboardFocus::Actions => ClipboardFocus::Preview,
            ClipboardFocus::SyncStatus => ClipboardFocus::Actions,
        };
    }

    /// Refresh devices list.
    pub fn refresh_devices(&mut self) {
        self.load_devices();
    }
}

impl Default for ClipboardView {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a duration for display.
fn format_duration(duration: chrono::Duration) -> String {
    let total_secs = duration.num_seconds().unsigned_abs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(chrono::Duration::seconds(65)), "1:05");
        assert_eq!(format_duration(chrono::Duration::seconds(3661)), "1:01:01");
        assert_eq!(format_duration(chrono::Duration::seconds(30)), "0:30");
    }
}
