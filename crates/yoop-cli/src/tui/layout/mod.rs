//! TUI layout system.
//!
//! Provides responsive layouts that adapt to terminal size:
//! - Split layout for wide terminals (120+ cols)
//! - Tab layout for medium terminals (80-119 cols)
//! - Minimal layout for narrow terminals (<80 cols)

use ratatui::layout::Rect;

mod minimal;
mod split;
mod tabs;

/// Layout mode based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Wide terminal (120+ cols): split panels with sidebar
    Split,
    /// Medium terminal (80-119 cols): horizontal tabs
    Tabs,
    /// Narrow terminal (<80 cols): minimal single-column
    Minimal,
}

impl LayoutMode {
    /// Determine layout mode from terminal size.
    pub fn from_size(width: u16, height: u16) -> Self {
        if width >= 120 && height >= 30 {
            LayoutMode::Split
        } else if width >= 80 && height >= 24 {
            LayoutMode::Tabs
        } else {
            LayoutMode::Minimal
        }
    }

    /// Get the minimum width for this layout mode.
    pub const fn min_width(&self) -> u16 {
        match self {
            LayoutMode::Split => 120,
            LayoutMode::Tabs => 80,
            LayoutMode::Minimal => 40,
        }
    }

    /// Get the minimum height for this layout mode.
    pub const fn min_height(&self) -> u16 {
        match self {
            LayoutMode::Split => 30,
            LayoutMode::Tabs => 24,
            LayoutMode::Minimal => 16,
        }
    }
}

/// Computed layout areas for rendering
#[derive(Debug, Clone)]
pub struct ComputedLayout {
    /// Layout mode being used
    pub mode: LayoutMode,
    /// Header area (title bar)
    pub header: Rect,
    /// Navigation area (sidebar or tabs)
    pub navigation: Rect,
    /// Main content area
    pub content: Rect,
    /// Status bar area
    pub status: Rect,
    /// Log panel area (if visible)
    pub log: Option<Rect>,
    /// Transfers panel area (if expanded)
    pub transfers: Option<Rect>,
}

impl ComputedLayout {
    /// Compute layout for the given terminal size and visibility options.
    pub fn compute(size: Rect, log_visible: bool, transfers_expanded: bool) -> Self {
        let mode = LayoutMode::from_size(size.width, size.height);

        match mode {
            LayoutMode::Split => split::compute(size, log_visible, transfers_expanded),
            LayoutMode::Tabs => tabs::compute(size, log_visible, transfers_expanded),
            LayoutMode::Minimal => minimal::compute(size, log_visible, transfers_expanded),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_mode_detection() {
        assert_eq!(LayoutMode::from_size(150, 40), LayoutMode::Split);
        assert_eq!(LayoutMode::from_size(120, 30), LayoutMode::Split);

        assert_eq!(LayoutMode::from_size(100, 30), LayoutMode::Tabs);
        assert_eq!(LayoutMode::from_size(80, 24), LayoutMode::Tabs);

        assert_eq!(LayoutMode::from_size(79, 24), LayoutMode::Minimal);
        assert_eq!(LayoutMode::from_size(60, 20), LayoutMode::Minimal);

        assert_eq!(LayoutMode::from_size(150, 25), LayoutMode::Tabs);
        assert_eq!(LayoutMode::from_size(100, 20), LayoutMode::Minimal);
    }

    #[test]
    fn test_computed_layout_basic() {
        let size = Rect::new(0, 0, 120, 40);
        let layout = ComputedLayout::compute(size, false, false);

        assert_eq!(layout.mode, LayoutMode::Split);
        assert!(layout.header.height > 0);
        assert!(layout.navigation.width > 0);
        assert!(layout.content.width > 0);
        assert!(layout.status.height > 0);
        assert!(layout.log.is_none());
        assert!(layout.transfers.is_none());
    }

    #[test]
    fn test_computed_layout_with_log() {
        let size = Rect::new(0, 0, 120, 40);
        let layout = ComputedLayout::compute(size, true, false);

        assert_eq!(layout.mode, LayoutMode::Split);
        assert!(layout.log.is_some());
    }

    #[test]
    fn test_computed_layout_with_transfers() {
        let size = Rect::new(0, 0, 120, 40);
        let layout = ComputedLayout::compute(size, false, true);

        assert_eq!(layout.mode, LayoutMode::Split);
        assert!(layout.transfers.is_some());
    }

    #[test]
    fn test_tabs_layout() {
        let size = Rect::new(0, 0, 100, 30);
        let layout = ComputedLayout::compute(size, false, false);

        assert_eq!(layout.mode, LayoutMode::Tabs);
    }

    #[test]
    fn test_minimal_layout() {
        let size = Rect::new(0, 0, 60, 20);
        let layout = ComputedLayout::compute(size, false, false);

        assert_eq!(layout.mode, LayoutMode::Minimal);
    }
}
