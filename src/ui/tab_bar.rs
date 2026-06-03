//! Tab bar UI for managing multiple log file views.

use eframe::egui;

use crate::app::SnowLVApp;

impl SnowLVApp {
    /// Render the tab bar for switching between log files
    pub fn render_tab_bar(&mut self, ui: &mut egui::Ui) {
        if self.tabs.is_empty() {
            return;
        }

        let mut tab_to_activate: Option<usize> = None;
        let mut tab_to_close: Option<usize> = None;
        let theme = self.theme();

        // Collect tab info to avoid borrow issues
        let tab_info: Vec<(String, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.name.clone(), self.active_tab == Some(i)))
            .collect();

        egui::ScrollArea::horizontal()
            .id_salt("log_tab_bar")
            .auto_shrink([false, true])
            .max_height(30.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (i, (name, is_active)) in tab_info.iter().enumerate() {
                        let tab_color = if *is_active {
                            theme.color(theme.selection)
                        } else {
                            theme.color(theme.card)
                        };

                        let text_color = if *is_active {
                            theme.color(theme.text)
                        } else {
                            theme.color(theme.muted_text)
                        };

                        let border_color = if *is_active {
                            theme.color(theme.accent)
                        } else {
                            theme.color(theme.grid)
                        };

                        egui::Frame::NONE
                            .fill(tab_color)
                            .corner_radius(egui::CornerRadius::same(4))
                            .stroke(egui::Stroke::new(
                                if *is_active { 1.5 } else { 1.0 },
                                border_color,
                            ))
                            .inner_margin(egui::Margin {
                                left: 10,
                                right: 6,
                                top: 4,
                                bottom: 4,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Tab name (clickable)
                                    let font_12 = self.scaled_font(12.0);
                                    let label_response = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(name)
                                                .color(text_color)
                                                .size(font_12),
                                        )
                                        .sense(egui::Sense::click()),
                                    );

                                    if label_response.clicked() {
                                        tab_to_activate = Some(i);
                                    }
                                    if label_response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    ui.add_space(3.0);

                                    // Close button
                                    let close_btn = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new("x")
                                                .color(theme.color(theme.muted_text))
                                                .size(font_12),
                                        )
                                        .sense(egui::Sense::click()),
                                    );

                                    if close_btn.clicked() {
                                        tab_to_close = Some(i);
                                    }

                                    if close_btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                });
                            });
                    }
                });
            });

        // Handle deferred tab activation
        if let Some(index) = tab_to_activate {
            self.active_tab = Some(index);
            self.selected_file = Some(self.tabs[index].file_index);
        }

        // Handle deferred tab close
        if let Some(index) = tab_to_close {
            self.close_tab(index);
        }
    }
}
