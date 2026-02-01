//! Status bar component.
//!
//! Displays active transfers, clipboard status, and keybinding hints.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::layout::LayoutMode;
use crate::tui::state::AppState;
use crate::tui::theme::Theme;

/// Status bar component.
pub struct StatusBar;

impl StatusBar {
    /// Render the status bar.
    pub fn render(frame: &mut Frame, area: Rect, state: &AppState, mode: LayoutMode) {
        let theme = &Theme::default();

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme.border));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        match mode {
            LayoutMode::Split | LayoutMode::Tabs => {
                Self::render_full(frame, inner_area, state, theme);
            }
            LayoutMode::Minimal => {
                Self::render_compact(frame, inner_area, state, theme);
            }
        }
    }

    /// Render full status bar with sections.
    fn render_full(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ])
            .split(area);

        let transfer_content = if state.transfers.is_empty() {
            Line::from(Span::styled(
                "No active transfers",
                Style::default().fg(theme.text_muted),
            ))
        } else {
            let count = state.transfers.len();
            let total_progress = Self::calculate_total_progress(&state.transfers);
            Line::from(vec![
                Span::styled("● ", Style::default().fg(theme.success)),
                Span::styled(
                    format!("{} transfer(s) - {}%", count, total_progress),
                    Style::default().fg(theme.text_primary),
                ),
            ])
        };
        frame.render_widget(Paragraph::new(transfer_content), chunks[0]);

        let clipboard_content = match &state.clipboard_sync {
            Some(sync) => Line::from(vec![
                Span::styled("● ", Style::default().fg(theme.accent)),
                Span::styled(
                    format!("Synced: {}", sync.peer_name),
                    Style::default().fg(theme.text_primary),
                ),
            ]),
            None => Line::from(Span::styled(
                "Clipboard: idle",
                Style::default().fg(theme.text_muted),
            )),
        };
        frame.render_widget(Paragraph::new(clipboard_content), chunks[1]);

        let hints = Line::from(vec![
            Span::styled("[?]", Style::default().fg(theme.text_secondary)),
            Span::raw(" Help  "),
            Span::styled("[L]", Style::default().fg(theme.text_secondary)),
            Span::raw(" Log  "),
            Span::styled("[Q]", Style::default().fg(theme.text_secondary)),
            Span::raw(" Quit"),
        ]);
        frame.render_widget(
            Paragraph::new(hints).style(Style::default().fg(theme.text_muted)),
            chunks[2],
        );
    }

    /// Render compact status bar for minimal layout.
    fn render_compact(frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let transfer_content = if state.transfers.is_empty() {
            Span::styled("No transfers", Style::default().fg(theme.text_muted))
        } else {
            let count = state.transfers.len();
            let progress = Self::calculate_total_progress(&state.transfers);
            Span::styled(
                format!("●{} {}%", count, progress),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )
        };
        frame.render_widget(Paragraph::new(Line::from(transfer_content)), chunks[0]);

        let clipboard_content = match &state.clipboard_sync {
            Some(_) => Span::styled("Clip:sync", Style::default().fg(theme.accent)),
            None => Span::styled("Clip:idle", Style::default().fg(theme.text_muted)),
        };
        frame.render_widget(Paragraph::new(Line::from(clipboard_content)), chunks[1]);
    }

    /// Calculate total transfer progress percentage.
    #[allow(clippy::cast_possible_truncation)]
    fn calculate_total_progress(transfers: &[crate::tui::state::TransferSession]) -> u8 {
        if transfers.is_empty() {
            return 0;
        }

        let total: u64 = transfers.iter().map(|t| t.progress.total).sum();
        let transferred: u64 = transfers.iter().map(|t| t.progress.transferred).sum();

        if total == 0 {
            0
        } else {
            ((transferred * 100) / total) as u8
        }
    }
}
