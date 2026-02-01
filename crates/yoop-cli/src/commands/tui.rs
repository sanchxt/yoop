//! TUI command handler.

use anyhow::Result;

use crate::tui;

/// Arguments for the TUI command.
#[derive(clap::Parser)]
pub struct TuiArgs {
    /// Initial view to display (share, receive, clipboard, sync, devices, history, config)
    #[arg(long, short)]
    pub view: Option<String>,

    /// Theme to use (dark, light)
    #[arg(long)]
    pub theme: Option<String>,
}

/// Run the TUI application.
pub async fn run(args: TuiArgs) -> Result<()> {
    let tui_args = tui::TuiArgs {
        view: args.view,
        theme: args.theme,
    };

    tui::run(tui_args).await
}
