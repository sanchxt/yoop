//! TUI application state types.

use std::collections::HashSet;
use std::path::PathBuf;

/// Active view in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    /// Share files with other devices
    #[default]
    Share,
    /// Receive files using a share code
    Receive,
    /// Clipboard sharing and sync
    Clipboard,
    /// Directory synchronization
    Sync,
    /// Trusted devices management
    Devices,
    /// Transfer history
    History,
    /// Configuration settings
    Config,
}

impl View {
    /// Get all views in order for navigation.
    pub const fn all() -> [View; 7] {
        [
            View::Share,
            View::Receive,
            View::Clipboard,
            View::Sync,
            View::Devices,
            View::History,
            View::Config,
        ]
    }

    /// Get the display name for this view.
    pub const fn name(&self) -> &'static str {
        match self {
            View::Share => "Share",
            View::Receive => "Receive",
            View::Clipboard => "Clipboard",
            View::Sync => "Sync",
            View::Devices => "Devices",
            View::History => "History",
            View::Config => "Config",
        }
    }

    /// Get the full display name for this view (used in help overlay).
    pub const fn display_name(&self) -> &'static str {
        match self {
            View::Share => "Share",
            View::Receive => "Receive",
            View::Clipboard => "Clipboard",
            View::Sync => "Sync",
            View::Devices => "Devices",
            View::History => "History",
            View::Config => "Config",
        }
    }

    /// Get the single-character shortcut for this view.
    pub const fn shortcut(&self) -> char {
        match self {
            View::Share => 'S',
            View::Receive => 'R',
            View::Clipboard => 'C',
            View::Sync => 'Y',
            View::Devices => 'D',
            View::History => 'H',
            View::Config => 'G',
        }
    }

    /// Create a view from its shortcut character.
    pub fn from_shortcut(c: char) -> Option<View> {
        match c.to_ascii_uppercase() {
            'S' => Some(View::Share),
            'R' => Some(View::Receive),
            'C' => Some(View::Clipboard),
            'Y' => Some(View::Sync),
            'D' => Some(View::Devices),
            'H' => Some(View::History),
            'G' => Some(View::Config),
            _ => None,
        }
    }

    /// Get the next view in the cycle (wraps around).
    #[must_use]
    pub const fn next(self) -> View {
        match self {
            View::Share => View::Receive,
            View::Receive => View::Clipboard,
            View::Clipboard => View::Sync,
            View::Sync => View::Devices,
            View::Devices => View::History,
            View::History => View::Config,
            View::Config => View::Share,
        }
    }

    /// Get the previous view in the cycle (wraps around).
    #[must_use]
    pub const fn prev(self) -> View {
        match self {
            View::Share => View::Config,
            View::Receive => View::Share,
            View::Clipboard => View::Receive,
            View::Sync => View::Clipboard,
            View::Devices => View::Sync,
            View::History => View::Devices,
            View::Config => View::History,
        }
    }
}

/// Main application state
#[derive(Debug)]
pub struct AppState {
    /// Current active view
    pub active_view: View,
    /// Is log panel visible
    pub log_visible: bool,
    /// Is transfers panel expanded
    pub transfers_expanded: bool,
    /// Is help overlay visible
    pub help_visible: bool,
    /// Share view state
    pub share: ShareState,
    /// Receive view state
    pub receive: ReceiveState,
    /// Clipboard view state
    pub clipboard: ClipboardState,
    /// Sync view state
    pub sync: SyncState,
    /// Devices view state
    pub devices: DevicesState,
    /// History view state
    pub history: HistoryState,
    /// Config view state
    pub config: ConfigState,
    /// Active transfers (from CLI or TUI)
    pub transfers: Vec<TransferSession>,
    /// Clipboard sync session
    pub clipboard_sync: Option<ClipboardSyncSession>,
    /// Log entries
    pub log: Vec<LogEntry>,
    /// Terminal size (width, height)
    pub size: (u16, u16),
    /// Spinner state for loading animations
    pub spinner: super::components::SpinnerState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_view: View::default(),
            log_visible: false,
            transfers_expanded: false,
            help_visible: false,
            share: ShareState::default(),
            receive: ReceiveState::default(),
            clipboard: ClipboardState::default(),
            sync: SyncState::default(),
            devices: DevicesState::default(),
            history: HistoryState::default(),
            config: ConfigState::default(),
            transfers: Vec::new(),
            clipboard_sync: None,
            log: Vec::new(),
            size: (80, 24),
            spinner: super::components::SpinnerState::new(),
        }
    }
}

/// Focus state within share view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShareFocus {
    /// File list is focused
    #[default]
    FileList,
    /// Options panel is focused
    Options,
}

impl ShareFocus {
    /// Move to the next focus area.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::FileList => Self::Options,
            Self::Options => Self::FileList,
        }
    }
}

/// Focus state within share options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShareOptionFocus {
    /// Expiration time option
    #[default]
    Expire,
    /// PIN requirement option
    Pin,
    /// Approval requirement option
    Approval,
    /// Compression option
    Compress,
}

impl ShareOptionFocus {
    /// Move to the next option (skips disabled options).
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Expire => Self::Compress,
            Self::Pin => Self::Compress,
            Self::Approval => Self::Compress,
            Self::Compress => Self::Expire,
        }
    }

    /// Move to the previous option (skips disabled options).
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Expire => Self::Compress,
            Self::Pin => Self::Expire,
            Self::Approval => Self::Expire,
            Self::Compress => Self::Expire,
        }
    }
}

/// State for share view
#[derive(Debug, Default)]
pub struct ShareState {
    /// Selected files for sharing
    pub selected_files: Vec<PathBuf>,
    /// File browser state (if open)
    pub file_browser: Option<FileBrowserState>,
    /// Share options
    pub options: ShareOptions,
    /// Active share session (if any)
    pub active_session: Option<ShareSession>,
    /// Selected file index in the list
    pub selected_index: usize,
    /// Current focus within the view
    pub focus: ShareFocus,
    /// Which option is currently focused (when Options panel is focused)
    pub option_focus: Option<ShareOptionFocus>,
}

/// Share options
#[derive(Debug, Clone)]
pub struct ShareOptions {
    /// Expiration time string (e.g., "5m")
    pub expire: String,
    /// Require PIN for extra security
    pub require_pin: bool,
    /// Require manual approval of receiver
    pub require_approval: bool,
    /// Enable compression
    pub compress: bool,
    /// Compression level (1-3)
    pub compression_level: u8,
}

impl Default for ShareOptions {
    fn default() -> Self {
        Self {
            expire: "5m".to_string(),
            require_pin: false,
            require_approval: false,
            compress: true,
            compression_level: 1,
        }
    }
}

/// Active share session
#[derive(Debug, Clone)]
pub struct ShareSession {
    /// Session ID
    pub id: uuid::Uuid,
    /// Share code
    pub code: String,
    /// Files being shared
    pub files: Vec<String>,
    /// Total size in bytes
    pub total_size: u64,
    /// When the session started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the session expires
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Connected peer name (if any)
    pub peer_name: Option<String>,
    /// Transfer progress
    pub progress: TransferProgress,
}

/// Segmented IPv4 address input state.
#[derive(Debug, Clone, Default)]
pub struct IpInputState {
    /// Four IPv4 octets (each 0-255)
    pub octets: [String; 4],
    /// Port number (1-65535)
    pub port: String,
    /// Currently focused segment (0-3 for octets, 4 for port)
    pub cursor_position: usize,
}

impl IpInputState {
    /// Convert the input state to an address string for connection.
    /// Returns format "IP:PORT" or just "IP" if port is empty.
    pub fn to_address_string(&self) -> String {
        let ip = self
            .octets
            .iter()
            .map(|o| if o.is_empty() { "0" } else { o.as_str() })
            .collect::<Vec<_>>()
            .join(".");

        if self.port.is_empty() {
            ip
        } else {
            format!("{}:{}", ip, self.port)
        }
    }

    /// Check if all four octets are filled with valid values.
    pub fn is_complete(&self) -> bool {
        self.octets
            .iter()
            .all(|o| !o.is_empty() && Self::is_valid_octet(o))
    }

    /// Check if the entire address (octets + optional port) is valid.
    pub fn is_valid(&self) -> bool {
        if !self
            .octets
            .iter()
            .all(|o| o.is_empty() || Self::is_valid_octet(o))
        {
            return false;
        }
        if !self.port.is_empty() && !Self::is_valid_port(&self.port) {
            return false;
        }
        true
    }

    /// Check if a single octet string is valid (0-255).
    pub fn is_valid_octet(s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        match s.parse::<u16>() {
            Ok(n) => n <= 255,
            Err(_) => false,
        }
    }

    /// Check if a port string is valid (1-65535).
    pub fn is_valid_port(s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        match s.parse::<u16>() {
            Ok(n) => n >= 1,
            Err(_) => false,
        }
    }

    /// Clear all input.
    pub fn clear(&mut self) {
        self.octets = [String::new(), String::new(), String::new(), String::new()];
        self.port = String::new();
        self.cursor_position = 0;
    }

    /// Get the current segment value.
    pub fn current_segment(&self) -> &str {
        if self.cursor_position < 4 {
            &self.octets[self.cursor_position]
        } else {
            &self.port
        }
    }

    /// Get a mutable reference to the current segment.
    pub fn current_segment_mut(&mut self) -> &mut String {
        if self.cursor_position < 4 {
            &mut self.octets[self.cursor_position]
        } else {
            &mut self.port
        }
    }

    /// Move cursor to next segment, returns true if moved.
    pub fn cursor_next(&mut self) -> bool {
        if self.cursor_position < 4 {
            self.cursor_position += 1;
            true
        } else {
            false
        }
    }

    /// Move cursor to previous segment, returns true if moved.
    pub fn cursor_prev(&mut self) -> bool {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            true
        } else {
            false
        }
    }

    /// Get max length for current segment (3 for octets, 5 for port).
    pub fn current_max_len(&self) -> usize {
        if self.cursor_position < 4 {
            3
        } else {
            5
        }
    }

    /// Check if current segment is at max length.
    pub fn current_at_max(&self) -> bool {
        self.current_segment().len() >= self.current_max_len()
    }

    /// Check if current segment is an octet (vs port).
    pub fn is_octet_segment(&self) -> bool {
        self.cursor_position < 4
    }

    /// Check if any input has been entered.
    pub fn is_empty(&self) -> bool {
        self.octets.iter().all(String::is_empty) && self.port.is_empty()
    }
}

/// State for receive view
#[derive(Debug, Default)]
pub struct ReceiveState {
    /// Code input buffer
    pub code_input: String,
    /// Selected input mode
    pub input_mode: ReceiveInputMode,
    /// Direct IP input (segmented)
    pub ip_input: IpInputState,
    /// Selected trusted device index
    pub selected_device: usize,
    /// Output directory
    pub output_dir: Option<PathBuf>,
    /// Active receive session (if any)
    pub active_session: Option<ReceiveSession>,
    /// Whether we're in the connecting/searching phase (background task running)
    pub is_connecting: bool,
}

/// Active receive session
#[derive(Debug, Clone)]
pub struct ReceiveSession {
    /// Session ID
    pub id: uuid::Uuid,
    /// Sender name
    pub sender_name: String,
    /// Sender address
    pub sender_addr: String,
    /// Files to receive
    pub files: Vec<ReceiveFile>,
    /// Total size in bytes
    pub total_size: u64,
    /// Transfer progress
    pub progress: TransferProgress,
    /// Current file being transferred
    pub current_file: String,
    /// Session status
    pub status: ReceiveSessionStatus,
}

/// File info for receive
#[derive(Debug, Clone)]
pub struct ReceiveFile {
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// MIME type (if known)
    pub mime_type: Option<String>,
    /// Is this a directory
    pub is_directory: bool,
}

/// Receive session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReceiveSessionStatus {
    /// Waiting for user to accept/decline
    #[default]
    Pending,
    /// Transfer in progress
    Transferring,
    /// Transfer completed
    Completed,
    /// Transfer failed
    Failed,
    /// Transfer cancelled
    Cancelled,
}

/// Input mode for receive view
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveInputMode {
    /// Enter a 4-character share code
    #[default]
    Code,
    /// Select from trusted devices
    TrustedDevice,
    /// Enter IP address directly
    DirectIp,
}

/// Focus state within clipboard view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipboardFocus {
    /// Preview panel is focused
    #[default]
    Preview,
    /// Actions panel is focused
    Actions,
    /// Sync status is focused
    SyncStatus,
}

/// State for clipboard view
#[derive(Debug, Default)]
pub struct ClipboardState {
    /// Current clipboard preview (truncated)
    pub preview: Option<String>,
    /// Clipboard content type
    pub content_type: Option<ClipboardContentType>,
    /// Clipboard content size
    pub content_size: Option<usize>,
    /// Current focus within the view
    pub focus: ClipboardFocus,
    /// Code input for receiving clipboard
    pub code_input: String,
    /// Whether a clipboard operation is in progress
    pub operation_in_progress: Option<ClipboardOperation>,
    /// Status message
    pub status_message: Option<String>,
}

/// Active clipboard operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOperation {
    /// Sharing clipboard
    Sharing,
    /// Receiving clipboard
    Receiving,
    /// Starting sync
    StartingSync,
}

/// Result from an async clipboard task.
#[derive(Debug)]
pub enum ClipboardTaskResult {
    /// Share operation completed successfully
    ShareComplete,
    /// Share operation failed
    ShareFailed(String),
    /// Receive operation completed successfully
    ReceiveComplete,
    /// Receive operation failed
    ReceiveFailed(String),
    /// Sync host connected to peer
    SyncHostConnected {
        /// Peer device name
        peer_name: String,
        /// Peer address
        peer_addr: String,
    },
    /// Sync host failed to connect
    SyncHostFailed(String),
}

/// Clipboard content type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardContentType {
    /// Text content
    Text,
    /// Image content
    Image,
}

/// Focus state within sync view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncFocus {
    /// Directory selection is focused
    #[default]
    Directory,
    /// Options panel is focused
    Options,
    /// Code input for joining sync is focused
    CodeInput,
    /// Exclude patterns section is focused
    ExcludePatterns,
    /// Events list is focused (only visible during active session)
    Events,
}

impl SyncFocus {
    /// Move to the next focus area.
    /// When `has_active_session` is true, includes Events in the cycle.
    /// When false, skips Events (since it's not visible in setup mode).
    #[must_use]
    pub fn next(self, has_active_session: bool) -> Self {
        match self {
            Self::Directory => Self::Options,
            Self::Options => Self::CodeInput,
            Self::CodeInput => Self::ExcludePatterns,
            Self::ExcludePatterns => {
                if has_active_session {
                    Self::Events
                } else {
                    Self::Directory
                }
            }
            Self::Events => Self::Directory,
        }
    }

    /// Move to the previous focus area.
    /// When `has_active_session` is true, includes Events in the cycle.
    /// When false, skips Events (since it's not visible in setup mode).
    #[must_use]
    pub fn prev(self, has_active_session: bool) -> Self {
        match self {
            Self::Directory => {
                if has_active_session {
                    Self::Events
                } else {
                    Self::ExcludePatterns
                }
            }
            Self::Options => Self::Directory,
            Self::CodeInput => Self::Options,
            Self::ExcludePatterns => Self::CodeInput,
            Self::Events => Self::ExcludePatterns,
        }
    }
}

/// Focus state within sync options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncOptionFocus {
    /// Sync deletions option
    #[default]
    SyncDeletions,
    /// Follow symlinks option
    FollowSymlinks,
}

impl SyncOptionFocus {
    /// Move to the next option.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::SyncDeletions => Self::FollowSymlinks,
            Self::FollowSymlinks => Self::SyncDeletions,
        }
    }

    /// Move to the previous option.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::SyncDeletions => Self::FollowSymlinks,
            Self::FollowSymlinks => Self::SyncDeletions,
        }
    }
}

/// State for sync view
#[derive(Debug)]
pub struct SyncState {
    /// Selected directory
    pub directory: Option<PathBuf>,
    /// Exclude patterns
    pub exclude_patterns: Vec<String>,
    /// Active sync session
    pub active_session: Option<SyncSession>,
    /// Current focus within the view
    pub focus: SyncFocus,
    /// Which option is currently focused (when Options panel is focused)
    pub option_focus: Option<SyncOptionFocus>,
    /// Code input for joining sync
    pub code_input: String,
    /// File browser state (if open)
    pub file_browser: Option<FileBrowserState>,
    /// Sync events for display
    pub events: Vec<SyncEventEntry>,
    /// Sync stats
    pub stats: SyncStats,
    /// Whether to sync deletions
    pub sync_deletions: bool,
    /// Follow symlinks
    pub follow_symlinks: bool,
    /// Whether currently editing an exclude pattern
    pub editing_pattern: bool,
    /// Input buffer for new exclude pattern
    pub pattern_input: String,
    /// Currently selected pattern index (for removal)
    pub selected_pattern_index: usize,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            directory: None,
            exclude_patterns: Vec::new(),
            active_session: None,
            focus: SyncFocus::default(),
            option_focus: None,
            code_input: String::new(),
            file_browser: None,
            events: Vec::new(),
            stats: SyncStats::default(),
            sync_deletions: true,
            follow_symlinks: false,
            editing_pattern: false,
            pattern_input: String::new(),
            selected_pattern_index: 0,
        }
    }
}

/// A sync event entry for display.
#[derive(Debug, Clone)]
pub struct SyncEventEntry {
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event type
    pub event_type: SyncEventType,
    /// Associated path (if any)
    pub path: Option<String>,
    /// Message
    pub message: Option<String>,
}

/// Type of sync event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncEventType {
    /// Connected to peer
    Connected,
    /// Index exchanged
    IndexExchanged,
    /// File being sent
    FileSending,
    /// File sent
    FileSent,
    /// File being received
    FileReceiving,
    /// File received
    FileReceived,
    /// File deleted
    FileDeleted,
    /// Conflict occurred
    Conflict,
    /// Error occurred
    Error,
}

/// Sync statistics.
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Files sent
    pub files_sent: u64,
    /// Files received
    pub files_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Number of conflicts
    pub conflicts: u64,
    /// Number of errors
    pub errors: u64,
}

/// Active sync session
#[derive(Debug, Clone)]
pub struct SyncSession {
    /// Session ID
    pub id: uuid::Uuid,
    /// Share code (if hosting)
    pub code: Option<String>,
    /// Peer name
    pub peer_name: Option<String>,
    /// Files synced
    pub files_synced: u64,
    /// Last sync event
    pub last_event: Option<String>,
}

/// Focus state within devices view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevicesFocus {
    /// Device list is focused
    #[default]
    DeviceList,
    /// Details panel is focused
    Details,
}

/// State for devices view
#[derive(Debug, Default)]
pub struct DevicesState {
    /// Currently selected device index
    pub selected_index: usize,
    /// Current focus within the view
    pub focus: DevicesFocus,
    /// Whether editing trust level
    pub editing_trust_level: bool,
    /// Confirmation state for removal
    pub confirm_remove: bool,
    /// Scroll offset for device list
    pub scroll_offset: usize,
}

/// Focus state within history view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryFocus {
    /// History list is focused
    #[default]
    HistoryList,
    /// Details panel is focused
    Details,
}

/// State for history view
#[derive(Debug, Default)]
pub struct HistoryState {
    /// Currently selected history entry index
    pub selected_index: usize,
    /// Current focus within the view
    pub focus: HistoryFocus,
    /// Scroll offset for history list
    pub scroll_offset: usize,
    /// Confirmation state for clearing history
    pub confirm_clear: bool,
}

/// State for file browser component
#[derive(Debug)]
pub struct FileBrowserState {
    /// Current directory
    pub current_dir: PathBuf,
    /// Directory entries
    pub entries: Vec<DirEntry>,
    /// Selected index
    pub selected: usize,
    /// Scroll offset
    pub scroll: usize,
    /// Show hidden files
    pub show_hidden: bool,
    /// Search filter
    pub filter: Option<String>,
    /// Multi-select mode selections
    pub selections: HashSet<PathBuf>,
}

/// Directory entry for file browser
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Full path
    pub path: PathBuf,
    /// Is this a directory
    pub is_dir: bool,
    /// File size in bytes
    pub size: u64,
    /// Is this a hidden file
    pub is_hidden: bool,
}

/// Transfer session (shared with CLI)
#[derive(Debug, Clone)]
pub struct TransferSession {
    /// Session ID
    pub id: uuid::Uuid,
    /// Session type
    pub session_type: TransferType,
    /// Share code (if applicable)
    pub code: Option<String>,
    /// Peer name
    pub peer_name: Option<String>,
    /// Peer address
    pub peer_address: Option<String>,
    /// Files in transfer
    pub files: Vec<TransferFile>,
    /// Overall progress
    pub progress: TransferProgress,
    /// When the session started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the session expires (if applicable)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Process ID (for CLI coordination)
    pub pid: u32,
}

/// Transfer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    /// Sharing files (waiting for receiver)
    Share,
    /// Receiving files from a share
    Receive,
    /// Sending to a trusted device
    Send,
    /// Directory sync
    Sync,
}

/// Individual file in a transfer
#[derive(Debug, Clone)]
pub struct TransferFile {
    /// File name
    pub name: String,
    /// Total size in bytes
    pub size: u64,
    /// Bytes transferred
    pub transferred: u64,
    /// File status
    pub status: FileStatus,
}

/// Status of a file in transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Waiting to start
    Pending,
    /// Currently transferring
    Transferring,
    /// Successfully completed
    Completed,
    /// Transfer failed
    Failed,
}

/// Transfer progress
#[derive(Debug, Clone, Default)]
pub struct TransferProgress {
    /// Bytes transferred
    pub transferred: u64,
    /// Total bytes
    pub total: u64,
    /// Speed in bytes per second
    pub speed_bps: u64,
}

impl TransferProgress {
    /// Calculate percentage complete.
    #[allow(clippy::cast_precision_loss)]
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.transferred as f64 / self.total as f64) * 100.0
        }
    }
}

/// Clipboard sync session
#[derive(Debug, Clone)]
pub struct ClipboardSyncSession {
    /// Peer name
    pub peer_name: String,
    /// Peer address
    pub peer_address: String,
    /// Items sent
    pub items_sent: u64,
    /// Items received
    pub items_received: u64,
    /// When the session started
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Log level
    pub level: LogLevel,
    /// Message
    pub message: String,
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Information message
    Info,
    /// Warning message
    Warn,
    /// Error message
    Error,
}

impl LogLevel {
    /// Get the display string for this level.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// Focus state within config view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigFocus {
    /// Section list is focused
    #[default]
    SectionList,
    /// Settings panel is focused
    Settings,
}

/// Configuration section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigSection {
    /// General settings
    #[default]
    General,
    /// Network settings
    Network,
    /// Transfer settings
    Transfer,
    /// Security settings
    Security,
    /// Preview settings
    Preview,
    /// History settings
    History,
    /// Trust settings
    Trust,
    /// Web interface settings
    Web,
    /// UI settings
    Ui,
    /// Update settings
    Update,
}

impl ConfigSection {
    /// Get all sections in order.
    pub const fn all() -> [ConfigSection; 10] {
        [
            ConfigSection::General,
            ConfigSection::Network,
            ConfigSection::Transfer,
            ConfigSection::Security,
            ConfigSection::Preview,
            ConfigSection::History,
            ConfigSection::Trust,
            ConfigSection::Web,
            ConfigSection::Ui,
            ConfigSection::Update,
        ]
    }

    /// Get the display name for this section.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Network => "Network",
            Self::Transfer => "Transfer",
            Self::Security => "Security",
            Self::Preview => "Preview",
            Self::History => "History",
            Self::Trust => "Trust",
            Self::Web => "Web",
            Self::Ui => "UI",
            Self::Update => "Update",
        }
    }

    /// Get the description for this section.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::General => "Device name, default expiration, output directory",
            Self::Network => "Port settings, interface, IPv6 support",
            Self::Transfer => "Chunk size, compression, bandwidth limit",
            Self::Security => "PIN, approval, TLS, rate limiting",
            Self::Preview => "Image thumbnails, text preview settings",
            Self::History => "Transfer history retention settings",
            Self::Trust => "Trusted devices, auto-prompt, default level",
            Self::Web => "Web interface server settings",
            Self::Ui => "Theme, QR codes, notifications, sound",
            Self::Update => "Auto-check, interval, package manager",
        }
    }
}

/// State for config view.
#[derive(Debug, Default)]
pub struct ConfigState {
    /// Currently selected section index
    pub selected_section: usize,
    /// Current focus within the view
    pub focus: ConfigFocus,
    /// Currently selected setting index within section
    pub selected_setting: usize,
    /// Whether currently editing a value
    pub editing: bool,
    /// Edit buffer for text values
    pub edit_buffer: String,
    /// Whether there are unsaved changes
    pub has_changes: bool,
    /// Status message
    pub status_message: Option<String>,
    /// Confirmation state for saving
    pub confirm_save: bool,
    /// Confirmation state for reverting
    pub confirm_revert: bool,
}

impl ConfigState {
    /// Get the currently selected section.
    pub fn current_section(&self) -> ConfigSection {
        ConfigSection::all()
            .get(self.selected_section)
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_shortcuts() {
        assert_eq!(View::from_shortcut('S'), Some(View::Share));
        assert_eq!(View::from_shortcut('s'), Some(View::Share));
        assert_eq!(View::from_shortcut('R'), Some(View::Receive));
        assert_eq!(View::from_shortcut('X'), None);
    }

    #[test]
    fn test_view_next() {
        assert_eq!(View::Share.next(), View::Receive);
        assert_eq!(View::Receive.next(), View::Clipboard);
        assert_eq!(View::Clipboard.next(), View::Sync);
        assert_eq!(View::Sync.next(), View::Devices);
        assert_eq!(View::Devices.next(), View::History);
        assert_eq!(View::History.next(), View::Config);
        assert_eq!(View::Config.next(), View::Share);
    }

    #[test]
    fn test_view_prev() {
        assert_eq!(View::Share.prev(), View::Config);
        assert_eq!(View::Receive.prev(), View::Share);
        assert_eq!(View::Clipboard.prev(), View::Receive);
        assert_eq!(View::Sync.prev(), View::Clipboard);
        assert_eq!(View::Devices.prev(), View::Sync);
        assert_eq!(View::History.prev(), View::Devices);
        assert_eq!(View::Config.prev(), View::History);
    }

    #[test]
    fn test_view_next_prev_cycle() {
        for view in View::all() {
            assert_eq!(view.next().prev(), view);
            assert_eq!(view.prev().next(), view);
        }
    }

    #[test]
    fn test_transfer_progress_percentage() {
        let progress = TransferProgress {
            transferred: 50,
            total: 100,
            speed_bps: 0,
        };
        assert!((progress.percentage() - 50.0).abs() < f64::EPSILON);

        let empty = TransferProgress::default();
        assert!((empty.percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_section_all() {
        let sections = ConfigSection::all();
        assert_eq!(sections.len(), 10);
        assert_eq!(sections[0], ConfigSection::General);
        assert_eq!(sections[4], ConfigSection::Preview);
        assert_eq!(sections[5], ConfigSection::History);
        assert_eq!(sections[7], ConfigSection::Web);
        assert_eq!(sections[9], ConfigSection::Update);
    }

    #[test]
    fn test_config_section_name() {
        assert_eq!(ConfigSection::General.name(), "General");
        assert_eq!(ConfigSection::Network.name(), "Network");
        assert_eq!(ConfigSection::Preview.name(), "Preview");
        assert_eq!(ConfigSection::History.name(), "History");
        assert_eq!(ConfigSection::Web.name(), "Web");
        assert_eq!(ConfigSection::Ui.name(), "UI");
    }

    #[test]
    fn test_config_section_description() {
        assert!(!ConfigSection::General.description().is_empty());
        assert!(!ConfigSection::Security.description().is_empty());
    }

    #[test]
    fn test_config_state_default() {
        let state = ConfigState::default();
        assert_eq!(state.selected_section, 0);
        assert_eq!(state.focus, ConfigFocus::SectionList);
        assert_eq!(state.selected_setting, 0);
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
        assert!(!state.has_changes);
        assert!(state.status_message.is_none());
        assert!(!state.confirm_save);
        assert!(!state.confirm_revert);
    }

    #[test]
    fn test_config_state_current_section() {
        let mut state = ConfigState::default();
        assert_eq!(state.current_section(), ConfigSection::General);

        state.selected_section = 3;
        assert_eq!(state.current_section(), ConfigSection::Security);

        state.selected_section = 100;
        assert_eq!(state.current_section(), ConfigSection::General);
    }

    #[test]
    fn test_config_focus_default() {
        let focus = ConfigFocus::default();
        assert_eq!(focus, ConfigFocus::SectionList);
    }

    #[test]
    fn test_ip_input_state_default() {
        let state = IpInputState::default();
        assert!(state.octets.iter().all(String::is_empty));
        assert!(state.port.is_empty());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_ip_input_state_to_address_string() {
        let mut state = IpInputState {
            octets: [
                "192".to_string(),
                "168".to_string(),
                "1".to_string(),
                "100".to_string(),
            ],
            ..Default::default()
        };
        assert_eq!(state.to_address_string(), "192.168.1.100");

        state.port = "52530".to_string();
        assert_eq!(state.to_address_string(), "192.168.1.100:52530");
    }

    #[test]
    fn test_ip_input_state_is_complete() {
        let mut state = IpInputState::default();
        assert!(!state.is_complete());

        state.octets = [
            "192".to_string(),
            "168".to_string(),
            "1".to_string(),
            String::new(),
        ];
        assert!(!state.is_complete());

        state.octets[3] = "100".to_string();
        assert!(state.is_complete());
    }

    #[test]
    fn test_ip_input_state_is_valid_octet() {
        assert!(IpInputState::is_valid_octet("0"));
        assert!(IpInputState::is_valid_octet("255"));
        assert!(IpInputState::is_valid_octet("192"));
        assert!(IpInputState::is_valid_octet(""));
        assert!(!IpInputState::is_valid_octet("256"));
        assert!(!IpInputState::is_valid_octet("999"));
        assert!(!IpInputState::is_valid_octet("abc"));
    }

    #[test]
    fn test_ip_input_state_is_valid_port() {
        assert!(IpInputState::is_valid_port("1"));
        assert!(IpInputState::is_valid_port("80"));
        assert!(IpInputState::is_valid_port("65535"));
        assert!(IpInputState::is_valid_port(""));
        assert!(!IpInputState::is_valid_port("0"));
        assert!(!IpInputState::is_valid_port("abc"));
    }

    #[test]
    fn test_ip_input_state_cursor_navigation() {
        let mut state = IpInputState::default();
        assert_eq!(state.cursor_position, 0);

        assert!(state.cursor_next());
        assert_eq!(state.cursor_position, 1);

        assert!(state.cursor_next());
        assert!(state.cursor_next());
        assert!(state.cursor_next());
        assert_eq!(state.cursor_position, 4);

        assert!(!state.cursor_next());
        assert_eq!(state.cursor_position, 4);

        assert!(state.cursor_prev());
        assert_eq!(state.cursor_position, 3);

        state.cursor_position = 0;
        assert!(!state.cursor_prev());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_ip_input_state_clear() {
        let mut state = IpInputState {
            octets: [
                "192".to_string(),
                "168".to_string(),
                "1".to_string(),
                "100".to_string(),
            ],
            port: "52530".to_string(),
            cursor_position: 3,
        };

        state.clear();

        assert!(state.octets.iter().all(String::is_empty));
        assert!(state.port.is_empty());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_ip_input_state_is_empty() {
        let mut state = IpInputState::default();
        assert!(state.is_empty());

        state.octets[0] = "1".to_string();
        assert!(!state.is_empty());

        state.octets[0].clear();
        state.port = "80".to_string();
        assert!(!state.is_empty());
    }

    #[test]
    fn test_sync_focus_next_with_session() {
        assert_eq!(SyncFocus::Directory.next(true), SyncFocus::Options);
        assert_eq!(SyncFocus::Options.next(true), SyncFocus::CodeInput);
        assert_eq!(SyncFocus::CodeInput.next(true), SyncFocus::ExcludePatterns);
        assert_eq!(SyncFocus::ExcludePatterns.next(true), SyncFocus::Events);
        assert_eq!(SyncFocus::Events.next(true), SyncFocus::Directory);
    }

    #[test]
    fn test_sync_focus_next_without_session() {
        assert_eq!(SyncFocus::Directory.next(false), SyncFocus::Options);
        assert_eq!(SyncFocus::Options.next(false), SyncFocus::CodeInput);
        assert_eq!(SyncFocus::CodeInput.next(false), SyncFocus::ExcludePatterns);
        assert_eq!(SyncFocus::ExcludePatterns.next(false), SyncFocus::Directory);
    }

    #[test]
    fn test_sync_focus_prev_with_session() {
        assert_eq!(SyncFocus::Directory.prev(true), SyncFocus::Events);
        assert_eq!(SyncFocus::Options.prev(true), SyncFocus::Directory);
        assert_eq!(SyncFocus::CodeInput.prev(true), SyncFocus::Options);
        assert_eq!(SyncFocus::ExcludePatterns.prev(true), SyncFocus::CodeInput);
        assert_eq!(SyncFocus::Events.prev(true), SyncFocus::ExcludePatterns);
    }

    #[test]
    fn test_sync_focus_prev_without_session() {
        assert_eq!(SyncFocus::Directory.prev(false), SyncFocus::ExcludePatterns);
        assert_eq!(SyncFocus::Options.prev(false), SyncFocus::Directory);
        assert_eq!(SyncFocus::CodeInput.prev(false), SyncFocus::Options);
        assert_eq!(SyncFocus::ExcludePatterns.prev(false), SyncFocus::CodeInput);
    }

    #[test]
    fn test_sync_focus_next_prev_cycle() {
        let focuses_with_session = [
            SyncFocus::Directory,
            SyncFocus::Options,
            SyncFocus::CodeInput,
            SyncFocus::ExcludePatterns,
            SyncFocus::Events,
        ];
        for focus in focuses_with_session {
            assert_eq!(focus.next(true).prev(true), focus);
            assert_eq!(focus.prev(true).next(true), focus);
        }

        let focuses_without_session = [
            SyncFocus::Directory,
            SyncFocus::Options,
            SyncFocus::CodeInput,
            SyncFocus::ExcludePatterns,
        ];
        for focus in focuses_without_session {
            assert_eq!(focus.next(false).prev(false), focus);
            assert_eq!(focus.prev(false).next(false), focus);
        }
    }

    #[test]
    fn test_sync_option_focus_next() {
        assert_eq!(
            SyncOptionFocus::SyncDeletions.next(),
            SyncOptionFocus::FollowSymlinks
        );
        assert_eq!(
            SyncOptionFocus::FollowSymlinks.next(),
            SyncOptionFocus::SyncDeletions
        );
    }

    #[test]
    fn test_sync_option_focus_prev() {
        assert_eq!(
            SyncOptionFocus::SyncDeletions.prev(),
            SyncOptionFocus::FollowSymlinks
        );
        assert_eq!(
            SyncOptionFocus::FollowSymlinks.prev(),
            SyncOptionFocus::SyncDeletions
        );
    }

    #[test]
    fn test_sync_option_focus_next_prev_cycle() {
        let focuses = [
            SyncOptionFocus::SyncDeletions,
            SyncOptionFocus::FollowSymlinks,
        ];
        for focus in focuses {
            assert_eq!(focus.next().prev(), focus);
            assert_eq!(focus.prev().next(), focus);
        }
    }

    #[test]
    fn test_sync_state_default_matches_cli() {
        let state = SyncState::default();
        assert!(
            state.sync_deletions,
            "sync_deletions should default to true to match CLI"
        );
        assert!(
            !state.follow_symlinks,
            "follow_symlinks should default to false"
        );
    }
}
