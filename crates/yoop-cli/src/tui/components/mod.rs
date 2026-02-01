//! TUI reusable components.

mod clipboard_preview;
mod code_display;
mod code_input;
mod device_list;
mod file_browser;
mod file_list;
mod file_preview;
mod help_overlay;
mod ip_input;
mod nav_menu;
mod share_options;
mod spinner;
mod status_bar;
mod sync_events;

pub use clipboard_preview::ClipboardPreview;
pub use code_display::{CodeDisplay, ProgressDisplay};
pub use code_input::CodeInput;
pub use device_list::{DeviceInfo, DeviceList};
pub use file_browser::{init_browser_state, load_directories_only, load_directory, FileBrowser};
pub use file_list::FileList;
pub use file_preview::{FilePreview, IncomingFile};
pub use help_overlay::HelpOverlay;
pub use ip_input::IpInput;
pub use nav_menu::NavMenu;
pub use share_options::{
    next_expire_option, prev_expire_option, ShareOptionsWidget, EXPIRE_OPTIONS,
};
pub use spinner::{LoadingIndicator, Spinner, SpinnerState, SpinnerStyle};
pub use status_bar::StatusBar;
pub use sync_events::{SyncEventDisplay, SyncEventType, SyncEventsList, SyncStatsDisplay};
