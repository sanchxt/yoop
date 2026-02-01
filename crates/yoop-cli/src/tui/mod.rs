//! TUI (Terminal User Interface) mode for Yoop.
//!
//! Provides a full-featured dashboard-style interface for managing
//! file transfers, clipboard sync, trusted devices, and configuration.
//!
//! # Usage
//!
//! ```bash
//! # Launch TUI dashboard
//! yoop tui
//!
//! # Launch directly to specific view
//! yoop tui --view share
//! yoop tui --view receive
//! ```
//!
//! # Architecture
//!
//! The TUI is built using `ratatui` and `crossterm`:
//!
//! - `app`: Main application loop and terminal setup
//! - `state`: Application state types
//! - `action`: User action types
//! - `event`: Terminal event handling
//! - `layout`: Responsive layout system
//! - `views`: View components for each feature
//! - `components`: Reusable UI components
//! - `theme`: Color theme system

// Allow some clippy lints that are overly pedantic for this module
// These will be addressed incrementally in later implementation phases
#![allow(
    clippy::use_self,
    clippy::missing_const_for_fn,
    clippy::match_same_arms,
    clippy::option_if_let_else,
    clippy::inefficient_to_string
)]

pub mod action;
pub mod app;
pub mod components;
pub mod event;
pub mod layout;
pub mod session;
pub mod state;
pub mod theme;
pub mod views;

pub use app::{App, TuiArgs};
pub use state::AppState;
pub use theme::Theme;

/// Run the TUI application.
pub async fn run(args: TuiArgs) -> anyhow::Result<()> {
    let _guard = suppress_logging();

    let mut app = App::new(args)?;
    app.run().await
}

/// Suppress console logging to prevent corruption of the TUI's alternate screen buffer.
/// Returns a guard that restores the default subscriber when dropped.
fn suppress_logging() -> tracing::subscriber::DefaultGuard {
    use tracing_subscriber::layer::SubscriberExt;

    let noop_subscriber =
        tracing_subscriber::registry().with(tracing_subscriber::filter::LevelFilter::OFF);

    tracing::subscriber::set_default(noop_subscriber)
}
