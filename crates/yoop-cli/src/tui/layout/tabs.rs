//! Tab layout for medium terminals (80-119 cols).
//!
//! Layout structure:
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ Header                                               │
//! ├─────────────────────────────────────────────────────┤
//! │ [Share] [Receive] [Clipboard] [Sync] [Devices] ...  │ <- Navigation tabs
//! ├─────────────────────────────────────────────────────┤
//! │ Content                                              │
//! │                                                      │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │ Log? (if visible)                                    │
//! ├─────────────────────────────────────────────────────┤
//! │ Status bar                                           │
//! └─────────────────────────────────────────────────────┘
//! ```

use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::{ComputedLayout, LayoutMode};

/// Compute tab layout for medium terminals.
pub fn compute(size: Rect, log_visible: bool, _transfers_expanded: bool) -> ComputedLayout {
    let mut constraints = vec![
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(10),
    ];

    if log_visible {
        constraints.push(Constraint::Length(10));
    }

    constraints.push(Constraint::Length(2));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    let mut idx = 0;
    let header = chunks[idx];
    idx += 1;

    let navigation = chunks[idx];
    idx += 1;

    let content = chunks[idx];
    idx += 1;

    let log = if log_visible {
        let log_area = chunks[idx];
        idx += 1;
        Some(log_area)
    } else {
        None
    };

    let status = chunks[idx];

    let transfers = None;

    ComputedLayout {
        mode: LayoutMode::Tabs,
        header,
        navigation,
        content,
        status,
        log,
        transfers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tabs_layout_areas_valid() {
        let size = Rect::new(0, 0, 100, 30);
        let layout = compute(size, false, false);

        assert!(layout.header.x + layout.header.width <= size.width);
        assert!(layout.navigation.x + layout.navigation.width <= size.width);
        assert!(layout.content.x + layout.content.width <= size.width);
        assert!(layout.status.x + layout.status.width <= size.width);
    }

    #[test]
    fn test_tabs_layout_navigation_spans_width() {
        let size = Rect::new(0, 0, 100, 30);
        let layout = compute(size, false, false);

        assert_eq!(layout.navigation.width, size.width);
    }

    #[test]
    fn test_tabs_layout_with_log() {
        let size = Rect::new(0, 0, 100, 30);
        let layout = compute(size, true, false);

        let log = layout.log.expect("Log should be present");
        assert!(log.height >= 8);
    }

    #[test]
    fn test_tabs_layout_no_transfers_panel() {
        let size = Rect::new(0, 0, 100, 30);
        let layout = compute(size, false, true);

        assert!(layout.transfers.is_none());
    }
}
