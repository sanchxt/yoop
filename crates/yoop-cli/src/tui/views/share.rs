//! Share view for TUI.
//!
//! Provides the interface for selecting files and sharing them.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::components::{
    CodeDisplay, FileBrowser, FileList, ProgressDisplay, ShareOptionsWidget,
};
use crate::tui::state::{AppState, ShareFocus, ShareOptionFocus, ShareState, TransferProgress};
use crate::tui::theme::Theme;

/// Share view component.
pub struct ShareView {
    /// File browser component
    file_browser: FileBrowser,
    /// File list component
    file_list: FileList,
}

impl ShareView {
    /// Create a new share view.
    pub fn new() -> Self {
        Self {
            file_browser: FileBrowser::new(),
            file_list: FileList::new(),
        }
    }

    /// Render the share view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        if let Some(ref browser_state) = state.share.file_browser {
            self.file_browser.render(frame, area, browser_state, theme);
            return;
        }

        if let Some(ref session) = state.share.active_session {
            Self::render_active_session(frame, area, session, &state.share, theme);
            return;
        }

        self.render_file_selection(frame, area, state, theme);
    }

    /// Render the file selection interface.
    fn render_file_selection(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let share_state = &state.share;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(area);

        self.file_list.render(
            frame,
            chunks[0],
            &share_state.selected_files,
            share_state.selected_index,
            share_state.focus == ShareFocus::FileList,
            theme,
        );

        if share_state.focus == ShareFocus::Options {
            ShareOptionsWidget::render(
                frame,
                chunks[1],
                &share_state.options,
                share_state.option_focus,
                true,
                theme,
            );
        } else {
            let options_block = Block::default()
                .title(" Options ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border));

            let options_inner = options_block.inner(chunks[1]);
            frame.render_widget(options_block, chunks[1]);
            ShareOptionsWidget::render_compact(frame, options_inner, &share_state.options, theme);
        }

        Self::render_actions(frame, chunks[2], share_state, theme);
    }

    /// Render the actions bar.
    fn render_actions(frame: &mut Frame, area: Rect, share_state: &ShareState, theme: &Theme) {
        let has_files = !share_state.selected_files.is_empty();

        let mut spans = vec![
            Span::styled(
                "[A]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("dd files  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "[Tab]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Options  ", Style::default().fg(theme.text_muted)),
        ];

        if has_files {
            spans.extend([
                Span::styled(
                    "[X]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Remove  ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    "[Enter]",
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Share", Style::default().fg(theme.text_muted)),
            ]);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render an active share session.
    fn render_active_session(
        frame: &mut Frame,
        area: Rect,
        session: &crate::tui::state::ShareSession,
        share_state: &ShareState,
        theme: &Theme,
    ) {
        let has_progress = session.progress.total > 0 && session.progress.transferred > 0;

        let chunks = if has_progress {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(10),
                    Constraint::Length(5),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Min(10)])
                .split(area)
        };

        Self::render_files_summary(frame, chunks[0], &share_state.selected_files, theme);

        let status = if session.peer_name.is_some() {
            "Connected - Transferring..."
        } else {
            "Waiting for receiver..."
        };

        CodeDisplay::render(
            frame,
            chunks[1],
            &session.code,
            Some(session.expires_at),
            status,
            session.peer_name.as_deref(),
            theme,
        );

        if has_progress && chunks.len() > 2 {
            let current_file = session.files.first().map_or("Unknown", |s| s.as_str());
            let eta = calculate_eta(&session.progress);

            ProgressDisplay::render(
                frame,
                chunks[2],
                current_file,
                session.progress.percentage(),
                session.progress.speed_bps,
                eta,
                theme,
            );
        }
    }

    /// Render files summary.
    fn render_files_summary(
        frame: &mut Frame,
        area: Rect,
        files: &[std::path::PathBuf],
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Sharing ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let inner = block.inner(area);
        let available_width = inner.width.saturating_sub(2) as usize;

        let total_size: u64 = files
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .sum();

        let file_names: Vec<String> = files
            .iter()
            .map(|p| {
                p.file_name().map_or_else(
                    || "Unknown".to_string(),
                    |n| n.to_string_lossy().to_string(),
                )
            })
            .collect();

        let mut file_display = String::new();
        let mut shown_count = 0;
        for (i, name) in file_names.iter().enumerate() {
            let separator = if i == 0 { "" } else { ", " };
            let remaining = files.len() - i;
            let more_suffix = if remaining > 1 {
                format!(" +{} more", remaining - 1)
            } else {
                String::new()
            };

            let candidate = format!("{}{}{}", file_display, separator, name);
            if candidate.len() + more_suffix.len() <= available_width || i == 0 {
                file_display = candidate;
                shown_count = i + 1;
            } else {
                break;
            }
        }

        if shown_count < files.len() {
            use std::fmt::Write;
            let _ = write!(file_display, " +{} more", files.len() - shown_count);
        }

        let content = vec![
            Line::from(Span::styled(
                format!("{} items ({})", files.len(), format_size(total_size)),
                Style::default().fg(theme.text_primary),
            )),
            Line::from(Span::styled(
                file_display,
                Style::default().fg(theme.text_secondary),
            )),
        ];

        let paragraph = Paragraph::new(content).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Cycle focus to next element.
    pub fn focus_next(&mut self, state: &mut ShareState) {
        state.focus = state.focus.next();
        if state.focus == ShareFocus::Options {
            state.option_focus = Some(ShareOptionFocus::default());
        } else {
            state.option_focus = None;
        }
    }
}

impl Default for ShareView {
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
    use crate::tui::state::ShareOptions;

    #[test]
    fn test_focus_cycle() {
        let mut state = ShareState {
            focus: ShareFocus::FileList,
            option_focus: None,
            selected_files: Vec::new(),
            file_browser: None,
            options: ShareOptions::default(),
            active_session: None,
            selected_index: 0,
        };
        let mut view = ShareView::new();
        view.focus_next(&mut state);
        assert_eq!(state.focus, ShareFocus::Options);
        assert!(state.option_focus.is_some());
        view.focus_next(&mut state);
        assert_eq!(state.focus, ShareFocus::FileList);
        assert!(state.option_focus.is_none());
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

        let no_speed = TransferProgress {
            transferred: 50,
            total: 100,
            speed_bps: 0,
        };
        assert_eq!(calculate_eta(&no_speed), None);
    }
}
