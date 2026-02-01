//! File list component for TUI.
//!
//! Displays the list of files selected for sharing with their sizes and checkboxes.

use std::path::PathBuf;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// File list component for displaying selected files.
pub struct FileList {
    /// List state for ratatui
    list_state: ListState,
}

impl FileList {
    /// Create a new file list.
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { list_state }
    }

    /// Render the file list.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        files: &[PathBuf],
        selected_index: usize,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let total_size = calculate_total_size(files);
        let title = format!(
            " Selected Files ({} items, {}) ",
            files.len(),
            format_size(total_size)
        );

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        if files.is_empty() {
            let help_text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No files selected",
                    Style::default().fg(theme.text_muted),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Press ", Style::default().fg(theme.text_muted)),
                    Span::styled(
                        "[A]",
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" to add files", Style::default().fg(theme.text_muted)),
                ]),
            ];

            let paragraph = Paragraph::new(help_text)
                .block(block)
                .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = files
            .iter()
            .enumerate()
            .map(|(idx, path)| {
                Self::create_list_item(
                    path,
                    idx == selected_index,
                    theme,
                    area.width.saturating_sub(4) as usize,
                )
            })
            .collect();

        self.list_state.select(Some(selected_index));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Create a list item for a file.
    fn create_list_item(
        path: &PathBuf,
        _is_selected: bool,
        theme: &Theme,
        max_width: usize,
    ) -> ListItem<'static> {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().to_string(),
        );

        let is_dir = path.is_dir();
        let size = if is_dir {
            calculate_dir_size(path).unwrap_or(0)
        } else {
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
        };

        let icon = if is_dir { "/" } else { "" };
        let size_str = format_size(size);

        let prefix_len = 4;
        let suffix_len = size_str.len() + 2;
        let available = max_width.saturating_sub(prefix_len + suffix_len);
        let display_name = truncate_name(&name, available);

        let name_style = if is_dir {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.text_primary)
        };

        let spans = vec![
            Span::styled("[x] ", Style::default().fg(theme.success)),
            Span::styled(format!("{}{}", display_name, icon), name_style),
            Span::styled(
                format!(
                    "{:>width$}",
                    size_str,
                    width = max_width.saturating_sub(prefix_len + display_name.len() + icon.len())
                ),
                Style::default().fg(theme.text_secondary),
            ),
        ];

        ListItem::new(Line::from(spans))
    }
}

impl Default for FileList {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate total size of files.
fn calculate_total_size(files: &[PathBuf]) -> u64 {
    files
        .iter()
        .map(|path| {
            if path.is_dir() {
                calculate_dir_size(path).unwrap_or(0)
            } else {
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// Calculate size of a directory recursively.
fn calculate_dir_size(path: &PathBuf) -> std::io::Result<u64> {
    let mut total = 0;

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            total += calculate_dir_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }

    Ok(total)
}

/// Format file size for display.
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

/// Truncate a file name to fit within a width.
fn truncate_name(name: &str, max_width: usize) -> String {
    if name.len() <= max_width {
        return name.to_string();
    }

    if max_width <= 3 {
        return "...".to_string();
    }

    let keep = max_width - 3;
    format!("{}...", &name[..keep])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_calculate_total_size() {
        let temp_dir = TempDir::new().unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        std::fs::write(&file1, "hello").unwrap(); // 5 bytes
        std::fs::write(&file2, "world!").unwrap(); // 6 bytes

        let file_paths = vec![file1, file2];
        assert_eq!(calculate_total_size(&file_paths), 11);
    }

    #[test]
    fn test_truncate_name() {
        assert_eq!(truncate_name("short.txt", 20), "short.txt");
        assert_eq!(
            truncate_name("very_long_filename.txt", 15),
            "very_long_fi..."
        );
    }
}
