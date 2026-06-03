//! Timeline scrubber and playback controls UI.

use eframe::egui;
use rust_i18n::t;

use crate::analytics;
use crate::app::SnowLVApp;

impl SnowLVApp {
    /// Render the timeline scrubber bar
    pub fn render_timeline_scrubber(&mut self, ui: &mut egui::Ui) {
        // Pre-compute scaled font size
        let font_12 = self.scaled_font(12.0);

        let Some((min_time, max_time)) = self.get_time_range() else {
            return;
        };

        let total_duration = max_time - min_time;
        if total_duration <= 0.0 {
            return;
        }

        // Time labels row
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(Self::format_time(min_time))
                    .color(egui::Color32::LIGHT_GRAY)
                    .size(font_12),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(Self::format_time(max_time))
                        .color(egui::Color32::LIGHT_GRAY)
                        .size(font_12),
                );
            });
        });

        // Full-width slider - set slider_width to use available space
        let current_time = self.get_cursor_time().unwrap_or(min_time);
        let mut slider_value = current_time;
        let available_width = ui.available_width();

        // Temporarily set slider width to fill available space
        let old_slider_width = ui.spacing().slider_width;
        ui.spacing_mut().slider_width = available_width - 10.0; // Small margin for aesthetics

        let slider = egui::Slider::new(&mut slider_value, min_time..=max_time)
            .show_value(false)
            .clamping(egui::SliderClamping::Always);

        let slider_response = ui.add(slider);

        // Restore original slider width
        ui.spacing_mut().slider_width = old_slider_width;

        if slider_response.changed() {
            // Stop playback when user manually scrubs
            self.is_playing = false;
            self.last_frame_time = None;
            self.playback_record_position = None;

            self.set_cursor_time(Some(slider_value));
            let record = self.find_record_at_time(slider_value);
            self.set_cursor_record(record);
            // Force repaint to update legend values
            ui.ctx().request_repaint();
        }
    }

    /// Render the record/time indicator bar with playback controls
    pub fn render_record_indicator(&mut self, ui: &mut egui::Ui) {
        // Pre-compute scaled font size
        let font_14 = self.scaled_font(14.0);
        let theme = self.theme();

        ui.horizontal(|ui| {
            // Playback controls
            let button_size = egui::vec2(28.0, 28.0);

            // Play/Pause button
            let play_text = if self.is_playing {
                "\u{23F8}"
            } else {
                "\u{25B6}"
            };
            let play_button = egui::Button::new(
                egui::RichText::new(play_text)
                    .size(self.scaled_font(16.0))
                    .color(if self.is_playing {
                        theme.color(theme.warning)
                    } else {
                        theme.color(theme.success)
                    }),
            )
            .min_size(button_size);

            if ui.add(play_button).clicked() {
                self.is_playing = !self.is_playing;
                if self.is_playing {
                    // Track playback start for analytics
                    analytics::track_playback_started(self.playback_speed);
                    // Reset frame time when starting playback
                    self.last_frame_time = Some(std::time::Instant::now());
                    self.playback_record_position = self.get_cursor_record().map(|r| r as f64);
                    // Initialize cursor if not set
                    if self.get_cursor_time().is_none() {
                        if let Some((min, _)) = self.get_time_range() {
                            self.set_cursor_time(Some(min));
                            let record = self.find_record_at_time(min);
                            self.set_cursor_record(record);
                            self.playback_record_position = record.map(|r| r as f64);
                        }
                    }
                }
            }

            // Stop button (resets to beginning)
            let stop_button = egui::Button::new(
                egui::RichText::new("\u{23F9}")
                    .size(self.scaled_font(16.0))
                    .color(theme.color(theme.error)),
            )
            .min_size(button_size);

            if ui.add(stop_button).clicked() {
                self.is_playing = false;
                self.last_frame_time = None;
                self.playback_record_position = None;
                // Reset cursor to beginning
                if let Some((min, _)) = self.get_time_range() {
                    self.set_cursor_time(Some(min));
                    let record = self.find_record_at_time(min);
                    self.set_cursor_record(record);
                }
            }

            ui.separator();

            // Playback speed selector
            ui.label(
                egui::RichText::new(t!("timeline.speed"))
                    .color(theme.color(theme.muted_text))
                    .size(font_14),
            );

            let speed_options = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
            egui::ComboBox::from_id_salt("playback_speed")
                .selected_text(format!("{}x", self.playback_speed))
                .width(60.0)
                .show_ui(ui, |ui| {
                    for speed in speed_options {
                        ui.selectable_value(&mut self.playback_speed, speed, format!("{}x", speed));
                    }
                });

            ui.separator();

            self.render_playback_rate_controls(ui);

            ui.separator();

            // Current time display
            if let Some(time) = self.get_cursor_time() {
                ui.label(
                    egui::RichText::new(t!("timeline.time", time = Self::format_time(time)))
                        .strong()
                        .color(theme.color(theme.playhead))
                        .size(font_14),
                );
            }

            ui.separator();

            // Record indicator - use active tab's file for record count
            if let Some(record) = self.get_cursor_record() {
                if let Some(tab_idx) = self.active_tab {
                    let file_index = self.tabs[tab_idx].file_index;
                    if file_index < self.files.len() {
                        let total_records = self.files[file_index].log.data.len();
                        ui.label(
                            egui::RichText::new(t!(
                                "timeline.record",
                                current = record + 1,
                                total = total_records
                            ))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(font_14),
                        );
                    }
                }
            }
        });
    }

    /// Render optional record-rate playback controls for logs with bad/missing timing.
    fn render_playback_rate_controls(&mut self, ui: &mut egui::Ui) {
        let font_14 = self.scaled_font(14.0);
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let old_override = self.tabs[tab_idx].playback_rate_override;
        ui.checkbox(
            &mut self.tabs[tab_idx].playback_rate_override,
            egui::RichText::new(t!("timeline.logger_rate")).size(font_14),
        );

        if self.tabs[tab_idx].playback_rate_override != old_override {
            self.playback_record_position = self.get_cursor_record().map(|r| r as f64);
        }

        if self.tabs[tab_idx].playback_rate_override {
            let rate = &mut self.tabs[tab_idx].playback_rate_hz;
            ui.add(
                egui::DragValue::new(rate)
                    .range(0.1..=1000.0)
                    .speed(1.0)
                    .fixed_decimals(1)
                    .suffix(" Hz"),
            );
            *rate = rate.clamp(0.1, 1000.0);
        }
    }

    /// Update playback state - advances cursor based on elapsed time
    pub fn update_playback(&mut self, ctx: &egui::Context) {
        if !self.is_playing {
            return;
        }

        let Some((min_time, max_time)) = self.get_time_range() else {
            self.is_playing = false;
            return;
        };

        let now = std::time::Instant::now();
        let delta = if let Some(last) = self.last_frame_time {
            now.duration_since(last).as_secs_f64()
        } else {
            0.0
        };
        self.last_frame_time = Some(now);

        if self.playback_rate_override_enabled() {
            self.update_record_rate_playback(delta, max_time);
            ctx.request_repaint();
            return;
        }

        // Advance cursor by delta * playback_speed
        if let Some(current_time) = self.get_cursor_time() {
            let new_time = current_time + (delta * self.playback_speed);

            if new_time >= max_time {
                // Reached end - stop playback
                self.set_cursor_time(Some(max_time));
                let record = self.find_record_at_time(max_time);
                self.set_cursor_record(record);
                self.is_playing = false;
                self.last_frame_time = None;
                self.playback_record_position = None;
            } else {
                self.set_cursor_time(Some(new_time));
                let record = self.find_record_at_time(new_time);
                self.set_cursor_record(record);
            }
        } else {
            // No cursor set, start from beginning
            self.set_cursor_time(Some(min_time));
            let record = self.find_record_at_time(min_time);
            self.set_cursor_record(record);
        }

        // Request continuous repaint during playback
        ctx.request_repaint();
    }

    fn playback_rate_override_enabled(&self) -> bool {
        self.active_tab
            .map(|idx| self.tabs[idx].playback_rate_override)
            .unwrap_or(false)
    }

    fn update_record_rate_playback(&mut self, delta: f64, max_time: f64) {
        let Some(tab_idx) = self.active_tab else {
            self.is_playing = false;
            return;
        };

        let file_index = self.tabs[tab_idx].file_index;
        if file_index >= self.files.len() {
            self.is_playing = false;
            return;
        }

        let times = self.files[file_index].log.get_times_as_f64();
        if times.is_empty() {
            self.is_playing = false;
            return;
        }
        let time_count = times.len();
        let last_time = *times.last().unwrap_or(&max_time);

        let rate_hz = self.tabs[tab_idx].playback_rate_hz.clamp(0.1, 1000.0);
        self.tabs[tab_idx].playback_rate_hz = rate_hz;

        let current_record = self.get_cursor_record().unwrap_or(0).min(time_count - 1);
        let current_position = self
            .playback_record_position
            .unwrap_or(current_record as f64)
            .max(0.0);
        let new_position = current_position + (delta * rate_hz * self.playback_speed);
        let new_record = new_position.floor() as usize;

        if new_record >= time_count - 1 {
            self.set_cursor_record(Some(time_count - 1));
            self.set_cursor_time(Some(last_time));
            self.is_playing = false;
            self.last_frame_time = None;
            self.playback_record_position = None;
            return;
        }

        let new_time = times[new_record];
        self.playback_record_position = Some(new_position);
        self.set_cursor_record(Some(new_record));
        self.set_cursor_time(Some(new_time));
    }
}
