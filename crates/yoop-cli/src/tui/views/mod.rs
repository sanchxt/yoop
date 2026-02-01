//! TUI views.
//!
//! Each view represents a major feature area in the TUI.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::state::{AppState, View};
use crate::tui::theme::Theme;

pub mod clipboard;
pub mod config;
pub mod devices;
pub mod history;
mod placeholder;
pub mod receive;
pub mod share;
pub mod sync;

pub use clipboard::ClipboardView;
pub use config::ConfigView;
pub use devices::DevicesView;
pub use history::HistoryView;
pub use receive::ReceiveView;
pub use share::ShareView;
pub use sync::SyncView;

/// Shared view state container.
///
/// Holds instances of all views to preserve their state across renders.
pub struct ViewState {
    /// Share view instance
    pub share: ShareView,
    /// Receive view instance
    pub receive: ReceiveView,
    /// Clipboard view instance
    pub clipboard: ClipboardView,
    /// Sync view instance
    pub sync: SyncView,
    /// Devices view instance
    pub devices: DevicesView,
    /// History view instance
    pub history: HistoryView,
    /// Config view instance
    pub config: ConfigView,
}

impl ViewState {
    /// Create a new view state container.
    pub fn new() -> Self {
        let mut devices = DevicesView::new();
        devices.load_devices();

        let mut history = HistoryView::new();
        history.load_history();

        let mut config = ConfigView::new();
        config.load_config();

        Self {
            share: ShareView::new(),
            receive: ReceiveView::new(),
            clipboard: ClipboardView::new(),
            sync: SyncView::new(),
            devices,
            history,
            config,
        }
    }
}

impl Default for ViewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the appropriate view based on active view state.
pub fn render_view(frame: &mut Frame, area: Rect, state: &AppState, _theme: &Theme) {
    match state.active_view {
        View::Share => {
            placeholder::render(frame, area, "Share", "Use app.views.share.render() instead");
        }
        View::Receive => {
            placeholder::render(
                frame,
                area,
                "Receive",
                "Use app.views.receive.render() instead",
            );
        }
        View::Clipboard => {
            placeholder::render(frame, area, "Clipboard", "Share and sync clipboard content");
        }
        View::Sync => {
            placeholder::render(
                frame,
                area,
                "Sync",
                "Synchronize directories with another device",
            );
        }
        View::Devices => {
            placeholder::render(frame, area, "Trusted Devices", "Manage trusted devices");
        }
        View::History => {
            placeholder::render(frame, area, "Transfer History", "View past transfers");
        }
        View::Config => {
            placeholder::render(frame, area, "Configuration", "Manage settings");
        }
    }
}

/// Render the appropriate view with view state.
pub fn render_view_with_state(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    views: &mut ViewState,
    theme: &Theme,
) {
    match state.active_view {
        View::Share => {
            views.share.render(frame, area, state, theme);
        }
        View::Receive => {
            views.receive.render(frame, area, state, theme);
        }
        View::Clipboard => {
            views.clipboard.render(frame, area, state, theme);
        }
        View::Sync => {
            views.sync.render(frame, area, state, theme);
        }
        View::Devices => {
            views.devices.render(frame, area, state, theme);
        }
        View::History => {
            views.history.render(frame, area, state, theme);
        }
        View::Config => {
            views.config.render(frame, area, state, theme);
        }
    }
}
