//! Menu bar UI components (File and View menus).
//!
//! Simplified menu structure - settings moved to Settings panel.

use eframe::egui;
use rust_i18n::t;

use crate::app::SnowLVApp;
use crate::state::{ActiveTool, LoadingState};

impl SnowLVApp {
    /// Render the compact application header.
    pub fn render_app_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.set_height(30.0);

            ui.allocate_ui_with_layout(
                egui::vec2(86.0, 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    self.render_menu_bar(ui);
                },
            );

            if !self.tabs.is_empty() {
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 30.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        self.render_tab_bar(ui);
                    },
                );
            }
        });
    }

    /// Render the application menu bar
    pub fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        // Pre-compute scaled font sizes for use in closures
        let font_14 = self.scaled_font(14.0);
        let font_15 = self.scaled_font(15.0);

        ui.scope(|ui| {
            // Increase font size for menu items
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(font_15));

            // File menu
            ui.menu_button(t!("menu.file"), |ui| {
                ui.set_min_width(180.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

                // Open file option
                if ui
                    .add_enabled(!is_loading, egui::Button::new(t!("menu.open_log_file")))
                    .on_hover_text("\u{2318}O")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Log Files", crate::state::SUPPORTED_EXTENSIONS)
                        .pick_file()
                    {
                        self.start_loading_file(path);
                    }
                    ui.close();
                }

                ui.separator();

                // Close current tab
                let has_tabs = !self.tabs.is_empty();
                if ui
                    .add_enabled(has_tabs, egui::Button::new(t!("menu.close_tab")))
                    .on_hover_text("\u{2318}W")
                    .clicked()
                {
                    if let Some(tab_idx) = self.active_tab {
                        self.close_tab(tab_idx);
                    }
                    ui.close();
                }

                ui.separator();

                // Export submenu - context-aware based on active tool
                let has_chart_data =
                    !self.files.is_empty() && !self.get_selected_channels().is_empty();
                let has_histogram_data = !self.files.is_empty()
                    && self.active_tool == ActiveTool::Histogram
                    && self.active_tab.is_some()
                    && {
                        let tab_idx = self.active_tab.unwrap();
                        let config = &self.tabs[tab_idx].histogram_state.config;
                        config.x_channel.is_some() && config.y_channel.is_some()
                    };

                let can_export = has_chart_data || has_histogram_data;

                ui.add_enabled_ui(can_export, |ui| {
                    ui.menu_button(t!("menu.export"), |ui| {
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));

                        if self.active_tool == ActiveTool::Histogram && has_histogram_data {
                            if ui.button(t!("menu.export_histogram_pdf")).clicked() {
                                self.export_histogram_pdf();
                                ui.close();
                            }
                        } else if has_chart_data {
                            if ui.button(t!("menu.export_png")).clicked() {
                                self.export_chart_png();
                                ui.close();
                            }
                            if ui.button(t!("menu.export_pdf")).clicked() {
                                self.export_chart_pdf();
                                ui.close();
                            }
                        }
                    });
                });
            });

            // View menu - display options
            ui.menu_button(t!("menu.view"), |ui| {
                ui.set_min_width(200.0);

                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                // Chart grid toggle
                let old_show_grid = self.show_grid;
                ui.checkbox(
                    &mut self.show_grid,
                    egui::RichText::new(t!("menu.show_grid")).size(font_14),
                );
                if self.show_grid != old_show_grid {
                    self.user_settings.show_grid = self.show_grid;
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }
            });
        });
    }
}
