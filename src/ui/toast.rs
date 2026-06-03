//! Toast notification system for user feedback.

use eframe::egui;

use crate::app::SnowLVApp;

impl SnowLVApp {
    /// Render toast notifications in the bottom right corner
    pub fn render_toast(&mut self, ctx: &egui::Context) {
        if let Some((message, time, toast_type)) = &self.toast_message {
            if time.elapsed().as_secs() < 3 {
                let margin = 20.0;

                let theme = self.theme();
                let bg_color = match toast_type {
                    crate::state::ToastType::Info => theme.accent_alt,
                    crate::state::ToastType::Success => theme.success,
                    crate::state::ToastType::Warning => theme.warning,
                    crate::state::ToastType::Error => theme.error,
                };
                let text_color =
                    if matches!(toast_type, crate::state::ToastType::Warning) && !theme.dark {
                        theme.text
                    } else if matches!(toast_type, crate::state::ToastType::Warning) {
                        [20, 20, 20]
                    } else {
                        theme.text
                    };

                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-margin, -margin))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgb(
                                bg_color[0],
                                bg_color[1],
                                bg_color[2],
                            ))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .shadow(egui::epaint::Shadow {
                                offset: [2, 2],
                                blur: 8,
                                spread: 0,
                                color: egui::Color32::from_black_alpha(60),
                            })
                            .show(ui, |ui| {
                                // Set min/max width for proper text wrapping
                                ui.set_min_width(200.0);
                                ui.set_max_width(400.0);
                                ui.label(
                                    egui::RichText::new(message)
                                        .color(egui::Color32::from_rgb(
                                            text_color[0],
                                            text_color[1],
                                            text_color[2],
                                        ))
                                        .size(self.scaled_font(14.0)),
                                );
                            });
                    });
            } else {
                self.toast_message = None;
            }
        }
    }
}
