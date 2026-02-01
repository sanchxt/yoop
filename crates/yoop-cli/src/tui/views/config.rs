//! Configuration view for TUI.
//!
//! Provides the interface for viewing and editing Yoop configuration.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::state::{AppState, ConfigFocus, ConfigSection, ConfigState};
use crate::tui::theme::Theme;

/// Configuration setting type.
#[derive(Debug, Clone)]
pub enum ConfigSettingType {
    /// String value
    String,
    /// Integer value
    Integer,
    /// Boolean value (toggle)
    Boolean,
    /// Duration value (e.g., "5m", "300s")
    Duration,
    /// Enum with options
    Enum(Vec<&'static str>),
    /// Optional path
    OptionalPath,
}

/// A displayable configuration setting.
#[derive(Debug, Clone)]
pub struct ConfigSetting {
    /// Setting name (key)
    pub key: &'static str,
    /// Display label
    pub label: &'static str,
    /// Description
    pub description: &'static str,
    /// Current value (formatted as string)
    pub value: String,
    /// Pending value (if different from saved)
    pub pending_value: Option<String>,
    /// Setting type
    pub setting_type: ConfigSettingType,
}

impl ConfigSetting {
    /// Check if this setting has pending changes.
    pub fn has_changes(&self) -> bool {
        self.pending_value.is_some()
    }

    /// Get the display value (pending if exists, otherwise current).
    pub fn display_value(&self) -> &str {
        self.pending_value.as_deref().unwrap_or(&self.value)
    }
}

/// Configuration view component.
pub struct ConfigView {
    /// List state for section selection
    section_list_state: ListState,
    /// List state for setting selection
    setting_list_state: ListState,
    /// Cached config (loaded from file)
    pub config: Option<yoop_core::config::Config>,
    /// Settings for each section
    settings_cache: Vec<Vec<ConfigSetting>>,
    /// Whether config has been modified
    has_modifications: bool,
}

impl ConfigView {
    /// Create a new config view.
    pub fn new() -> Self {
        let mut section_list_state = ListState::default();
        section_list_state.select(Some(0));
        let mut setting_list_state = ListState::default();
        setting_list_state.select(Some(0));

        Self {
            section_list_state,
            setting_list_state,
            config: None,
            settings_cache: Vec::new(),
            has_modifications: false,
        }
    }

    /// Load configuration from file.
    pub fn load_config(&mut self) {
        match yoop_core::config::Config::load() {
            Ok(config) => {
                self.rebuild_settings_cache(&config);
                self.config = Some(config);
                self.has_modifications = false;
            }
            Err(e) => {
                tracing::warn!("Failed to load config: {}", e);
                let config = yoop_core::config::Config::default();
                self.rebuild_settings_cache(&config);
                self.config = Some(config);
            }
        }
    }

    /// Rebuild the settings cache from the config.
    #[allow(clippy::too_many_lines)]
    fn rebuild_settings_cache(&mut self, config: &yoop_core::config::Config) {
        self.settings_cache.clear();

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "device_name",
                label: "Device Name",
                description: "Display name on the network",
                value: config.general.device_name.clone(),
                pending_value: None,
                setting_type: ConfigSettingType::String,
            },
            ConfigSetting {
                key: "default_expire",
                label: "Default Expiration",
                description: "Default code expiration time",
                value: format!("{}s", config.general.default_expire.as_secs()),
                pending_value: None,
                setting_type: ConfigSettingType::Duration,
            },
            ConfigSetting {
                key: "default_output",
                label: "Default Output",
                description: "Default directory for received files",
                value: config
                    .general
                    .default_output
                    .as_ref()
                    .map_or_else(|| "(not set)".to_string(), |p| p.display().to_string()),
                pending_value: None,
                setting_type: ConfigSettingType::OptionalPath,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "port",
                label: "Discovery Port",
                description: "UDP port for discovery",
                value: config.network.port.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "transfer_port_range",
                label: "Transfer Port Range",
                description: "Port range for file transfers",
                value: format!(
                    "{}-{}",
                    config.network.transfer_port_range.0, config.network.transfer_port_range.1
                ),
                pending_value: None,
                setting_type: ConfigSettingType::String,
            },
            ConfigSetting {
                key: "interface",
                label: "Network Interface",
                description: "Network interface (auto or specific)",
                value: config.network.interface.clone(),
                pending_value: None,
                setting_type: ConfigSettingType::String,
            },
            ConfigSetting {
                key: "ipv6",
                label: "Enable IPv6",
                description: "Enable IPv6 support",
                value: if config.network.ipv6 {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "chunk_size",
                label: "Chunk Size",
                description: "Size of transfer chunks",
                value: format_bytes(config.transfer.chunk_size),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "parallel_chunks",
                label: "Parallel Chunks",
                description: "Number of parallel chunk streams",
                value: config.transfer.parallel_chunks.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "bandwidth_limit",
                label: "Bandwidth Limit",
                description: "Maximum transfer speed (unlimited if not set)",
                value: config
                    .transfer
                    .bandwidth_limit
                    .map_or_else(|| "unlimited".to_string(), format_bytes_u64),
                pending_value: None,
                setting_type: ConfigSettingType::String,
            },
            ConfigSetting {
                key: "compression",
                label: "Compression",
                description: "Compression mode",
                value: format!("{:?}", config.transfer.compression).to_lowercase(),
                pending_value: None,
                setting_type: ConfigSettingType::Enum(vec!["auto", "always", "never"]),
            },
            ConfigSetting {
                key: "compression_level",
                label: "Compression Level",
                description: "Compression level (1-3, lower = faster)",
                value: config.transfer.compression_level.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "verify_checksum",
                label: "Verify Checksums",
                description: "Verify checksums after transfer",
                value: if config.transfer.verify_checksum {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "require_pin",
                label: "Require PIN",
                description: "Require additional PIN for transfers",
                value: if config.security.require_pin {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "require_approval",
                label: "Require Approval",
                description: "Require manual approval for transfers",
                value: if config.security.require_approval {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "tls_verify",
                label: "TLS Verification",
                description: "Verify TLS certificates",
                value: if config.security.tls_verify {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "rate_limit_attempts",
                label: "Rate Limit Attempts",
                description: "Failed attempts before lockout",
                value: config.security.rate_limit_attempts.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "rate_limit_window",
                label: "Lockout Duration",
                description: "Duration of lockout after failed attempts",
                value: format!("{}s", config.security.rate_limit_window.as_secs()),
                pending_value: None,
                setting_type: ConfigSettingType::Duration,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "preview.enabled",
                label: "Preview Generation",
                description: "Enable file preview generation",
                value: if config.preview.enabled {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "preview.max_image_size",
                label: "Max Image Size",
                description: "Maximum thumbnail size",
                value: format_bytes(config.preview.max_image_size),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "preview.max_text_length",
                label: "Max Text Length",
                description: "Maximum text preview characters",
                value: config.preview.max_text_length.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "history.enabled",
                label: "Transfer History",
                description: "Enable transfer history tracking",
                value: if config.history.enabled {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "history.max_entries",
                label: "Max Entries",
                description: "Maximum history entries to keep",
                value: config.history.max_entries.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "history.auto_clear_days",
                label: "Auto Clear Days",
                description: "Auto-delete entries older than N days",
                value: config
                    .history
                    .auto_clear_days
                    .map_or_else(|| "disabled".to_string(), |d| d.to_string()),
                pending_value: None,
                setting_type: ConfigSettingType::String,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "enabled",
                label: "Trusted Devices",
                description: "Enable trusted devices feature",
                value: if config.trust.enabled {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "auto_prompt",
                label: "Auto Prompt",
                description: "Prompt to trust after transfer",
                value: if config.trust.auto_prompt {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "default_level",
                label: "Default Trust Level",
                description: "Default trust level for new devices",
                value: match config.trust.default_level {
                    yoop_core::config::TrustLevel::Full => "Full".to_string(),
                    yoop_core::config::TrustLevel::AskEachTime => "Ask Each Time".to_string(),
                },
                pending_value: None,
                setting_type: ConfigSettingType::Enum(vec!["full", "ask_each_time"]),
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "web.enabled",
                label: "Web Server",
                description: "Enable web interface by default",
                value: if config.web.enabled {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "web.port",
                label: "Web Port",
                description: "Web server port",
                value: config.web.port.to_string(),
                pending_value: None,
                setting_type: ConfigSettingType::Integer,
            },
            ConfigSetting {
                key: "web.auth",
                label: "Require Auth",
                description: "Require authentication for web interface",
                value: if config.web.auth {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "web.localhost_only",
                label: "Localhost Only",
                description: "Bind web server to localhost only",
                value: if config.web.localhost_only {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "theme",
                label: "Theme",
                description: "Color theme (dark, light, auto)",
                value: config.ui.theme.clone(),
                pending_value: None,
                setting_type: ConfigSettingType::Enum(vec!["dark", "light", "auto"]),
            },
            ConfigSetting {
                key: "show_qr",
                label: "Show QR Codes",
                description: "Display QR codes for share codes",
                value: if config.ui.show_qr {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "notifications",
                label: "Notifications",
                description: "Enable desktop notifications",
                value: if config.ui.notifications {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "sound",
                label: "Sound",
                description: "Play sound on completion",
                value: if config.ui.sound {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
        ]);

        self.settings_cache.push(vec![
            ConfigSetting {
                key: "auto_check",
                label: "Auto Check",
                description: "Automatically check for updates",
                value: if config.update.auto_check {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
            ConfigSetting {
                key: "check_interval",
                label: "Check Interval",
                description: "Interval between update checks",
                value: format!("{}s", config.update.check_interval.as_secs()),
                pending_value: None,
                setting_type: ConfigSettingType::Duration,
            },
            ConfigSetting {
                key: "package_manager",
                label: "Package Manager",
                description: "Preferred package manager for updates",
                value: config.update.package_manager.map_or_else(
                    || "auto".to_string(),
                    |pm| format!("{:?}", pm).to_lowercase(),
                ),
                pending_value: None,
                setting_type: ConfigSettingType::Enum(vec!["auto", "npm", "pnpm", "yarn", "bun"]),
            },
            ConfigSetting {
                key: "notify",
                label: "Update Notifications",
                description: "Show update notifications",
                value: if config.update.notify {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                pending_value: None,
                setting_type: ConfigSettingType::Boolean,
            },
        ]);
    }

    /// Render the config view.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        if self.config.is_none() {
            self.load_config();
        }

        if state.config.confirm_save {
            self.render_save_confirmation(frame, area, theme);
            return;
        }
        if state.config.confirm_revert {
            self.render_revert_confirmation(frame, area, theme);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(25), Constraint::Min(40)])
            .split(area);

        self.render_section_list(frame, chunks[0], state, theme);

        self.render_settings(frame, chunks[1], state, theme);
    }

    /// Render the section list.
    fn render_section_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let focused = state.config.focus == ConfigFocus::SectionList;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let block = Block::default()
            .title(" Sections ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let sections = ConfigSection::all();
        let items: Vec<ListItem> = sections
            .iter()
            .enumerate()
            .map(|(i, section)| {
                let is_selected = i == state.config.selected_section;
                let has_changes = self.section_has_changes(i);

                let prefix = if has_changes { "* " } else { "  " };
                let style = if is_selected && focused {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else if has_changes {
                    Style::default().fg(theme.warning)
                } else {
                    Style::default().fg(theme.text_primary)
                };

                ListItem::new(format!("{}{}", prefix, section.name())).style(style)
            })
            .collect();

        self.section_list_state
            .select(Some(state.config.selected_section));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(theme.selection))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.section_list_state);
    }

    /// Check if a section has pending changes.
    fn section_has_changes(&self, section_index: usize) -> bool {
        self.settings_cache
            .get(section_index)
            .is_some_and(|settings| settings.iter().any(ConfigSetting::has_changes))
    }

    /// Render the settings panel.
    fn render_settings(&mut self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let focused = state.config.focus == ConfigFocus::Settings;
        let border_style = if focused {
            Style::default().fg(theme.border_focused)
        } else {
            Style::default().fg(theme.border)
        };

        let section = state.config.current_section();
        let block = Block::default()
            .title(format!(" {} Settings ", section.name()))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(5)])
            .split(inner);

        self.render_settings_list(frame, chunks[0], state, theme);

        self.render_help_status(frame, chunks[1], state, theme);
    }

    /// Render the settings list for current section.
    fn render_settings_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let section_index = state.config.selected_section;
        let settings = self.settings_cache.get(section_index);

        if let Some(settings) = settings {
            let items: Vec<ListItem> = settings
                .iter()
                .enumerate()
                .map(|(i, setting)| {
                    let is_selected = i == state.config.selected_setting
                        && state.config.focus == ConfigFocus::Settings;
                    let is_editing = is_selected && state.config.editing;

                    let value_display = if is_editing {
                        format!("[{}]", state.config.edit_buffer)
                    } else {
                        setting.display_value().to_string()
                    };

                    let change_indicator = if setting.has_changes() { "*" } else { " " };

                    let style = if is_selected {
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD)
                    } else if setting.has_changes() {
                        Style::default().fg(theme.warning)
                    } else {
                        Style::default().fg(theme.text_primary)
                    };

                    let value_style = if is_editing {
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD)
                    } else if setting.has_changes() {
                        Style::default().fg(theme.warning)
                    } else {
                        Style::default().fg(theme.text_secondary)
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{} {}: ", change_indicator, setting.label), style),
                        Span::styled(value_display, value_style),
                    ]))
                })
                .collect();

            self.setting_list_state
                .select(Some(state.config.selected_setting));

            let list = List::new(items).highlight_symbol("> ");

            frame.render_stateful_widget(list, area, &mut self.setting_list_state);
        }
    }

    /// Render help and status area.
    #[allow(clippy::unused_self)]
    fn render_help_status(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let has_changes = state.config.has_changes || self.has_modifications;

        let mut lines = Vec::new();

        if let Some(ref msg) = state.config.status_message {
            lines.push(Line::from(Span::styled(
                msg.clone(),
                Style::default().fg(theme.info),
            )));
        }

        if has_changes {
            lines.push(Line::from(Span::styled(
                "* Unsaved changes",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::ITALIC),
            )));
        }

        let help_text = if state.config.editing {
            "[Enter] Confirm  [Esc] Cancel"
        } else {
            match state.config.focus {
                ConfigFocus::SectionList => {
                    "[j/k] Navigate  [Tab/Enter] Switch to settings"
                }
                ConfigFocus::Settings => {
                    "[j/k] Navigate  [Enter] Edit  [Space] Toggle  [Tab] Sections  [Ctrl+S] Save  [Ctrl+R] Revert"
                }
            }
        };

        lines.push(Line::from(Span::styled(
            help_text,
            Style::default().fg(theme.text_muted),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
    }

    /// Render save confirmation dialog.
    #[allow(clippy::unused_self)]
    fn render_save_confirmation(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .title(" Save Configuration ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_focused));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Save changes to configuration file?",
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "This will overwrite your existing config.toml",
                Style::default().fg(theme.text_secondary),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Y] ", Style::default().fg(theme.accent)),
                Span::styled("Yes, save", Style::default().fg(theme.text_primary)),
                Span::styled("    ", Style::default()),
                Span::styled("[N/Esc] ", Style::default().fg(theme.accent)),
                Span::styled("No, cancel", Style::default().fg(theme.text_primary)),
            ]),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Render revert confirmation dialog.
    #[allow(clippy::unused_self)]
    fn render_revert_confirmation(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .title(" Revert Changes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_focused));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Discard all unsaved changes?",
                Style::default()
                    .fg(theme.text_primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "This will reload the configuration from disk.",
                Style::default().fg(theme.text_secondary),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Y] ", Style::default().fg(theme.accent)),
                Span::styled("Yes, revert", Style::default().fg(theme.text_primary)),
                Span::styled("    ", Style::default()),
                Span::styled("[N/Esc] ", Style::default().fg(theme.accent)),
                Span::styled("No, cancel", Style::default().fg(theme.text_primary)),
            ]),
        ];

        let paragraph = Paragraph::new(content).alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }

    /// Focus on the next element.
    pub fn focus_next(&mut self, state: &mut ConfigState) {
        state.focus = match state.focus {
            ConfigFocus::SectionList => ConfigFocus::Settings,
            ConfigFocus::Settings => ConfigFocus::SectionList,
        };
    }

    /// Get settings for current section.
    pub fn current_settings(&self, state: &ConfigState) -> Option<&[ConfigSetting]> {
        self.settings_cache
            .get(state.selected_section)
            .map(Vec::as_slice)
    }

    /// Get mutable settings for current section.
    pub fn current_settings_mut(&mut self, state: &ConfigState) -> Option<&mut Vec<ConfigSetting>> {
        self.settings_cache.get_mut(state.selected_section)
    }

    /// Start editing the current setting.
    pub fn start_edit(&self, state: &mut ConfigState) {
        if let Some(settings) = self.settings_cache.get(state.selected_section) {
            if let Some(setting) = settings.get(state.selected_setting) {
                state.edit_buffer = setting.display_value().to_string();
                state.editing = true;
            }
        }
    }

    /// Toggle a boolean setting.
    pub fn toggle_setting(&mut self, state: &mut ConfigState) {
        if let Some(settings) = self.settings_cache.get_mut(state.selected_section) {
            if let Some(setting) = settings.get_mut(state.selected_setting) {
                if matches!(setting.setting_type, ConfigSettingType::Boolean) {
                    let current = setting.display_value();
                    let new_value = match current {
                        "Yes" | "Enabled" => {
                            if current == "Enabled" {
                                "Disabled".to_string()
                            } else {
                                "No".to_string()
                            }
                        }
                        _ => {
                            if setting.value == "Enabled" || setting.value == "Disabled" {
                                "Enabled".to_string()
                            } else {
                                "Yes".to_string()
                            }
                        }
                    };
                    setting.pending_value = Some(new_value);
                    state.has_changes = true;
                    self.has_modifications = true;
                }
            }
        }
    }

    /// Cycle an enum setting to the next value.
    pub fn cycle_setting(&mut self, state: &mut ConfigState) {
        if let Some(settings) = self.settings_cache.get_mut(state.selected_section) {
            if let Some(setting) = settings.get_mut(state.selected_setting) {
                if let ConfigSettingType::Enum(ref options) = setting.setting_type {
                    let current = setting.display_value().to_lowercase();
                    let current_clean = current.replace('_', " ");

                    let current_index = options
                        .iter()
                        .position(|o| {
                            let o_clean = o.replace('_', " ");
                            o.to_lowercase() == current || o_clean == current_clean
                        })
                        .unwrap_or(0);

                    let next_index = (current_index + 1) % options.len();
                    setting.pending_value = Some(options[next_index].to_string());
                    state.has_changes = true;
                    self.has_modifications = true;
                }
            }
        }
    }

    /// Confirm the current edit.
    pub fn confirm_edit(&mut self, state: &mut ConfigState) {
        if state.editing {
            if let Some(settings) = self.settings_cache.get_mut(state.selected_section) {
                if let Some(setting) = settings.get_mut(state.selected_setting) {
                    let valid = match &setting.setting_type {
                        ConfigSettingType::Integer => state.edit_buffer.parse::<i64>().is_ok(),
                        ConfigSettingType::Boolean => {
                            matches!(
                                state.edit_buffer.to_lowercase().as_str(),
                                "yes" | "no" | "true" | "false" | "enabled" | "disabled"
                            )
                        }
                        ConfigSettingType::Duration => {
                            state.edit_buffer.ends_with('s')
                                || state.edit_buffer.ends_with('m')
                                || state.edit_buffer.ends_with('h')
                        }
                        _ => true,
                    };

                    if valid {
                        setting.pending_value = Some(state.edit_buffer.clone());
                        state.has_changes = true;
                        self.has_modifications = true;
                    }
                }
            }
            state.editing = false;
            state.edit_buffer.clear();
        }
    }

    /// Cancel the current edit.
    pub fn cancel_edit(&mut self, state: &mut ConfigState) {
        state.editing = false;
        state.edit_buffer.clear();
    }

    /// Apply pending changes to the config and save.
    pub fn save_config(&mut self, state: &mut ConfigState) -> Result<(), String> {
        let pending_changes: Vec<(usize, &'static str, String)> = self
            .settings_cache
            .iter()
            .enumerate()
            .flat_map(|(section_idx, settings)| {
                settings.iter().filter_map(move |setting| {
                    setting
                        .pending_value
                        .as_ref()
                        .map(|v| (section_idx, setting.key, v.clone()))
                })
            })
            .collect();

        let Some(ref mut config) = self.config else {
            return Err("No config loaded".to_string());
        };

        for (section_index, key, value) in &pending_changes {
            Self::apply_setting_value_static(config, *section_index, key, value)?;
        }

        config
            .save()
            .map_err(|e| format!("Failed to save config: {}", e))?;

        for section_settings in &mut self.settings_cache {
            for setting in section_settings {
                if setting.pending_value.is_some() {
                    setting.value = setting.pending_value.take().unwrap();
                }
            }
        }

        state.has_changes = false;
        self.has_modifications = false;
        state.status_message = Some("Configuration saved successfully".to_string());

        Ok(())
    }

    /// Apply a single setting value to the config.
    #[allow(clippy::too_many_lines)]
    fn apply_setting_value_static(
        config: &mut yoop_core::config::Config,
        section_index: usize,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        match section_index {
            0 => match key {
                "device_name" => config.general.device_name = value.to_string(),
                "default_expire" => {
                    config.general.default_expire = parse_duration(value)?;
                }
                "default_output" => {
                    config.general.default_output = if value == "(not set)" {
                        None
                    } else {
                        Some(std::path::PathBuf::from(value))
                    };
                }
                _ => {}
            },
            1 => match key {
                "port" => {
                    config.network.port = value
                        .parse()
                        .map_err(|_| "Invalid port number".to_string())?;
                }
                "transfer_port_range" => {
                    let parts: Vec<&str> = value.split('-').collect();
                    if parts.len() != 2 {
                        return Err("Invalid port range format (use: start-end)".to_string());
                    }
                    let start: u16 = parts[0].trim().parse().map_err(|_| "Invalid start port")?;
                    let end: u16 = parts[1].trim().parse().map_err(|_| "Invalid end port")?;
                    config.network.transfer_port_range = (start, end);
                }
                "interface" => config.network.interface = value.to_string(),
                "ipv6" => config.network.ipv6 = parse_bool(value),
                _ => {}
            },
            2 => match key {
                "chunk_size" => {
                    config.transfer.chunk_size = parse_bytes(value)?;
                }
                "parallel_chunks" => {
                    config.transfer.parallel_chunks =
                        value.parse().map_err(|_| "Invalid number".to_string())?;
                }
                "bandwidth_limit" => {
                    if value == "unlimited" || value.is_empty() {
                        config.transfer.bandwidth_limit = None;
                    } else {
                        config.transfer.bandwidth_limit = Some(parse_bytes(value)? as u64);
                    }
                }
                "compression" => {
                    config.transfer.compression = match value.to_lowercase().as_str() {
                        "auto" => yoop_core::config::CompressionMode::Auto,
                        "always" => yoop_core::config::CompressionMode::Always,
                        "never" => yoop_core::config::CompressionMode::Never,
                        _ => return Err(format!("Invalid compression mode: {}", value)),
                    };
                }
                "compression_level" => {
                    config.transfer.compression_level = value
                        .parse()
                        .map_err(|_| "Invalid compression level".to_string())?;
                }
                "verify_checksum" => config.transfer.verify_checksum = parse_bool(value),
                _ => {}
            },
            3 => match key {
                "require_pin" => config.security.require_pin = parse_bool(value),
                "require_approval" => config.security.require_approval = parse_bool(value),
                "tls_verify" => config.security.tls_verify = parse_bool(value),
                "rate_limit_attempts" => {
                    config.security.rate_limit_attempts =
                        value.parse().map_err(|_| "Invalid number".to_string())?;
                }
                "rate_limit_window" => {
                    config.security.rate_limit_window = parse_duration(value)?;
                }
                _ => {}
            },
            4 => match key {
                "preview.enabled" => config.preview.enabled = parse_bool(value),
                "preview.max_image_size" => {
                    config.preview.max_image_size = parse_bytes(value)?;
                }
                "preview.max_text_length" => {
                    config.preview.max_text_length =
                        value.parse().map_err(|_| "Invalid number".to_string())?;
                }
                _ => {}
            },
            5 => match key {
                "history.enabled" => config.history.enabled = parse_bool(value),
                "history.max_entries" => {
                    config.history.max_entries =
                        value.parse().map_err(|_| "Invalid number".to_string())?;
                }
                "history.auto_clear_days" => {
                    if value == "disabled" || value.is_empty() {
                        config.history.auto_clear_days = None;
                    } else {
                        config.history.auto_clear_days =
                            Some(value.parse().map_err(|_| "Invalid number".to_string())?);
                    }
                }
                _ => {}
            },
            6 => match key {
                "enabled" => config.trust.enabled = parse_bool(value),
                "auto_prompt" => config.trust.auto_prompt = parse_bool(value),
                "default_level" => {
                    config.trust.default_level = match value.to_lowercase().as_str() {
                        "full" => yoop_core::config::TrustLevel::Full,
                        "ask_each_time" | "ask each time" => {
                            yoop_core::config::TrustLevel::AskEachTime
                        }
                        _ => return Err(format!("Invalid trust level: {}", value)),
                    };
                }
                _ => {}
            },
            7 => match key {
                "web.enabled" => config.web.enabled = parse_bool(value),
                "web.port" => {
                    config.web.port = value
                        .parse()
                        .map_err(|_| "Invalid port number".to_string())?;
                }
                "web.auth" => config.web.auth = parse_bool(value),
                "web.localhost_only" => config.web.localhost_only = parse_bool(value),
                _ => {}
            },
            8 => match key {
                "theme" => config.ui.theme = value.to_string(),
                "show_qr" => config.ui.show_qr = parse_bool(value),
                "notifications" => config.ui.notifications = parse_bool(value),
                "sound" => config.ui.sound = parse_bool(value),
                _ => {}
            },
            9 => match key {
                "auto_check" => config.update.auto_check = parse_bool(value),
                "check_interval" => {
                    config.update.check_interval = parse_duration(value)?;
                }
                "package_manager" => {
                    if value == "auto" || value.is_empty() {
                        config.update.package_manager = None;
                    } else {
                        config.update.package_manager = Some(match value.to_lowercase().as_str() {
                            "npm" => yoop_core::config::PackageManagerKind::Npm,
                            "pnpm" => yoop_core::config::PackageManagerKind::Pnpm,
                            "yarn" => yoop_core::config::PackageManagerKind::Yarn,
                            "bun" => yoop_core::config::PackageManagerKind::Bun,
                            _ => return Err(format!("Invalid package manager: {}", value)),
                        });
                    }
                }
                "notify" => config.update.notify = parse_bool(value),
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Revert all pending changes.
    pub fn revert_changes(&mut self, state: &mut ConfigState) {
        self.load_config();
        state.has_changes = false;
        state.selected_setting = 0;
        state.status_message = Some("Changes reverted".to_string());
    }

    /// Check if there are any unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        self.has_modifications
            || self
                .settings_cache
                .iter()
                .any(|section| section.iter().any(|s| s.pending_value.is_some()))
    }

    /// Get the current theme setting value.
    pub fn get_theme_value(&self) -> Option<&str> {
        self.settings_cache
            .get(8)
            .and_then(|settings| settings.first().map(ConfigSetting::display_value))
    }
}

impl Default for ConfigView {
    fn default() -> Self {
        Self::new()
    }
}

/// Format bytes as human-readable string.
fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;

    if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Format bytes (u64) as human-readable string.
fn format_bytes_u64(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Parse bytes from human-readable string.
fn parse_bytes(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if s.ends_with("MB") || s.ends_with(" MB") {
        let num: usize = s
            .trim_end_matches("MB")
            .trim()
            .parse()
            .map_err(|_| "Invalid number")?;
        Ok(num * 1024 * 1024)
    } else if s.ends_with("KB") || s.ends_with(" KB") {
        let num: usize = s
            .trim_end_matches("KB")
            .trim()
            .parse()
            .map_err(|_| "Invalid number")?;
        Ok(num * 1024)
    } else if s.ends_with('B') || s.ends_with(" B") {
        let num: usize = s
            .trim_end_matches('B')
            .trim()
            .parse()
            .map_err(|_| "Invalid number")?;
        Ok(num)
    } else {
        s.parse().map_err(|_| "Invalid number".to_string())
    }
}

/// Parse duration from string (e.g., "5m", "300s").
fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    let s = s.trim();
    if s.ends_with('s') {
        let secs: u64 = s
            .trim_end_matches('s')
            .parse()
            .map_err(|_| "Invalid seconds")?;
        Ok(std::time::Duration::from_secs(secs))
    } else if s.ends_with('m') {
        let mins: u64 = s
            .trim_end_matches('m')
            .parse()
            .map_err(|_| "Invalid minutes")?;
        Ok(std::time::Duration::from_secs(mins * 60))
    } else if s.ends_with('h') {
        let hours: u64 = s
            .trim_end_matches('h')
            .parse()
            .map_err(|_| "Invalid hours")?;
        Ok(std::time::Duration::from_secs(hours * 3600))
    } else {
        let secs: u64 = s.parse().map_err(|_| "Invalid duration")?;
        Ok(std::time::Duration::from_secs(secs))
    }
}

/// Parse boolean from string.
fn parse_bool(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "yes" | "true" | "enabled" | "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1024 * 1024), "1 MB");
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn test_parse_bytes() {
        assert_eq!(parse_bytes("1 KB").unwrap(), 1024);
        assert_eq!(parse_bytes("1 MB").unwrap(), 1024 * 1024);
        assert_eq!(parse_bytes("512 B").unwrap(), 512);
        assert_eq!(parse_bytes("1024").unwrap(), 1024);
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("300s").unwrap(),
            std::time::Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration("5m").unwrap(),
            std::time::Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration("1h").unwrap(),
            std::time::Duration::from_secs(3600)
        );
    }

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool("yes"));
        assert!(parse_bool("Yes"));
        assert!(parse_bool("true"));
        assert!(parse_bool("Enabled"));
        assert!(!parse_bool("no"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("disabled"));
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
    fn test_config_setting_has_changes() {
        let mut setting = ConfigSetting {
            key: "test",
            label: "Test",
            description: "Test setting",
            value: "old".to_string(),
            pending_value: None,
            setting_type: ConfigSettingType::String,
        };

        assert!(!setting.has_changes());
        assert_eq!(setting.display_value(), "old");

        setting.pending_value = Some("new".to_string());
        assert!(setting.has_changes());
        assert_eq!(setting.display_value(), "new");
    }
}
