//! TUI application main loop.

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use tokio::sync::{mpsc, watch};

use super::action::Action;
use super::components::{
    init_browser_state, load_directories_only, load_directory, HelpOverlay, IncomingFile, NavMenu,
    SpinnerStyle, StatusBar,
};
use super::event::{map_key_event, EventHandler};
use super::layout::ComputedLayout;
use super::session::{SessionStateFile, StateWatcherHandle};
use super::state::{
    AppState, ClipboardSyncSession as TuiClipboardSync, FileBrowserState, FileStatus, LogEntry,
    LogLevel, ReceiveFile, ReceiveSession, ReceiveSessionStatus, ShareSession, SyncOptionFocus,
    TransferFile, TransferProgress, TransferSession, TransferType, View,
};
use super::theme::Theme;
use super::views::receive::ReceiveStatus;
use super::views::{self, ViewState};

/// TUI command-line arguments.
#[derive(Debug, Clone, Default)]
pub struct TuiArgs {
    /// Initial view to display
    pub view: Option<String>,
    /// Theme name
    pub theme: Option<String>,
}

/// Handle for controlling an active share session from the TUI.
struct ShareSessionHandle {
    /// Sender to signal cancellation to the background task
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Task handle for the share session
    task_handle: tokio::task::JoinHandle<()>,
}

/// Handle for controlling an active receive session from the TUI.
struct ReceiveSessionHandle {
    /// Sender to signal cancellation to the background task
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Task handle for the receive session
    task_handle: tokio::task::JoinHandle<()>,
}

/// Handle for controlling an active clipboard session from the TUI.
struct ClipboardSessionHandle {
    /// Task handle for the clipboard session (aborted on cancel)
    task_handle: tokio::task::JoinHandle<()>,
}

/// Handle for controlling an active sync session from the TUI.
struct SyncSessionHandle {
    /// Task handle for the sync session (aborted on cancel)
    task_handle: tokio::task::JoinHandle<()>,
}

/// Events sent from the sync background task to the main loop.
enum SyncHostEvent {
    /// Host session started, code generated, waiting for peer
    Started { code: String },
    /// Peer connected
    PeerConnected { peer_name: String },
    /// Sync session failed to start
    Failed(String),
}

/// Command sent from main loop to receive background task.
enum ReceiveCommand {
    /// Accept the transfer
    Accept,
    /// Decline the transfer
    Decline,
}

/// Events sent from the receive background task to the main loop.
enum ReceiveEvent {
    /// Connection established, ready for accept/decline
    Connected {
        sender_name: String,
        sender_addr: String,
        files: Vec<yoop_core::file::FileMetadata>,
        total_size: u64,
    },
    /// Connection failed
    ConnectionFailed(String),
    /// Transfer progress update
    TransferProgress(yoop_core::transfer::TransferProgress),
    /// Transfer completed successfully
    TransferComplete,
    /// Transfer failed
    TransferFailed(String),
    /// Transfer was cancelled
    TransferCancelled,
}

/// Main TUI application.
pub struct App {
    /// Terminal instance
    terminal: Terminal<CrosstermBackend<Stdout>>,
    /// Application state
    state: AppState,
    /// View state (holds view instances)
    views: ViewState,
    /// Event handler
    events: EventHandler,
    /// Theme
    theme: Theme,
    /// Whether the app should quit
    should_quit: bool,
    /// Active share session handle (for progress updates)
    share_progress_rx: Option<watch::Receiver<yoop_core::transfer::TransferProgress>>,
    /// Active receive session handle (for progress updates)
    receive_progress_rx: Option<watch::Receiver<yoop_core::transfer::TransferProgress>>,
    /// Session state file watcher
    state_watcher: StateWatcherHandle,
    /// Handle to the active share session for cancellation
    share_session_handle: Option<ShareSessionHandle>,
    /// Handle to the active receive session for cancellation
    receive_session_handle: Option<ReceiveSessionHandle>,
    /// Receiver for events from the receive background task
    receive_event_rx: Option<mpsc::UnboundedReceiver<ReceiveEvent>>,
    /// Sender for commands to the receive background task
    receive_command_tx: Option<mpsc::UnboundedSender<ReceiveCommand>>,
    /// Receiver for clipboard task results
    clipboard_task_rx: mpsc::UnboundedReceiver<super::state::ClipboardTaskResult>,
    /// Sender for clipboard task results (cloned into spawned tasks)
    clipboard_task_tx: mpsc::UnboundedSender<super::state::ClipboardTaskResult>,
    /// Handle to the active clipboard session for cancellation
    clipboard_session_handle: Option<ClipboardSessionHandle>,
    /// Receiver for sync host events from background task
    sync_host_event_rx: Option<mpsc::UnboundedReceiver<SyncHostEvent>>,
    /// Handle to the active sync session for cancellation
    sync_session_handle: Option<SyncSessionHandle>,
}

impl App {
    /// Create a new TUI application.
    pub fn new(args: TuiArgs) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        stdout.execute(EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let mut state = AppState::default();

        if let Some(view_name) = args.view {
            if let Some(view) = View::from_shortcut(view_name.chars().next().unwrap_or('S')) {
                state.active_view = view;
            }
        }

        let theme = args
            .theme
            .as_deref()
            .map(Theme::from_name)
            .unwrap_or_default();

        let events = EventHandler::new(Duration::from_millis(100));

        let views = ViewState::new();

        let state_watcher = StateWatcherHandle::start();

        let session_file = SessionStateFile::load_or_create();
        let (transfers, clipboard_sync) = load_cli_sessions(&session_file);
        state.transfers = transfers;
        state.clipboard_sync = clipboard_sync;

        let (clipboard_task_tx, clipboard_task_rx) = mpsc::unbounded_channel();

        Ok(Self {
            terminal,
            state,
            views,
            events,
            theme,
            should_quit: false,
            share_progress_rx: None,
            receive_progress_rx: None,
            state_watcher,
            share_session_handle: None,
            receive_session_handle: None,
            receive_event_rx: None,
            receive_command_tx: None,
            clipboard_task_rx,
            clipboard_task_tx,
            clipboard_session_handle: None,
            sync_host_event_rx: None,
            sync_session_handle: None,
        })
    }

    /// Run the TUI application.
    pub async fn run(&mut self) -> Result<()> {
        let size = self.terminal.size()?;
        self.state.size = (size.width, size.height);

        if self.state.active_view == View::Clipboard {
            self.refresh_clipboard_preview();
        }

        let tick_rate = Duration::from_millis(250);

        loop {
            self.state.spinner.tick(SpinnerStyle::Braille);

            self.poll_share_progress();

            self.poll_receive_progress();

            self.poll_receive_events();

            self.poll_clipboard_task_results();

            self.poll_sync_host_events();

            self.poll_session_state();

            self.draw()?;

            if let Ok(Some(event)) = tokio::time::timeout(tick_rate, self.events.next()).await {
                self.handle_event(&event).await;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Poll for session state file changes from CLI processes.
    fn poll_session_state(&mut self) {
        if let Some(event) = self.state_watcher.try_recv() {
            match event {
                super::session::watcher::StateEvent::Modified => {
                    let session_file = SessionStateFile::load_or_create();
                    let (transfers, clipboard_sync) = load_cli_sessions(&session_file);

                    self.merge_cli_sessions(transfers, clipboard_sync);
                }
                super::session::watcher::StateEvent::Deleted => {
                    self.state.transfers.retain(|t| t.pid == std::process::id());
                    self.state.clipboard_sync = None;
                }
                super::session::watcher::StateEvent::Error(e) => {
                    self.log_error(&format!("Session state watcher error: {}", e));
                }
            }
        }
    }

    /// Merge CLI sessions with TUI sessions.
    fn merge_cli_sessions(
        &mut self,
        cli_transfers: Vec<TransferSession>,
        cli_clipboard_sync: Option<TuiClipboardSync>,
    ) {
        let my_pid = std::process::id();

        self.state.transfers.retain(|t| t.pid == my_pid);

        for transfer in cli_transfers {
            if transfer.pid != my_pid {
                if let Some(existing) = self
                    .state
                    .transfers
                    .iter_mut()
                    .find(|t| t.id == transfer.id)
                {
                    existing.progress = transfer.progress;
                    existing.peer_name = transfer.peer_name;
                    existing.peer_address = transfer.peer_address;
                } else {
                    self.state.transfers.push(transfer);
                }
            }
        }

        if let Some(sync) = cli_clipboard_sync {
            if self.state.clipboard_sync.is_none() {
                self.state.clipboard_sync = Some(sync);
            }
        }
    }

    /// Poll for share progress updates.
    fn poll_share_progress(&mut self) {
        if let Some(ref mut rx) = self.share_progress_rx {
            if rx.has_changed().unwrap_or(false) {
                let progress = rx.borrow_and_update().clone();

                if let Some(ref mut session) = self.state.share.active_session {
                    session.progress.transferred = progress.total_bytes_transferred;
                    session.progress.total = progress.total_bytes;
                    session.progress.speed_bps = progress.speed_bps;

                    if progress.state == yoop_core::transfer::TransferState::Completed {
                        self.log_info("Transfer completed!");
                        self.state.share.active_session = None;
                        self.share_progress_rx = None;
                    } else if progress.state == yoop_core::transfer::TransferState::Failed {
                        self.log_error("Transfer failed");
                        self.state.share.active_session = None;
                        self.share_progress_rx = None;
                    } else if progress.state == yoop_core::transfer::TransferState::Cancelled {
                        self.log_info("Transfer cancelled");
                        self.state.share.active_session = None;
                        self.share_progress_rx = None;
                    }
                }
            }
        }
    }

    /// Poll for receive progress updates.
    fn poll_receive_progress(&mut self) {
        if let Some(ref mut rx) = self.receive_progress_rx {
            if rx.has_changed().unwrap_or(false) {
                let progress = rx.borrow_and_update().clone();

                let (sender_name, current_file, files_count, total_size) =
                    if let Some(ref mut session) = self.state.receive.active_session {
                        session.progress.transferred = progress.total_bytes_transferred;
                        session.progress.total = progress.total_bytes;
                        session.progress.speed_bps = progress.speed_bps;
                        session.current_file.clone_from(&progress.current_file_name);

                        (
                            session.sender_name.clone(),
                            session.current_file.clone(),
                            session.files.len(),
                            session.total_size,
                        )
                    } else {
                        return;
                    };

                let view_progress = self
                    .state
                    .receive
                    .active_session
                    .as_ref()
                    .map(|s| s.progress.clone())
                    .unwrap_or_default();

                self.views.receive.status = ReceiveStatus::Transferring {
                    sender_name,
                    progress: view_progress,
                    current_file,
                };

                if progress.state == yoop_core::transfer::TransferState::Completed {
                    self.views.receive.status = ReceiveStatus::Completed {
                        files_count,
                        total_size,
                    };
                    self.state.receive.active_session = None;
                    self.receive_progress_rx = None;
                    self.log_info("Receive completed!");
                } else if progress.state == yoop_core::transfer::TransferState::Failed {
                    self.views.receive.status = ReceiveStatus::Failed {
                        error: "Transfer failed".to_string(),
                    };
                    self.state.receive.active_session = None;
                    self.receive_progress_rx = None;
                    self.log_error("Receive failed");
                } else if progress.state == yoop_core::transfer::TransferState::Cancelled {
                    self.views.receive.reset();
                    self.state.receive.active_session = None;
                    self.receive_progress_rx = None;
                    self.log_info("Receive cancelled");
                }
            }
        }
    }

    /// Poll for receive events from the background task.
    fn poll_receive_events(&mut self) {
        let mut events = Vec::new();
        if let Some(ref mut rx) = self.receive_event_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        for event in events {
            match event {
                ReceiveEvent::Connected {
                    sender_name,
                    sender_addr,
                    files,
                    total_size,
                } => {
                    self.log_info(&format!(
                        "Connected to {} - {} file(s)",
                        sender_name,
                        files.len()
                    ));

                    let incoming_files: Vec<IncomingFile> =
                        files.iter().map(IncomingFile::from_metadata).collect();
                    let receive_files: Vec<ReceiveFile> = files
                        .iter()
                        .map(|f| ReceiveFile {
                            name: f.file_name().to_string(),
                            size: f.size,
                            mime_type: f.mime_type.clone(),
                            is_directory: f.is_directory,
                        })
                        .collect();

                    self.views.receive.status = ReceiveStatus::Connected {
                        sender_name: sender_name.clone(),
                        sender_addr: sender_addr.clone(),
                        files: incoming_files,
                    };

                    self.state.receive.active_session = Some(ReceiveSession {
                        id: uuid::Uuid::new_v4(),
                        sender_name,
                        sender_addr,
                        files: receive_files,
                        total_size,
                        progress: super::state::TransferProgress::default(),
                        current_file: String::new(),
                        status: ReceiveSessionStatus::Pending,
                    });
                }
                ReceiveEvent::ConnectionFailed(err) => {
                    self.log_error(&format!("Connection failed: {}", err));
                    self.views.receive.status = ReceiveStatus::Failed { error: err };
                    self.cleanup_receive_handles();
                }
                ReceiveEvent::TransferProgress(progress) => {
                    if let Some(ref mut session) = self.state.receive.active_session {
                        session.progress.transferred = progress.total_bytes_transferred;
                        session.progress.total = progress.total_bytes;
                        session.progress.speed_bps = progress.speed_bps;
                        session.current_file.clone_from(&progress.current_file_name);

                        let view_progress = session.progress.clone();
                        self.views.receive.status = ReceiveStatus::Transferring {
                            sender_name: session.sender_name.clone(),
                            progress: view_progress,
                            current_file: session.current_file.clone(),
                        };
                    }
                }
                ReceiveEvent::TransferComplete => {
                    let (files_count, total_size) =
                        if let Some(ref session) = self.state.receive.active_session {
                            (session.files.len(), session.total_size)
                        } else {
                            (0, 0)
                        };

                    self.views.receive.status = ReceiveStatus::Completed {
                        files_count,
                        total_size,
                    };
                    self.state.receive.active_session = None;
                    self.cleanup_receive_handles();
                    self.log_info("Receive completed!");
                }
                ReceiveEvent::TransferFailed(err) => {
                    self.views.receive.status = ReceiveStatus::Failed { error: err.clone() };
                    self.state.receive.active_session = None;
                    self.cleanup_receive_handles();
                    self.log_error(&format!("Transfer failed: {}", err));
                }
                ReceiveEvent::TransferCancelled => {
                    self.views.receive.reset();
                    self.state.receive.active_session = None;
                    self.cleanup_receive_handles();
                    self.log_info("Receive cancelled");
                }
            }
        }
    }

    /// Clean up receive session handles.
    fn cleanup_receive_handles(&mut self) {
        self.receive_session_handle = None;
        self.receive_event_rx = None;
        self.receive_command_tx = None;
        self.receive_progress_rx = None;
        self.state.receive.is_connecting = false;
    }

    /// Poll for clipboard task results.
    fn poll_clipboard_task_results(&mut self) {
        use super::state::ClipboardTaskResult;

        while let Ok(result) = self.clipboard_task_rx.try_recv() {
            self.clipboard_session_handle = None;

            match result {
                ClipboardTaskResult::ShareComplete => {
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                    self.log_info("Clipboard shared successfully!");
                }
                ClipboardTaskResult::ShareFailed(err) => {
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                    self.log_error(&format!("Clipboard share failed: {}", err));
                }
                ClipboardTaskResult::ReceiveComplete => {
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard.code_input.clear();
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                    self.refresh_clipboard_preview();
                    self.log_info("Clipboard received successfully!");
                }
                ClipboardTaskResult::ReceiveFailed(err) => {
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message =
                        Some(format!("Failed to receive: {}", err));
                    self.state.clipboard.code_input.clear();
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                    self.log_error(&format!("Failed to receive clipboard: {}", err));
                }
                ClipboardTaskResult::SyncHostConnected {
                    peer_name,
                    peer_addr,
                } => {
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard_sync = Some(super::state::ClipboardSyncSession {
                        peer_name: peer_name.clone(),
                        peer_address: peer_addr,
                        items_sent: 0,
                        items_received: 0,
                        started_at: chrono::Utc::now(),
                    });
                    self.log_info(&format!("Clipboard sync started with {}", peer_name));
                }
                ClipboardTaskResult::SyncHostFailed(err) => {
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                    self.log_error(&format!("Sync host failed: {}", err));
                }
            }
        }
    }

    /// Poll for sync host events from the background task.
    fn poll_sync_host_events(&mut self) {
        let mut events = Vec::new();
        if let Some(ref mut rx) = self.sync_host_event_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        for event in events {
            match event {
                SyncHostEvent::Started { code } => {
                    self.log_info(&format!("Sync session started with code: {}", code));
                    self.state.sync.active_session = Some(super::state::SyncSession {
                        id: uuid::Uuid::new_v4(),
                        code: Some(code),
                        peer_name: None,
                        files_synced: 0,
                        last_event: Some("Waiting for peer...".to_string()),
                    });
                }
                SyncHostEvent::PeerConnected { peer_name } => {
                    self.log_info(&format!("Peer connected: {}", peer_name));
                    if let Some(ref mut session) = self.state.sync.active_session {
                        session.peer_name = Some(peer_name);
                        session.last_event = Some("Connected".to_string());
                    }
                }
                SyncHostEvent::Failed(err) => {
                    self.log_error(&format!("Sync failed: {}", err));
                    self.state.sync.active_session = None;
                    self.sync_host_event_rx = None;
                    self.sync_session_handle = None;
                }
            }
        }
    }

    /// Draw the UI.
    fn draw(&mut self) -> Result<()> {
        let state = &self.state;
        let theme = &self.theme;
        let views = &mut self.views;

        self.terminal.draw(|frame| {
            let size = frame.area();

            let layout = ComputedLayout::compute(size, state.log_visible, state.transfers_expanded);

            Self::render_header(frame, layout.header, theme);

            NavMenu::render(frame, layout.navigation, state, layout.mode);

            views::render_view_with_state(frame, layout.content, state, views, theme);

            StatusBar::render(frame, layout.status, state, layout.mode);

            if let Some(log_area) = layout.log {
                Self::render_log(frame, log_area, state, theme);
            }

            if let Some(transfers_area) = layout.transfers {
                Self::render_transfers(frame, transfers_area, state, theme);
            }

            if state.help_visible {
                HelpOverlay::render(frame, size, state.active_view, theme);
            }
        })?;

        Ok(())
    }

    /// Render the header bar.
    fn render_header(frame: &mut ratatui::Frame, area: Rect, theme: &Theme) {
        let version = env!("CARGO_PKG_VERSION");
        let title = format!(" Yoop v{} ", version);

        let header = Paragraph::new(Span::styled(
            title,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));

        frame.render_widget(header, area);
    }

    /// Render the log panel.
    fn render_log(frame: &mut ratatui::Frame, area: Rect, state: &AppState, theme: &Theme) {
        let block = Block::default()
            .title(" Log ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let inner = block.inner(area);
        let max_width = inner.width.saturating_sub(2) as usize;

        let items: Vec<ListItem> = state
            .log
            .iter()
            .rev()
            .take(inner.height as usize)
            .map(|entry| {
                let level_style = match entry.level {
                    LogLevel::Info => Style::default().fg(theme.info),
                    LogLevel::Warn => Style::default().fg(theme.warning),
                    LogLevel::Error => Style::default().fg(theme.error),
                };

                let time = entry.timestamp.format("%H:%M:%S");
                let prefix = format!("{} [{}] ", time, entry.level.as_str());
                let msg_width = max_width.saturating_sub(prefix.len());
                let truncated_msg = truncate_str(&entry.message, msg_width);
                let text = format!("{}{}", prefix, truncated_msg);

                ListItem::new(Span::styled(text, level_style))
            })
            .collect();

        let list = if items.is_empty() {
            List::new(vec![ListItem::new(Span::styled(
                "No log entries",
                Style::default().fg(theme.text_muted),
            ))])
            .block(block)
        } else {
            List::new(items).block(block)
        };

        frame.render_widget(list, area);
    }

    /// Render the transfers panel.
    fn render_transfers(frame: &mut ratatui::Frame, area: Rect, state: &AppState, theme: &Theme) {
        let block = Block::default()
            .title(" Active Transfers ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let items: Vec<ListItem> = state
            .transfers
            .iter()
            .map(|transfer| {
                let direction = match transfer.session_type {
                    super::state::TransferType::Share | super::state::TransferType::Send => "↑",
                    super::state::TransferType::Receive => "↓",
                    super::state::TransferType::Sync => "↔",
                };

                let code = transfer.code.as_deref().unwrap_or("---");
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let progress = transfer.progress.percentage().clamp(0.0, 100.0) as u8;
                let peer = transfer.peer_name.as_deref().unwrap_or("connecting...");

                let text = format!("{} {} {}% {}", direction, code, progress, peer);
                ListItem::new(Span::styled(text, Style::default().fg(theme.text_primary)))
            })
            .collect();

        let list = if items.is_empty() {
            List::new(vec![ListItem::new(Span::styled(
                "No active transfers",
                Style::default().fg(theme.text_muted),
            ))])
            .block(block)
        } else {
            List::new(items).block(block)
        };

        frame.render_widget(list, area);
    }

    /// Handle a terminal event.
    async fn handle_event(&mut self, event: &Event) {
        match event {
            Event::Key(key) => {
                let action = map_key_event(*key, &self.state);
                self.handle_action(action).await;
            }
            Event::Resize(width, height) => {
                self.handle_resize(*width, *height);
            }
            _ => {}
        }
    }

    /// Handle terminal resize event gracefully.
    fn handle_resize(&mut self, width: u16, height: u16) {
        let old_size = self.state.size;
        self.state.size = (width, height);

        let old_mode = super::layout::LayoutMode::from_size(old_size.0, old_size.1);
        let new_mode = super::layout::LayoutMode::from_size(width, height);

        if old_mode != new_mode {
            self.log_info(&format!(
                "Layout adjusted for terminal size ({}x{})",
                width, height
            ));
        }

        self.adjust_scroll_bounds();
    }

    /// Adjust scroll positions to stay within bounds after resize.
    fn adjust_scroll_bounds(&mut self) {
        if let Some(ref mut browser) = self.state.share.file_browser {
            let max_index = browser.entries.len().saturating_sub(1);
            browser.selected = browser.selected.min(max_index);
        }

        if let Some(ref mut browser) = self.state.sync.file_browser {
            let max_index = browser.entries.len().saturating_sub(1);
            browser.selected = browser.selected.min(max_index);
        }

        let device_count = self.views.devices.devices.len();
        if device_count > 0 {
            self.state.devices.selected_index =
                self.state.devices.selected_index.min(device_count - 1);
        }

        let history_count = self.views.history.entries.len();
        if history_count > 0 {
            self.state.history.selected_index =
                self.state.history.selected_index.min(history_count - 1);
        }

        let file_count = self.state.share.selected_files.len();
        if file_count > 0 {
            self.state.share.selected_index = self.state.share.selected_index.min(file_count - 1);
        }
    }

    /// Handle an action.
    #[allow(clippy::too_many_lines)]
    async fn handle_action(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.events.cancel();
                self.should_quit = true;
            }
            Action::SwitchView(view) => {
                self.state.active_view = view;
                if view == View::Clipboard {
                    self.refresh_clipboard_preview();
                }
            }
            Action::NextView => {
                self.state.active_view = self.state.active_view.next();
                if self.state.active_view == View::Clipboard {
                    self.refresh_clipboard_preview();
                }
            }
            Action::PrevView => {
                self.state.active_view = self.state.active_view.prev();
                if self.state.active_view == View::Clipboard {
                    self.refresh_clipboard_preview();
                }
            }
            Action::ToggleLog => {
                self.state.log_visible = !self.state.log_visible;
            }
            Action::ExpandTransfers => {
                self.state.transfers_expanded = !self.state.transfers_expanded;
            }
            Action::CollapseTransfers => {
                self.state.transfers_expanded = false;
            }
            Action::Help => {
                self.log_info("Help: Press [?] for help, [Q] to quit");
            }
            Action::ToggleHelp => {
                self.state.help_visible = !self.state.help_visible;
            }
            Action::Tick => {
                self.state.spinner.tick(SpinnerStyle::Braille);
            }
            Action::FocusNext => match self.state.active_view {
                View::Share => {
                    self.views.share.focus_next(&mut self.state.share);
                }
                View::Receive => {
                    self.views.receive.focus_next();
                    self.state.receive.input_mode = self.views.receive.focus.to_input_mode();
                }
                View::Clipboard => {
                    self.state.clipboard.status_message = None;
                    self.views.clipboard.focus_next(&mut self.state.clipboard);
                }
                View::Sync => {
                    self.views.sync.focus_next(&mut self.state.sync);
                }
                View::Devices => {
                    self.views.devices.focus_next(&mut self.state.devices);
                }
                View::History => {
                    self.views.history.focus_next(&mut self.state.history);
                }
                View::Config => {
                    self.views.config.focus_next(&mut self.state.config);
                }
            },
            Action::FocusPrev => match self.state.active_view {
                View::Receive => {
                    self.views.receive.focus_prev();
                    self.state.receive.input_mode = self.views.receive.focus.to_input_mode();
                }
                View::Clipboard => {
                    self.state.clipboard.status_message = None;
                    self.views.clipboard.focus_prev(&mut self.state.clipboard);
                }
                View::Sync => {
                    self.views.sync.focus_prev(&mut self.state.sync);
                }
                View::Devices => {
                    self.views.devices.focus_next(&mut self.state.devices);
                }
                View::History => {
                    self.views.history.focus_next(&mut self.state.history);
                }
                _ => {}
            },

            Action::OpenFileBrowser => {
                self.open_file_browser();
            }
            Action::CloseFileBrowser => {
                self.state.share.file_browser = None;
            }
            Action::FileBrowserUp => {
                let browser = self.get_active_file_browser_mut();
                if let Some(browser) = browser {
                    if browser.selected > 0 {
                        browser.selected -= 1;
                    }
                }
            }
            Action::FileBrowserDown => {
                let browser = self.get_active_file_browser_mut();
                if let Some(browser) = browser {
                    if browser.selected < browser.entries.len().saturating_sub(1) {
                        browser.selected += 1;
                    }
                }
            }
            Action::FileBrowserEnter => {
                self.file_browser_enter();
            }
            Action::FileBrowserBack => {
                self.file_browser_back();
            }
            Action::FileBrowserToggleSelection => {
                self.file_browser_toggle_selection();
            }
            Action::FileBrowserToggleHidden => {
                self.file_browser_toggle_hidden();
            }
            Action::FileBrowserConfirm => {
                self.file_browser_confirm();
            }

            Action::ListUp => match self.state.active_view {
                View::Share => {
                    if self.state.share.selected_index > 0 {
                        self.state.share.selected_index -= 1;
                    }
                }
                View::Receive => {
                    if self.state.receive.selected_device > 0 {
                        self.state.receive.selected_device -= 1;
                    }
                }
                View::Sync => {
                    if self.state.sync.focus == super::state::SyncFocus::ExcludePatterns
                        && self.state.sync.selected_pattern_index > 0
                    {
                        self.state.sync.selected_pattern_index -= 1;
                    }
                }
                View::Devices => {
                    if self.state.devices.selected_index > 0 {
                        self.state.devices.selected_index -= 1;
                    }
                }
                View::History => {
                    if self.state.history.selected_index > 0 {
                        self.state.history.selected_index -= 1;
                    }
                }
                View::Config => match self.state.config.focus {
                    super::state::ConfigFocus::SectionList => {
                        if self.state.config.selected_section > 0 {
                            self.state.config.selected_section -= 1;
                            self.state.config.selected_setting = 0;
                        }
                    }
                    super::state::ConfigFocus::Settings => {
                        if self.state.config.selected_setting > 0 {
                            self.state.config.selected_setting -= 1;
                        }
                    }
                },
                View::Clipboard => {}
            },
            Action::ListDown => match self.state.active_view {
                View::Share => {
                    let max = self.state.share.selected_files.len().saturating_sub(1);
                    if self.state.share.selected_index < max {
                        self.state.share.selected_index += 1;
                    }
                }
                View::Receive => {
                    let device_count = self.views.receive.devices.len();
                    if device_count > 0 && self.state.receive.selected_device < device_count - 1 {
                        self.state.receive.selected_device += 1;
                    }
                }
                View::Sync => {
                    if self.state.sync.focus == super::state::SyncFocus::ExcludePatterns {
                        let max = self.state.sync.exclude_patterns.len().saturating_sub(1);
                        if self.state.sync.selected_pattern_index < max {
                            self.state.sync.selected_pattern_index += 1;
                        }
                    }
                }
                View::Devices => {
                    let device_count = self.views.devices.devices.len();
                    if device_count > 0 && self.state.devices.selected_index < device_count - 1 {
                        self.state.devices.selected_index += 1;
                    }
                }
                View::History => {
                    let entry_count = self.views.history.entries.len();
                    if entry_count > 0 && self.state.history.selected_index < entry_count - 1 {
                        self.state.history.selected_index += 1;
                    }
                }
                View::Config => match self.state.config.focus {
                    super::state::ConfigFocus::SectionList => {
                        let section_count = super::state::ConfigSection::all().len();
                        if self.state.config.selected_section < section_count - 1 {
                            self.state.config.selected_section += 1;
                            self.state.config.selected_setting = 0;
                        }
                    }
                    super::state::ConfigFocus::Settings => {
                        if let Some(settings) =
                            self.views.config.current_settings(&self.state.config)
                        {
                            if self.state.config.selected_setting < settings.len().saturating_sub(1)
                            {
                                self.state.config.selected_setting += 1;
                            }
                        }
                    }
                },
                View::Clipboard => {}
            },
            Action::ListFirst => match self.state.active_view {
                View::Devices => {
                    self.state.devices.selected_index = 0;
                }
                View::History => {
                    self.state.history.selected_index = 0;
                }
                View::Config => match self.state.config.focus {
                    super::state::ConfigFocus::SectionList => {
                        self.state.config.selected_section = 0;
                        self.state.config.selected_setting = 0;
                    }
                    super::state::ConfigFocus::Settings => {
                        self.state.config.selected_setting = 0;
                    }
                },
                _ => {}
            },
            Action::ListLast => match self.state.active_view {
                View::Devices => {
                    self.state.devices.selected_index =
                        self.views.devices.devices.len().saturating_sub(1);
                }
                View::History => {
                    self.state.history.selected_index =
                        self.views.history.entries.len().saturating_sub(1);
                }
                View::Config => match self.state.config.focus {
                    super::state::ConfigFocus::SectionList => {
                        self.state.config.selected_section =
                            super::state::ConfigSection::all().len().saturating_sub(1);
                        self.state.config.selected_setting = 0;
                    }
                    super::state::ConfigFocus::Settings => {
                        if let Some(settings) =
                            self.views.config.current_settings(&self.state.config)
                        {
                            self.state.config.selected_setting = settings.len().saturating_sub(1);
                        }
                    }
                },
                _ => {}
            },

            Action::AddFiles(files) => {
                self.state.share.selected_files.extend(files);
            }
            Action::RemoveFile(index) => {
                if index < self.state.share.selected_files.len() {
                    self.state.share.selected_files.remove(index);
                    if self.state.share.selected_index >= self.state.share.selected_files.len()
                        && self.state.share.selected_index > 0
                    {
                        self.state.share.selected_index -= 1;
                    }
                }
            }
            Action::ToggleFile(index) => {
                self.state.share.selected_index = index;
            }
            Action::StartShare => {
                self.start_share().await;
            }
            Action::CancelShare => {
                self.cancel_share();
            }
            Action::RegenerateCode => {
                self.regenerate_code().await;
            }
            Action::NextShareOption => {
                if let Some(ref mut focus) = self.state.share.option_focus {
                    *focus = focus.next();
                } else {
                    self.state.share.option_focus = Some(super::state::ShareOptionFocus::default());
                }
            }
            Action::PrevShareOption => {
                if let Some(ref mut focus) = self.state.share.option_focus {
                    *focus = focus.prev();
                } else {
                    self.state.share.option_focus = Some(super::state::ShareOptionFocus::default());
                }
            }
            Action::ToggleShareOption => {
                if let Some(focus) = self.state.share.option_focus {
                    match focus {
                        super::state::ShareOptionFocus::Pin => {
                            self.state.share.options.require_pin =
                                !self.state.share.options.require_pin;
                        }
                        super::state::ShareOptionFocus::Approval => {
                            self.state.share.options.require_approval =
                                !self.state.share.options.require_approval;
                        }
                        super::state::ShareOptionFocus::Compress => {
                            self.state.share.options.compress = !self.state.share.options.compress;
                        }
                        super::state::ShareOptionFocus::Expire => {}
                    }
                }
            }
            Action::CycleExpireForward => {
                self.state.share.options.expire =
                    super::components::next_expire_option(&self.state.share.options.expire);
            }
            Action::CycleExpireBackward => {
                self.state.share.options.expire =
                    super::components::prev_expire_option(&self.state.share.options.expire);
            }

            Action::UpdateCodeInput(code) => {
                let code: String = code
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(4)
                    .collect::<String>()
                    .to_uppercase();
                self.state.receive.code_input = code;
            }
            Action::UpdateIpInput(_ip) => {}
            Action::IpSegmentAppend(c) => {
                let ip_input = &mut self.state.receive.ip_input;
                let max_len = ip_input.current_max_len();
                let segment = ip_input.current_segment_mut();
                if segment.len() < max_len {
                    segment.push(c);
                    if segment.len() >= max_len && ip_input.is_octet_segment() {
                        ip_input.cursor_next();
                    }
                }
            }
            Action::IpSegmentBackspace => {
                let segment = self.state.receive.ip_input.current_segment_mut();
                segment.pop();
            }
            Action::IpCursorNext => {
                self.state.receive.ip_input.cursor_next();
            }
            Action::IpCursorPrev => {
                self.state.receive.ip_input.cursor_prev();
            }
            Action::IpClear => {
                self.state.receive.ip_input.clear();
            }
            Action::SelectDevice(index) => {
                self.state.receive.selected_device = index;
            }
            Action::SwitchReceiveMode(mode) => {
                self.state.receive.input_mode = mode;
                self.views.receive.focus =
                    super::views::receive::ReceiveFocus::from_input_mode(mode);
            }
            Action::StartReceive => {
                self.start_receive().await;
            }
            Action::AcceptTransfer => {
                self.accept_transfer().await;
            }
            Action::DeclineTransfer => {
                self.decline_transfer();
            }
            Action::CancelReceive => {
                self.cancel_receive();
            }

            Action::UpdateClipboardCodeInput(code) => {
                self.state.clipboard.status_message = None;
                let code: String = code
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(4)
                    .collect::<String>()
                    .to_uppercase();
                self.state.clipboard.code_input = code;
            }
            Action::RefreshClipboard => {
                self.state.clipboard.status_message = None;
                self.refresh_clipboard_preview();
            }
            Action::ShareClipboard => {
                self.share_clipboard().await;
            }
            Action::ReceiveClipboard => {
                self.receive_clipboard().await;
            }
            Action::StartClipboardSync => {
                self.start_clipboard_sync().await;
            }
            Action::StopClipboardSync => {
                self.stop_clipboard_sync();
            }
            Action::CancelClipboardOperation => {
                self.cancel_clipboard_operation();
            }

            Action::UpdateSyncCodeInput(code) => {
                let code: String = code
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .take(4)
                    .collect::<String>()
                    .to_uppercase();
                self.state.sync.code_input = code;
            }
            Action::StartSyncHost => {
                self.start_sync_host();
            }
            Action::JoinSync => {
                self.join_sync().await;
            }
            Action::StopSync => {
                self.stop_sync();
            }
            Action::ToggleSyncDeletions => {
                self.state.sync.sync_deletions = !self.state.sync.sync_deletions;
            }
            Action::ToggleFollowSymlinks => {
                self.state.sync.follow_symlinks = !self.state.sync.follow_symlinks;
            }
            Action::NextSyncOption => {
                if let Some(ref mut focus) = self.state.sync.option_focus {
                    *focus = focus.next();
                } else {
                    self.state.sync.option_focus = Some(SyncOptionFocus::default());
                }
            }
            Action::PrevSyncOption => {
                if let Some(ref mut focus) = self.state.sync.option_focus {
                    *focus = focus.prev();
                } else {
                    self.state.sync.option_focus = Some(SyncOptionFocus::default());
                }
            }
            Action::ToggleSyncOption => {
                if let Some(focus) = self.state.sync.option_focus {
                    match focus {
                        SyncOptionFocus::SyncDeletions => {
                            self.state.sync.sync_deletions = !self.state.sync.sync_deletions;
                        }
                        SyncOptionFocus::FollowSymlinks => {
                            self.state.sync.follow_symlinks = !self.state.sync.follow_symlinks;
                        }
                    }
                }
            }
            Action::OpenSyncDirectoryBrowser => {
                self.open_sync_directory_browser();
            }
            Action::CloseSyncDirectoryBrowser => {
                self.state.sync.file_browser = None;
            }
            Action::ScrollSyncEventsUp => {
                let events_count = self.state.sync.events.len();
                self.views.sync.scroll_events_up(events_count);
            }
            Action::ScrollSyncEventsDown => {
                let events_count = self.state.sync.events.len();
                self.views.sync.scroll_events_down(events_count);
            }
            Action::AddExcludePattern(pattern) => {
                self.state.sync.exclude_patterns.push(pattern);
            }
            Action::RemoveExcludePattern(index) => {
                if index < self.state.sync.exclude_patterns.len() {
                    self.state.sync.exclude_patterns.remove(index);
                    if self.state.sync.selected_pattern_index
                        >= self.state.sync.exclude_patterns.len()
                    {
                        self.state.sync.selected_pattern_index =
                            self.state.sync.exclude_patterns.len().saturating_sub(1);
                    }
                }
            }
            Action::StartAddExcludePattern => {
                self.state.sync.editing_pattern = true;
                self.state.sync.pattern_input.clear();
            }
            Action::UpdatePatternInput(input) => {
                self.state.sync.pattern_input = input;
            }
            Action::ConfirmAddExcludePattern => {
                let pattern = self.state.sync.pattern_input.trim().to_string();
                if !pattern.is_empty() {
                    self.state.sync.exclude_patterns.push(pattern);
                }
                self.state.sync.pattern_input.clear();
                self.state.sync.editing_pattern = false;
            }
            Action::CancelAddExcludePattern => {
                self.state.sync.pattern_input.clear();
                self.state.sync.editing_pattern = false;
            }
            Action::FocusExcludePatterns => {
                self.state.sync.focus = super::state::SyncFocus::ExcludePatterns;
            }

            Action::SelectDeviceIndex(index) => {
                if index < self.views.devices.devices.len() {
                    self.state.devices.selected_index = index;
                }
            }
            Action::CycleTrustLevel => {
                if let Some(device) = self.views.devices.get_selected_device(&self.state) {
                    let device_id = device.device_id;
                    if self.views.devices.toggle_trust_level(&device_id) {
                        self.log_info("Trust level updated");
                    } else {
                        self.log_error("Failed to update trust level");
                    }
                }
            }
            Action::StartEditTrustLevel => {
                self.state.devices.editing_trust_level = true;
            }
            Action::ConfirmTrustLevel => {
                self.state.devices.editing_trust_level = false;
            }
            Action::CancelTrustLevelEdit => {
                self.state.devices.editing_trust_level = false;
            }
            Action::RequestRemoveDevice => {
                self.state.devices.confirm_remove = true;
            }
            Action::ConfirmRemoveDevice => {
                if let Some(device) = self.views.devices.get_selected_device(&self.state) {
                    let device_id = device.device_id;
                    let device_name = device.device_name.clone();
                    if self.views.devices.remove_device(&device_id) {
                        self.log_info(&format!("Removed device: {}", device_name));
                        if self.state.devices.selected_index >= self.views.devices.devices.len() {
                            self.state.devices.selected_index =
                                self.views.devices.devices.len().saturating_sub(1);
                        }
                    } else {
                        self.log_error("Failed to remove device");
                    }
                }
                self.state.devices.confirm_remove = false;
            }
            Action::CancelRemoveDevice => {
                self.state.devices.confirm_remove = false;
            }
            Action::SendToDevice => {
                if let Some(device) = self.views.devices.get_selected_device(&self.state) {
                    self.log_info(&format!(
                        "To send to '{}', go to Share view [S]",
                        device.device_name
                    ));
                }
            }
            Action::RefreshDevices => {
                self.views.devices.load_devices();
                self.log_info("Devices list refreshed");
            }

            Action::SelectHistoryIndex(index) => {
                if index < self.views.history.entries.len() {
                    self.state.history.selected_index = index;
                }
            }
            Action::ViewHistoryDetails => {
                self.state.history.focus = super::state::HistoryFocus::Details;
            }
            Action::RetryTransfer => {
                if let Some(entry) = self.views.history.get_selected_entry(&self.state) {
                    if entry.is_failed {
                        if entry.is_sent {
                            self.log_info(
                                "To retry sending, go to Share view [S] and share the same files",
                            );
                        } else {
                            self.log_info(&format!(
                                "To retry receiving, go to Receive view [R] and enter code: {}",
                                entry.share_code
                            ));
                        }
                    }
                }
            }
            Action::OpenTransferDirectory => {
                if let Some(entry) = self.views.history.get_selected_entry(&self.state) {
                    if let Some(ref dir) = entry.output_dir {
                        if dir.exists() {
                            match open::that(dir) {
                                Ok(()) => {
                                    self.log_info(&format!("Opened: {}", dir.display()));
                                }
                                Err(e) => {
                                    self.log_error(&format!("Failed to open directory: {}", e));
                                }
                            }
                        } else {
                            self.log_error(&format!(
                                "Directory no longer exists: {}",
                                dir.display()
                            ));
                        }
                    } else {
                        self.log_info("No output directory for sent transfers");
                    }
                }
            }
            Action::RequestClearHistory => {
                self.state.history.confirm_clear = true;
            }
            Action::ConfirmClearHistory => {
                if self.views.history.clear_history() {
                    self.log_info("History cleared");
                } else {
                    self.log_error("Failed to clear history");
                }
                self.state.history.confirm_clear = false;
            }
            Action::CancelClearHistory => {
                self.state.history.confirm_clear = false;
            }
            Action::RefreshHistory => {
                self.views.history.load_history();
                self.log_info("History refreshed");
            }

            Action::CancelTransfer(session_id) => {
                self.cancel_external_transfer(session_id);
            }

            Action::SelectConfigSection(section) => {
                let sections = super::state::ConfigSection::all();
                if let Some(index) = sections.iter().position(|s| *s == section) {
                    self.state.config.selected_section = index;
                    self.state.config.selected_setting = 0;
                }
            }
            Action::SelectConfigSectionIndex(index) => {
                if index < super::state::ConfigSection::all().len() {
                    self.state.config.selected_section = index;
                    self.state.config.selected_setting = 0;
                }
            }
            Action::SelectConfigSetting(index) => {
                if let Some(settings) = self.views.config.current_settings(&self.state.config) {
                    if index < settings.len() {
                        self.state.config.selected_setting = index;
                    }
                }
            }
            Action::StartEditSetting => {
                if self.state.config.focus == super::state::ConfigFocus::Settings {
                    self.views.config.start_edit(&mut self.state.config);
                }
            }
            Action::UpdateEditBuffer(s) => {
                self.state.config.edit_buffer = s;
            }
            Action::ConfirmEdit => {
                self.views.config.confirm_edit(&mut self.state.config);
            }
            Action::CancelEdit => {
                self.views.config.cancel_edit(&mut self.state.config);
            }
            Action::ToggleConfigSetting => {
                self.views.config.toggle_setting(&mut self.state.config);
            }
            Action::CycleConfigSetting => {
                self.views.config.cycle_setting(&mut self.state.config);
            }
            Action::RequestSaveConfig => {
                if self.views.config.has_unsaved_changes() {
                    self.state.config.confirm_save = true;
                } else {
                    self.state.config.status_message = Some("No changes to save".to_string());
                }
            }
            Action::ConfirmSaveConfig => {
                match self.views.config.save_config(&mut self.state.config) {
                    Ok(()) => {
                        self.log_info("Configuration saved");
                        if let Some(theme_name) = self.views.config.get_theme_value() {
                            self.theme = Theme::from_name(theme_name);
                        }
                    }
                    Err(e) => {
                        self.log_error(&format!("Failed to save config: {}", e));
                        self.state.config.status_message = Some(format!("Save failed: {}", e));
                    }
                }
                self.state.config.confirm_save = false;
            }
            Action::CancelSaveConfig => {
                self.state.config.confirm_save = false;
            }
            Action::RequestRevertConfig => {
                if self.views.config.has_unsaved_changes() {
                    self.state.config.confirm_revert = true;
                } else {
                    self.state.config.status_message = Some("No changes to revert".to_string());
                }
            }
            Action::ConfirmRevertConfig => {
                self.views.config.revert_changes(&mut self.state.config);
                self.log_info("Configuration changes reverted");
                self.state.config.confirm_revert = false;
            }
            Action::CancelRevertConfig => {
                self.state.config.confirm_revert = false;
            }
            Action::RefreshConfig => {
                self.views.config.load_config();
                self.state.config.selected_setting = 0;
                self.log_info("Configuration reloaded");
            }

            Action::Refresh => {
                if let Some(ref session) = self.state.receive.active_session {
                    if matches!(
                        session.status,
                        ReceiveSessionStatus::Completed
                            | ReceiveSessionStatus::Failed
                            | ReceiveSessionStatus::Cancelled
                    ) {
                        self.state.receive.active_session = None;
                        self.views.receive.reset();
                    }
                } else {
                    match &self.views.receive.status {
                        super::views::receive::ReceiveStatus::Completed { .. }
                        | super::views::receive::ReceiveStatus::Failed { .. } => {
                            self.views.receive.reset();
                        }
                        _ => {}
                    }
                }
            }

            _ => {}
        }
    }

    /// Open the file browser.
    fn open_file_browser(&mut self) {
        match init_browser_state(None, self.state.share.options.compress) {
            Ok(browser_state) => {
                self.state.share.file_browser = Some(browser_state);
            }
            Err(e) => {
                self.log_error(&format!("Failed to open file browser: {}", e));
            }
        }
    }

    /// Get mutable reference to the active file browser based on current view.
    fn get_active_file_browser_mut(&mut self) -> Option<&mut FileBrowserState> {
        match self.state.active_view {
            View::Share => self.state.share.file_browser.as_mut(),
            View::Sync => self.state.sync.file_browser.as_mut(),
            _ => None,
        }
    }

    /// Check if we're in sync directory-only mode.
    fn is_sync_directory_browser(&self) -> bool {
        self.state.active_view == View::Sync && self.state.sync.file_browser.is_some()
    }

    /// Enter a directory or select a file in the file browser.
    fn file_browser_enter(&mut self) {
        let is_sync_mode = self.is_sync_directory_browser();
        let browser = self.get_active_file_browser_mut();

        if let Some(browser) = browser {
            if let Some(entry) = browser.entries.get(browser.selected).cloned() {
                if entry.is_dir {
                    let new_path = if entry.path.file_name().is_some_and(|n| n == "..") {
                        browser.current_dir.parent().map(PathBuf::from)
                    } else {
                        Some(entry.path.clone())
                    };

                    if let Some(path) = new_path {
                        let load_result = if is_sync_mode {
                            load_directories_only(&path, browser.show_hidden)
                        } else {
                            load_directory(&path, browser.show_hidden)
                        };
                        match load_result {
                            Ok(entries) => {
                                browser.current_dir = path;
                                browser.entries = entries;
                                browser.selected = 0;
                            }
                            Err(e) => {
                                tracing::error!("Failed to read directory: {}", e);
                            }
                        }
                    }
                } else if !is_sync_mode {
                    if browser.selections.contains(&entry.path) {
                        browser.selections.remove(&entry.path);
                    } else {
                        browser.selections.insert(entry.path);
                    }
                }
            }
        }
    }

    /// Go back to parent directory in file browser.
    fn file_browser_back(&mut self) {
        let is_sync_mode = self.is_sync_directory_browser();
        let browser = self.get_active_file_browser_mut();

        if let Some(browser) = browser {
            if let Some(parent) = browser.current_dir.parent() {
                let parent = parent.to_path_buf();
                let load_result = if is_sync_mode {
                    load_directories_only(&parent, browser.show_hidden)
                } else {
                    load_directory(&parent, browser.show_hidden)
                };
                match load_result {
                    Ok(entries) => {
                        browser.current_dir = parent;
                        browser.entries = entries;
                        browser.selected = 0;
                    }
                    Err(e) => {
                        tracing::error!("Failed to read directory: {}", e);
                    }
                }
            }
        }
    }

    /// Toggle selection in file browser.
    fn file_browser_toggle_selection(&mut self) {
        let is_sync_mode = self.is_sync_directory_browser();
        let browser = self.get_active_file_browser_mut();

        if let Some(browser) = browser {
            if let Some(entry) = browser.entries.get(browser.selected).cloned() {
                if is_sync_mode {
                    if entry.is_dir && entry.path.file_name().is_none_or(|n| n != "..") {
                        if browser.selections.contains(&entry.path) {
                            browser.selections.remove(&entry.path);
                        } else {
                            browser.selections.clear();
                            browser.selections.insert(entry.path);
                        }
                    }
                } else if !entry.is_dir || entry.path.file_name().is_some_and(|n| n != "..") {
                    if browser.selections.contains(&entry.path) {
                        browser.selections.remove(&entry.path);
                    } else {
                        browser.selections.insert(entry.path);
                    }
                }
            }
        }
    }

    /// Toggle hidden files visibility in file browser.
    fn file_browser_toggle_hidden(&mut self) {
        let is_sync_mode = self.is_sync_directory_browser();
        let browser = self.get_active_file_browser_mut();

        if let Some(browser) = browser {
            browser.show_hidden = !browser.show_hidden;
            let load_result = if is_sync_mode {
                load_directories_only(&browser.current_dir, browser.show_hidden)
            } else {
                load_directory(&browser.current_dir, browser.show_hidden)
            };
            match load_result {
                Ok(entries) => {
                    browser.entries = entries;
                    browser.selected = browser
                        .selected
                        .min(browser.entries.len().saturating_sub(1));
                }
                Err(e) => {
                    tracing::error!("Failed to reload directory: {}", e);
                }
            }
        }
    }

    /// Confirm file browser selection.
    fn file_browser_confirm(&mut self) {
        match self.state.active_view {
            View::Share => {
                if let Some(browser) = self.state.share.file_browser.take() {
                    let new_files: Vec<PathBuf> = browser.selections.into_iter().collect();
                    if !new_files.is_empty() {
                        self.log_info(&format!("Added {} files", new_files.len()));
                        self.state.share.selected_files.extend(new_files);
                    }
                }
            }
            View::Sync => {
                if let Some(browser) = self.state.sync.file_browser.take() {
                    let selected_dir = browser
                        .selections
                        .iter()
                        .find(|p| p.is_dir())
                        .cloned()
                        .unwrap_or(browser.current_dir);
                    self.state.sync.directory = Some(selected_dir);
                    self.log_info("Directory selected for sync");
                }
            }
            _ => {}
        }
    }

    /// Start a share session.
    async fn start_share(&mut self) {
        if self.state.share.selected_files.is_empty() {
            self.log_error("No files selected to share");
            return;
        }

        self.log_info("Starting share session...");

        let config = yoop_core::transfer::TransferConfig {
            compression: if self.state.share.options.compress {
                yoop_core::config::CompressionMode::Always
            } else {
                yoop_core::config::CompressionMode::Never
            },
            compression_level: self.state.share.options.compression_level,
            ..Default::default()
        };

        match yoop_core::transfer::ShareSession::new(&self.state.share.selected_files, config).await
        {
            Ok(session) => {
                let code = session.code().to_string();
                let files: Vec<String> = session
                    .files()
                    .iter()
                    .map(|f| f.file_name().to_string())
                    .collect();
                let total_size: u64 = session.files().iter().map(|f| f.size).sum();

                let expire_duration = parse_duration(&self.state.share.options.expire)
                    .unwrap_or(Duration::from_secs(300));
                let expires_at = chrono::Utc::now()
                    + chrono::Duration::from_std(expire_duration).unwrap_or_default();

                let progress_rx = session.progress();
                self.share_progress_rx = Some(progress_rx);

                self.state.share.active_session = Some(ShareSession {
                    id: uuid::Uuid::new_v4(),
                    code: code.clone(),
                    files,
                    total_size,
                    started_at: chrono::Utc::now(),
                    expires_at,
                    peer_name: None,
                    progress: super::state::TransferProgress::default(),
                });

                self.log_info(&format!("Share code: {}", code));

                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

                let task_handle = tokio::spawn(async move {
                    let mut session = session;
                    tokio::select! {
                        biased;
                        _ = cancel_rx => {
                            session.cancel().await;
                        }
                        result = session.wait() => {
                            let _ = result;
                        }
                    }
                });

                self.share_session_handle = Some(ShareSessionHandle {
                    cancel_tx: Some(cancel_tx),
                    task_handle,
                });
            }
            Err(e) => {
                self.log_error(&format!("Failed to start share: {}", e));
            }
        }
    }

    /// Cancel the active share session.
    fn cancel_share(&mut self) {
        if self.state.share.active_session.is_some() {
            if let Some(mut handle) = self.share_session_handle.take() {
                if let Some(tx) = handle.cancel_tx.take() {
                    let _ = tx.send(());
                }
                handle.task_handle.abort();
            }

            self.state.share.active_session = None;
            self.share_progress_rx = None;
            self.state.share.focus = super::state::ShareFocus::FileList;
            self.state.share.option_focus = None;
            self.log_info("Share cancelled");
        }
    }

    /// Regenerate the share code (cancel current session and start new one).
    async fn regenerate_code(&mut self) {
        if self.state.share.active_session.is_some() {
            if let Some(mut handle) = self.share_session_handle.take() {
                if let Some(tx) = handle.cancel_tx.take() {
                    let _ = tx.send(());
                }
                handle.task_handle.abort();
            }

            self.state.share.active_session = None;
            self.share_progress_rx = None;
            self.state.share.focus = super::state::ShareFocus::FileList;
            self.state.share.option_focus = None;
            self.log_info("Generating new code...");

            self.start_share().await;
        }
    }

    /// Start a receive session based on current input mode.
    async fn start_receive(&mut self) {
        use super::state::ReceiveInputMode;

        let output_dir = self
            .state
            .receive
            .output_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let config = yoop_core::transfer::TransferConfig::default();

        match self.state.receive.input_mode {
            ReceiveInputMode::Code => {
                let code = self.state.receive.code_input.clone();
                if code.len() < 4 {
                    self.log_error("Please enter a 4-character code");
                    return;
                }

                self.log_info(&format!("Searching for code {}...", code));
                self.views.receive.status = ReceiveStatus::Searching { code: code.clone() };

                self.connect_with_code(&code, output_dir, None, config)
                    .await;
            }
            ReceiveInputMode::TrustedDevice => {
                if let Some(device) = self
                    .views
                    .receive
                    .selected_device(self.state.receive.selected_device)
                {
                    let device_name = device.name.clone();
                    self.log_info(&format!("Connecting to {}...", device_name));
                    self.views.receive.status = ReceiveStatus::Connecting {
                        peer: device_name.clone(),
                    };

                    self.connect_to_trusted_device(&device_name, output_dir, config)
                        .await;
                } else {
                    self.log_error("No device selected");
                }
            }
            ReceiveInputMode::DirectIp => {
                let ip_input = &self.state.receive.ip_input;
                let code = self.state.receive.code_input.clone();

                if ip_input.is_empty() {
                    self.log_error("Please enter an IP address");
                    return;
                }

                if !ip_input.is_complete() || !ip_input.is_valid() {
                    self.log_error("Please enter a valid IP address");
                    return;
                }

                if code.len() < 4 {
                    self.log_error("Please enter a share code for direct IP connection");
                    return;
                }

                let ip_str = ip_input.to_address_string();
                match yoop_core::connection::parse_host_address(&ip_str) {
                    Ok(addr) => {
                        self.log_info(&format!("Connecting to {} with code {}...", addr, code));
                        self.views.receive.status = ReceiveStatus::Connecting {
                            peer: addr.to_string(),
                        };

                        self.connect_with_code(&code, output_dir, Some(addr), config)
                            .await;
                    }
                    Err(e) => {
                        self.log_error(&format!("Invalid IP address: {}", e));
                    }
                }
            }
        }
    }

    /// Connect using a share code (spawns a background task).
    async fn connect_with_code(
        &mut self,
        code: &str,
        output_dir: PathBuf,
        direct_addr: Option<std::net::SocketAddr>,
        config: yoop_core::transfer::TransferConfig,
    ) {
        let code = match yoop_core::code::ShareCode::parse(code) {
            Ok(c) => c,
            Err(e) => {
                self.log_error(&format!("Invalid code: {}", e));
                self.views.receive.reset();
                return;
            }
        };

        let (event_tx, event_rx) = mpsc::unbounded_channel::<ReceiveEvent>();
        let (command_tx, command_rx) = mpsc::unbounded_channel::<ReceiveCommand>();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        self.receive_event_rx = Some(event_rx);
        self.receive_command_tx = Some(command_tx);

        let task_handle = tokio::spawn(async move {
            run_receive_task(
                code,
                output_dir,
                direct_addr,
                config,
                event_tx,
                command_rx,
                cancel_rx,
            )
            .await;
        });

        self.receive_session_handle = Some(ReceiveSessionHandle {
            cancel_tx: Some(cancel_tx),
            task_handle,
        });
        self.state.receive.is_connecting = true;
    }

    /// Connect to a trusted device (spawns a background task).
    async fn connect_to_trusted_device(
        &mut self,
        device_name: &str,
        output_dir: PathBuf,
        config: yoop_core::transfer::TransferConfig,
    ) {
        let trust_store = match yoop_core::trust::TrustStore::load() {
            Ok(store) => store,
            Err(e) => {
                self.log_error(&format!("Failed to load trust store: {}", e));
                self.views.receive.reset();
                return;
            }
        };

        let Some(device) = trust_store.find_by_name(device_name).cloned() else {
            self.log_error(&format!("Device '{}' not found", device_name));
            self.views.receive.reset();
            return;
        };

        let (event_tx, event_rx) = mpsc::unbounded_channel::<ReceiveEvent>();
        let (command_tx, command_rx) = mpsc::unbounded_channel::<ReceiveCommand>();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        self.receive_event_rx = Some(event_rx);
        self.receive_command_tx = Some(command_tx);

        let task_handle = tokio::spawn(async move {
            run_receive_trusted_task(device, output_dir, config, event_tx, command_rx, cancel_rx)
                .await;
        });

        self.receive_session_handle = Some(ReceiveSessionHandle {
            cancel_tx: Some(cancel_tx),
            task_handle,
        });
        self.state.receive.is_connecting = true;
    }

    /// Accept the pending transfer.
    async fn accept_transfer(&mut self) {
        let session_data = if let Some(ref mut session) = self.state.receive.active_session {
            session.status = ReceiveSessionStatus::Transferring;
            Some((
                session.sender_name.clone(),
                session.progress.clone(),
                session.current_file.clone(),
            ))
        } else {
            None
        };

        if let Some((sender_name, progress, current_file)) = session_data {
            if let Some(ref tx) = self.receive_command_tx {
                if tx.send(ReceiveCommand::Accept).is_err() {
                    self.log_error("Failed to send accept command");
                    return;
                }
            } else {
                self.log_error("No active receive session");
                return;
            }

            self.log_info("Transfer accepted - starting download...");

            self.views.receive.status = ReceiveStatus::Transferring {
                sender_name,
                progress,
                current_file,
            };
        } else {
            self.log_error("No pending transfer to accept");
        }
    }

    /// Decline the pending transfer.
    fn decline_transfer(&mut self) {
        if self.state.receive.active_session.is_some() {
            if let Some(ref tx) = self.receive_command_tx {
                let _ = tx.send(ReceiveCommand::Decline);
            }
            self.state.receive.active_session = None;
            self.cleanup_receive_handles();
            self.views.receive.reset();
            self.log_info("Transfer declined");
        } else {
            self.log_error("No pending transfer to decline");
        }
    }

    /// Cancel the active receive session.
    fn cancel_receive(&mut self) {
        if let Some(mut handle) = self.receive_session_handle.take() {
            if let Some(tx) = handle.cancel_tx.take() {
                let _ = tx.send(());
            }
            handle.task_handle.abort();
        }
        self.state.receive.active_session = None;
        self.state.receive.is_connecting = false;
        self.receive_event_rx = None;
        self.receive_command_tx = None;
        self.receive_progress_rx = None;
        self.views.receive.reset();
        self.log_info("Receive cancelled");
    }

    /// Refresh clipboard content preview.
    fn refresh_clipboard_preview(&mut self) {
        use yoop_core::clipboard::{ClipboardAccess, ClipboardContent, NativeClipboard};
        use yoop_core::protocol::ClipboardContentType;

        let result = (|| -> Result<Option<ClipboardContent>, yoop_core::Error> {
            let mut clipboard = NativeClipboard::new()?;
            clipboard.read()
        })();

        match result {
            Ok(Some(content)) => {
                self.state.clipboard.content_type = Some(match content.content_type() {
                    ClipboardContentType::PlainText => super::state::ClipboardContentType::Text,
                    ClipboardContentType::ImagePng => super::state::ClipboardContentType::Image,
                });
                self.state.clipboard.preview = Some(content.preview(100));
                #[allow(clippy::cast_possible_truncation)]
                {
                    self.state.clipboard.content_size = Some(content.size() as usize);
                }
            }
            Ok(None) => {
                self.state.clipboard.content_type = None;
                self.state.clipboard.preview = Some("(empty clipboard)".to_string());
                self.state.clipboard.content_size = None;
            }
            Err(e) => {
                self.log_error(&format!("Failed to read clipboard: {}", e));
                self.state.clipboard.content_type = None;
                self.state.clipboard.preview = None;
                self.state.clipboard.content_size = None;
            }
        }
    }

    /// Share clipboard content (one-shot).
    async fn share_clipboard(&mut self) {
        use super::state::ClipboardTaskResult;
        use yoop_core::clipboard::ClipboardShareSession;
        use yoop_core::transfer::TransferConfig;

        self.log_info("Starting clipboard share...");

        if self.state.clipboard.content_type.is_none() {
            self.refresh_clipboard_preview();
            if self.state.clipboard.content_type.is_none() {
                self.log_error("Nothing to share - clipboard is empty");
                return;
            }
        }

        self.state.clipboard.operation_in_progress =
            Some(super::state::ClipboardOperation::Sharing);
        self.state.clipboard.status_message = Some("Starting share...".to_string());

        let config = TransferConfig::default();

        match ClipboardShareSession::new(config).await {
            Ok(session) => {
                let code = session.code().to_string();
                let content_preview = session.content().preview(50);

                self.log_info(&format!(
                    "Sharing clipboard: {} (code: {})",
                    content_preview, code
                ));
                self.state.clipboard.status_message = Some(format!("Code: {} - Waiting...", code));

                let tx = self.clipboard_task_tx.clone();
                let task_handle = tokio::spawn(async move {
                    match session.wait().await {
                        Ok(()) => {
                            let _ = tx.send(ClipboardTaskResult::ShareComplete);
                        }
                        Err(e) => {
                            let _ = tx.send(ClipboardTaskResult::ShareFailed(e.to_string()));
                        }
                    }
                });
                self.clipboard_session_handle = Some(ClipboardSessionHandle { task_handle });
            }
            Err(e) => {
                self.log_error(&format!("Failed to share clipboard: {}", e));
                self.state.clipboard.operation_in_progress = None;
                self.state.clipboard.status_message = Some(format!("Share failed: {}", e));
                self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
            }
        }
    }

    /// Receive clipboard content.
    async fn receive_clipboard(&mut self) {
        use super::state::ClipboardTaskResult;
        use yoop_core::clipboard::ClipboardReceiveSession;
        use yoop_core::transfer::TransferConfig;

        let code = self.state.clipboard.code_input.clone();
        if code.len() < 4 {
            self.log_error("Please enter a 4-character code");
            self.state.clipboard.status_message =
                Some("Enter a 4-character code first".to_string());
            self.state.clipboard.focus = super::state::ClipboardFocus::SyncStatus;
            return;
        }

        self.state.clipboard.operation_in_progress =
            Some(super::state::ClipboardOperation::Receiving);
        self.state.clipboard.status_message = Some(format!("Searching for {}...", code));

        let config = TransferConfig::default();
        let tx = self.clipboard_task_tx.clone();

        let task_handle = tokio::spawn(async move {
            match ClipboardReceiveSession::connect(&code, config).await {
                Ok(mut session) => match session.accept_to_clipboard().await {
                    Ok(()) => {
                        let _ = tx.send(ClipboardTaskResult::ReceiveComplete);
                    }
                    Err(e) => {
                        let _ = tx.send(ClipboardTaskResult::ReceiveFailed(e.to_string()));
                    }
                },
                Err(e) => {
                    let _ = tx.send(ClipboardTaskResult::ReceiveFailed(e.to_string()));
                }
            }
        });

        self.clipboard_session_handle = Some(ClipboardSessionHandle { task_handle });
    }

    /// Start clipboard sync session.
    async fn start_clipboard_sync(&mut self) {
        use super::state::ClipboardTaskResult;
        use yoop_core::clipboard::ClipboardSyncSession;
        use yoop_core::transfer::TransferConfig;

        let code = self.state.clipboard.code_input.clone();
        let config = TransferConfig::default();

        self.state.clipboard.operation_in_progress =
            Some(super::state::ClipboardOperation::StartingSync);

        if code.len() >= 4 {
            self.state.clipboard.status_message = Some(format!("Connecting to {}...", code));

            match ClipboardSyncSession::connect(&code, config).await {
                Ok((session, runner)) => {
                    let peer_name = session.peer_name().to_string();
                    let peer_addr = session.peer_addr().to_string();

                    self.state.clipboard_sync = Some(super::state::ClipboardSyncSession {
                        peer_name: peer_name.clone(),
                        peer_address: peer_addr,
                        items_sent: 0,
                        items_received: 0,
                        started_at: chrono::Utc::now(),
                    });
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;

                    self.log_info(&format!("Clipboard sync started with {}", peer_name));

                    tokio::spawn(async move {
                        let _ = runner.run().await;
                        session.shutdown();
                    });
                }
                Err(e) => {
                    self.log_error(&format!("Failed to join sync: {}", e));
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                }
            }
        } else {
            self.state.clipboard.status_message = Some("Starting sync host...".to_string());

            match ClipboardSyncSession::host(config).await {
                Ok(host_session) => {
                    let code = host_session.code().to_string();
                    self.log_info(&format!("Hosting clipboard sync: {}", code));
                    self.state.clipboard.status_message =
                        Some(format!("Code: {} - Waiting for peer...", code));

                    let tx = self.clipboard_task_tx.clone();
                    let task_handle = tokio::spawn(async move {
                        match host_session.wait_for_peer().await {
                            Ok((session, runner)) => {
                                let peer_name = session.peer_name().to_string();
                                let peer_addr = session.peer_addr().to_string();
                                let _ = tx.send(ClipboardTaskResult::SyncHostConnected {
                                    peer_name,
                                    peer_addr,
                                });
                                let _ = runner.run().await;
                                session.shutdown();
                            }
                            Err(e) => {
                                let _ = tx.send(ClipboardTaskResult::SyncHostFailed(e.to_string()));
                            }
                        }
                    });
                    self.clipboard_session_handle = Some(ClipboardSessionHandle { task_handle });
                }
                Err(e) => {
                    self.log_error(&format!("Failed to host sync: {}", e));
                    self.state.clipboard.operation_in_progress = None;
                    self.state.clipboard.status_message = None;
                    self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
                }
            }
        }

        self.state.clipboard.code_input.clear();
    }

    /// Stop clipboard sync session.
    fn stop_clipboard_sync(&mut self) {
        if self.state.clipboard_sync.is_some() {
            self.state.clipboard_sync = None;
            self.log_info("Clipboard sync stopped");
        }
    }

    /// Cancel an ongoing clipboard operation (share/receive/sync start).
    fn cancel_clipboard_operation(&mut self) {
        if let Some(handle) = self.clipboard_session_handle.take() {
            handle.task_handle.abort();
        }

        if let Some(op) = self.state.clipboard.operation_in_progress.take() {
            self.state.clipboard.status_message = None;
            self.state.clipboard.focus = super::state::ClipboardFocus::Actions;
            let op_name = match op {
                super::state::ClipboardOperation::Sharing => "Share",
                super::state::ClipboardOperation::Receiving => "Receive",
                super::state::ClipboardOperation::StartingSync => "Sync",
            };
            self.log_info(&format!("{} cancelled", op_name));
        }
    }

    /// Open directory browser for sync (directories only).
    fn open_sync_directory_browser(&mut self) {
        use std::collections::HashSet;

        let start_dir = self
            .state
            .sync
            .directory
            .clone()
            .or_else(|| std::env::current_dir().ok());

        let current_dir = start_dir.unwrap_or_else(|| PathBuf::from("/"));

        match load_directories_only(&current_dir, false) {
            Ok(entries) => {
                self.state.sync.file_browser = Some(FileBrowserState {
                    current_dir,
                    entries,
                    selected: 0,
                    scroll: 0,
                    show_hidden: false,
                    filter: None,
                    selections: HashSet::new(),
                });
            }
            Err(e) => {
                self.log_error(&format!("Failed to open directory browser: {}", e));
            }
        }
    }

    /// Start hosting a sync session.
    fn start_sync_host(&mut self) {
        use yoop_core::sync::SyncConfig;
        use yoop_core::transfer::TransferConfig;

        let Some(ref sync_dir) = self.state.sync.directory else {
            self.log_error("Please select a directory first");
            return;
        };

        if self.state.sync.active_session.is_some() {
            self.log_error("Sync session already active");
            return;
        }

        let sync_config = SyncConfig {
            sync_root: sync_dir.clone(),
            follow_symlinks: self.state.sync.follow_symlinks,
            sync_deletions: self.state.sync.sync_deletions,
            exclude_patterns: self.state.sync.exclude_patterns.clone(),
            ..Default::default()
        };

        let transfer_config = TransferConfig::default();

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        self.sync_host_event_rx = Some(event_rx);

        self.log_info("Starting sync session...");

        self.state.sync.active_session = Some(super::state::SyncSession {
            id: uuid::Uuid::new_v4(),
            code: None,
            peer_name: None,
            files_synced: 0,
            last_event: Some("Starting...".to_string()),
        });

        let sync_events_tx = Self::create_sync_event_sender();

        let task_handle = tokio::spawn(async move {
            use yoop_core::sync::SyncSession;

            match SyncSession::host_start(sync_config, transfer_config).await {
                Ok(host_session) => {
                    let _ = event_tx.send(SyncHostEvent::Started {
                        code: host_session.code().to_string(),
                    });

                    match host_session.wait_for_connection().await {
                        Ok(session) => {
                            let peer_name = session.peer_name().to_string();

                            let _ = event_tx.send(SyncHostEvent::PeerConnected { peer_name });

                            let mut session = session;
                            let _ = session
                                .run(move |event| {
                                    let _ = sync_events_tx.send(event);
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = event_tx.send(SyncHostEvent::Failed(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(SyncHostEvent::Failed(e.to_string()));
                }
            }
        });

        self.sync_session_handle = Some(SyncSessionHandle { task_handle });
    }

    /// Join an existing sync session.
    async fn join_sync(&mut self) {
        use yoop_core::sync::{SyncConfig, SyncSession};
        use yoop_core::transfer::TransferConfig;

        let code = self.state.sync.code_input.clone();
        if code.len() < 4 {
            self.log_error("Please enter a 4-character code");
            return;
        }

        let Some(ref sync_dir) = self.state.sync.directory else {
            self.log_error("Please select a directory first");
            return;
        };

        let sync_config = SyncConfig {
            sync_root: sync_dir.clone(),
            follow_symlinks: self.state.sync.follow_symlinks,
            sync_deletions: self.state.sync.sync_deletions,
            exclude_patterns: self.state.sync.exclude_patterns.clone(),
            ..Default::default()
        };

        let transfer_config = TransferConfig::default();

        match SyncSession::connect(&code, sync_config, transfer_config).await {
            Ok(session) => {
                let peer_name = session.peer_name().to_string();
                self.log_info(&format!("Connected to sync session: {}", peer_name));

                self.state.sync.active_session = Some(super::state::SyncSession {
                    id: uuid::Uuid::new_v4(),
                    code: None,
                    peer_name: Some(peer_name),
                    files_synced: 0,
                    last_event: None,
                });

                let events_tx = Self::create_sync_event_sender();
                tokio::spawn(async move {
                    let mut session = session;
                    let _ = session
                        .run(move |event| {
                            let _ = events_tx.send(event);
                        })
                        .await;
                });
            }
            Err(e) => {
                self.log_error(&format!("Failed to join sync: {}", e));
            }
        }

        self.state.sync.code_input.clear();
    }

    /// Stop sync session.
    fn stop_sync(&mut self) {
        if self.state.sync.active_session.is_some() {
            if let Some(handle) = self.sync_session_handle.take() {
                handle.task_handle.abort();
            }
            self.sync_host_event_rx = None;

            self.state.sync.active_session = None;
            self.state.sync.events.clear();
            self.state.sync.stats = super::state::SyncStats::default();

            self.state.sync.focus = super::state::SyncFocus::Directory;
            self.state.sync.option_focus = None;

            self.state.sync.file_browser = None;

            self.state.sync.code_input.clear();

            self.log_info("Sync session stopped");
        }
    }

    /// Create a channel sender for sync events.
    fn create_sync_event_sender() -> std::sync::mpsc::Sender<yoop_core::sync::SyncEvent> {
        let (tx, _rx) = std::sync::mpsc::channel();
        tx
    }

    /// Log an info message.
    fn log_info(&mut self, message: &str) {
        self.state.log.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level: LogLevel::Info,
            message: message.to_string(),
        });
    }

    /// Log an error message.
    fn log_error(&mut self, message: &str) {
        self.state.log.push(LogEntry {
            timestamp: chrono::Utc::now(),
            level: LogLevel::Error,
            message: message.to_string(),
        });
    }

    /// Cancel an external (CLI) transfer by session ID.
    fn cancel_external_transfer(&mut self, session_id: uuid::Uuid) {
        if let Some(session) = self.state.transfers.iter().find(|t| t.id == session_id) {
            let pid = session.pid;
            let code = session.code.clone().unwrap_or_else(|| "???".to_string());

            if pid == std::process::id() {
                self.log_error("Use the view-specific cancel for TUI sessions");
                return;
            }

            #[cfg(unix)]
            {
                if super::session::state_file::signal_cancel(pid) {
                    self.log_info(&format!(
                        "Cancel signal sent to session {} (code: {})",
                        session_id, code
                    ));
                    self.state.transfers.retain(|t| t.id != session_id);
                    return;
                }
            }

            if let Err(e) = super::session::state_file::write_cancel_command(pid) {
                self.log_error(&format!("Failed to cancel session: {}", e));
            } else {
                self.log_info(&format!(
                    "Cancel request sent to session {} (code: {})",
                    session_id, code
                ));
                self.state.transfers.retain(|t| t.id != session_id);
            }
        } else {
            self.log_error("Session not found");
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = self.terminal.backend_mut().execute(LeaveAlternateScreen);
        let _ = self.terminal.backend_mut().execute(DisableMouseCapture);
        let _ = self.terminal.show_cursor();
    }
}

/// Truncate a string to a maximum length, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        ".".repeat(max_len)
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Parse a duration string like "5m", "1h", "30s".
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, multiplier) = if let Some(stripped) = s.strip_suffix('s') {
        (stripped, 1)
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 60)
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 3600)
    } else {
        return None;
    };

    let num: u64 = num_str.parse().ok()?;
    Some(Duration::from_secs(num * multiplier))
}

/// Load CLI sessions from the session state file.
fn load_cli_sessions(
    session_file: &SessionStateFile,
) -> (Vec<TransferSession>, Option<TuiClipboardSync>) {
    let transfers: Vec<TransferSession> = session_file
        .sessions
        .iter()
        .map(|entry| {
            let session_type = match entry.session_type.as_str() {
                "share" => TransferType::Share,
                "receive" => TransferType::Receive,
                "send" => TransferType::Send,
                "sync" => TransferType::Sync,
                _ => TransferType::Share,
            };

            let files: Vec<TransferFile> = entry
                .files
                .iter()
                .map(|f| {
                    let status = match f.status.as_str() {
                        "pending" => FileStatus::Pending,
                        "transferring" => FileStatus::Transferring,
                        "completed" => FileStatus::Completed,
                        "failed" => FileStatus::Failed,
                        _ => FileStatus::Pending,
                    };
                    TransferFile {
                        name: f.name.clone(),
                        size: f.size,
                        transferred: f.transferred,
                        status,
                    }
                })
                .collect();

            TransferSession {
                id: entry.id,
                session_type,
                code: entry.code.clone(),
                peer_name: entry.peer.as_ref().map(|p| p.name.clone()),
                peer_address: entry.peer.as_ref().map(|p| p.address.clone()),
                files,
                progress: TransferProgress {
                    transferred: entry.progress.transferred,
                    total: entry.progress.total,
                    speed_bps: entry.progress.speed_bps,
                },
                started_at: entry.started_at,
                expires_at: entry.expires_at,
                pid: entry.pid,
            }
        })
        .collect();

    let clipboard_sync = session_file
        .clipboard_sync
        .as_ref()
        .map(|sync| TuiClipboardSync {
            peer_name: sync.peer_name.clone(),
            peer_address: sync.peer_address.clone(),
            items_sent: sync.items_sent,
            items_received: sync.items_received,
            started_at: sync.started_at,
        });

    (transfers, clipboard_sync)
}

/// Background task that handles receive session connection and transfer.
async fn run_receive_task(
    code: yoop_core::code::ShareCode,
    output_dir: PathBuf,
    direct_addr: Option<std::net::SocketAddr>,
    config: yoop_core::transfer::TransferConfig,
    event_tx: mpsc::UnboundedSender<ReceiveEvent>,
    mut command_rx: mpsc::UnboundedReceiver<ReceiveCommand>,
    mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let session_result = tokio::select! {
        biased;
        _ = &mut cancel_rx => {
            let _ = event_tx.send(ReceiveEvent::TransferCancelled);
            return;
        }
        result = yoop_core::transfer::ReceiveSession::connect_with_options(
            &code,
            output_dir,
            direct_addr,
            config,
        ) => result
    };

    let mut session = match session_result {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(ReceiveEvent::ConnectionFailed(e.to_string()));
            return;
        }
    };

    let (sender_addr, sender_name) = session.sender();
    let files = session.files().to_vec();
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    let _ = event_tx.send(ReceiveEvent::Connected {
        sender_name: sender_name.to_string(),
        sender_addr: sender_addr.to_string(),
        files: files.clone(),
        total_size,
    });

    let command = tokio::select! {
        biased;
        _ = &mut cancel_rx => {
            let _ = event_tx.send(ReceiveEvent::TransferCancelled);
            return;
        }
        cmd = command_rx.recv() => {
            let Some(c) = cmd else {
                let _ = event_tx.send(ReceiveEvent::TransferCancelled);
                return;
            };
            c
        }
    };

    match command {
        ReceiveCommand::Accept => {
            let progress_rx = session.progress();
            let event_tx_clone = event_tx.clone();

            tokio::spawn(async move {
                let mut progress_rx = progress_rx;
                loop {
                    if progress_rx.changed().await.is_err() {
                        break;
                    }
                    let progress = progress_rx.borrow().clone();
                    let state = progress.state;
                    let _ = event_tx_clone.send(ReceiveEvent::TransferProgress(progress));
                    if state == yoop_core::transfer::TransferState::Completed
                        || state == yoop_core::transfer::TransferState::Failed
                        || state == yoop_core::transfer::TransferState::Cancelled
                    {
                        break;
                    }
                }
            });

            let accept_result = tokio::select! {
                biased;
                _ = cancel_rx => {
                    let _ = event_tx.send(ReceiveEvent::TransferCancelled);
                    return;
                }
                result = session.accept() => result
            };

            match accept_result {
                Ok(()) => {
                    let _ = event_tx.send(ReceiveEvent::TransferComplete);
                }
                Err(e) => {
                    let _ = event_tx.send(ReceiveEvent::TransferFailed(e.to_string()));
                }
            }
        }
        ReceiveCommand::Decline => {
            session.decline().await;
            let _ = event_tx.send(ReceiveEvent::TransferCancelled);
        }
    }
}

/// Background task that handles receive session from trusted device.
async fn run_receive_trusted_task(
    device: yoop_core::trust::TrustedDevice,
    output_dir: PathBuf,
    config: yoop_core::transfer::TransferConfig,
    event_tx: mpsc::UnboundedSender<ReceiveEvent>,
    mut command_rx: mpsc::UnboundedReceiver<ReceiveCommand>,
    mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let session_result = tokio::select! {
        biased;
        _ = &mut cancel_rx => {
            let _ = event_tx.send(ReceiveEvent::TransferCancelled);
            return;
        }
        result = yoop_core::transfer::ReceiveSession::connect_trusted(&device, output_dir, config) => result
    };

    let mut session = match session_result {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(ReceiveEvent::ConnectionFailed(e.to_string()));
            return;
        }
    };

    let (sender_addr, sender_name) = session.sender();
    let files = session.files().to_vec();
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    let _ = event_tx.send(ReceiveEvent::Connected {
        sender_name: sender_name.to_string(),
        sender_addr: sender_addr.to_string(),
        files: files.clone(),
        total_size,
    });

    let command = tokio::select! {
        biased;
        _ = &mut cancel_rx => {
            let _ = event_tx.send(ReceiveEvent::TransferCancelled);
            return;
        }
        cmd = command_rx.recv() => {
            let Some(c) = cmd else {
                let _ = event_tx.send(ReceiveEvent::TransferCancelled);
                return;
            };
            c
        }
    };

    match command {
        ReceiveCommand::Accept => {
            let progress_rx = session.progress();
            let event_tx_clone = event_tx.clone();

            tokio::spawn(async move {
                let mut progress_rx = progress_rx;
                loop {
                    if progress_rx.changed().await.is_err() {
                        break;
                    }
                    let progress = progress_rx.borrow().clone();
                    let state = progress.state;
                    let _ = event_tx_clone.send(ReceiveEvent::TransferProgress(progress));
                    if state == yoop_core::transfer::TransferState::Completed
                        || state == yoop_core::transfer::TransferState::Failed
                        || state == yoop_core::transfer::TransferState::Cancelled
                    {
                        break;
                    }
                }
            });

            let accept_result = tokio::select! {
                biased;
                _ = cancel_rx => {
                    let _ = event_tx.send(ReceiveEvent::TransferCancelled);
                    return;
                }
                result = session.accept() => result
            };

            match accept_result {
                Ok(()) => {
                    let _ = event_tx.send(ReceiveEvent::TransferComplete);
                }
                Err(e) => {
                    let _ = event_tx.send(ReceiveEvent::TransferFailed(e.to_string()));
                }
            }
        }
        ReceiveCommand::Decline => {
            session.decline().await;
            let _ = event_tx.send(ReceiveEvent::TransferCancelled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("invalid"), None);
    }
}
