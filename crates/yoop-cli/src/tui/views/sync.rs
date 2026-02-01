//! Sync view for TUI.
//!
//! Provides the interface for directory synchronization.

use std::path::PathBuf;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::tui::components::{
    init_browser_state, CodeInput, FileBrowser, SyncEventsList, SyncStatsDisplay,
};
use crate::tui::state::{AppState, SyncEventEntry, SyncFocus, SyncOptionFocus, SyncSession};
use crate::tui::theme::Theme;

/// Sync view component.
pub struct SyncView {
    /// File browser component
    file_browser: FileBrowser,
    /// Sync events list component
    events_list: SyncEventsList,
}

#[allow(clippy::unused_self)]
impl SyncView {
    /// Create a new sync view.
    pub fn new() -> Self {
        Self {
            file_browser: FileBrowser::new(),
            events_list: SyncEventsList::new(),
        }
    }

    /// Render the sync view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        frame.render_widget(Clear, area);

        if let Some(ref browser_state) = state.sync.file_browser {
            self.file_browser.render(frame, area, browser_state, theme);
            return;
        }

        if let Some(ref session) = state.sync.active_session {
            self.render_active_session(frame, area, state, session, theme);
            return;
        }

        self.render_setup_view(frame, area, state, theme);
    }

    /// Render the setup view for starting a sync session.
    fn render_setup_view(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Min(4),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_directory_selection(frame, chunks[0], state, theme);

        self.render_options(frame, chunks[1], state, theme);

        self.render_join_section(frame, chunks[2], state, theme);

        self.render_exclude_patterns(frame, chunks[3], state, theme);

        self.render_actions(frame, chunks[4], state, theme);
    }

    /// Render directory selection section.
    fn render_directory_selection(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let focused = state.sync.focus == SyncFocus::Directory;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Sync Directory ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let dir_text = state.sync.directory.as_ref().map_or_else(
            || "No directory selected".to_string(),
            |p| p.display().to_string(),
        );

        let dir_style = if state.sync.directory.is_some() {
            Style::default().fg(theme.text_primary)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let content = vec![
            Line::from(Span::styled(dir_text, dir_style)),
            Line::from(""),
            Line::from(Span::styled(
                "Press [B] to browse",
                Style::default().fg(theme.text_muted),
            )),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Render sync options.
    fn render_options(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let focused = state.sync.focus == SyncFocus::Options;
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

        let deletion_indicator = if state.sync.sync_deletions {
            "[x]"
        } else {
            "[ ]"
        };
        let symlink_indicator = if state.sync.follow_symlinks {
            "[x]"
        } else {
            "[ ]"
        };

        let deletion_focused =
            focused && state.sync.option_focus == Some(SyncOptionFocus::SyncDeletions);
        let symlink_focused =
            focused && state.sync.option_focus == Some(SyncOptionFocus::FollowSymlinks);

        let deletion_style = if deletion_focused {
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_primary)
        };
        let symlink_style = if symlink_focused {
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_primary)
        };

        let deletion_prefix = if deletion_focused { "> " } else { "  " };
        let symlink_prefix = if symlink_focused { "> " } else { "  " };

        let content = vec![
            Line::from(vec![
                Span::styled(
                    deletion_prefix,
                    if deletion_focused {
                        Style::default().fg(theme.accent)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    format!("{} ", deletion_indicator),
                    Style::default().fg(theme.accent),
                ),
                Span::styled("Sync deletions", deletion_style),
                Span::styled(
                    "  (files deleted on one side are deleted on the other)",
                    Style::default().fg(theme.text_muted),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    symlink_prefix,
                    if symlink_focused {
                        Style::default().fg(theme.accent)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    format!("{} ", symlink_indicator),
                    Style::default().fg(theme.accent),
                ),
                Span::styled("Follow symlinks", symlink_style),
                Span::styled(
                    "  (sync the content of symbolic links)",
                    Style::default().fg(theme.text_muted),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(content);
        frame.render_widget(paragraph, inner);
    }

    /// Render the join section with code input.
    fn render_join_section(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        CodeInput::render(
            frame,
            chunks[0],
            &state.sync.code_input,
            state.sync.focus == SyncFocus::CodeInput,
            theme,
        );

        let hint_block = Block::default()
            .title(" Or Host ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let hint_content = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Press [H] to host a sync session",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(
                "(generates a code for others to join)",
                Style::default().fg(theme.text_muted),
            )),
        ];

        let paragraph = Paragraph::new(hint_content)
            .block(hint_block)
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, chunks[1]);
    }

    /// Render exclude patterns section.
    fn render_exclude_patterns(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let focused = state.sync.focus == SyncFocus::ExcludePatterns;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Exclude Patterns ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if state.sync.editing_pattern {
            let input_line = Line::from(vec![
                Span::styled("Pattern: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    &state.sync.pattern_input,
                    Style::default().fg(theme.text_primary),
                ),
                Span::styled("█", Style::default().fg(theme.accent)),
            ]);
            let hint_line = Line::from(Span::styled(
                "[Enter] Add  [Esc] Cancel",
                Style::default().fg(theme.text_muted),
            ));
            let content = vec![input_line, Line::from(""), hint_line];
            let paragraph = Paragraph::new(content).alignment(Alignment::Center);
            frame.render_widget(paragraph, inner);
        } else if state.sync.exclude_patterns.is_empty() {
            let hint = if focused {
                "[E] Add pattern  [Tab] Next section"
            } else {
                "No exclude patterns. Use [E] to add."
            };
            let paragraph = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(hint, Style::default().fg(theme.text_muted))),
            ])
            .alignment(Alignment::Center);
            frame.render_widget(paragraph, inner);
        } else {
            let items: Vec<ListItem> = state
                .sync
                .exclude_patterns
                .iter()
                .enumerate()
                .map(|(i, pattern)| {
                    let is_selected = focused && i == state.sync.selected_pattern_index;
                    let prefix = if is_selected { "> " } else { "  " };
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.text_primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };
                    let line = Line::from(vec![
                        Span::styled(
                            prefix,
                            if is_selected {
                                Style::default().fg(theme.accent)
                            } else {
                                Style::default()
                            },
                        ),
                        Span::styled(
                            format!("{}. ", i + 1),
                            Style::default().fg(theme.text_muted),
                        ),
                        Span::styled(pattern, style),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, inner);
        }
    }

    /// Render action hints.
    fn render_actions(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let has_directory = state.sync.directory.is_some();
        let has_code = state.sync.code_input.len() >= 4;

        let mut spans = vec![
            Span::styled(
                "[B]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("rowse  ", Style::default().fg(theme.text_muted)),
        ];

        if has_directory {
            spans.extend([
                Span::styled(
                    "[H]",
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("ost  ", Style::default().fg(theme.text_muted)),
            ]);
        }

        if has_code && has_directory {
            spans.extend([
                Span::styled(
                    "[Enter]",
                    Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" Join", Style::default().fg(theme.text_muted)),
            ]);
        }

        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render active sync session monitor.
    #[allow(clippy::too_many_lines)]
    fn render_active_session(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        session: &SyncSession,
        theme: &Theme,
    ) {
        let available_height = area.height;

        let chunks = if available_height >= 20 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Length(8),
                    Constraint::Min(5),
                    Constraint::Length(4),
                    Constraint::Length(3),
                ])
                .split(area)
        } else if available_height >= 14 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Min(4),
                    Constraint::Length(4),
                    Constraint::Length(3),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(3),
                    Constraint::Length(3),
                ])
                .split(area)
        };

        if available_height >= 20 {
            self.render_directory_info(frame, chunks[0], state, theme);
            self.render_code_display(frame, chunks[1], session, theme);

            let event_displays: Vec<_> = state
                .sync
                .events
                .iter()
                .map(|e| crate::tui::components::SyncEventDisplay {
                    timestamp: e.timestamp,
                    event_type: convert_event_type(e.event_type),
                    path: e.path.clone(),
                    message: e.message.clone(),
                })
                .collect();

            self.events_list.render(
                frame,
                chunks[2],
                &event_displays,
                state.sync.focus == SyncFocus::Events,
                theme,
            );

            SyncStatsDisplay::render(
                frame,
                chunks[3],
                state.sync.stats.files_sent,
                state.sync.stats.files_received,
                state.sync.stats.bytes_sent,
                state.sync.stats.bytes_received,
                state.sync.stats.conflicts,
                theme,
            );

            self.render_session_actions(frame, chunks[4], theme);
        } else if available_height >= 14 {
            self.render_code_display(frame, chunks[0], session, theme);

            let event_displays: Vec<_> = state
                .sync
                .events
                .iter()
                .map(|e| crate::tui::components::SyncEventDisplay {
                    timestamp: e.timestamp,
                    event_type: convert_event_type(e.event_type),
                    path: e.path.clone(),
                    message: e.message.clone(),
                })
                .collect();

            self.events_list.render(
                frame,
                chunks[1],
                &event_displays,
                state.sync.focus == SyncFocus::Events,
                theme,
            );

            SyncStatsDisplay::render(
                frame,
                chunks[2],
                state.sync.stats.files_sent,
                state.sync.stats.files_received,
                state.sync.stats.bytes_sent,
                state.sync.stats.bytes_received,
                state.sync.stats.conflicts,
                theme,
            );

            self.render_session_actions(frame, chunks[3], theme);
        } else {
            self.render_code_display_compact(frame, chunks[0], session, theme);

            SyncStatsDisplay::render(
                frame,
                chunks[1],
                state.sync.stats.files_sent,
                state.sync.stats.files_received,
                state.sync.stats.bytes_sent,
                state.sync.stats.bytes_received,
                state.sync.stats.conflicts,
                theme,
            );

            self.render_session_actions(frame, chunks[2], theme);
        }
    }

    /// Render directory info section.
    fn render_directory_info(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Syncing ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let dir_display = state.sync.directory.as_ref().map_or_else(
            || "Unknown directory".to_string(),
            |p| p.display().to_string(),
        );

        let inner = block.inner(area);
        let max_width = inner.width.saturating_sub(2) as usize;
        let truncated_dir = if dir_display.len() > max_width {
            format!(
                "...{}",
                &dir_display[dir_display.len().saturating_sub(max_width - 3)..]
            )
        } else {
            dir_display
        };

        let content = vec![Line::from(Span::styled(
            truncated_dir,
            Style::default().fg(theme.text_primary),
        ))];

        let paragraph = Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    /// Render the code display prominently.
    #[allow(clippy::format_collect)]
    fn render_code_display(
        &self,
        frame: &mut Frame,
        area: Rect,
        session: &SyncSession,
        theme: &Theme,
    ) {
        let border_color = if session.peer_name.is_some() {
            theme.success
        } else {
            theme.warning
        };

        let block = Block::default()
            .title(" Sync Session ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let clear = Paragraph::new("");
        frame.render_widget(clear, inner);

        let code_text = session.code.as_ref().map_or_else(
            || "Connected".to_string(),
            |c| c.chars().map(|ch| format!(" {} ", ch)).collect::<String>(),
        );

        let status_text = session.peer_name.as_ref().map_or_else(
            || "Waiting for peer to connect...".to_string(),
            |p| format!("Connected to: {}", p),
        );

        let code_style = Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD);

        let status_style = if session.peer_name.is_some() {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.warning)
        };

        let content = vec![
            Line::from(""),
            Line::from(Span::styled(&code_text, code_style)),
            Line::from(""),
            Line::from(Span::styled(&status_text, status_style)),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Render compact code display for smaller screens.
    fn render_code_display_compact(
        &self,
        frame: &mut Frame,
        area: Rect,
        session: &SyncSession,
        theme: &Theme,
    ) {
        let border_color = if session.peer_name.is_some() {
            theme.success
        } else {
            theme.warning
        };

        let block = Block::default()
            .title(" Sync Session ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let clear = Paragraph::new("");
        frame.render_widget(clear, inner);

        let code_line = if let Some(ref code) = session.code {
            Line::from(vec![
                Span::styled("Code: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    code.clone(),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(Span::styled(
                "Connected",
                Style::default().fg(theme.success),
            ))
        };

        let status_line = if let Some(ref peer) = session.peer_name {
            Line::from(vec![
                Span::styled("Peer: ", Style::default().fg(theme.text_muted)),
                Span::styled(peer.clone(), Style::default().fg(theme.text_primary)),
            ])
        } else {
            Line::from(Span::styled(
                "Waiting for peer...",
                Style::default().fg(theme.warning),
            ))
        };

        let content = vec![code_line, status_line];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Render session action hints.
    fn render_session_actions(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let spans = vec![
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Stop sync  ", Style::default().fg(theme.text_muted)),
            Span::styled(
                "[j/k]",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Scroll events", Style::default().fg(theme.text_muted)),
        ];

        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Cycle focus to next element.
    pub fn focus_next(&self, state: &mut crate::tui::state::SyncState) {
        let has_active_session = state.active_session.is_some();
        state.focus = state.focus.next(has_active_session);
        if state.focus == SyncFocus::Options && state.option_focus.is_none() {
            state.option_focus = Some(crate::tui::state::SyncOptionFocus::default());
        }
    }

    /// Cycle focus to previous element.
    pub fn focus_prev(&self, state: &mut crate::tui::state::SyncState) {
        let has_active_session = state.active_session.is_some();
        state.focus = state.focus.prev(has_active_session);
        if state.focus == SyncFocus::Options && state.option_focus.is_none() {
            state.option_focus = Some(crate::tui::state::SyncOptionFocus::default());
        }
    }

    /// Open file browser for directory selection.
    pub fn open_browser(
        &mut self,
        state: &mut crate::tui::state::SyncState,
    ) -> Result<(), std::io::Error> {
        let start_dir = state
            .directory
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from);

        let browser_state = init_browser_state(start_dir.as_deref(), false)?;
        state.file_browser = Some(browser_state);
        Ok(())
    }

    /// Confirm directory selection from browser.
    pub fn confirm_browser_selection(&mut self, state: &mut crate::tui::state::SyncState) {
        if let Some(browser) = state.file_browser.take() {
            let selected_dir = browser
                .selections
                .iter()
                .find(|p| p.is_dir())
                .cloned()
                .unwrap_or(browser.current_dir);

            state.directory = Some(selected_dir);
        }
    }

    /// Scroll events list up.
    pub fn scroll_events_up(&mut self, events_count: usize) {
        self.events_list.scroll_up(events_count);
    }

    /// Scroll events list down.
    pub fn scroll_events_down(&mut self, events_count: usize) {
        self.events_list.scroll_down(events_count);
    }

    /// Add a sync event.
    pub fn add_event(&mut self, event: &SyncEventEntry) {
        let _ = event;
    }
}

impl Default for SyncView {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert internal event type to component event type.
fn convert_event_type(
    event_type: crate::tui::state::SyncEventType,
) -> crate::tui::components::SyncEventType {
    use crate::tui::components::SyncEventType as ComponentType;
    use crate::tui::state::SyncEventType as StateType;

    match event_type {
        StateType::Connected => ComponentType::Connected,
        StateType::IndexExchanged => ComponentType::IndexExchanged,
        StateType::FileSending => ComponentType::FileSending,
        StateType::FileSent => ComponentType::FileSent,
        StateType::FileReceiving => ComponentType::FileReceiving,
        StateType::FileReceived => ComponentType::FileReceived,
        StateType::FileDeleted => ComponentType::FileDeleted,
        StateType::Conflict => ComponentType::Conflict,
        StateType::Error => ComponentType::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_view_creation() {
        let _view = SyncView::new();
    }

    #[test]
    fn test_convert_event_type() {
        use crate::tui::components::SyncEventType as ComponentType;
        use crate::tui::state::SyncEventType as StateType;

        assert!(matches!(
            convert_event_type(StateType::Connected),
            ComponentType::Connected
        ));
        assert!(matches!(
            convert_event_type(StateType::FileSent),
            ComponentType::FileSent
        ));
        assert!(matches!(
            convert_event_type(StateType::Error),
            ComponentType::Error
        ));
    }
}
