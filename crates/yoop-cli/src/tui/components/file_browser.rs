//! File browser component for TUI.
//!
//! Provides vim-style navigation for browsing directories and selecting files.

use std::collections::HashSet;
use std::path::Path;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::state::{DirEntry, FileBrowserState};
use crate::tui::theme::Theme;

/// File browser component for selecting files to share.
pub struct FileBrowser {
    /// List state for ratatui
    list_state: ListState,
}

impl FileBrowser {
    /// Create a new file browser.
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { list_state }
    }

    /// Render the file browser.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &FileBrowserState,
        theme: &Theme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(2),
            ])
            .split(area);

        Self::render_path_bar(frame, chunks[0], state, theme);
        self.render_file_list(frame, chunks[1], state, theme);
        Self::render_help_bar(frame, chunks[2], state, theme);
    }

    /// Render the current path bar.
    fn render_path_bar(frame: &mut Frame, area: Rect, state: &FileBrowserState, theme: &Theme) {
        let path_str = state.current_dir.display().to_string();
        let short_path = shorten_path(&path_str, area.width.saturating_sub(10) as usize);

        let block = Block::default()
            .title(" Browse Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_focused));

        let paragraph = Paragraph::new(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(theme.text_muted)),
            Span::styled(short_path, Style::default().fg(theme.accent)),
        ]))
        .block(block);

        frame.render_widget(paragraph, area);
    }

    /// Render the file list.
    fn render_file_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &FileBrowserState,
        theme: &Theme,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let visible_height = area.height.saturating_sub(2) as usize;
        let total_entries = state.entries.len();

        let scroll_offset = calculate_scroll_offset(state.selected, visible_height, total_entries);

        let items: Vec<ListItem> = state
            .entries
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(idx, entry)| {
                let is_selected = state.selections.contains(&entry.path);
                let is_cursor = idx == state.selected;

                Self::create_list_item(
                    entry,
                    is_selected,
                    is_cursor,
                    theme,
                    area.width.saturating_sub(4) as usize,
                )
            })
            .collect();

        let adjusted_selected = state.selected.saturating_sub(scroll_offset);
        self.list_state.select(Some(adjusted_selected));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Render help bar at the bottom.
    fn render_help_bar(frame: &mut Frame, area: Rect, state: &FileBrowserState, theme: &Theme) {
        let selected_count = state.selections.len();
        let total_size = calculate_selected_size(state);

        let left = format!(" {} selected ({})", selected_count, format_size(total_size));

        let right = "[j/k] Nav  [Space] Select  [.] Hidden  [Tab] Confirm  [Esc] Cancel";

        let text = if area.width as usize > left.len() + right.len() + 2 {
            let padding = area.width as usize - left.len() - right.len() - 2;
            format!("{}{:>width$}", left, right, width = padding + right.len())
        } else {
            right.to_string()
        };

        let paragraph = Paragraph::new(Line::from(Span::styled(
            text,
            Style::default().fg(theme.text_muted),
        )));

        frame.render_widget(paragraph, area);
    }

    /// Create a list item for a directory entry.
    fn create_list_item(
        entry: &DirEntry,
        is_selected: bool,
        _is_cursor: bool,
        theme: &Theme,
        max_width: usize,
    ) -> ListItem<'static> {
        let checkbox = if is_selected { "[x]" } else { "[ ]" };
        let icon = if entry.is_dir { "/" } else { " " };

        let name = entry
            .path
            .file_name()
            .map_or_else(|| "..".to_string(), |n| n.to_string_lossy().to_string());

        let size_str = if entry.is_dir {
            "<DIR>".to_string()
        } else {
            format_size(entry.size)
        };

        let prefix_len = checkbox.len() + 1 + icon.len();
        let suffix_len = size_str.len() + 2;
        let available_for_name = max_width.saturating_sub(prefix_len + suffix_len);
        let display_name = truncate_name(&name, available_for_name);

        let style = if entry.is_dir {
            Style::default().fg(theme.accent)
        } else if entry.is_hidden {
            Style::default().fg(theme.text_muted)
        } else {
            Style::default().fg(theme.text_primary)
        };

        let checkbox_style = if is_selected {
            Style::default().fg(theme.success)
        } else {
            Style::default().fg(theme.text_muted)
        };

        let spans = vec![
            Span::styled(checkbox.to_string(), checkbox_style),
            Span::raw(" "),
            Span::styled(format!("{}{}", display_name, icon), style),
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

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// Load directory entries for the file browser.
pub fn load_directory(path: &Path, show_hidden: bool) -> std::io::Result<Vec<DirEntry>> {
    load_directory_filtered(path, show_hidden, false)
}

/// Load only directories for the file browser (for sync view).
pub fn load_directories_only(path: &Path, show_hidden: bool) -> std::io::Result<Vec<DirEntry>> {
    load_directory_filtered(path, show_hidden, true)
}

/// Load directory entries with optional filtering for directories only.
fn load_directory_filtered(
    path: &Path,
    show_hidden: bool,
    directories_only: bool,
) -> std::io::Result<Vec<DirEntry>> {
    let mut entries = Vec::new();

    if path.parent().is_some() {
        entries.push(DirEntry {
            path: path.join(".."),
            is_dir: true,
            size: 0,
            is_hidden: false,
        });
    }

    let read_dir = std::fs::read_dir(path)?;
    for result in read_dir {
        let entry = result?;
        let entry_path = entry.path();
        let metadata = entry.metadata()?;

        let file_name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let is_hidden = file_name.starts_with('.');

        if !show_hidden && is_hidden {
            continue;
        }

        let is_dir = metadata.is_dir();

        if directories_only && !is_dir {
            continue;
        }

        entries.push(DirEntry {
            path: entry_path,
            is_dir,
            size: if is_dir { 0 } else { metadata.len() },
            is_hidden,
        });
    }

    entries.sort_by(|a, b| {
        let a_is_parent = a.path.file_name().is_some_and(|n| n == "..");
        let b_is_parent = b.path.file_name().is_some_and(|n| n == "..");

        if a_is_parent {
            return std::cmp::Ordering::Less;
        }
        if b_is_parent {
            return std::cmp::Ordering::Greater;
        }

        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase());
                let b_name = b
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase());
                a_name.cmp(&b_name)
            }
        }
    });

    Ok(entries)
}

/// Format file size for display.
#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Shorten a path to fit within a width.
fn shorten_path(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        return path.to_string();
    }

    let ellipsis = "...";
    let keep_len = max_width.saturating_sub(ellipsis.len());

    if keep_len > 0 {
        format!(
            "{}{}",
            ellipsis,
            &path[path.len().saturating_sub(keep_len)..]
        )
    } else {
        ellipsis.to_string()
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

/// Calculate scroll offset to keep selection visible.
fn calculate_scroll_offset(selected: usize, visible_height: usize, total: usize) -> usize {
    if total <= visible_height {
        return 0;
    }

    let half = visible_height / 2;
    if selected < half {
        0
    } else if selected >= total.saturating_sub(half) {
        total.saturating_sub(visible_height)
    } else {
        selected.saturating_sub(half)
    }
}

/// Calculate total size of selected files.
fn calculate_selected_size(state: &FileBrowserState) -> u64 {
    state
        .entries
        .iter()
        .filter(|e| state.selections.contains(&e.path))
        .map(|e| e.size)
        .sum()
}

/// Initialize a new file browser state at the given directory.
pub fn init_browser_state(
    start_dir: Option<&Path>,
    show_hidden: bool,
) -> std::io::Result<FileBrowserState> {
    let current_dir = match start_dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    };

    let entries = load_directory(&current_dir, show_hidden)?;

    Ok(FileBrowserState {
        current_dir,
        entries,
        selected: 0,
        scroll: 0,
        show_hidden,
        filter: None,
        selections: HashSet::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(1536), "1.5KB");
        assert_eq!(format_size(1024 * 1024), "1.0MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0GB");
    }

    #[test]
    fn test_truncate_name() {
        assert_eq!(truncate_name("short", 10), "short");
        assert_eq!(truncate_name("verylongfilename.txt", 10), "verylon...");
        assert_eq!(truncate_name("abc", 3), "abc");
        assert_eq!(truncate_name("abcd", 3), "...");
    }

    #[test]
    fn test_load_directory() {
        let temp_dir = TempDir::new().unwrap();

        std::fs::write(temp_dir.path().join("file1.txt"), "test").unwrap();
        std::fs::write(temp_dir.path().join(".hidden"), "hidden").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let entries = load_directory(temp_dir.path(), false).unwrap();
        assert!(!entries
            .iter()
            .any(|e| e.path.file_name().is_some_and(|n| n == ".hidden")));

        let entries = load_directory(temp_dir.path(), true).unwrap();
        assert!(entries
            .iter()
            .any(|e| e.path.file_name().is_some_and(|n| n == ".hidden")));
    }

    #[test]
    fn test_scroll_offset() {
        assert_eq!(calculate_scroll_offset(0, 10, 20), 0);
        assert_eq!(calculate_scroll_offset(5, 10, 20), 0);
        assert_eq!(calculate_scroll_offset(10, 10, 20), 5);
        assert_eq!(calculate_scroll_offset(18, 10, 20), 10);
    }
}
