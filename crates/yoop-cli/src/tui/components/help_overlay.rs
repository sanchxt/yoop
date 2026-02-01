//! Help overlay modal component.
//!
//! Displays a comprehensive keybinding reference overlay.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::state::View;
use crate::tui::theme::Theme;

/// Help overlay component.
pub struct HelpOverlay;

impl HelpOverlay {
    /// Render the help overlay.
    pub fn render(frame: &mut Frame, area: Rect, current_view: View, theme: &Theme) {
        let overlay_area = centered_rect(80, 85, area);

        frame.render_widget(Clear, overlay_area);

        let block = Block::default()
            .title(" Help - Press Esc to close ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        render_left_column(frame, sections[0], theme);

        render_right_column(frame, sections[1], current_view, theme);
    }
}

fn render_left_column(frame: &mut Frame, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(9),
            Constraint::Min(0),
        ])
        .split(area);

    let global_text = vec![
        Line::from(Span::styled(
            " GLOBAL",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        binding_line("Q / Ctrl+C", "Quit application"),
        binding_line("?", "Show this help"),
        binding_line("L", "Toggle log panel"),
        binding_line("T", "Toggle transfers panel"),
        binding_line("Left / Right", "Switch views"),
        binding_line("Tab", "Cycle focus"),
    ];
    let global = Paragraph::new(global_text).wrap(Wrap { trim: false });
    frame.render_widget(global, chunks[0]);

    let nav_text = vec![
        Line::from(Span::styled(
            " NAVIGATION",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        binding_line("j / Down", "Move down"),
        binding_line("k / Up", "Move up"),
        binding_line("h / Left", "Go back / collapse"),
        binding_line("l / Right / Enter", "Enter / confirm"),
        binding_line("g / Home", "Go to first item"),
        binding_line("G / End", "Go to last item"),
        binding_line("Space", "Toggle selection"),
    ];
    let nav = Paragraph::new(nav_text).wrap(Wrap { trim: false });
    frame.render_widget(nav, chunks[1]);

    let browser_text = vec![
        Line::from(Span::styled(
            " FILE BROWSER",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        binding_line(".", "Toggle hidden files"),
        binding_line("Space", "Toggle selection"),
        binding_line("Tab", "Confirm selection"),
        binding_line("Esc", "Cancel"),
        binding_line("Backspace", "Go to parent"),
    ];
    let browser = Paragraph::new(browser_text).wrap(Wrap { trim: false });
    frame.render_widget(browser, chunks[2]);
}

fn render_right_column(frame: &mut Frame, area: Rect, current_view: View, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(12),
            Constraint::Min(0),
        ])
        .split(area);

    let view_text = vec![
        Line::from(Span::styled(
            " VIEW SHORTCUTS",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        binding_line("S", "Share view"),
        binding_line("R", "Receive view"),
        binding_line("C", "Clipboard view"),
        binding_line("Y", "Sync view"),
        binding_line("D", "Devices view"),
    ];
    let views = Paragraph::new(view_text).wrap(Wrap { trim: false });
    frame.render_widget(views, chunks[0]);

    let current_view_text = get_view_bindings(current_view, theme);
    let current = Paragraph::new(current_view_text).wrap(Wrap { trim: false });
    frame.render_widget(current, chunks[1]);
}

fn get_view_bindings(view: View, theme: &Theme) -> Vec<Line<'static>> {
    let title = format!(" {} VIEW", view.display_name().to_uppercase());
    let mut lines = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    match view {
        View::Share => {
            lines.extend([
                binding_line("A / B", "Add files / Browse"),
                binding_line("X", "Remove selected file"),
                binding_line("Enter / S", "Start share"),
                binding_line("Esc", "Cancel active share"),
            ]);
        }
        View::Receive => {
            lines.extend([
                binding_line("Tab", "Switch input mode"),
                binding_line("Enter", "Connect"),
                binding_line("A", "Accept transfer"),
                binding_line("D / Esc", "Decline transfer"),
            ]);
        }
        View::Clipboard => {
            lines.extend([
                binding_line("S", "Share clipboard*"),
                binding_line("R (shift)", "Receive clipboard*"),
                binding_line("r", "Refresh preview*"),
                binding_line("Y", "Start/join sync*"),
                binding_line("D / Esc", "Disconnect sync"),
                binding_line("", "*when not in code input"),
            ]);
        }
        View::Sync => {
            lines.extend([
                binding_line("B", "Browse directory"),
                binding_line("H", "Host sync session"),
                binding_line("Enter", "Join/Host sync"),
                binding_line("d", "Toggle deletions"),
                binding_line("l", "Toggle symlinks"),
                binding_line("Esc", "Stop sync"),
            ]);
        }
        View::Devices => {
            lines.extend([
                binding_line("Enter", "Send files to device"),
                binding_line("E", "Edit trust level"),
                binding_line("X / Delete", "Remove device"),
                binding_line("r", "Refresh list"),
            ]);
        }
        View::History => {
            lines.extend([
                binding_line("Enter", "View details"),
                binding_line("r", "Retry failed"),
                binding_line("O", "Open directory"),
                binding_line("X", "Clear history"),
                binding_line("R (shift)", "Refresh"),
            ]);
        }
        View::Config => {
            lines.extend([
                binding_line("Enter / l", "Edit setting"),
                binding_line("Space", "Toggle boolean"),
                binding_line("c", "Cycle enum value"),
                binding_line("Ctrl+S", "Save changes"),
                binding_line("Ctrl+R", "Revert changes"),
            ]);
        }
    }

    lines
}

fn binding_line(key: &'static str, description: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<15}", key),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(description),
    ])
}

/// Calculate a centered rectangle.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
