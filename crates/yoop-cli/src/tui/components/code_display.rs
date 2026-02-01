//! Code display component for TUI.
//!
//! Displays the share code prominently with expiration countdown.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Code display component for showing share codes.
pub struct CodeDisplay;

impl CodeDisplay {
    /// Render the code display.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        code: &str,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        status: &str,
        peer_name: Option<&str>,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Share Active ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.success));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let available_height = inner.height;

        let chunks = if available_height >= 12 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(5),
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Min(1),
                ])
                .split(inner)
        } else if available_height >= 8 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(1),
                ])
                .split(inner)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(1),
                    Constraint::Min(1),
                ])
                .split(inner)
        };

        if available_height >= 12 {
            Self::render_code_box(frame, chunks[1], code, theme);
            Self::render_expiration(frame, chunks[2], expires_at, theme);
            Self::render_status(frame, chunks[3], status, peer_name, theme);
            Self::render_actions(frame, chunks[4], theme);
        } else if available_height >= 8 {
            Self::render_code_box(frame, chunks[0], code, theme);
            Self::render_expiration(frame, chunks[1], expires_at, theme);
            Self::render_status(frame, chunks[2], status, peer_name, theme);
            Self::render_actions(frame, chunks[3], theme);
        } else {
            Self::render_code_compact(frame, chunks[0], code, theme);
            Self::render_status_compact(frame, chunks[1], status, expires_at, theme);
            Self::render_actions(frame, chunks[2], theme);
        }
    }

    /// Render the code box with large formatted characters.
    fn render_code_box(frame: &mut Frame, area: Rect, code: &str, theme: &Theme) {
        let formatted: String = code
            .chars()
            .map(|c| format!(" {} ", c))
            .collect::<Vec<_>>()
            .join(" ");

        let code_style = Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD);

        let code_line = Line::from(Span::styled(formatted, code_style));

        let code_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_focused));

        let paragraph = Paragraph::new(vec![Line::from(""), code_line, Line::from("")])
            .block(code_block)
            .alignment(Alignment::Center);

        #[allow(clippy::cast_possible_truncation)]
        let code_width = (code.len() * 4 + 4).min(area.width as usize) as u16;
        let x_offset = area.x + (area.width.saturating_sub(code_width)) / 2;
        let code_area = Rect::new(x_offset, area.y, code_width, area.height);

        frame.render_widget(paragraph, code_area);
    }

    /// Render expiration countdown.
    fn render_expiration(
        frame: &mut Frame,
        area: Rect,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        theme: &Theme,
    ) {
        let text = match expires_at {
            Some(exp) => {
                let now = chrono::Utc::now();
                let remaining = exp.signed_duration_since(now);

                if remaining.num_seconds() <= 0 {
                    "Expired".to_string()
                } else {
                    let mins = remaining.num_minutes();
                    let secs = remaining.num_seconds() % 60;
                    format!("Expires in: {}:{:02}", mins, secs)
                }
            }
            None => "No expiration set".to_string(),
        };

        let style = Style::default().fg(theme.text_secondary);

        let paragraph =
            Paragraph::new(Line::from(Span::styled(text, style))).alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render current status.
    fn render_status(
        frame: &mut Frame,
        area: Rect,
        status: &str,
        peer_name: Option<&str>,
        theme: &Theme,
    ) {
        let available_width = area.width as usize;

        let text = match peer_name {
            Some(name) => {
                let full_text = format!("{} - {}", status, name);
                truncate_str(&full_text, available_width)
            }
            None => truncate_str(status, available_width),
        };

        let style = if status.contains("Waiting") {
            Style::default().fg(theme.warning)
        } else if status.contains("Transfer") || status.contains("Connect") {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.text_primary)
        };

        let paragraph =
            Paragraph::new(Line::from(Span::styled(text, style))).alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render available actions.
    fn render_actions(frame: &mut Frame, area: Rect, theme: &Theme) {
        let available_width = area.width as usize;

        let spans = if available_width < 25 {
            vec![
                Span::styled(
                    "[C]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    "[N]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]
        } else {
            vec![
                Span::styled(
                    "[C]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("ancel  ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    "[N]",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("ew code", Style::default().fg(theme.text_muted)),
            ]
        };

        let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render a compact code display for very small screens.
    fn render_code_compact(frame: &mut Frame, area: Rect, code: &str, theme: &Theme) {
        let code_style = Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD);

        let paragraph = Paragraph::new(Line::from(vec![
            Span::styled("Code: ", Style::default().fg(theme.text_muted)),
            Span::styled(code, code_style),
        ]))
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render a compact status with expiration for very small screens.
    fn render_status_compact(
        frame: &mut Frame,
        area: Rect,
        status: &str,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        theme: &Theme,
    ) {
        let available_width = area.width as usize;

        let expiry_str = match expires_at {
            Some(exp) => {
                let now = chrono::Utc::now();
                let remaining = exp.signed_duration_since(now);
                if remaining.num_seconds() <= 0 {
                    "Exp".to_string()
                } else {
                    let mins = remaining.num_minutes();
                    let secs = remaining.num_seconds() % 60;
                    format!("{}:{:02}", mins, secs)
                }
            }
            None => String::new(),
        };

        let text = if expiry_str.is_empty() {
            truncate_str(status, available_width)
        } else {
            let combined = format!("{} ({})", status, expiry_str);
            truncate_str(&combined, available_width)
        };

        let style = if status.contains("Waiting") {
            Style::default().fg(theme.warning)
        } else {
            Style::default().fg(theme.text_secondary)
        };

        let paragraph =
            Paragraph::new(Line::from(Span::styled(text, style))).alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    /// Render a simple code display (inline, for status bar).
    pub fn render_inline(frame: &mut Frame, area: Rect, code: &str, theme: &Theme) {
        let spans = vec![
            Span::styled("Code: ", Style::default().fg(theme.text_muted)),
            Span::styled(
                code,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }
}

/// Progress display component for showing transfer progress.
pub struct ProgressDisplay;

impl ProgressDisplay {
    /// Render transfer progress.
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        current_file: &str,
        progress_pct: f64,
        speed_bps: u64,
        eta_secs: Option<u64>,
        theme: &Theme,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let file_line = Paragraph::new(Line::from(Span::styled(
            format!(
                "Transferring: {}",
                truncate_str(current_file, inner.width as usize - 14)
            ),
            Style::default().fg(theme.text_primary),
        )));
        frame.render_widget(file_line, chunks[0]);

        Self::render_progress_bar(frame, chunks[1], progress_pct, theme);

        let eta_str = match eta_secs {
            Some(s) if s > 0 => format!("ETA: {}s", s),
            _ => "ETA: --".to_string(),
        };

        let stats = format!(
            "{:.1}%  |  {}/s  |  {}",
            progress_pct,
            format_speed(speed_bps),
            eta_str
        );

        let stats_line = Paragraph::new(Line::from(Span::styled(
            stats,
            Style::default().fg(theme.text_secondary),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(stats_line, chunks[2]);
    }

    /// Render a progress bar.
    #[allow(clippy::cast_precision_loss)]
    fn render_progress_bar(frame: &mut Frame, area: Rect, progress_pct: f64, theme: &Theme) {
        let bar_width = area.width.saturating_sub(2) as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let filled =
            ((progress_pct / 100.0) * bar_width as f64).clamp(0.0, bar_width as f64) as usize;
        let empty = bar_width.saturating_sub(filled);

        let bar = format!("[{}{}]", "\u{2588}".repeat(filled), " ".repeat(empty));

        let paragraph = Paragraph::new(Line::from(Span::styled(
            bar,
            Style::default().fg(theme.progress_bar),
        )));

        frame.render_widget(paragraph, area);
    }
}

/// Format speed for display.
#[allow(clippy::cast_precision_loss)]
fn format_speed(bytes_per_sec: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes_per_sec >= GB {
        format!("{:.1} GB", bytes_per_sec as f64 / GB as f64)
    } else if bytes_per_sec >= MB {
        format!("{:.1} MB", bytes_per_sec as f64 / MB as f64)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB", bytes_per_sec as f64 / KB as f64)
    } else {
        format!("{} B", bytes_per_sec)
    }
}

/// Truncate a string to fit within a width.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(512), "512 B");
        assert_eq!(format_speed(1024), "1.0 KB");
        assert_eq!(format_speed(1024 * 1024), "1.0 MB");
        assert_eq!(format_speed(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("verylongstring", 10), "verylon...");
        assert_eq!(truncate_str("abc", 3), "abc");
        assert_eq!(truncate_str("abcd", 3), "...");
    }
}
