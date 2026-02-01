//! Navigation menu component.
//!
//! Displays available views and handles view switching.
//! Adapts to layout mode (sidebar vs tabs vs compact).

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs};
use ratatui::Frame;

use crate::tui::layout::LayoutMode;
use crate::tui::state::{AppState, View};
use crate::tui::theme::Theme;

/// Navigation menu component.
pub struct NavMenu;

impl NavMenu {
    /// Render the navigation menu based on layout mode.
    pub fn render(frame: &mut Frame, area: Rect, state: &AppState, mode: LayoutMode) {
        match mode {
            LayoutMode::Split => Self::render_sidebar(frame, area, state),
            LayoutMode::Tabs => Self::render_tabs(frame, area, state),
            LayoutMode::Minimal => Self::render_minimal(frame, area, state),
        }
    }

    /// Render as a vertical sidebar (for split layout).
    fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState) {
        let theme = &Theme::default();

        let block = Block::default()
            .title(" Navigation ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let items: Vec<ListItem> = View::all()
            .iter()
            .map(|view| {
                let is_active = *view == state.active_view;
                let style = if is_active {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_primary)
                };

                let prefix = if is_active { "> " } else { "  " };
                let shortcut = format!("[{}]", view.shortcut());
                let text = format!("{}{} {}", prefix, shortcut, view.name());

                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect();

        let mut all_items = items;
        all_items.push(ListItem::new(Line::from("")));
        all_items.push(ListItem::new(Line::from(Span::styled(
            "───────────────",
            Style::default().fg(theme.border),
        ))));
        all_items.push(ListItem::new(Line::from("")));

        all_items.push(ListItem::new(Line::from(Span::styled(
            "Trusted Online",
            Style::default().fg(theme.text_muted),
        ))));
        all_items.push(ListItem::new(Line::from(Span::styled(
            "  (none)",
            Style::default().fg(theme.text_muted),
        ))));

        all_items.push(ListItem::new(Line::from("")));
        all_items.push(ListItem::new(Line::from(Span::styled(
            "───────────────",
            Style::default().fg(theme.border),
        ))));
        all_items.push(ListItem::new(Line::from(Span::styled(
            "  [?] Help",
            Style::default().fg(theme.text_muted),
        ))));
        all_items.push(ListItem::new(Line::from(Span::styled(
            "  [Q] Quit",
            Style::default().fg(theme.text_muted),
        ))));

        let list = List::new(all_items).block(block);
        frame.render_widget(list, area);
    }

    /// Render as horizontal tabs (for tabs layout).
    fn render_tabs(frame: &mut Frame, area: Rect, state: &AppState) {
        let theme = &Theme::default();

        let titles: Vec<Line> = View::all()
            .iter()
            .map(|view| {
                let style = if *view == state.active_view {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(theme.text_secondary)
                };

                Line::from(Span::styled(format!(" {} ", view.name()), style))
            })
            .collect();

        let selected = View::all()
            .iter()
            .position(|v| *v == state.active_view)
            .unwrap_or(0);

        let tabs = Tabs::new(titles)
            .select(selected)
            .style(Style::default().fg(theme.text_muted))
            .highlight_style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::styled(" │ ", Style::default().fg(theme.border)));

        frame.render_widget(tabs, area);
    }

    /// Render as compact shortcuts (for minimal layout).
    fn render_minimal(frame: &mut Frame, area: Rect, state: &AppState) {
        let theme = &Theme::default();

        let spans: Vec<Span> = View::all()
            .iter()
            .enumerate()
            .flat_map(|(i, view)| {
                let is_active = *view == state.active_view;
                let style = if is_active {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_muted)
                };

                let mut spans = vec![Span::styled(
                    format!("[{}]{}", view.shortcut(), &view.name()[..1]),
                    style,
                )];

                if i < View::all().len() - 1 {
                    spans.push(Span::raw(" "));
                }

                spans
            })
            .collect();

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}
