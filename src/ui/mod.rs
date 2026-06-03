//! UI rendering modules for the SnowLV application.
//!
//! This module organizes the various UI components into logical submodules:
//!
//! ## New Activity Bar Architecture
//! - `activity_bar` - VS Code-style vertical icon strip for panel navigation
//! - `side_panel` - Container that routes to the appropriate panel
//! - `files_panel` - File management, loading, and file list
//! - `channels_panel` - Channel selection (works in all modes)
//! - `tools_panel` - Analysis tools, computed channels, export
//! - `settings_panel` - Consolidated settings (display, units, normalization)
//!
//! ## Core UI Components
//! - `sidebar` - Legacy files panel (being replaced by files_panel)
//! - `channels` - Legacy channel selection (being replaced by channels_panel)
//! - `chart` - Main chart rendering and legends
//! - `timeline` - Timeline scrubber and playback controls
//! - `menu` - Menu bar (File, Edit, View, Help)
//! - `toast` - Toast notification system
//! - `icons` - Custom icon drawing utilities
//! - `export` - Chart export functionality (PNG, PDF)
//! - `normalization_editor` - Field normalization customization window
//! - `tool_switcher` - Tool mode selection (Log Viewer, Scatter Plot, Histogram)
//! - `scatter_plot` - Scatter plot visualization view
//! - `histogram` - Histogram visualization view
//! - `tab_bar` - Chrome-style tabs for managing multiple log files
//! - `analysis_panel` - Signal analysis tools window
//! - `computed_channels_manager` - Computed channels library manager
//! - `formula_editor` - Formula creation and editing

// New activity bar architecture
pub mod activity_bar;
pub mod files_panel;
pub mod settings_panel;
pub mod side_panel;
pub mod tool_properties_panel;
pub mod tools_panel;

use eframe::egui;
use std::hash::Hash;

// Core UI components
pub mod analysis_panel;
pub mod channels;
pub mod chart;
pub mod computed_channels_manager;
pub mod export;
pub mod formula_editor;
pub mod histogram;
pub mod icons;
pub mod menu;
pub mod normalization_editor;
pub mod scatter_plot;
pub mod sidebar;

/// Searchable dropdown helper used by UI modules that need typeahead-like filtering.
pub fn searchable_combo_box<'a, S, I>(
    ui: &mut egui::Ui,
    id_source: S,
    selected_text: impl Into<String>,
    search_query: &mut String,
    options: I,
    current_index: Option<usize>,
    new_selection: &mut Option<usize>,
) where
    S: Hash,
    I: IntoIterator<Item = (usize, &'a str)>,
{
    let selected_text = selected_text.into();
    let combo = egui::ComboBox::from_id_salt(id_source)
        .selected_text(selected_text)
        .width(ui.available_width());
    combo.show_ui(ui, |ui| {
        let search = ui.add(
            egui::TextEdit::singleline(search_query)
                .hint_text("Type to filter...")
                .desired_width(ui.available_width()),
        );
        if !search.has_focus() {
            search.request_focus();
        }

        ui.separator();
        let filter = search_query.to_lowercase();
        let mut any_match = false;
        for (idx, name) in options {
            if filter.is_empty() || name.to_lowercase().contains(&filter) {
                any_match = true;
                if ui
                    .selectable_label(current_index == Some(idx), name)
                    .clicked()
                {
                    *new_selection = Some(idx);
                }
            }
        }
        if !any_match {
            ui.label(egui::RichText::new("No matches").color(egui::Color32::LIGHT_RED));
        }
    });
}
pub mod tab_bar;
pub mod timeline;
pub mod toast;
pub mod tool_switcher;
