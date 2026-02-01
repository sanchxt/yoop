//! Split layout for wide terminals (120+ cols).
//!
//! Layout structure:
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ Header                                               │
//! ├────────────┬────────────────────────┬───────────────┤
//! │ Navigation │ Content                │ Transfers?    │
//! │ (sidebar)  │                        │               │
//! │            │                        │               │
//! │            ├────────────────────────┤               │
//! │            │ Log? (if visible)      │               │
//! ├────────────┴────────────────────────┴───────────────┤
//! │ Status bar                                           │
//! └─────────────────────────────────────────────────────┘
//! ```

use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::{ComputedLayout, LayoutMode};

/// Compute split layout for wide terminals.
pub fn compute(size: Rect, log_visible: bool, transfers_expanded: bool) -> ComputedLayout {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    let header = main_chunks[0];
    let body = main_chunks[1];
    let status = main_chunks[2];

    let body_constraints = if transfers_expanded {
        vec![
            Constraint::Length(20),
            Constraint::Min(40),
            Constraint::Length(35),
        ]
    } else {
        vec![Constraint::Length(20), Constraint::Min(40)]
    };

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(body_constraints)
        .split(body);

    let navigation = body_chunks[0];
    let mut content = body_chunks[1];

    let transfers = if transfers_expanded {
        Some(body_chunks[2])
    } else {
        None
    };

    let log = if log_visible {
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(15), Constraint::Length(12)])
            .split(content);
        content = content_chunks[0];
        Some(content_chunks[1])
    } else {
        None
    };

    ComputedLayout {
        mode: LayoutMode::Split,
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
    fn test_split_layout_areas_valid() {
        let size = Rect::new(0, 0, 150, 40);
        let layout = compute(size, false, false);

        assert!(layout.header.x + layout.header.width <= size.width);
        assert!(layout.header.y + layout.header.height <= size.height);
        assert!(layout.navigation.x + layout.navigation.width <= size.width);
        assert!(layout.content.x + layout.content.width <= size.width);
        assert!(layout.status.x + layout.status.width <= size.width);
    }

    #[test]
    fn test_split_layout_with_log() {
        let size = Rect::new(0, 0, 150, 40);
        let layout = compute(size, true, false);

        let log = layout.log.expect("Log should be present");
        assert!(log.height >= 10);
        assert!(log.y > layout.content.y);
    }

    #[test]
    fn test_split_layout_with_transfers() {
        let size = Rect::new(0, 0, 150, 40);
        let layout = compute(size, false, true);

        let transfers = layout.transfers.expect("Transfers should be present");
        assert!(transfers.width >= 30);
        assert!(transfers.x > layout.content.x);
    }

    #[test]
    fn test_split_layout_sidebar_width() {
        let size = Rect::new(0, 0, 150, 40);
        let layout = compute(size, false, false);

        assert_eq!(layout.navigation.width, 20);
    }
}
