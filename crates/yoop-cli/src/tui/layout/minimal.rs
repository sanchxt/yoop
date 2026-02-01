//! Minimal layout for narrow terminals (<80 cols).
//!
//! Layout structure:
//! ```text
//! ┌────────────────────────────────┐
//! │ Header + Mode shortcuts        │
//! ├────────────────────────────────┤
//! │ Content (single column)        │
//! │                                │
//! │                                │
//! ├────────────────────────────────┤
//! │ Status (compact)               │
//! └────────────────────────────────┘
//! ```

use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::{ComputedLayout, LayoutMode};

/// Compute minimal layout for narrow terminals.
pub fn compute(size: Rect, log_visible: bool, _transfers_expanded: bool) -> ComputedLayout {
    let mut constraints = vec![Constraint::Length(2), Constraint::Min(8)];

    if log_visible {
        constraints.push(Constraint::Length(6));
    }

    constraints.push(Constraint::Length(2));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    let mut idx = 0;

    let header_nav = chunks[idx];
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

    let header_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(header_nav);

    ComputedLayout {
        mode: LayoutMode::Minimal,
        header: header_chunks[0],
        navigation: header_chunks[1],
        content,
        status,
        log,
        transfers: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_layout_areas_valid() {
        let size = Rect::new(0, 0, 60, 20);
        let layout = compute(size, false, false);

        assert!(layout.header.x + layout.header.width <= size.width);
        assert!(layout.navigation.x + layout.navigation.width <= size.width);
        assert!(layout.content.x + layout.content.width <= size.width);
        assert!(layout.status.x + layout.status.width <= size.width);
    }

    #[test]
    fn test_minimal_layout_compact() {
        let size = Rect::new(0, 0, 60, 20);
        let layout = compute(size, false, false);

        assert_eq!(layout.header.height, 1);
        assert_eq!(layout.navigation.height, 1);
        assert_eq!(layout.status.height, 2);
    }

    #[test]
    fn test_minimal_layout_with_log() {
        let size = Rect::new(0, 0, 60, 20);
        let layout = compute(size, true, false);

        let log = layout.log.expect("Log should be present");
        assert_eq!(log.height, 6);
    }

    #[test]
    fn test_minimal_layout_no_transfers() {
        let size = Rect::new(0, 0, 60, 20);
        let layout = compute(size, false, true);

        assert!(layout.transfers.is_none());
    }
}
