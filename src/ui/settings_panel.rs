//! Settings panel - consolidated settings for display, units, and normalization.
//!
//! This panel provides a single location for all user preferences.

use eframe::egui;
use rust_i18n::t;

use crate::analytics;
use crate::app::SnowLVApp;
use crate::i18n::Language;
use crate::settings::UserSettings;
use crate::state::FontScale;
use crate::theme::ThemeRegistry;
use crate::units::{
    AccelerationUnit, AfrLambdaUnit, DistanceUnit, FlowUnit, FuelEconomyUnit, PressureUnit,
    SpeedUnit, TemperatureUnit, UnitPreferences, UnitPreset, VolumeUnit,
};

impl SnowLVApp {
    /// Render the settings panel content (called from side_panel.rs)
    pub fn render_settings_panel_content(&mut self, ui: &mut egui::Ui) {
        // Language settings section
        self.render_language_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Display settings section
        self.render_display_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Default parameters
        self.render_default_parameters_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Discord RPC settings
        self.render_discord_rpc_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Field normalization settings
        self.render_normalization_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Unit preferences
        self.render_unit_settings(ui);
    }

    /// Render language settings section
    fn render_language_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(format!("\u{1F310} {}", t!("settings.language")))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("settings.language_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(8.0);

            egui::ComboBox::from_id_salt("language_selector")
                .selected_text(self.language.display_name())
                .width(140.0)
                .show_ui(ui, |ui| {
                    for lang in Language::all() {
                        if ui
                            .selectable_value(&mut self.language, *lang, lang.display_name())
                            .changed()
                        {
                            // Update locale immediately
                            rust_i18n::set_locale(self.language.locale_code());

                            // Save to persistent settings
                            self.user_settings.language = self.language;
                            if let Err(e) = self.user_settings.save() {
                                self.show_toast_error(&t!("toast.failed_to_save", error = e));
                            }

                            // Request repaint to refresh all UI text
                            ui.ctx().request_repaint();
                        }
                    }
                });
        });
    }

    /// Render display settings section
    fn render_display_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(format!("\u{1F5A5} {}", t!("settings.display")))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(t!("settings.theme")).size(font_14));
            let theme_options: Vec<(String, String)> = self
                .theme_registry
                .themes()
                .iter()
                .map(|theme| (theme.id.clone(), theme.display_name.clone()))
                .collect();
            let mut selected_theme = self.theme_id.clone();
            egui::ComboBox::from_id_salt("theme_selector")
                .selected_text(self.theme_display_name())
                .width(180.0)
                .show_ui(ui, |ui| {
                    for (theme_id, display_name) in theme_options {
                        ui.selectable_value(&mut selected_theme, theme_id, display_name);
                    }
                });

            if selected_theme != self.theme_id {
                self.theme_id = selected_theme.clone();
                self.user_settings.theme = selected_theme;
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
                ui.ctx().request_repaint();
            }

            ui.add_space(8.0);

            self.render_theme_preview(ui);

            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                if ui.button("Open Themes Folder").clicked() {
                    if let Some(themes_dir) = UserSettings::get_themes_dir() {
                        match ThemeRegistry::ensure_default_theme_files(&themes_dir).and_then(
                            |_| {
                                open::that(&themes_dir)
                                    .map_err(|e| format!("Failed to open themes folder: {}", e))
                            },
                        ) {
                            Ok(()) => {}
                            Err(e) => self.show_toast_error(&e),
                        }
                    } else {
                        self.show_toast_error("Could not determine themes directory");
                    }
                }

                if ui.button("Reload Themes").clicked() {
                    self.reload_theme_registry();
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                    ui.ctx().request_repaint();
                }
            });

            if !self.theme_registry.load_errors().is_empty() {
                ui.add_space(4.0);
                for error in self.theme_registry.load_errors().iter().take(3) {
                    ui.label(
                        egui::RichText::new(error)
                            .size(font_12)
                            .color(egui::Color32::from_rgb(220, 90, 90)),
                    );
                }
            }

            ui.add_space(8.0);

            // Colorblind mode
            let old_color_blind_mode = self.color_blind_mode;
            ui.checkbox(
                &mut self.color_blind_mode,
                egui::RichText::new(t!("settings.color_blind_mode")).size(font_14),
            );
            if self.color_blind_mode != old_color_blind_mode {
                analytics::track_colorblind_mode_toggled(self.color_blind_mode);
            }
            ui.label(
                egui::RichText::new(t!("settings.color_blind_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(8.0);

            // Font size
            ui.label(egui::RichText::new(t!("settings.font_size")).size(font_14));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.font_scale, FontScale::Small, "S");
                ui.selectable_value(&mut self.font_scale, FontScale::Medium, "M");
                ui.selectable_value(&mut self.font_scale, FontScale::Large, "L");
                ui.selectable_value(&mut self.font_scale, FontScale::ExtraLarge, "XL");
            });

            ui.add_space(8.0);

            // Cursor tracking
            ui.checkbox(
                &mut self.cursor_tracking,
                egui::RichText::new(t!("settings.cursor_tracking")).size(font_14),
            );
            ui.label(
                egui::RichText::new(t!("settings.cursor_tracking_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );

            if self.cursor_tracking {
                ui.add_space(4.0);
                let mut window_resp = None;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!("settings.window")).size(font_12));
                    window_resp = Some(
                        ui.add(
                            egui::Slider::new(&mut self.view_window_seconds, 5.0..=120.0)
                                .suffix("s")
                                .logarithmic(true)
                                .text(""),
                        ),
                    );
                });
                if window_resp.is_some_and(|r| r.changed()) {
                    self.set_current_view_window(self.view_window_seconds);
                }
            }

            ui.add_space(8.0);

            // Values follow cursor
            let old_values_follow_cursor = self.values_follow_cursor;
            ui.checkbox(
                &mut self.values_follow_cursor,
                egui::RichText::new(t!("settings.values_follow_cursor")).size(font_14),
            );
            ui.label(
                egui::RichText::new(t!("settings.values_follow_cursor_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            if self.values_follow_cursor != old_values_follow_cursor {
                self.user_settings.values_follow_cursor = self.values_follow_cursor;
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
            }

            ui.add_space(8.0);

            // Chart grid
            let old_show_grid = self.show_grid;
            ui.checkbox(
                &mut self.show_grid,
                egui::RichText::new(t!("settings.show_grid")).size(font_14),
            );
            ui.label(
                egui::RichText::new(t!("settings.show_grid_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            if self.show_grid != old_show_grid {
                self.user_settings.show_grid = self.show_grid;
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
            }

            if self.show_grid {
                ui.add_space(4.0);
                let mut slider_response = None;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!("settings.grid_opacity")).size(font_12));
                    slider_response =
                        Some(ui.add(egui::Slider::new(&mut self.grid_opacity, 0..=255)));
                });
                // Persist only when the user releases the slider, otherwise
                // every drag pixel rewrites settings.json.
                let committed = slider_response
                    .map(|r| r.drag_stopped() || r.lost_focus())
                    .unwrap_or(false);
                if committed && self.user_settings.grid_opacity != self.grid_opacity {
                    self.user_settings.grid_opacity = self.grid_opacity;
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }
            }

            ui.add_space(8.0);

            ui.label(egui::RichText::new(t!("settings.default_y_axis_scale")).size(font_14));
            ui.horizontal(|ui| {
                let mut default_shared_y_axis = self.user_settings.default_shared_y_axis;
                let independent_changed = ui
                    .selectable_value(
                        &mut default_shared_y_axis,
                        false,
                        t!("settings.y_axis_independent").to_string(),
                    )
                    .changed();
                let shared_changed = ui
                    .selectable_value(
                        &mut default_shared_y_axis,
                        true,
                        t!("settings.y_axis_shared").to_string(),
                    )
                    .changed();

                if independent_changed || shared_changed {
                    self.user_settings.default_shared_y_axis = default_shared_y_axis;
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }
            });
            ui.label(
                egui::RichText::new(t!("settings.default_y_axis_scale_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
        });
    }

    /// Render default parameter settings
    fn render_default_parameters_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(t!("settings.default_parameters"))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("settings.default_parameters_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(6.0);

            let response = ui.add(
                egui::TextEdit::multiline(&mut self.default_enabled_parameters_input)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text(t!("settings.default_parameters_hint")),
            );

            if response.changed() {
                self.user_settings.default_enabled_parameters =
                    parse_default_parameters(&self.default_enabled_parameters_input);
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
            }

            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                if ui.button(t!("settings.save_default_parameters")).clicked() {
                    self.user_settings.default_enabled_parameters =
                        parse_default_parameters(&self.default_enabled_parameters_input);
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }

                if ui.button(t!("settings.clear_default_parameters")).clicked() {
                    self.default_enabled_parameters_input.clear();
                    self.user_settings.default_enabled_parameters.clear();
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }
            });
        });
    }

    /// Render Discord RPC settings section
    fn render_discord_rpc_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(t!("settings.discord_rpc"))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            let old_enabled = self.discord_rpc_enabled;
            ui.checkbox(
                &mut self.discord_rpc_enabled,
                egui::RichText::new(t!("settings.discord_rpc_enabled")).size(font_14),
            );
            ui.label(
                egui::RichText::new(t!("settings.discord_rpc_enabled_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );

            if self.discord_rpc_enabled != old_enabled {
                self.user_settings.discord_rpc_enabled = self.discord_rpc_enabled;
                if !self.discord_rpc_enabled {
                    self.discord_presence.shutdown();
                }
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
            }

            ui.add_enabled_ui(self.discord_rpc_enabled, |ui| {
                let old_show_log_filename = self.discord_rpc_show_log_filename;
                ui.checkbox(
                    &mut self.discord_rpc_show_log_filename,
                    egui::RichText::new(t!("settings.discord_rpc_show_log_filename")).size(font_14),
                );
                ui.label(
                    egui::RichText::new(t!("settings.discord_rpc_show_log_filename_desc"))
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                if self.discord_rpc_show_log_filename != old_show_log_filename {
                    self.user_settings.discord_rpc_show_log_filename =
                        self.discord_rpc_show_log_filename;
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }
            });
        });
    }

    fn render_theme_preview(&self, ui: &mut egui::Ui) {
        let theme = self.theme();
        ui.horizontal_wrapped(|ui| {
            for color in theme.chart.iter().take(10) {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, theme.color(*color));
            }
        });
    }

    /// Render field normalization settings
    fn render_normalization_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(format!("\u{1F4DD} {}", t!("settings.field_names")))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(
                &mut self.field_normalization,
                egui::RichText::new(t!("settings.field_normalization")).size(font_14),
            );
            ui.label(
                egui::RichText::new(t!("settings.field_normalization_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(8.0);

            // Custom mappings count
            let custom_count = self.custom_normalizations.len();
            if custom_count > 0 {
                ui.label(
                    egui::RichText::new(t!("settings.custom_mappings", count = custom_count))
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );
            }

            // Edit mappings button
            let btn = egui::Frame::NONE
                .fill(egui::Color32::from_rgb(60, 60, 60))
                .corner_radius(4)
                .inner_margin(egui::vec2(12.0, 6.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(t!("settings.edit_custom_mappings"))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(font_14),
                    );
                });

            if btn.response.interact(egui::Sense::click()).clicked() {
                self.show_normalization_editor = true;
            }

            if btn.response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    }

    /// Render unit preferences
    fn render_unit_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(format!("\u{1F4D0} {}", t!("settings.units")))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            let previous_preferences = self.unit_preferences.clone();

            ui.label(
                egui::RichText::new(t!("settings.units_desc"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Preset:").size(font_12));

                let preset = self.unit_preferences.preset();
                egui::ComboBox::from_id_salt("unit_preset")
                    .selected_text(preset.label())
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                preset == UnitPreset::Metric,
                                UnitPreset::Metric.label(),
                            )
                            .clicked()
                        {
                            self.unit_preferences = UnitPreferences::metric();
                            ui.close();
                        }

                        if ui
                            .selectable_label(
                                preset == UnitPreset::Imperial,
                                UnitPreset::Imperial.label(),
                            )
                            .clicked()
                        {
                            self.unit_preferences = UnitPreferences::imperial();
                            ui.close();
                        }

                        ui.add_enabled_ui(false, |ui| {
                            let _ = ui.selectable_label(
                                preset == UnitPreset::Custom,
                                UnitPreset::Custom.label(),
                            );
                        });
                    });
            });
            ui.add_space(8.0);

            // Create a grid for unit selections
            egui::Grid::new("unit_settings_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    // Temperature
                    ui.label(egui::RichText::new(t!("settings.temperature")).size(font_12));
                    egui::ComboBox::from_id_salt("temp_unit")
                        .selected_text(self.unit_preferences.temperature.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.temperature,
                                TemperatureUnit::Celsius,
                                "°C",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.temperature,
                                TemperatureUnit::Fahrenheit,
                                "°F",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.temperature,
                                TemperatureUnit::Kelvin,
                                "K",
                            );
                        });
                    ui.end_row();

                    // Pressure
                    ui.label(egui::RichText::new(t!("settings.pressure")).size(font_12));
                    egui::ComboBox::from_id_salt("pressure_unit")
                        .selected_text(self.unit_preferences.pressure.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.pressure,
                                PressureUnit::KPa,
                                "kPa",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.pressure,
                                PressureUnit::HPa,
                                "hPa",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.pressure,
                                PressureUnit::PSI,
                                "psi",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.pressure,
                                PressureUnit::Bar,
                                "bar",
                            );
                        });
                    ui.end_row();

                    // Speed
                    ui.label(egui::RichText::new(t!("settings.speed")).size(font_12));
                    egui::ComboBox::from_id_salt("speed_unit")
                        .selected_text(self.unit_preferences.speed.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.speed,
                                SpeedUnit::KmH,
                                "km/h",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.speed,
                                SpeedUnit::Mph,
                                "mph",
                            );
                        });
                    ui.end_row();

                    // Distance
                    ui.label(egui::RichText::new(t!("settings.distance")).size(font_12));
                    egui::ComboBox::from_id_salt("distance_unit")
                        .selected_text(self.unit_preferences.distance.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.distance,
                                DistanceUnit::Kilometers,
                                "km",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.distance,
                                DistanceUnit::Miles,
                                "mi",
                            );
                        });
                    ui.end_row();

                    // Fuel Economy
                    ui.label(egui::RichText::new(t!("settings.fuel_economy")).size(font_12));
                    egui::ComboBox::from_id_salt("fuel_unit")
                        .selected_text(self.unit_preferences.fuel_economy.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.fuel_economy,
                                FuelEconomyUnit::LPer100Km,
                                "L/100km",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.fuel_economy,
                                FuelEconomyUnit::MpgUs,
                                "mpg US",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.fuel_economy,
                                FuelEconomyUnit::MpgUk,
                                "mpg UK",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.fuel_economy,
                                FuelEconomyUnit::KmPerL,
                                "km/L",
                            );
                        });
                    ui.end_row();

                    // Volume
                    ui.label(egui::RichText::new(t!("settings.volume")).size(font_12));
                    egui::ComboBox::from_id_salt("volume_unit")
                        .selected_text(self.unit_preferences.volume.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.volume,
                                VolumeUnit::Liters,
                                "L",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.volume,
                                VolumeUnit::Gallons,
                                "gal",
                            );
                        });
                    ui.end_row();

                    // Flow Rate
                    ui.label(egui::RichText::new(t!("settings.flow_rate")).size(font_12));
                    egui::ComboBox::from_id_salt("flow_unit")
                        .selected_text(self.unit_preferences.flow.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.flow,
                                FlowUnit::CcPerMin,
                                "cc/min",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.flow,
                                FlowUnit::LbPerHr,
                                "lb/hr",
                            );
                        });
                    ui.end_row();

                    // Acceleration
                    ui.label(egui::RichText::new(t!("settings.acceleration")).size(font_12));
                    egui::ComboBox::from_id_salt("accel_unit")
                        .selected_text(self.unit_preferences.acceleration.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.acceleration,
                                AccelerationUnit::MPerS2,
                                "m/s²",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.acceleration,
                                AccelerationUnit::G,
                                "g",
                            );
                        });
                    ui.end_row();

                    // AFR/Lambda
                    ui.label(egui::RichText::new(t!("settings.afr_lambda")).size(font_12));
                    egui::ComboBox::from_id_salt("afr_lambda_unit")
                        .selected_text(self.unit_preferences.afr_lambda.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.afr_lambda,
                                AfrLambdaUnit::AFR,
                                "AFR",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.afr_lambda,
                                AfrLambdaUnit::Lambda,
                                "λ",
                            );
                        });
                    ui.end_row();
                });

            if self.unit_preferences != previous_preferences {
                self.user_settings.unit_preferences = self.unit_preferences.clone();
                if let Err(e) = self.user_settings.save() {
                    eprintln!("Failed to save settings: {}", e);
                }
            }
        });
    }
}

fn parse_default_parameters(input: &str) -> Vec<String> {
    input
        .split(|c| c == '\n' || c == ',' || c == ';')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
