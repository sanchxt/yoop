//! TUI event handling.
//!
//! This module handles terminal events (key presses, resize, etc.)
//! and converts them into actions.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::action::Action;
use super::state::{
    AppState, ClipboardFocus, ReceiveInputMode, ShareFocus, ShareOptionFocus, SyncFocus,
    SyncOptionFocus, View,
};

/// Event handler that polls for terminal events.
pub struct EventHandler {
    /// Receiver for events
    rx: mpsc::UnboundedReceiver<Event>,
    /// Cancellation token for the background polling task
    cancel_token: CancellationToken,
}

impl EventHandler {
    /// Create a new event handler.
    ///
    /// This spawns a background task that polls for terminal events.
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let cancel_token = CancellationToken::new();
        let token = cancel_token.clone();

        tokio::spawn(async move {
            loop {
                if token.is_cancelled() {
                    break;
                }
                if event::poll(tick_rate).unwrap_or(false) {
                    if let Ok(event) = event::read() {
                        if tx.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx, cancel_token }
    }

    /// Cancel the event polling task.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Get the next event, if available.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

/// Map a key event to an action based on current state.
pub fn map_key_event(key: KeyEvent, state: &AppState) -> Action {
    if state.help_visible {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?' | 'q') | KeyCode::F(1) => Action::ToggleHelp,
            _ => Action::None,
        };
    }

    if let Some(action) = handle_global_keys(key, state) {
        return action;
    }

    match state.active_view {
        View::Share => handle_share_keys(key, state),
        View::Receive => handle_receive_keys(key, state),
        View::Clipboard => handle_clipboard_keys(key, state),
        View::Sync => handle_sync_keys(key, state),
        View::Devices => handle_devices_keys(key, state),
        View::History => handle_history_keys(key, state),
        View::Config => handle_config_keys(key, state),
    }
}

/// Handle global keybindings that work in any view.
fn handle_global_keys(key: KeyEvent, state: &AppState) -> Option<Action> {
    let file_browser_open = state.share.file_browser.is_some() || state.sync.file_browser.is_some();

    let in_input_mode = is_in_input_mode(state);

    match key.code {
        // ctrl+c always quits
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),

        // quit - only if not in input mode
        KeyCode::Char('q' | 'Q') if !in_input_mode => Some(Action::Quit),

        // help - always available
        KeyCode::Char('?') | KeyCode::F(1) => Some(Action::ToggleHelp),

        // toggle log panel
        KeyCode::Char('L') if !in_input_mode => Some(Action::ToggleLog),

        // toggle transfers panel
        KeyCode::Char('T') if !in_input_mode => Some(Action::ExpandTransfers),

        // arrow keys for view navigation - skip if in input mode or file browser
        KeyCode::Left if !in_input_mode && !file_browser_open => Some(Action::PrevView),
        KeyCode::Right if !in_input_mode && !file_browser_open => Some(Action::NextView),

        // tab for focus cycling - skip if file browser is open
        KeyCode::Tab if !file_browser_open => Some(Action::FocusNext),
        KeyCode::BackTab if !file_browser_open => Some(Action::FocusPrev),

        // view switching - only if not in input mode
        KeyCode::Char(c) if !in_input_mode => {
            let skip_for_clipboard = state.active_view == View::Clipboard
                && matches!(c.to_ascii_uppercase(), 'S' | 'R' | 'Y');
            let skip_for_sync = state.active_view == View::Sync
                && c.eq_ignore_ascii_case(&'H')
                && state.sync.directory.is_some();

            if skip_for_clipboard || skip_for_sync {
                None
            } else {
                View::from_shortcut(c).map(Action::SwitchView)
            }
        }

        _ => None,
    }
}

/// Check if the user is currently in an input field.
fn is_in_input_mode(state: &AppState) -> bool {
    match state.active_view {
        View::Share => {
            state.share.active_session.is_some() || state.share.focus == ShareFocus::Options
        }
        View::Receive => {
            state.receive.active_session.is_none()
                && matches!(
                    state.receive.input_mode,
                    ReceiveInputMode::Code | ReceiveInputMode::DirectIp
                )
        }
        View::Clipboard => {
            state.clipboard.focus == ClipboardFocus::SyncStatus
                && state.clipboard_sync.is_none()
                && state.clipboard.operation_in_progress.is_none()
        }
        View::Sync => {
            (state.sync.focus == SyncFocus::CodeInput
                && state.sync.active_session.is_none()
                && state.sync.file_browser.is_none())
                || state.sync.editing_pattern
        }
        View::Config => state.config.editing,
        _ => false,
    }
}

/// Handle Share view keybindings.
fn handle_share_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.share.file_browser.is_some() {
        return handle_file_browser_keys(key);
    }

    if state.share.active_session.is_some() {
        match key.code {
            KeyCode::Char('c' | 'C') => return Action::CancelShare,
            KeyCode::Char('n' | 'N') => return Action::RegenerateCode,
            KeyCode::Esc => return Action::CancelShare,
            _ => {}
        }
    }

    if state.share.focus == ShareFocus::Options {
        return handle_share_options_keys(key, state);
    }

    match key.code {
        KeyCode::Char('a' | 'A') => Action::OpenFileBrowser,
        KeyCode::Char(' ') => Action::ToggleFile(state.share.selected_index),
        KeyCode::Enter => {
            if state.share.selected_files.is_empty() {
                Action::OpenFileBrowser
            } else {
                Action::StartShare
            }
        }
        KeyCode::Char('x' | 'X') => Action::RemoveFile(state.share.selected_index),
        KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
        KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
        KeyCode::Esc => Action::None,
        _ => Action::None,
    }
}

/// Handle share options keybindings when options panel is focused.
fn handle_share_options_keys(key: KeyEvent, state: &AppState) -> Action {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => Action::PrevShareOption,
        KeyCode::Right | KeyCode::Char('l') => Action::NextShareOption,
        KeyCode::Char(' ') | KeyCode::Enter => {
            if let Some(focus) = state.share.option_focus {
                match focus {
                    ShareOptionFocus::Expire => Action::CycleExpireForward,
                    _ => Action::ToggleShareOption,
                }
            } else {
                Action::None
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.share.option_focus == Some(ShareOptionFocus::Expire) {
                Action::CycleExpireBackward
            } else {
                Action::None
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.share.option_focus == Some(ShareOptionFocus::Expire) {
                Action::CycleExpireForward
            } else {
                Action::None
            }
        }
        _ => Action::None,
    }
}

/// Handle Receive view keybindings.
fn handle_receive_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.receive.is_connecting && state.receive.active_session.is_none() {
        return match key.code {
            KeyCode::Esc => Action::CancelReceive,
            _ => Action::None,
        };
    }

    if let Some(ref session) = state.receive.active_session {
        return match session.status {
            super::state::ReceiveSessionStatus::Pending => match key.code {
                KeyCode::Char('a' | 'A') | KeyCode::Enter => Action::AcceptTransfer,
                KeyCode::Char('d' | 'D') | KeyCode::Esc => Action::DeclineTransfer,
                _ => Action::None,
            },
            super::state::ReceiveSessionStatus::Transferring => match key.code {
                KeyCode::Esc => Action::CancelReceive,
                _ => Action::None,
            },
            super::state::ReceiveSessionStatus::Completed
            | super::state::ReceiveSessionStatus::Failed
            | super::state::ReceiveSessionStatus::Cancelled => Action::Refresh,
        };
    }

    match state.receive.input_mode {
        super::state::ReceiveInputMode::Code => handle_code_input_keys(key, state),
        super::state::ReceiveInputMode::TrustedDevice => handle_device_selection_keys(key, state),
        super::state::ReceiveInputMode::DirectIp => handle_ip_input_keys(key, state),
    }
}

/// Handle code input keybindings.
fn handle_code_input_keys(key: KeyEvent, state: &AppState) -> Action {
    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Enter => {
            if state.receive.code_input.len() >= 4 {
                Action::StartReceive
            } else {
                Action::None
            }
        }
        KeyCode::Char(c) if c.is_alphanumeric() => {
            if state.receive.code_input.len() < 4 {
                let mut input = state.receive.code_input.clone();
                input.push(c.to_ascii_uppercase());
                Action::UpdateCodeInput(input)
            } else {
                Action::None
            }
        }
        KeyCode::Backspace => {
            let mut input = state.receive.code_input.clone();
            input.pop();
            Action::UpdateCodeInput(input)
        }
        KeyCode::Esc => {
            if state.receive.code_input.is_empty() {
                Action::None
            } else {
                Action::UpdateCodeInput(String::new())
            }
        }
        _ => Action::None,
    }
}

/// Handle trusted device selection keybindings.
fn handle_device_selection_keys(key: KeyEvent, _state: &AppState) -> Action {
    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
        KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
        KeyCode::Enter => Action::StartReceive,
        _ => Action::None,
    }
}

/// Handle IP input keybindings (segmented input).
fn handle_ip_input_keys(key: KeyEvent, state: &AppState) -> Action {
    let ip_input = &state.receive.ip_input;

    match key.code {
        KeyCode::Tab => {
            if ip_input.cursor_position < 4 {
                Action::IpCursorNext
            } else {
                Action::FocusNext
            }
        }
        KeyCode::BackTab => {
            if ip_input.cursor_position > 0 {
                Action::IpCursorPrev
            } else {
                Action::FocusPrev
            }
        }
        KeyCode::Enter => {
            if ip_input.is_complete() && ip_input.is_valid() && state.receive.code_input.len() >= 4
            {
                Action::StartReceive
            } else {
                Action::None
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if ip_input.current_at_max() {
                if ip_input.is_octet_segment() {
                    Action::IpCursorNext
                } else {
                    Action::None
                }
            } else {
                Action::IpSegmentAppend(c)
            }
        }
        KeyCode::Char('.') => {
            if ip_input.is_octet_segment() && ip_input.cursor_position < 3 {
                Action::IpCursorNext
            } else {
                Action::None
            }
        }
        KeyCode::Char(':') => {
            if ip_input.cursor_position < 4 {
                Action::IpCursorNext
            } else {
                Action::None
            }
        }
        KeyCode::Right => Action::IpCursorNext,
        KeyCode::Left => Action::IpCursorPrev,
        KeyCode::Backspace => {
            if ip_input.current_segment().is_empty() {
                if ip_input.cursor_position > 0 {
                    Action::IpCursorPrev
                } else {
                    Action::None
                }
            } else {
                Action::IpSegmentBackspace
            }
        }
        KeyCode::Esc => {
            if ip_input.is_empty() {
                Action::None
            } else {
                Action::IpClear
            }
        }
        _ => Action::None,
    }
}

/// Handle Clipboard view keybindings.
fn handle_clipboard_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.clipboard_sync.is_some() {
        return match key.code {
            KeyCode::Char('d' | 'D') | KeyCode::Esc => Action::StopClipboardSync,
            _ => Action::None,
        };
    }

    if state.clipboard.operation_in_progress.is_some() {
        return match key.code {
            KeyCode::Esc => Action::CancelClipboardOperation,
            _ => Action::None,
        };
    }

    let code_input_focused = state.clipboard.focus == ClipboardFocus::SyncStatus;

    if code_input_focused {
        match key.code {
            KeyCode::Tab => Action::FocusNext,
            KeyCode::BackTab => Action::FocusPrev,
            KeyCode::Enter => {
                if state.clipboard.code_input.len() >= 4 {
                    Action::ReceiveClipboard
                } else if state.clipboard.code_input.is_empty() {
                    Action::StartClipboardSync
                } else {
                    Action::None
                }
            }
            KeyCode::Char(c) if c.is_alphanumeric() => {
                if state.clipboard.code_input.len() < 4 {
                    let mut input = state.clipboard.code_input.clone();
                    input.push(c.to_ascii_uppercase());
                    Action::UpdateClipboardCodeInput(input)
                } else {
                    Action::None
                }
            }
            KeyCode::Backspace => {
                let mut input = state.clipboard.code_input.clone();
                input.pop();
                Action::UpdateClipboardCodeInput(input)
            }
            KeyCode::Esc => {
                if state.clipboard.code_input.is_empty() {
                    Action::None
                } else {
                    Action::UpdateClipboardCodeInput(String::new())
                }
            }
            _ => Action::None,
        }
    } else {
        match key.code {
            KeyCode::Tab => Action::FocusNext,
            KeyCode::BackTab => Action::FocusPrev,
            KeyCode::Char('s' | 'S') => Action::ShareClipboard,
            KeyCode::Char('r' | 'R') => Action::ReceiveClipboard,
            KeyCode::Char('y' | 'Y') => Action::StartClipboardSync,
            KeyCode::Char('f' | 'F') => Action::RefreshClipboard,
            KeyCode::Enter => Action::StartClipboardSync,
            _ => Action::None,
        }
    }
}

/// Handle Sync view keybindings.
fn handle_sync_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.sync.file_browser.is_some() {
        return handle_sync_file_browser_keys(key);
    }

    if state.sync.active_session.is_some() {
        return match key.code {
            KeyCode::Esc => Action::StopSync,
            KeyCode::Up | KeyCode::Char('k') => Action::ScrollSyncEventsUp,
            KeyCode::Down | KeyCode::Char('j') => Action::ScrollSyncEventsDown,
            _ => Action::None,
        };
    }

    match state.sync.focus {
        SyncFocus::Directory => handle_sync_directory_keys(key, state),
        SyncFocus::Options => handle_sync_options_keys(key, state),
        SyncFocus::CodeInput => handle_sync_code_input_keys(key, state),
        SyncFocus::ExcludePatterns => handle_sync_exclude_keys(key, state),
        SyncFocus::Events => handle_sync_events_keys(key),
    }
}

/// Handle sync directory focus keybindings.
fn handle_sync_directory_keys(key: KeyEvent, state: &AppState) -> Action {
    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Char('b' | 'B') | KeyCode::Enter => Action::OpenSyncDirectoryBrowser,
        KeyCode::Char('h' | 'H') if state.sync.directory.is_some() => Action::StartSyncHost,
        KeyCode::Char('e' | 'E') => Action::FocusExcludePatterns,
        _ => Action::None,
    }
}

/// Handle sync options keybindings when Options panel is focused.
fn handle_sync_options_keys(key: KeyEvent, state: &AppState) -> Action {
    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Up | KeyCode::Char('k') => Action::PrevSyncOption,
        KeyCode::Down | KeyCode::Char('j') => Action::NextSyncOption,
        KeyCode::Left => Action::PrevSyncOption,
        KeyCode::Right | KeyCode::Char('l') => Action::NextSyncOption,
        KeyCode::Char('h' | 'H') if state.sync.directory.is_some() => Action::StartSyncHost,
        KeyCode::Char(' ') | KeyCode::Enter => {
            if let Some(focus) = state.sync.option_focus {
                match focus {
                    SyncOptionFocus::SyncDeletions => Action::ToggleSyncDeletions,
                    SyncOptionFocus::FollowSymlinks => Action::ToggleFollowSymlinks,
                }
            } else {
                Action::ToggleSyncOption
            }
        }
        _ => Action::None,
    }
}

/// Handle sync code input keybindings.
fn handle_sync_code_input_keys(key: KeyEvent, state: &AppState) -> Action {
    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Enter => {
            if state.sync.code_input.len() >= 4 && state.sync.directory.is_some() {
                Action::JoinSync
            } else if state.sync.directory.is_some() {
                Action::StartSyncHost
            } else {
                Action::OpenSyncDirectoryBrowser
            }
        }
        KeyCode::Char(c) if c.is_alphanumeric() => {
            if state.sync.code_input.len() < 4 {
                let mut input = state.sync.code_input.clone();
                input.push(c.to_ascii_uppercase());
                Action::UpdateSyncCodeInput(input)
            } else {
                Action::None
            }
        }
        KeyCode::Backspace => {
            let mut input = state.sync.code_input.clone();
            input.pop();
            Action::UpdateSyncCodeInput(input)
        }
        KeyCode::Esc => {
            if state.sync.code_input.is_empty() {
                Action::None
            } else {
                Action::UpdateSyncCodeInput(String::new())
            }
        }
        _ => Action::None,
    }
}

/// Handle sync exclude patterns keybindings.
fn handle_sync_exclude_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.sync.editing_pattern {
        match key.code {
            KeyCode::Enter => Action::ConfirmAddExcludePattern,
            KeyCode::Esc => Action::CancelAddExcludePattern,
            KeyCode::Char(c) => {
                let mut input = state.sync.pattern_input.clone();
                input.push(c);
                Action::UpdatePatternInput(input)
            }
            KeyCode::Backspace => {
                let mut input = state.sync.pattern_input.clone();
                input.pop();
                Action::UpdatePatternInput(input)
            }
            _ => Action::None,
        }
    } else {
        match key.code {
            KeyCode::Tab => Action::FocusNext,
            KeyCode::BackTab => Action::FocusPrev,
            KeyCode::Char('e' | 'E') | KeyCode::Enter => Action::StartAddExcludePattern,
            KeyCode::Char('x' | 'X') | KeyCode::Delete => {
                Action::RemoveExcludePattern(state.sync.selected_pattern_index)
            }
            KeyCode::Char('h' | 'H') if state.sync.directory.is_some() => Action::StartSyncHost,
            KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
            KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
            _ => Action::None,
        }
    }
}

/// Handle sync events focus keybindings (when no active session, acts as placeholder).
fn handle_sync_events_keys(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        _ => Action::None,
    }
}

/// Handle file browser keys for sync view.
fn handle_sync_file_browser_keys(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::FileBrowserUp,
        KeyCode::Down | KeyCode::Char('j') => Action::FileBrowserDown,
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => Action::FileBrowserBack,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => Action::FileBrowserEnter,
        KeyCode::Char(' ') => Action::FileBrowserToggleSelection,
        KeyCode::Char('.') => Action::FileBrowserToggleHidden,
        KeyCode::Tab => Action::FileBrowserConfirm,
        KeyCode::Esc => Action::CloseSyncDirectoryBrowser,
        _ => Action::None,
    }
}

/// Handle Devices view keybindings.
fn handle_devices_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.devices.confirm_remove {
        return match key.code {
            KeyCode::Char('y' | 'Y') => Action::ConfirmRemoveDevice,
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Action::CancelRemoveDevice,
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
        KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
        KeyCode::Home | KeyCode::Char('g') => Action::ListFirst,
        KeyCode::End => Action::ListLast,

        KeyCode::Enter => Action::SendToDevice,
        KeyCode::Char('e' | 'E') => Action::CycleTrustLevel,
        KeyCode::Char('x' | 'X') | KeyCode::Delete => Action::RequestRemoveDevice,
        KeyCode::Char('r') => Action::RefreshDevices,

        _ => Action::None,
    }
}

/// Handle History view keybindings.
fn handle_history_keys(key: KeyEvent, state: &AppState) -> Action {
    if state.history.confirm_clear {
        return match key.code {
            KeyCode::Char('y' | 'Y') => Action::ConfirmClearHistory,
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Action::CancelClearHistory,
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,
        KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
        KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
        KeyCode::Home | KeyCode::Char('g') => Action::ListFirst,
        KeyCode::End => Action::ListLast,

        KeyCode::Enter => Action::ViewHistoryDetails,
        KeyCode::Char('r') => Action::RetryTransfer,
        KeyCode::Char('o' | 'O') => Action::OpenTransferDirectory,
        KeyCode::Char('x' | 'X') => Action::RequestClearHistory,
        KeyCode::Char('R') => Action::RefreshHistory,

        _ => Action::None,
    }
}

/// Handle Config view keybindings.
fn handle_config_keys(key: KeyEvent, state: &AppState) -> Action {
    use super::state::ConfigFocus;

    if state.config.confirm_save {
        return match key.code {
            KeyCode::Char('y' | 'Y') => Action::ConfirmSaveConfig,
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Action::CancelSaveConfig,
            _ => Action::None,
        };
    }

    if state.config.confirm_revert {
        return match key.code {
            KeyCode::Char('y' | 'Y') => Action::ConfirmRevertConfig,
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Action::CancelRevertConfig,
            _ => Action::None,
        };
    }

    if state.config.editing {
        return match key.code {
            KeyCode::Enter => Action::ConfirmEdit,
            KeyCode::Esc => Action::CancelEdit,
            KeyCode::Backspace => {
                let mut buf = state.config.edit_buffer.clone();
                buf.pop();
                Action::UpdateEditBuffer(buf)
            }
            KeyCode::Char(c) => {
                let mut buf = state.config.edit_buffer.clone();
                buf.push(c);
                Action::UpdateEditBuffer(buf)
            }
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::ListUp,
        KeyCode::Down | KeyCode::Char('j') => Action::ListDown,
        KeyCode::Home | KeyCode::Char('g') => Action::ListFirst,
        KeyCode::End => Action::ListLast,

        KeyCode::Tab => Action::FocusNext,
        KeyCode::BackTab => Action::FocusPrev,

        KeyCode::Enter | KeyCode::Char('l') => match state.config.focus {
            ConfigFocus::SectionList => Action::FocusNext,
            ConfigFocus::Settings => Action::StartEditSetting,
        },
        KeyCode::Char(' ') => {
            if state.config.focus == ConfigFocus::Settings {
                Action::ToggleConfigSetting
            } else {
                Action::None
            }
        }
        KeyCode::Char('c') => {
            if state.config.focus == ConfigFocus::Settings {
                Action::CycleConfigSetting
            } else {
                Action::None
            }
        }

        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::RequestSaveConfig
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Action::RequestRevertConfig
        }

        KeyCode::Char('r' | 'R') => Action::RefreshConfig,

        KeyCode::Char('h') | KeyCode::Left => {
            if state.config.focus == ConfigFocus::Settings {
                Action::FocusPrev
            } else {
                Action::None
            }
        }

        _ => Action::None,
    }
}

/// Handle file browser keybindings.
fn handle_file_browser_keys(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::FileBrowserUp,
        KeyCode::Down | KeyCode::Char('j') => Action::FileBrowserDown,
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => Action::FileBrowserBack,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => Action::FileBrowserEnter,
        KeyCode::Char(' ') => Action::FileBrowserToggleSelection,
        KeyCode::Char('.') => Action::FileBrowserToggleHidden,
        KeyCode::Tab => Action::FileBrowserConfirm,
        KeyCode::Esc => Action::CloseFileBrowser,
        KeyCode::Home | KeyCode::Char('g') => Action::ListFirst,
        KeyCode::End | KeyCode::Char('G') => Action::ListLast,
        _ => Action::None,
    }
}
