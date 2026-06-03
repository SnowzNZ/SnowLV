//! Tools panel - analysis tools, computed channels library, and export options.
//!
//! Provides quick access to analysis and export functionality inline in the side panel.

use eframe::egui;

use crate::app::SnowLVApp;
use crate::state::ActiveTool;

impl SnowLVApp {
    /// Render the tools panel content (called from side_panel.rs)
    pub fn render_tools_panel_content(&mut self, ui: &mut egui::Ui) {
        let _font_12 = self.scaled_font(12.0);
        let _font_14 = self.scaled_font(14.0);

        // Analysis Tools Section
        self.render_tools_analysis_section(ui);

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Computed Channels Section
        self.render_tools_computed_section(ui);

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Export Section
        self.render_tools_export_section(ui);
    }

    /// Render the analysis tools section
    fn render_tools_analysis_section(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme();
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new("📈 Analysis Tools")
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Run signal processing and statistical analysis on log data.")
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(8.0);

            let has_file = self.selected_file.is_some() && !self.files.is_empty();

            if has_file {
                // Show available analyzers count
                let analyzer_count = self.analyzer_registry.all().len();
                ui.label(
                    egui::RichText::new(format!("{} analyzers available", analyzer_count))
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(4.0);

                // Open full analysis panel button
                let primary_color = theme.color(theme.accent);
                let primary_text = theme.readable_text_color(primary_color);
                let btn = egui::Frame::NONE
                    .fill(primary_color)
                    .corner_radius(4)
                    .inner_margin(egui::vec2(12.0, 6.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Open Analysis Panel")
                                .color(primary_text)
                                .size(font_14),
                        );
                    });

                if btn.response.interact(egui::Sense::click()).clicked() {
                    self.show_analysis_panel = true;
                }

                if btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Show results count if any
                if let Some(file_idx) = self.selected_file {
                    if let Some(results) = self.analysis_results.get(&file_idx) {
                        if !results.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "✓ {} analysis results for current file",
                                    results.len()
                                ))
                                .size(font_12)
                                .color(theme.color(theme.success)),
                            );
                        }
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new("Load a file to access analysis tools")
                        .size(font_12)
                        .color(egui::Color32::from_rgb(100, 100, 100))
                        .italics(),
                );
            }
        });
    }

    /// Render the computed channels section
    fn render_tools_computed_section(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme();
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new("ƒ Computed Channels")
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Create virtual channels from mathematical formulas.")
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(8.0);

            // Library count
            let template_count = self.computed_library.templates.len();
            ui.label(
                egui::RichText::new(format!("{} templates in library", template_count))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(4.0);

            // Buttons row
            ui.horizontal(|ui| {
                // New Channel button
                let accent_color = theme.color(theme.accent);
                let accent_text = theme.readable_text_color(accent_color);
                let new_btn = egui::Frame::NONE
                    .fill(accent_color)
                    .corner_radius(4)
                    .inner_margin(egui::vec2(10.0, 5.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("+ New")
                                .color(accent_text)
                                .size(font_12),
                        );
                    });

                if new_btn.response.interact(egui::Sense::click()).clicked() {
                    self.formula_editor_state.open_new();
                }

                if new_btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Manage Library button
                let manage_btn = egui::Frame::NONE
                    .fill(theme.color(theme.card_hover))
                    .corner_radius(4)
                    .inner_margin(egui::vec2(10.0, 5.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Manage Library")
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(font_12),
                        );
                    });

                if manage_btn.response.interact(egui::Sense::click()).clicked() {
                    self.show_computed_channels_manager = true;
                }

                if manage_btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });

            // Show applied channels for current file
            if let Some(file_idx) = self.selected_file {
                if let Some(channels) = self.file_computed_channels.get(&file_idx) {
                    if !channels.is_empty() {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "✓ {} computed channels on current file",
                                channels.len()
                            ))
                            .size(font_12)
                            .color(theme.color(theme.success)),
                        );
                    }
                }
            }

            // Quick apply section
            if !self.computed_library.templates.is_empty() && self.selected_file.is_some() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Quick Apply:")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                // Show first few templates as quick apply buttons
                let templates: Vec<_> = self
                    .computed_library
                    .templates
                    .iter()
                    .take(5)
                    .map(|t| (t.id.clone(), t.name.clone()))
                    .collect();

                for (id, name) in templates {
                    let response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("  • {}", name))
                                .size(font_12)
                                .color(egui::Color32::from_rgb(150, 180, 220)),
                        )
                        .sense(egui::Sense::click()),
                    );

                    if response.clicked() {
                        // Find and apply the template
                        if let Some(template) =
                            self.computed_library.templates.iter().find(|t| t.id == id)
                        {
                            let template_clone = template.clone();
                            self.apply_computed_channel_template(&template_clone);
                        }
                    }

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            }
        });
    }

    /// Render the export section
    fn render_tools_export_section(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme();
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(egui::RichText::new("📤 Export").size(font_14).strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Export visualizations as images or documents.")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);

                let has_file = self.selected_file.is_some() && !self.files.is_empty();
                let has_channels = !self.get_selected_channels().is_empty();

                // Determine what can be exported based on active tool
                let can_export_png = match self.active_tool {
                    ActiveTool::LogViewer => has_file && has_channels,
                    ActiveTool::ScatterPlot => has_file,
                    ActiveTool::Histogram => has_file,
                };

                let can_export_pdf = match self.active_tool {
                    ActiveTool::LogViewer => has_file && has_channels,
                    ActiveTool::ScatterPlot => has_file,
                    ActiveTool::Histogram => has_file,
                };

                ui.horizontal(|ui| {
                    // PNG Export
                    ui.add_enabled_ui(can_export_png, |ui| {
                        let fill = if can_export_png {
                            theme.color(theme.accent_alt)
                        } else {
                            theme.color(theme.card)
                        };
                        let btn = egui::Frame::NONE
                            .fill(fill)
                            .corner_radius(4)
                            .inner_margin(egui::vec2(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("PNG")
                                        .color(if can_export_png {
                                            theme.readable_text_color(fill)
                                        } else {
                                            egui::Color32::GRAY
                                        })
                                        .size(font_14),
                                );
                            });

                        if can_export_png && btn.response.interact(egui::Sense::click()).clicked() {
                            match self.active_tool {
                                ActiveTool::LogViewer => self.export_chart_png(),
                                ActiveTool::ScatterPlot => self.export_scatter_plot_png(),
                                ActiveTool::Histogram => self.export_histogram_png(),
                            }
                        }

                        if btn.response.hovered() && can_export_png {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    });

                    // PDF Export
                    ui.add_enabled_ui(can_export_pdf, |ui| {
                        let fill = if can_export_pdf {
                            theme.color(theme.error)
                        } else {
                            theme.color(theme.card)
                        };
                        let btn = egui::Frame::NONE
                            .fill(fill)
                            .corner_radius(4)
                            .inner_margin(egui::vec2(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("PDF")
                                        .color(if can_export_pdf {
                                            theme.readable_text_color(fill)
                                        } else {
                                            egui::Color32::GRAY
                                        })
                                        .size(font_14),
                                );
                            });

                        if can_export_pdf && btn.response.interact(egui::Sense::click()).clicked() {
                            match self.active_tool {
                                ActiveTool::LogViewer => self.export_chart_pdf(),
                                ActiveTool::ScatterPlot => self.export_scatter_plot_pdf(),
                                ActiveTool::Histogram => self.export_histogram_pdf(),
                            }
                        }

                        if btn.response.hovered() && can_export_pdf {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    });
                });

                // Show appropriate help text based on tool and state
                if !has_file {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Load a file to enable export")
                            .size(font_12)
                            .color(egui::Color32::from_rgb(100, 100, 100))
                            .italics(),
                    );
                } else if self.active_tool == ActiveTool::LogViewer && !has_channels {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Select channels to enable chart export")
                            .size(font_12)
                            .color(egui::Color32::from_rgb(100, 100, 100))
                            .italics(),
                    );
                }
            });
    }
}
