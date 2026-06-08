//! Tool switcher component for switching between different views.
//!
//! Renders compact buttons in the top header for Log Viewer, Scatter Plots,
//! and Histogram.

use eframe::egui;
use rust_i18n::t;

use crate::analytics;
use crate::app::SnowLVApp;
use crate::state::ActiveTool;

impl SnowLVApp {
    /// Render the tool switcher buttons
    pub fn render_tool_switcher(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let theme = self.theme();

            // Define available tools
            let tools = [
                ActiveTool::LogViewer,
                ActiveTool::ScatterPlot,
                ActiveTool::Histogram,
            ];

            for tool in tools {
                let is_selected = self.active_tool == tool;

                // Style the button based on selection state
                let button_fill = if is_selected {
                    theme.color(theme.selection)
                } else {
                    theme.color(theme.card)
                };

                let text_color = if is_selected {
                    theme.color(theme.text)
                } else {
                    theme.color(theme.muted_text)
                };

                let stroke = if is_selected {
                    egui::Stroke::new(1.5, theme.color(theme.accent))
                } else {
                    egui::Stroke::new(1.0, theme.color(theme.grid))
                };

                // Get translated tool name
                let tool_name = match tool {
                    ActiveTool::LogViewer => t!("tools.log_viewer"),
                    ActiveTool::ScatterPlot => t!("tools.scatter_plots"),
                    ActiveTool::Histogram => t!("tools.histogram"),
                };

                let shortcut = match tool {
                    ActiveTool::LogViewer => "\u{2318}1",
                    ActiveTool::ScatterPlot => "\u{2318}2",
                    ActiveTool::Histogram => "\u{2318}3",
                };

                // Create compact view-switch button
                let response = ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(tool_name.as_ref())
                                .size(self.scaled_font(13.0))
                                .color(text_color),
                        )
                        .fill(button_fill)
                        .stroke(stroke)
                        .corner_radius(egui::CornerRadius::same(4))
                        .min_size(egui::vec2(96.0, 26.0)),
                    )
                    .on_hover_text(shortcut);

                if response.clicked() {
                    self.active_tool = tool;
                    self.show_settings_view = false;
                    analytics::track_tool_switched(tool.name());
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });
    }
}
