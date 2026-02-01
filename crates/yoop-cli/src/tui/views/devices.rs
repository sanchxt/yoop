//! Devices view for TUI.
//!
//! Provides the interface for managing trusted devices.

use std::net::IpAddr;
use std::time::SystemTime;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::state::{AppState, DevicesFocus};
use crate::tui::theme::Theme;

/// Trusted device information for display.
#[derive(Debug, Clone)]
pub struct TuiTrustedDevice {
    /// Unique device identifier
    pub device_id: uuid::Uuid,
    /// Display name
    pub device_name: String,
    /// Trust level display string
    pub trust_level: String,
    /// Whether this is "Full" trust
    pub is_full_trust: bool,
    /// Number of transfers
    pub transfer_count: u32,
    /// Last seen timestamp
    pub last_seen: SystemTime,
    /// Last known IP address
    pub last_known_ip: Option<IpAddr>,
    /// Last known port
    pub last_known_port: Option<u16>,
    /// Whether the device is considered "online" (seen recently)
    pub is_online: bool,
}

impl TuiTrustedDevice {
    /// Format the last seen time as a human-readable string.
    pub fn last_seen_str(&self) -> String {
        let now = SystemTime::now();
        let duration = now.duration_since(self.last_seen).unwrap_or_default();

        let secs = duration.as_secs();
        if secs < 60 {
            "just now".to_string()
        } else if secs < 3600 {
            let mins = secs / 60;
            format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
        } else if secs < 86400 {
            let hours = secs / 3600;
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else {
            let days = secs / 86400;
            if days == 1 {
                "yesterday".to_string()
            } else {
                format!("{} days ago", days)
            }
        }
    }

    /// Get address string if available.
    pub fn address_str(&self) -> Option<String> {
        match (self.last_known_ip, self.last_known_port) {
            (Some(ip), Some(port)) => Some(format!("{}:{}", ip, port)),
            (Some(ip), None) => Some(ip.to_string()),
            _ => None,
        }
    }
}

/// Devices view component.
pub struct DevicesView {
    /// List state for device selection
    list_state: ListState,
    /// Cached list of devices
    pub devices: Vec<TuiTrustedDevice>,
}

impl DevicesView {
    /// Create a new devices view.
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            devices: Vec::new(),
        }
    }

    /// Render the devices view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        if self.devices.is_empty() {
            self.render_empty_state(frame, area, theme);
            return;
        }

        if state.devices.confirm_remove {
            self.render_remove_confirmation(frame, area, state, theme);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.render_device_list(frame, chunks[0], state, theme);

        self.render_device_details(frame, chunks[1], state, theme);
    }

    /// Render empty state when no devices are trusted.
    #[allow(clippy::unused_self)]
    fn render_empty_state(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .title(" Trusted Devices ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "No trusted devices yet",
                Style::default()
                    .fg(theme.text_muted)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Complete a transfer with another device to add them to your trusted list.",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [S] for Share or [R] for Receive to start.",
                Style::default().fg(theme.text_secondary),
            )),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Render the device list.
    fn render_device_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let focused = state.devices.focus == DevicesFocus::DeviceList;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Devices ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let mut online_devices: Vec<&TuiTrustedDevice> = Vec::new();
        let mut offline_devices: Vec<&TuiTrustedDevice> = Vec::new();

        for device in &self.devices {
            if device.is_online {
                online_devices.push(device);
            } else {
                offline_devices.push(device);
            }
        }

        let mut items: Vec<ListItem> = Vec::new();

        if !online_devices.is_empty() {
            items.push(ListItem::new(Line::from(Span::styled(
                "Online",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ))));

            for device in &online_devices {
                let status_icon = "●";
                let trust_icon = if device.is_full_trust { "★" } else { "☆" };
                let text = format!("  {} {} {}", status_icon, trust_icon, device.device_name);
                items.push(ListItem::new(Span::styled(
                    text,
                    Style::default().fg(theme.text_primary),
                )));
            }
        }

        if !offline_devices.is_empty() {
            if !online_devices.is_empty() {
                items.push(ListItem::new(Line::from("")));
            }
            items.push(ListItem::new(Line::from(Span::styled(
                "Offline",
                Style::default()
                    .fg(theme.text_muted)
                    .add_modifier(Modifier::BOLD),
            ))));

            for device in &offline_devices {
                let status_icon = "○";
                let trust_icon = if device.is_full_trust { "★" } else { "☆" };
                let text = format!("  {} {} {}", status_icon, trust_icon, device.device_name);
                items.push(ListItem::new(Span::styled(
                    text,
                    Style::default().fg(theme.text_muted),
                )));
            }
        }

        self.list_state.select(Some(state.devices.selected_index));

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(theme.selection)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Render device details panel.
    fn render_device_details(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let focused = state.devices.focus == DevicesFocus::Details;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Details ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let device = self.get_selected_device(state);

        if let Some(device) = device {
            self.render_device_info(frame, inner, device, state, theme);
        } else {
            let text = Paragraph::new(Span::styled(
                "No device selected",
                Style::default().fg(theme.text_muted),
            ))
            .alignment(Alignment::Center);
            frame.render_widget(text, inner);
        }
    }

    /// Render detailed info for a device.
    #[allow(clippy::unused_self, clippy::too_many_lines)]
    fn render_device_info(
        &self,
        frame: &mut Frame,
        area: Rect,
        device: &TuiTrustedDevice,
        state: &AppState,
        theme: &Theme,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(4),
            ])
            .split(area);

        let name_style = Style::default()
            .fg(theme.text_primary)
            .add_modifier(Modifier::BOLD);
        let name = Paragraph::new(vec![
            Line::from(Span::styled("Name", Style::default().fg(theme.text_muted))),
            Line::from(Span::styled(&device.device_name, name_style)),
        ]);
        frame.render_widget(name, chunks[0]);

        let (status_text, status_color) = if device.is_online {
            ("Online", theme.success)
        } else {
            ("Offline", theme.text_muted)
        };
        let status = Paragraph::new(vec![
            Line::from(Span::styled(
                "Status",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(status_text, Style::default().fg(status_color))),
        ]);
        frame.render_widget(status, chunks[1]);

        let trust_label = if state.devices.editing_trust_level {
            "Trust Level (editing)"
        } else {
            "Trust Level"
        };
        let trust_style = if state.devices.editing_trust_level {
            Style::default().fg(theme.accent)
        } else {
            Style::default().fg(theme.text_primary)
        };
        let trust = Paragraph::new(vec![
            Line::from(Span::styled(
                trust_label,
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(&device.trust_level, trust_style)),
        ]);
        frame.render_widget(trust, chunks[2]);

        let addr_text = device
            .address_str()
            .unwrap_or_else(|| "Unknown".to_string());
        let address = Paragraph::new(vec![
            Line::from(Span::styled(
                "Address",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(
                addr_text,
                Style::default().fg(theme.text_secondary),
            )),
        ]);
        frame.render_widget(address, chunks[3]);

        let transfers = Paragraph::new(vec![
            Line::from(Span::styled(
                "Transfers",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(
                device.transfer_count.to_string(),
                Style::default().fg(theme.text_primary),
            )),
        ]);
        frame.render_widget(transfers, chunks[4]);

        let last_seen = Paragraph::new(vec![
            Line::from(Span::styled(
                "Last Seen",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(
                device.last_seen_str(),
                Style::default().fg(theme.text_secondary),
            )),
        ]);
        frame.render_widget(last_seen, chunks[5]);

        let actions = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "[Enter] Send files  [E] Edit trust  [X] Remove",
                Style::default().fg(theme.text_muted),
            )),
            Line::from(Span::styled(
                "[Tab] Switch focus  [R] Refresh",
                Style::default().fg(theme.text_muted),
            )),
        ]);
        frame.render_widget(actions, chunks[6]);
    }

    /// Render removal confirmation dialog.
    fn render_remove_confirmation(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let block = Block::default()
            .title(" Confirm Removal ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.warning));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let device = self.get_selected_device(state);
        let device_name = device.map_or("Unknown", |d| d.device_name.as_str());

        let content = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                format!("Remove '{}' from trusted devices?", device_name),
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "This device will need to be re-trusted for future transfers.",
                Style::default().fg(theme.text_secondary),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Y] ", Style::default().fg(theme.error)),
                Span::styled("Yes, remove", Style::default().fg(theme.text_primary)),
                Span::styled("    ", Style::default()),
                Span::styled("[N] ", Style::default().fg(theme.success)),
                Span::styled("No, cancel", Style::default().fg(theme.text_primary)),
            ]),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Get the currently selected device.
    pub fn get_selected_device(&self, state: &AppState) -> Option<&TuiTrustedDevice> {
        if state.devices.selected_index < self.devices.len() {
            self.devices.get(state.devices.selected_index)
        } else {
            None
        }
    }

    /// Load devices from the trust store.
    pub fn load_devices(&mut self) {
        self.devices.clear();

        if let Ok(store) = yoop_core::trust::TrustStore::load() {
            let now = SystemTime::now();

            for device in store.list() {
                let is_online = now
                    .duration_since(device.last_seen)
                    .map(|d| d.as_secs() < 300)
                    .unwrap_or(false);

                let trust_level = match device.trust_level {
                    yoop_core::config::TrustLevel::Full => "Full".to_string(),
                    yoop_core::config::TrustLevel::AskEachTime => "Ask Each Time".to_string(),
                };

                self.devices.push(TuiTrustedDevice {
                    device_id: device.device_id,
                    device_name: device.device_name.clone(),
                    trust_level,
                    is_full_trust: device.trust_level == yoop_core::config::TrustLevel::Full,
                    transfer_count: device.transfer_count,
                    last_seen: device.last_seen,
                    last_known_ip: device.last_known_ip,
                    last_known_port: device.last_known_port,
                    is_online,
                });
            }

            self.devices
                .sort_by(|a, b| match (a.is_online, b.is_online) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.last_seen.cmp(&a.last_seen),
                });
        }
    }

    /// Cycle focus to next element.
    pub fn focus_next(&mut self, state: &mut super::super::state::DevicesState) {
        state.focus = match state.focus {
            DevicesFocus::DeviceList => DevicesFocus::Details,
            DevicesFocus::Details => DevicesFocus::DeviceList,
        };
    }

    /// Remove a device by ID.
    pub fn remove_device(&mut self, device_id: &uuid::Uuid) -> bool {
        if let Ok(mut store) = yoop_core::trust::TrustStore::load() {
            if store.remove(device_id).is_ok() {
                self.devices.retain(|d| &d.device_id != device_id);
                return true;
            }
        }
        false
    }

    /// Toggle trust level for a device.
    pub fn toggle_trust_level(&mut self, device_id: &uuid::Uuid) -> bool {
        if let Ok(mut store) = yoop_core::trust::TrustStore::load() {
            if let Some(device) = store.list().iter().find(|d| &d.device_id == device_id) {
                let new_level = match device.trust_level {
                    yoop_core::config::TrustLevel::Full => {
                        yoop_core::config::TrustLevel::AskEachTime
                    }
                    yoop_core::config::TrustLevel::AskEachTime => {
                        yoop_core::config::TrustLevel::Full
                    }
                };

                if store.set_trust_level(device_id, new_level).is_ok() {
                    if let Some(tui_device) =
                        self.devices.iter_mut().find(|d| &d.device_id == device_id)
                    {
                        tui_device.is_full_trust = new_level == yoop_core::config::TrustLevel::Full;
                        tui_device.trust_level = match new_level {
                            yoop_core::config::TrustLevel::Full => "Full".to_string(),
                            yoop_core::config::TrustLevel::AskEachTime => {
                                "Ask Each Time".to_string()
                            }
                        };
                    }
                    return true;
                }
            }
        }
        false
    }
}

impl Default for DevicesView {
    fn default() -> Self {
        Self::new()
    }
}
