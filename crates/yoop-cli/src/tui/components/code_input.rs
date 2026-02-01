//! Code input component for TUI.
//!
//! Provides a visual code input with individual character boxes.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Code input component for entering share codes.
pub struct CodeInput;

impl CodeInput {
    /// Render the code input component.
    ///
    /// Shows 4 character boxes for the share code with the current input highlighted.
    pub fn render(frame: &mut Frame, area: Rect, code_input: &str, focused: bool, theme: &Theme) {
        let block = Block::default()
            .title(" Enter Share Code ")
            .borders(Borders::ALL)
            .border_style(if focused {
                Style::default().fg(theme.border_focused)
            } else {
                Style::default().fg(theme.border)
            });

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        Self::render_code_boxes(frame, chunks[1], code_input, focused, theme);

        let instruction_style = Style::default().fg(theme.text_muted);
        let instruction = if code_input.len() >= 4 {
            "Press [Enter] to connect"
        } else {
            "Type the 4-character code"
        };

        let instruction_paragraph =
            Paragraph::new(Line::from(Span::styled(instruction, instruction_style)))
                .alignment(Alignment::Center);

        frame.render_widget(instruction_paragraph, chunks[2]);
    }

    /// Render the 4 individual code boxes.
    fn render_code_boxes(
        frame: &mut Frame,
        area: Rect,
        code_input: &str,
        focused: bool,
        theme: &Theme,
    ) {
        let box_width: u16 = 5;
        let spacing: u16 = 2;
        let total_width = (box_width * 4) + (spacing * 3);

        let start_x = area.x + area.width.saturating_sub(total_width) / 2;

        #[allow(clippy::cast_possible_truncation)]
        let box_areas: Vec<Rect> = (0..4_u16)
            .map(|i| {
                let x = start_x + (i * (box_width + spacing));
                Rect::new(x, area.y, box_width, 3)
            })
            .collect();

        let chars: Vec<char> = code_input
            .chars()
            .chain(std::iter::repeat(' '))
            .take(4)
            .collect();

        for (i, (rect, &ch)) in box_areas.iter().zip(chars.iter()).enumerate() {
            let is_current = i == code_input.len() && focused;
            let has_value = ch != ' ';

            let border_style = if is_current {
                Style::default().fg(theme.accent)
            } else if has_value {
                Style::default().fg(theme.success)
            } else {
                Style::default().fg(theme.border)
            };

            let box_block = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style);

            let char_style = if has_value {
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.text_muted)
            };

            let display_char = if has_value {
                ch.to_string()
            } else {
                "_".to_string()
            };

            let char_paragraph = Paragraph::new(Line::from(Span::styled(display_char, char_style)))
                .block(box_block)
                .alignment(Alignment::Center);

            frame.render_widget(char_paragraph, *rect);
        }
    }

    /// Render a compact inline code input (for narrow terminals).
    pub fn render_compact(
        frame: &mut Frame,
        area: Rect,
        code_input: &str,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Code ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let display: String = code_input
            .chars()
            .chain(std::iter::repeat('_'))
            .take(4)
            .collect::<Vec<_>>()
            .chunks(1)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(" ");

        let style = if code_input.len() >= 4 {
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        };

        let paragraph =
            Paragraph::new(Line::from(Span::styled(display, style))).alignment(Alignment::Center);

        frame.render_widget(paragraph, inner);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_code_input_padding() {
        let input = "AB";
        let chars: Vec<char> = input
            .chars()
            .chain(std::iter::repeat(' '))
            .take(4)
            .collect();

        assert_eq!(chars, vec!['A', 'B', ' ', ' ']);
    }
}
