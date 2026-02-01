//! TUI action types.
//!
//! Actions represent user intents that can be triggered by key events
//! and are processed by the application to update state.

use std::path::PathBuf;

use super::state::{ConfigSection, ReceiveInputMode, View};

/// Actions that can be triggered by user input
#[derive(Debug, Clone)]
pub enum Action {
    /// Switch to a specific view
    SwitchView(View),
    /// Switch to next view (right arrow)
    NextView,
    /// Switch to previous view (left arrow)
    PrevView,
    /// Focus next element within view
    FocusNext,
    /// Focus previous element within view
    FocusPrev,

    /// Add files to share list
    AddFiles(Vec<PathBuf>),
    /// Remove file at index from share list
    RemoveFile(usize),
    /// Toggle selection of file at index
    ToggleFile(usize),
    /// Open file browser
    OpenFileBrowser,
    /// Close file browser
    CloseFileBrowser,
    /// Start share session
    StartShare,
    /// Cancel active share session
    CancelShare,
    /// Regenerate share code (cancel and restart with new code)
    RegenerateCode,
    /// Focus next share option
    NextShareOption,
    /// Focus previous share option
    PrevShareOption,
    /// Toggle boolean share option (PIN, Approval, Compress)
    ToggleShareOption,
    /// Cycle expire option forward
    CycleExpireForward,
    /// Cycle expire option backward
    CycleExpireBackward,

    /// Update code input buffer
    UpdateCodeInput(String),
    /// Update IP input buffer (legacy, for string-based input)
    UpdateIpInput(String),
    /// Append a character to the current IP segment
    IpSegmentAppend(char),
    /// Delete the last character from the current IP segment
    IpSegmentBackspace,
    /// Move to the next IP segment
    IpCursorNext,
    /// Move to the previous IP segment
    IpCursorPrev,
    /// Clear all IP input
    IpClear,
    /// Select trusted device at index
    SelectDevice(usize),
    /// Switch receive input mode
    SwitchReceiveMode(ReceiveInputMode),
    /// Start receive with current inputs
    StartReceive,
    /// Accept incoming transfer
    AcceptTransfer,
    /// Decline incoming transfer
    DeclineTransfer,
    /// Cancel the receive session (during search/connect phase)
    CancelReceive,

    /// Share current clipboard content
    ShareClipboard,
    /// Receive clipboard content
    ReceiveClipboard,
    /// Start clipboard sync session
    StartClipboardSync,
    /// Stop clipboard sync session
    StopClipboardSync,
    /// Cancel ongoing clipboard operation (sharing/receiving/starting sync)
    CancelClipboardOperation,
    /// Update clipboard code input
    UpdateClipboardCodeInput(String),
    /// Refresh clipboard content preview
    RefreshClipboard,

    /// Start hosting sync session
    StartSyncHost,
    /// Join sync session with code
    JoinSync,
    /// Stop sync session
    StopSync,
    /// Update sync code input
    UpdateSyncCodeInput(String),
    /// Toggle sync deletions option
    ToggleSyncDeletions,
    /// Toggle follow symlinks option
    ToggleFollowSymlinks,
    /// Focus next sync option
    NextSyncOption,
    /// Focus previous sync option
    PrevSyncOption,
    /// Toggle current sync option
    ToggleSyncOption,
    /// Add exclude pattern
    AddExcludePattern(String),
    /// Remove exclude pattern
    RemoveExcludePattern(usize),
    /// Start adding a new exclude pattern (enter edit mode)
    StartAddExcludePattern,
    /// Update the pattern input buffer
    UpdatePatternInput(String),
    /// Confirm adding the exclude pattern
    ConfirmAddExcludePattern,
    /// Cancel adding exclude pattern
    CancelAddExcludePattern,
    /// Focus on the exclude patterns section
    FocusExcludePatterns,
    /// Open directory browser for sync
    OpenSyncDirectoryBrowser,
    /// Close directory browser for sync
    CloseSyncDirectoryBrowser,
    /// Scroll sync events up
    ScrollSyncEventsUp,
    /// Scroll sync events down
    ScrollSyncEventsDown,

    /// Select device by index
    SelectDeviceIndex(usize),
    /// Cycle trust level for selected device
    CycleTrustLevel,
    /// Start editing trust level
    StartEditTrustLevel,
    /// Confirm trust level change
    ConfirmTrustLevel,
    /// Cancel trust level edit
    CancelTrustLevelEdit,
    /// Request device removal
    RequestRemoveDevice,
    /// Confirm device removal
    ConfirmRemoveDevice,
    /// Cancel device removal
    CancelRemoveDevice,
    /// Send files to selected device
    SendToDevice,
    /// Refresh devices list
    RefreshDevices,

    /// Select history entry by index
    SelectHistoryIndex(usize),
    /// View details of selected history entry
    ViewHistoryDetails,
    /// Retry failed transfer
    RetryTransfer,
    /// Open output directory of selected transfer
    OpenTransferDirectory,
    /// Request clear history
    RequestClearHistory,
    /// Confirm clear history
    ConfirmClearHistory,
    /// Cancel clear history
    CancelClearHistory,
    /// Refresh history list
    RefreshHistory,

    /// Select config section
    SelectConfigSection(ConfigSection),
    /// Select config section by index
    SelectConfigSectionIndex(usize),
    /// Select setting by index within current section
    SelectConfigSetting(usize),
    /// Start editing current setting
    StartEditSetting,
    /// Update edit buffer
    UpdateEditBuffer(String),
    /// Confirm edit (apply to pending changes)
    ConfirmEdit,
    /// Cancel edit
    CancelEdit,
    /// Toggle boolean setting
    ToggleConfigSetting,
    /// Cycle enum setting (e.g., theme, compression mode)
    CycleConfigSetting,
    /// Request save config
    RequestSaveConfig,
    /// Confirm save config
    ConfirmSaveConfig,
    /// Cancel save config
    CancelSaveConfig,
    /// Request revert changes
    RequestRevertConfig,
    /// Confirm revert changes
    ConfirmRevertConfig,
    /// Cancel revert changes
    CancelRevertConfig,
    /// Refresh config from file
    RefreshConfig,

    /// Expand transfers panel
    ExpandTransfers,
    /// Collapse transfers panel
    CollapseTransfers,
    /// Cancel transfer by ID
    CancelTransfer(uuid::Uuid),

    /// Toggle log panel visibility
    ToggleLog,
    /// Toggle help overlay visibility
    ToggleHelp,
    /// Scroll log up
    ScrollLogUp,
    /// Scroll log down
    ScrollLogDown,
    /// Clear log entries
    ClearLog,
    /// Tick for animations (called on timer)
    Tick,

    /// Move up in file browser
    FileBrowserUp,
    /// Move down in file browser
    FileBrowserDown,
    /// Enter directory or select file
    FileBrowserEnter,
    /// Go to parent directory
    FileBrowserBack,
    /// Toggle file selection
    FileBrowserToggleSelection,
    /// Toggle hidden files visibility
    FileBrowserToggleHidden,
    /// Confirm file selection
    FileBrowserConfirm,
    /// Set search filter
    FileBrowserSearch(String),

    /// Move selection up in a list
    ListUp,
    /// Move selection down in a list
    ListDown,
    /// Page up in a list
    ListPageUp,
    /// Page down in a list
    ListPageDown,
    /// Go to first item
    ListFirst,
    /// Go to last item
    ListLast,

    /// Quit application
    Quit,
    /// Refresh state
    Refresh,
    /// Show help
    Help,
    /// No action (key not handled)
    None,
}

impl Action {
    /// Check if this action should trigger a state refresh.
    pub const fn requires_refresh(&self) -> bool {
        matches!(
            self,
            Self::StartShare
                | Self::CancelShare
                | Self::StartReceive
                | Self::ShareClipboard
                | Self::ReceiveClipboard
                | Self::StartClipboardSync
                | Self::StopClipboardSync
                | Self::CancelClipboardOperation
                | Self::RefreshClipboard
                | Self::StartSyncHost
                | Self::JoinSync
                | Self::StopSync
                | Self::CancelTransfer(_)
                | Self::ConfirmRemoveDevice
                | Self::ConfirmTrustLevel
                | Self::RefreshDevices
                | Self::ConfirmClearHistory
                | Self::RefreshHistory
                | Self::ConfirmSaveConfig
                | Self::ConfirmRevertConfig
                | Self::RefreshConfig
                | Self::Refresh
        )
    }

    /// Check if this action should quit the application.
    pub const fn is_quit(&self) -> bool {
        matches!(self, Self::Quit)
    }
}
