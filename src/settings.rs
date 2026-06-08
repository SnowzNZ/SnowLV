//! User settings persistence.
//!
//! This module handles loading and saving user preferences across sessions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::Language;
use crate::theme::ThemeId;
use crate::units::UnitPreferences;

/// User settings that persist across sessions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSettings {
    /// Settings file version for migration support
    #[serde(default = "default_version")]
    pub version: u32,
    /// Selected language
    #[serde(default)]
    pub language: Language,
    /// Selected application color theme
    #[serde(default = "default_theme")]
    pub theme: String,
    /// When true, current channel values are shown beside the chart cursor
    #[serde(default)]
    pub values_follow_cursor: bool,
    /// When true, draw the chart background grid
    #[serde(default = "default_show_grid")]
    pub show_grid: bool,
    /// Grid line opacity, 0..=255. Modulates the base grid color's alpha
    /// before egui_plot's distance-based fade
    #[serde(default = "default_grid_opacity")]
    pub grid_opacity: u8,
    /// Default Y-axis scale mode for newly opened tabs. When false, each
    /// channel is independently normalized to its own range.
    #[serde(default)]
    pub default_shared_y_axis: bool,
    /// When true, Discord Rich Presence is enabled
    #[serde(default = "default_discord_rpc_enabled")]
    pub discord_rpc_enabled: bool,
    /// When true, Discord Rich Presence includes the active log file name
    #[serde(default = "default_discord_rpc_show_log_filename")]
    pub discord_rpc_show_log_filename: bool,
    /// Preferred display units for converted chart values
    #[serde(default)]
    pub unit_preferences: UnitPreferences,
    /// Channel/parameter names to select automatically when a log is opened
    #[serde(default)]
    pub default_enabled_parameters: Vec<String>,
}

fn default_version() -> u32 {
    1
}

fn default_theme() -> String {
    ThemeId::default().id().to_string()
}

fn default_show_grid() -> bool {
    true
}

fn default_grid_opacity() -> u8 {
    255
}

fn default_discord_rpc_enabled() -> bool {
    true
}

fn default_discord_rpc_show_log_filename() -> bool {
    true
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            version: 1,
            language: Language::default(),
            theme: default_theme(),
            values_follow_cursor: false,
            show_grid: default_show_grid(),
            grid_opacity: default_grid_opacity(),
            default_shared_y_axis: false,
            discord_rpc_enabled: default_discord_rpc_enabled(),
            discord_rpc_show_log_filename: default_discord_rpc_show_log_filename(),
            unit_preferences: UnitPreferences::default(),
            default_enabled_parameters: Vec::new(),
        }
    }
}

impl UserSettings {
    /// Get the config directory path for SnowLV
    pub fn get_config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            dirs::data_dir().map(|p| p.join("SnowLV"))
        }
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir().map(|p| p.join("SnowLV"))
        }
        #[cfg(target_os = "linux")]
        {
            dirs::config_dir().map(|p| p.join("snowlv"))
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            dirs::config_dir().map(|p| p.join("snowlv"))
        }
    }

    /// Get the path to the settings JSON file
    pub fn get_settings_path() -> Option<PathBuf> {
        Self::get_config_dir().map(|p| p.join("settings.json"))
    }

    /// Get the directory where user-defined theme files are stored
    pub fn get_themes_dir() -> Option<PathBuf> {
        Self::get_config_dir().map(|p| p.join("themes"))
    }

    /// Load settings from disk
    pub fn load() -> Self {
        let path = match Self::get_settings_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save settings to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_settings_path()
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }
}
