//! Device list component for TUI.
//!
//! Displays a list of trusted devices with online status.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Information about a trusted device for display.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Device name
    pub name: String,
    /// Device address (if known)
    pub address: Option<String>,
    /// Last seen timestamp
    pub last_seen: Option<String>,
    /// Whether the device appears to be online
    pub is_online: bool,
    /// Trust level display string
    pub trust_level: String,
    /// Number of successful transfers
    pub transfer_count: u64,
}

impl DeviceInfo {
    /// Create a new device info from a trusted device.
    pub fn from_trusted_device(device: &yoop_core::trust::TrustedDevice) -> Self {
        let address = device
            .address()
            .map(|(ip, port)| format!("{}:{}", ip, port));

        let last_seen = {
            let duration = device.last_seen.elapsed().ok();
            duration.map(|d| {
                let secs = d.as_secs();
                if secs < 60 {
                    "just now".to_string()
                } else if secs < 3600 {
                    format!("{} min ago", secs / 60)
                } else if secs < 86400 {
                    format!("{} hour(s) ago", secs / 3600)
                } else {
                    format!("{} day(s) ago", secs / 86400)
                }
            })
        };

        let trust_level = match device.trust_level {
            yoop_core::config::TrustLevel::Full => "Full".to_string(),
            yoop_core::config::TrustLevel::AskEachTime => "Ask".to_string(),
        };

        Self {
            name: device.device_name.clone(),
            address,
            last_seen,
            is_online: false,
            trust_level,
            transfer_count: u64::from(device.transfer_count),
        }
    }
}

/// Device list component for displaying trusted devices.
pub struct DeviceList {
    /// List state for selection
    list_state: ListState,
}

impl DeviceList {
    /// Create a new device list.
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self { list_state }
    }

    /// Render the device list.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        devices: &[DeviceInfo],
        selected_index: usize,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Trusted Devices ")
            .borders(Borders::ALL)
            .border_style(border_style);

        if devices.is_empty() {
            let empty_text = "No trusted devices. Complete a transfer to trust a device.";
            let paragraph = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                empty_text,
                Style::default().fg(theme.text_muted),
            )))
            .block(block);

            frame.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = devices
            .iter()
            .enumerate()
            .map(|(i, device)| {
                let is_selected = i == selected_index;
                let status_icon = if device.is_online { "●" } else { "○" };
                let status_color = if device.is_online {
                    theme.success
                } else {
                    theme.text_muted
                };

                let name_style = if is_selected && focused {
                    Style::default()
                        .fg(theme.text_primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_primary)
                };

                let addr_text = device
                    .address
                    .as_ref()
                    .map_or_else(|| "no address".to_string(), Clone::clone);

                let last_seen_text = device
                    .last_seen
                    .as_ref()
                    .map_or_else(|| "never".to_string(), Clone::clone);

                let spans = vec![
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(&device.name, name_style),
                    Span::raw("  "),
                    Span::styled(
                        format!("({}) ", addr_text),
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::styled(
                        format!("Trust: {} | ", device.trust_level),
                        Style::default().fg(theme.text_muted),
                    ),
                    Span::styled(last_seen_text, Style::default().fg(theme.text_muted)),
                ];

                ListItem::new(Line::from(spans))
            })
            .collect();

        self.list_state.select(Some(selected_index));

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

    /// Render a compact version of the device list.
    pub fn render_compact(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        devices: &[DeviceInfo],
        selected_index: usize,
        focused: bool,
        theme: &Theme,
    ) {
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Devices ")
            .borders(Borders::ALL)
            .border_style(border_style);

        if devices.is_empty() {
            let paragraph = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                "No trusted devices",
                Style::default().fg(theme.text_muted),
            )))
            .block(block);

            frame.render_widget(paragraph, area);
            return;
        }

        let items: Vec<ListItem> = devices
            .iter()
            .enumerate()
            .map(|(i, device)| {
                let is_selected = i == selected_index;
                let status_icon = if device.is_online { "●" } else { "○" };
                let status_color = if device.is_online {
                    theme.success
                } else {
                    theme.text_muted
                };

                let name_style = if is_selected && focused {
                    Style::default()
                        .fg(theme.text_primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text_primary)
                };

                let spans = vec![
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(&device.name, name_style),
                ];

                ListItem::new(Line::from(spans))
            })
            .collect();

        self.list_state.select(Some(selected_index));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(theme.selection))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}

impl Default for DeviceList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_info_creation() {
        let info = DeviceInfo {
            name: "Test Device".to_string(),
            address: Some("192.168.1.1:52530".to_string()),
            last_seen: Some("2 min ago".to_string()),
            is_online: true,
            trust_level: "Full".to_string(),
            transfer_count: 5,
        };

        assert_eq!(info.name, "Test Device");
        assert!(info.is_online);
    }
}
