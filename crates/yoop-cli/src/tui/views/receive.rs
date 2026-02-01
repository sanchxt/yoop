//! Receive view for TUI.
//!
//! Provides the interface for receiving files via share code,
//! trusted devices, or direct IP connection.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::components::{
    CodeInput, DeviceInfo, DeviceList, FilePreview, IncomingFile, IpInput, ProgressDisplay,
    SpinnerStyle,
};
use crate::tui::state::{AppState, ReceiveInputMode, TransferProgress};
use crate::tui::theme::Theme;

/// Focus state within receive view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReceiveFocus {
    /// Code input is focused
    #[default]
    Code,
    /// Trusted devices list is focused
    Devices,
    /// Direct IP input is focused
    DirectIp,
}

impl ReceiveFocus {
    /// Move to the next focus area.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Code => Self::Devices,
            Self::Devices => Self::DirectIp,
            Self::DirectIp => Self::Code,
        }
    }

    /// Move to the previous focus area.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Code => Self::DirectIp,
            Self::Devices => Self::Code,
            Self::DirectIp => Self::Devices,
        }
    }

    /// Convert to ReceiveInputMode.
    #[must_use]
    pub fn to_input_mode(self) -> ReceiveInputMode {
        match self {
            Self::Code => ReceiveInputMode::Code,
            Self::Devices => ReceiveInputMode::TrustedDevice,
            Self::DirectIp => ReceiveInputMode::DirectIp,
        }
    }

    /// Create from ReceiveInputMode.
    pub fn from_input_mode(mode: ReceiveInputMode) -> Self {
        match mode {
            ReceiveInputMode::Code => Self::Code,
            ReceiveInputMode::TrustedDevice => Self::Devices,
            ReceiveInputMode::DirectIp => Self::DirectIp,
        }
    }
}

/// Receive session status for display.
#[derive(Debug, Clone)]
pub enum ReceiveStatus {
    /// Idle - waiting for user input
    Idle,
    /// Searching for the share code
    Searching {
        /// The share code being searched
        code: String,
    },
    /// Connecting to peer
    Connecting {
        /// The peer being connected to
        peer: String,
    },
    /// Connected, showing file preview
    Connected {
        /// Name of the sender
        sender_name: String,
        /// Address of the sender
        sender_addr: String,
        /// List of incoming files
        files: Vec<IncomingFile>,
    },
    /// Transfer in progress
    Transferring {
        /// Name of the sender
        sender_name: String,
        /// Current transfer progress
        progress: TransferProgress,
        /// Name of the file currently being transferred
        current_file: String,
    },
    /// Transfer completed
    Completed {
        /// Number of files received
        files_count: usize,
        /// Total size of all files
        total_size: u64,
    },
    /// Transfer failed
    Failed {
        /// Error message
        error: String,
    },
}

/// Receive view component.
pub struct ReceiveView {
    /// Current focus within the view
    pub focus: ReceiveFocus,
    /// Device list component
    device_list: DeviceList,
    /// Cached list of trusted devices
    pub devices: Vec<DeviceInfo>,
    /// Current receive status
    pub status: ReceiveStatus,
}

#[allow(clippy::unused_self)]
impl ReceiveView {
    /// Create a new receive view.
    pub fn new() -> Self {
        Self {
            focus: ReceiveFocus::default(),
            device_list: DeviceList::new(),
            devices: Vec::new(),
            status: ReceiveStatus::Idle,
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

    /// Render the receive view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        if self.devices.is_empty() {
            self.load_devices();
        }

        match &self.status {
            ReceiveStatus::Connected {
                sender_name, files, ..
            } => {
                self.render_file_preview(frame, area, sender_name, files, theme);
            }
            ReceiveStatus::Transferring {
                sender_name,
                progress,
                current_file,
            } => {
                self.render_transfer_progress(
                    frame,
                    area,
                    sender_name,
                    progress,
                    current_file,
                    theme,
                );
            }
            ReceiveStatus::Completed {
                files_count,
                total_size,
            } => {
                self.render_completed(frame, area, *files_count, *total_size, theme);
            }
            ReceiveStatus::Failed { error } => {
                self.render_failed(frame, area, error, theme);
            }
            ReceiveStatus::Searching { code } | ReceiveStatus::Connecting { peer: code } => {
                self.render_searching(frame, area, code, state, theme);
            }
            ReceiveStatus::Idle => {
                self.render_input_selection(frame, area, state, theme);
            }
        }
    }

    /// Render the input selection interface.
    fn render_input_selection(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),
                Constraint::Min(8),
                Constraint::Length(5),
                Constraint::Length(3),
            ])
            .split(area);

        self.focus = ReceiveFocus::from_input_mode(state.receive.input_mode);

        CodeInput::render(
            frame,
            chunks[0],
            &state.receive.code_input,
            self.focus == ReceiveFocus::Code,
            theme,
        );

        Self::render_separator(frame, chunks[1], " OR ", theme);

        let middle_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(6)])
            .split(chunks[1]);

        self.device_list.render(
            frame,
            middle_chunks[1],
            &self.devices,
            state.receive.selected_device,
            self.focus == ReceiveFocus::Devices,
            theme,
        );

        IpInput::render(
            frame,
            chunks[2],
            &state.receive.ip_input,
            self.focus == ReceiveFocus::DirectIp,
            theme,
        );

        self.render_actions(frame, chunks[3], state, theme);
    }

    /// Render a separator line with text.
    fn render_separator(frame: &mut Frame, area: Rect, text: &str, theme: &Theme) {
        let separator_area = Rect::new(area.x, area.y, area.width, 1);

        #[allow(clippy::cast_possible_truncation)]
        let text_len = text.len() as u16;
        let half_width = area.width.saturating_sub(text_len) / 2;
        let line = format!(
            "{}{}{}",
            "─".repeat(half_width as usize),
            text,
            "─".repeat(half_width as usize)
        );

        let paragraph = Paragraph::new(Line::from(Span::styled(
            line,
            Style::default().fg(theme.border),
        )))
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, separator_area);
    }

    /// Render the action hints.
    fn render_actions(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let can_connect = match self.focus {
            ReceiveFocus::Code => state.receive.code_input.len() >= 4,
            ReceiveFocus::Devices => !self.devices.is_empty(),
            ReceiveFocus::DirectIp => {
                state.receive.ip_input.is_complete()
                    && state.receive.ip_input.is_valid()
                    && state.receive.code_input.len() >= 4
            }
        };

        let mut spans = vec![
            Span::styled(
                "[Tab]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Switch mode  ", Style::default().fg(theme.text_muted)),
        ];

        if can_connect {
            spans.extend([
                Span::styled(
                    "[Enter]",
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Connect", Style::default().fg(theme.text_muted)),
            ]);
        }

        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render the file preview and accept/decline prompt.
    fn render_file_preview(
        &self,
        frame: &mut Frame,
        area: Rect,
        sender_name: &str,
        files: &[IncomingFile],
        theme: &Theme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(3)])
            .split(area);

        FilePreview::render(frame, chunks[0], files, sender_name, theme);
        FilePreview::render_accept_prompt(frame, chunks[1], theme);
    }

    /// Render transfer progress.
    fn render_transfer_progress(
        &self,
        frame: &mut Frame,
        area: Rect,
        sender_name: &str,
        progress: &TransferProgress,
        current_file: &str,
        theme: &Theme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        let header_block = Block::default()
            .title(" Receiving ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.success));

        let header_text = format!("Receiving from {}", sender_name);
        let header = Paragraph::new(Line::from(Span::styled(
            header_text,
            Style::default().fg(theme.text_primary),
        )))
        .block(header_block)
        .alignment(Alignment::Center);

        frame.render_widget(header, chunks[0]);

        let eta = calculate_eta(progress);
        ProgressDisplay::render(
            frame,
            chunks[1],
            current_file,
            progress.percentage(),
            progress.speed_bps,
            eta,
            theme,
        );

        let cancel_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let cancel_spans = vec![
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Cancel transfer", Style::default().fg(theme.text_muted)),
        ];

        let cancel = Paragraph::new(Line::from(cancel_spans))
            .block(cancel_block)
            .alignment(Alignment::Center);

        frame.render_widget(cancel, chunks[2]);
    }

    /// Render the searching/connecting status.
    fn render_searching(
        &self,
        frame: &mut Frame,
        area: Rect,
        info: &str,
        state: &AppState,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Connecting ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.warning));

        let spinner_char = state.spinner.current_frame(SpinnerStyle::Braille);
        let text = match &self.status {
            ReceiveStatus::Searching { code } => {
                format!("{} Searching for code {}...", spinner_char, code)
            }
            ReceiveStatus::Connecting { peer } => {
                format!("{} Connecting to {}...", spinner_char, peer)
            }
            _ => format!("{} {}", spinner_char, info),
        };

        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                text,
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [Esc] to cancel",
                Style::default().fg(theme.text_muted),
            )),
        ])
        .block(block)
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render the completed status.
    fn render_completed(
        &self,
        frame: &mut Frame,
        area: Rect,
        files_count: usize,
        total_size: u64,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Transfer Complete ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.success));

        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Transfer completed successfully!",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "Received {} file(s) ({})",
                    files_count,
                    format_size(total_size)
                ),
                Style::default().fg(theme.text_primary),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to continue",
                Style::default().fg(theme.text_muted),
            )),
        ])
        .block(block)
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render the failed status.
    fn render_failed(&self, frame: &mut Frame, area: Rect, error: &str, theme: &Theme) {
        let block = Block::default()
            .title(" Transfer Failed ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.error));

        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Transfer failed",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                error.to_string(),
                Style::default().fg(theme.text_secondary),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press any key to try again",
                Style::default().fg(theme.text_muted),
            )),
        ])
        .block(block)
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Cycle focus to next element.
    pub fn focus_next(&mut self) {
        self.focus = self.focus.next();
    }

    /// Cycle focus to previous element.
    pub fn focus_prev(&mut self) {
        self.focus = self.focus.prev();
    }

    /// Reset the view to idle state.
    pub fn reset(&mut self) {
        self.status = ReceiveStatus::Idle;
    }

    /// Get the currently selected device.
    pub fn selected_device(&self, index: usize) -> Option<&DeviceInfo> {
        self.devices.get(index)
    }

    /// Refresh devices list.
    pub fn refresh_devices(&mut self) {
        self.load_devices();
    }
}

impl Default for ReceiveView {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate ETA from progress.
fn calculate_eta(progress: &TransferProgress) -> Option<u64> {
    if progress.speed_bps == 0 || progress.transferred >= progress.total {
        return None;
    }

    let remaining_bytes = progress.total - progress.transferred;
    Some(remaining_bytes / progress.speed_bps)
}

/// Format file size.
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
    fn test_focus_cycle() {
        let mut focus = ReceiveFocus::Code;
        focus = focus.next();
        assert_eq!(focus, ReceiveFocus::Devices);
        focus = focus.next();
        assert_eq!(focus, ReceiveFocus::DirectIp);
        focus = focus.next();
        assert_eq!(focus, ReceiveFocus::Code);
    }

    #[test]
    fn test_focus_prev_cycle() {
        let mut focus = ReceiveFocus::Code;
        focus = focus.prev();
        assert_eq!(focus, ReceiveFocus::DirectIp);
        focus = focus.prev();
        assert_eq!(focus, ReceiveFocus::Devices);
    }

    #[test]
    fn test_calculate_eta() {
        let progress = TransferProgress {
            transferred: 50,
            total: 100,
            speed_bps: 10,
        };
        assert_eq!(calculate_eta(&progress), Some(5));

        let done = TransferProgress {
            transferred: 100,
            total: 100,
            speed_bps: 10,
        };
        assert_eq!(calculate_eta(&done), None);
    }
}
