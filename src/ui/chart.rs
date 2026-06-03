//! Chart rendering and data processing utilities.

use eframe::egui;
use egui_plot::{Line, Plot, PlotBounds, PlotPoints, VLine};
use rust_i18n::t;

use crate::app::SnowLVApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::state::{
    PlotArea, PlotChannelDragPayload, SelectedChannel, COLORBLIND_COLORS, MAX_CHART_POINTS,
    MIN_PLOT_HEIGHT, PLOT_RESIZE_HANDLE_HEIGHT,
};

/// Sensitivity multiplier for scroll-to-zoom (higher = faster zoom per scroll tick).
/// Shared between the cursor-tracking pre-show helper and the post-show closure.
const SCROLL_ZOOM_SENSITIVITY: f64 = 0.003;

/// Minimum visible time window allowed during zooming.
/// This is intentionally very small so users can zoom in as far as needed.
const MIN_ZOOM_WINDOW_SECONDS: f64 = 1e-6;

/// Synthetic plot area id used for the shared X viewport in stacked mode.
const STACKED_SHARED_X_BOUNDS_ID: usize = usize::MAX;

type ChartValueItems = Vec<(String, egui::Color32)>;

fn chart_scroll_delta_y(ui: &egui::Ui) -> f64 {
    ui.input(|i| {
        let smooth = i.smooth_scroll_delta.y as f64;
        let raw = i
            .raw
            .events
            .iter()
            .filter_map(|event| {
                if let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = event
                {
                    if modifiers.command {
                        return None;
                    }
                    let scale = match unit {
                        egui::MouseWheelUnit::Point => 1.0,
                        egui::MouseWheelUnit::Line => 50.0,
                        egui::MouseWheelUnit::Page => 400.0,
                    };
                    Some(delta.y as f64 * scale)
                } else {
                    None
                }
            })
            .sum::<f64>();
        if smooth.abs() >= raw.abs() {
            smooth
        } else {
            raw
        }
    })
}

fn clamp_chart_x_bounds(mut x_min: f64, mut x_max: f64, min_t: f64, max_t: f64) -> (f64, f64) {
    let data_width = (max_t - min_t).max(MIN_ZOOM_WINDOW_SECONDS);
    let width = (x_max - x_min).clamp(MIN_ZOOM_WINDOW_SECONDS, data_width);

    if width >= data_width {
        return (min_t, max_t);
    }

    if x_min < min_t {
        x_min = min_t;
        x_max = min_t + width;
    }
    if x_max > max_t {
        x_max = max_t;
        x_min = max_t - width;
    }

    (x_min, x_max)
}

fn pan_chart_x_bounds(
    x_bounds: (f64, f64),
    time_range: Option<(f64, f64)>,
    rect_width: f32,
    pointer_delta_x: f32,
) -> (f64, f64) {
    let Some((min_t, max_t)) = time_range else {
        return x_bounds;
    };
    if rect_width <= 1.0 || pointer_delta_x.abs() <= f32::EPSILON {
        return x_bounds;
    }

    let (x_min, x_max) = x_bounds;
    let seconds_per_pixel = (x_max - x_min) / rect_width as f64;
    let shift = -(pointer_delta_x as f64) * seconds_per_pixel;
    clamp_chart_x_bounds(x_min + shift, x_max + shift, min_t, max_t)
}

fn zoom_chart_x_bounds(
    x_bounds: (f64, f64),
    time_range: Option<(f64, f64)>,
    center: f64,
    zoom_factor: f64,
) -> (f64, f64) {
    let Some((min_t, max_t)) = time_range else {
        return x_bounds;
    };
    if (zoom_factor - 1.0).abs() <= f64::EPSILON {
        return x_bounds;
    }

    let (x_min, x_max) = x_bounds;
    let width = x_max - x_min;
    let new_width = (width * zoom_factor).clamp(
        MIN_ZOOM_WINDOW_SECONDS,
        (max_t - min_t).max(MIN_ZOOM_WINDOW_SECONDS),
    );
    let center = center.clamp(x_min, x_max);
    let ratio = if width > 0.0 {
        (center - x_min) / width
    } else {
        0.5
    };

    clamp_chart_x_bounds(
        center - new_width * ratio,
        center + new_width * (1.0 - ratio),
        min_t,
        max_t,
    )
}

fn initial_chart_x_bounds(
    time_range: Option<(f64, f64)>,
    cursor_time: Option<f64>,
    cursor_tracking: bool,
    chart_panned: bool,
    view_window: f64,
    chart_interacted: bool,
    initial_view_seconds: f64,
) -> Option<(f64, f64)> {
    let (min_t, max_t) = time_range?;
    if max_t <= min_t {
        return None;
    }

    let data_width = max_t - min_t;
    if cursor_tracking && !chart_panned {
        if let Some(cursor) = cursor_time {
            let window = view_window.min(data_width).max(MIN_ZOOM_WINDOW_SECONDS);
            let half_window = window / 2.0;
            return Some(clamp_chart_x_bounds(
                cursor - half_window,
                cursor + half_window,
                min_t,
                max_t,
            ));
        }
    }

    if !chart_interacted && data_width > initial_view_seconds {
        let window = initial_view_seconds
            .min(data_width)
            .max(MIN_ZOOM_WINDOW_SECONDS);
        if let Some(cursor) = cursor_time {
            let half_window = window / 2.0;
            Some(clamp_chart_x_bounds(
                cursor - half_window,
                cursor + half_window,
                min_t,
                max_t,
            ))
        } else {
            Some((min_t, min_t + window))
        }
    } else {
        Some((min_t, max_t))
    }
}

fn apply_chart_input_to_bounds(
    ui: &egui::Ui,
    x_bounds: (f64, f64),
    time_range: Option<(f64, f64)>,
    scroll_delta_y: f64,
) -> ((f64, f64), bool, bool) {
    let rect = ui.max_rect();
    let pointer_pos = ui.input(|i| i.pointer.latest_pos());
    let press_origin = ui.input(|i| i.pointer.press_origin());
    let pointer_in_rect = pointer_pos.is_some_and(|pos| rect.contains(pos));
    let drag_started_in_rect = press_origin.is_some_and(|pos| rect.contains(pos));

    let mut bounds = x_bounds;
    let mut panned = false;
    let mut zoomed = false;

    if drag_started_in_rect && ui.input(|i| i.pointer.primary_down()) {
        let delta_x = ui.input(|i| i.pointer.delta().x);
        let next_bounds = pan_chart_x_bounds(bounds, time_range, rect.width(), delta_x);
        panned = next_bounds != bounds;
        bounds = next_bounds;
    }

    if pointer_in_rect {
        let center = pointer_pos
            .filter(|_| rect.width() > 1.0)
            .map(|pos| {
                let ratio = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
                bounds.0 + ratio * (bounds.1 - bounds.0)
            })
            .unwrap_or((bounds.0 + bounds.1) / 2.0);

        if scroll_delta_y.abs() > 0.1 {
            let zoom_factor = (1.0 - scroll_delta_y * SCROLL_ZOOM_SENSITIVITY).clamp(0.8, 1.25);
            let next_bounds = zoom_chart_x_bounds(bounds, time_range, center, zoom_factor);
            zoomed |= next_bounds != bounds;
            bounds = next_bounds;
        }

        let zoom_delta = ui.input(|i| i.zoom_delta()) as f64;
        if zoom_delta != 1.0 {
            let next_bounds = zoom_chart_x_bounds(bounds, time_range, center, 1.0 / zoom_delta);
            zoomed |= next_bounds != bounds;
            bounds = next_bounds;
        }
    }

    (bounds, panned, zoomed)
}

fn render_chart_value_view(
    ctx: &egui::Context,
    id: egui::Id,
    pos: egui::Pos2,
    pivot: egui::Align2,
    values: &[(String, egui::Color32)],
) {
    if values.is_empty() {
        return;
    }

    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .interactable(false)
        .fixed_pos(pos)
        .pivot(pivot)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 230))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 3.0);
                    for (label, color) in values {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(14.0, 8.0), egui::Sense::hover());
                            ui.painter().line_segment(
                                [rect.left_center(), rect.right_center()],
                                egui::Stroke::new(2.0, *color),
                            );
                            ui.label(
                                egui::RichText::new(label)
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .size(12.0),
                            );
                        });
                    }
                });
        });
}

impl SnowLVApp {
    /// Render the main chart with cached downsampled data
    pub fn render_chart(&mut self, ui: &mut egui::Ui) {
        // Check if stacked mode is enabled
        let stacked_mode = self
            .active_tab
            .map(|idx| self.tabs[idx].stacked_mode)
            .unwrap_or(false);

        if stacked_mode {
            self.render_chart_stacked_mode(ui);
        } else {
            self.render_chart_single_mode(ui);
        }
    }

    /// Render single-plot mode chart (original implementation)
    fn render_chart_single_mode(&mut self, ui: &mut egui::Ui) {
        let total_selected = self.get_selected_channels().len();
        // Get visible selected channels from active tab
        let selected_channels: Vec<SelectedChannel> = self
            .get_selected_channels()
            .iter()
            .filter(|channel| !channel.hidden)
            .cloned()
            .collect();

        if selected_channels.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(if total_selected == 0 {
                        t!("chart.select_channels").to_string()
                    } else {
                        "All selected channels are hidden".to_string()
                    })
                    .size(self.scaled_font(20.0))
                    .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        let cursor_time = self.get_cursor_time();
        let cursor_tracking = self.cursor_tracking;
        let chart_panned = self.get_chart_panned();
        let view_window = self.get_current_view_window();
        let time_range = self.get_time_range();
        let chart_interacted = self.get_chart_interacted();
        let initial_view_seconds = self.initial_view_seconds;
        let scroll_delta_y = chart_scroll_delta_y(ui);
        let mut viewport = self
            .active_tab
            .and_then(|tab_idx| self.chart_last_x_bounds.get(&(tab_idx, 0)).copied())
            .or_else(|| {
                initial_chart_x_bounds(
                    time_range,
                    cursor_time,
                    cursor_tracking,
                    chart_panned,
                    view_window,
                    chart_interacted,
                    initial_view_seconds,
                )
            });
        let mut input_panned = false;
        let mut input_zoomed = false;
        if let Some(bounds) = viewport {
            let (next_bounds, panned, zoomed) =
                apply_chart_input_to_bounds(ui, bounds, time_range, scroll_delta_y);
            viewport = Some(next_bounds);
            input_panned = panned;
            input_zoomed = zoomed;
        }

        // Compute downsampled + normalized data sliced to the current viewport.
        // Detail scales with zoom level: a 1% viewport gets MAX_CHART_POINTS
        // over that 1%, not over the whole log.
        let chart_points: Vec<Option<Vec<[f64; 2]>>> = selected_channels
            .iter()
            .map(|selected| {
                self.compute_viewport_points(selected.file_index, selected.channel_index, viewport)
            })
            .collect();
        let use_normalization = self.field_normalization;
        let custom_mappings = &self.custom_normalizations;

        // Prepare data for the plot closure (can't borrow self mutably inside)
        let chart_points = &chart_points;
        let files = &self.files;
        let color_blind_mode = self.color_blind_mode;
        let theme = self.theme();
        let chart_palette = if color_blind_mode {
            COLORBLIND_COLORS
        } else {
            theme.chart.as_slice()
        };
        let playhead_color = theme.color(theme.playhead);
        let jump_to_time = self.get_jump_to_time();
        let values_follow_cursor = self.values_follow_cursor;
        let show_grid = self.show_grid;
        let grid_color = grid_color_with_opacity(ui, self.grid_opacity);

        let zooming = input_zoomed || ui.input(|i| i.zoom_delta() != 1.0);

        // Fixed Y bounds for normalized data (0-1 with small padding)
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;
        let default_x_bounds = viewport
            .or(time_range)
            .filter(|(min, max)| max > min)
            .unwrap_or((0.0, 1.0));

        // Build the plot - X-axis drag pans, wheel is handled below as zoom.
        let plot = Plot::new("log_chart")
            .y_axis_label("") // Hide Y axis label since values are normalized
            .show_axes([true, false]) // Show X axis (time), hide Y axis (normalized 0-1)
            .legend(egui_plot::Legend::default())
            .show_grid([show_grid, show_grid])
            .grid_color(grid_color)
            .default_x_bounds(default_x_bounds.0, default_x_bounds.1)
            .default_y_bounds(Y_MIN, Y_MAX)
            .auto_bounds([false, false])
            .allow_zoom([false, false])
            .allow_axis_zoom_drag([false, false])
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .allow_drag([false, false])
            .allow_scroll([false, false]);

        let response = plot.show(ui, |plot_ui| {
            // Get current bounds
            let current_bounds = plot_ui.plot_bounds();
            let mut x_min = current_bounds.min()[0];
            let mut x_max = current_bounds.max()[0];

            if let Some((viewport_min, viewport_max)) = viewport {
                x_min = viewport_min;
                x_max = viewport_max;
            }

            // Handle jump-to-time request (from min/max jump buttons)
            if let (Some(jump_time), Some((min_t, max_t))) = (jump_to_time, time_range) {
                // Center the view on the jump target time
                let current_width = (x_max - x_min).max(view_window);
                let half_width = current_width / 2.0;
                x_min = (jump_time - half_width).max(min_t);
                x_max = (jump_time + half_width).min(max_t);
                // Adjust if we hit a boundary
                if x_max - x_min < current_width {
                    if x_min == min_t {
                        x_max = (min_t + current_width).min(max_t);
                    } else {
                        x_min = (max_t - current_width).max(min_t);
                    }
                }
            } else if cursor_tracking && !chart_panned && !zooming {
                // In cursor tracking mode, keep a fixed-width window and only pan it.
                if let (Some(cursor), Some((min_t, max_t))) = (cursor_time, time_range) {
                    let data_width = max_t - min_t;
                    let window = view_window.min(data_width).max(MIN_ZOOM_WINDOW_SECONDS);
                    let half_window = window / 2.0;
                    x_min = cursor - half_window;
                    x_max = cursor + half_window;

                    if x_min < min_t {
                        x_min = min_t;
                        x_max = min_t + window;
                    }
                    if x_max > max_t {
                        x_max = max_t;
                        x_min = max_t - window;
                    }
                }
            } else if let Some((min_t, max_t)) = time_range {
                let data_width = max_t - min_t;

                // If chart hasn't been interacted with yet, use initial zoomed view
                if !chart_interacted && !zooming && data_width > initial_view_seconds {
                    if let Some(cursor) = cursor_time {
                        let half_window = initial_view_seconds / 2.0;
                        x_min = (cursor - half_window).max(min_t);
                        x_max = (cursor + half_window).min(max_t);
                        if x_max - x_min < initial_view_seconds {
                            if x_min == min_t {
                                x_max = (min_t + initial_view_seconds).min(max_t);
                            } else {
                                x_min = (max_t - initial_view_seconds).max(min_t);
                            }
                        }
                    } else {
                        // Show initial view window starting from the beginning
                        x_min = min_t;
                        x_max = min_t + initial_view_seconds;
                    }
                } else {
                    // Clamp X bounds to data range - prevent zooming out beyond data
                    let current_width = x_max - x_min;

                    // Don't allow view wider than data range
                    if current_width > data_width {
                        x_min = min_t;
                        x_max = max_t;
                    } else {
                        // Keep view within data bounds
                        if x_min < min_t {
                            x_min = min_t;
                            x_max = min_t + current_width;
                        }
                        if x_max > max_t {
                            x_max = max_t;
                            x_min = max_t - current_width;
                        }
                    }
                }
            }

            // Always enforce bounds: X clamped to data, Y fixed to normalized range
            let new_bounds = PlotBounds::from_min_max([x_min, Y_MIN], [x_max, Y_MAX]);
            plot_ui.set_plot_bounds(new_bounds);

            let pointer_plot_pos = plot_ui.pointer_coordinate();
            let pointer_screen_pos = plot_ui
                .ctx()
                .input(|i| i.pointer.latest_pos())
                .filter(|pos| plot_ui.transform().frame().contains(*pos));
            let hover_time = pointer_plot_pos.map(|pos| pos.x);
            let mouse_record = pointer_screen_pos
                .and_then(|_| hover_time.and_then(|t| self.find_record_at_time(t)));
            let record = mouse_record.or(self.get_cursor_record());

            let mut value_view_items: ChartValueItems = Vec::new();
            let palette = chart_palette;

            let line_names: Vec<String> = selected_channels
                .iter()
                .map(|selected| {
                    let original_name = selected.channel.name();
                    let base_name = if use_normalization {
                        normalize_channel_name_with_custom(&original_name, Some(custom_mappings))
                    } else {
                        original_name
                    };
                    if let Some(record) = record {
                        if let Some(value) = self.get_value_at_record(
                            selected.file_index,
                            selected.channel_index,
                            record,
                        ) {
                            let source_unit = selected.channel.unit();
                            let (converted_value, display_unit) =
                                self.unit_preferences.convert_value(value, source_unit);
                            let value_label = if display_unit.is_empty() {
                                format!("{}: {:.2}", base_name, converted_value)
                            } else {
                                format!("{}: {:.2} {}", base_name, converted_value, display_unit)
                            };

                            if values_follow_cursor && mouse_record.is_some() {
                                let color = palette[selected.color_index % palette.len()];
                                value_view_items.push((
                                    value_label.clone(),
                                    egui::Color32::from_rgb(color[0], color[1], color[2]),
                                ));
                            }
                            value_label
                        } else {
                            base_name
                        }
                    } else {
                        base_name
                    }
                })
                .collect();

            // Draw channel data lines
            for (i, selected) in selected_channels.iter().enumerate() {
                if selected.file_index >= files.len() {
                    continue;
                }

                if let Some(points) = chart_points.get(i).and_then(|p| p.as_ref()) {
                    let plot_points: PlotPoints = points.iter().copied().collect();
                    let color = palette[selected.color_index % palette.len()];

                    let name = &line_names[i];

                    plot_ui.line(
                        Line::new(name.clone(), plot_points)
                            .color(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .width(1.5),
                    );
                }
            }

            // Draw vertical cursor line
            if let Some(time) = cursor_time {
                plot_ui.vline(
                    VLine::new("Playhead", time)
                        .color(playhead_color)
                        .width(2.0),
                );
            }

            if !value_view_items.is_empty() {
                let frame = *plot_ui.transform().frame();
                if values_follow_cursor {
                    if let Some(pointer_screen_pos) = pointer_screen_pos {
                        let place_right = pointer_screen_pos.x + 180.0 < frame.right();
                        render_chart_value_view(
                            plot_ui.ctx(),
                            egui::Id::new("log_chart_mouse_value_view"),
                            pointer_screen_pos
                                + egui::vec2(if place_right { 14.0 } else { -14.0 }, 14.0),
                            if place_right {
                                egui::Align2::LEFT_TOP
                            } else {
                                egui::Align2::RIGHT_TOP
                            },
                            &value_view_items,
                        );
                    }
                }
            }

            // Return pointer position for clicks and the exact bounds we just set.
            (plot_ui.pointer_coordinate(), (x_min, x_max))
        });

        let x_bounds = response.inner.1;
        let viewport_changed = input_panned || input_zoomed;

        // Remember the X-axis bounds so the next frame can render and
        // downsample from the same viewport.
        if let Some(tab_idx) = self.active_tab {
            self.chart_last_x_bounds.insert((tab_idx, 0), x_bounds);
        }

        // Detect user interaction with chart (drag, zoom, scroll)
        // This marks the chart as "interacted" so we stop using the initial zoomed view
        let hovered_scroll_zoom = input_zoomed;
        let zooming = ui.input(|i| i.zoom_delta() != 1.0)
            || ui.input(|i| i.smooth_scroll_delta.x != 0.0)
            || hovered_scroll_zoom;

        if response.response.dragged()
            || response.response.drag_started()
            || zooming
            || viewport_changed
        {
            self.set_chart_interacted(true);
        }
        if input_panned
            || response.response.dragged()
            || response.response.drag_started()
            || (input_zoomed && cursor_tracking)
        {
            self.set_chart_panned(true);
        } else if (zooming || input_zoomed) && !cursor_tracking {
            self.set_chart_panned(false);
        }
        if viewport_changed {
            ui.ctx().request_repaint();
        }

        // Clear jump-to-time request after it's been processed
        if self.get_jump_to_time().is_some() {
            self.clear_jump_to_time();
            // Mark chart as interacted so future jumps work correctly
            self.set_chart_interacted(true);
        }

        // Handle click on chart to set cursor position
        if response.response.clicked() {
            if let Some(pos) = response.inner.0 {
                let clicked_time = pos.x;
                // Clamp to time range
                if let Some((min, max)) = self.get_time_range() {
                    // Stop playback when user clicks on chart
                    self.is_playing = false;
                    self.last_frame_time = None;

                    let clamped_time = clicked_time.clamp(min, max);
                    self.set_cursor_time(Some(clamped_time));
                    let record = self.find_record_at_time(clamped_time);
                    self.set_cursor_record(record);
                    // Force repaint to update legend values immediately
                    ui.ctx().request_repaint();
                }
            }
        }
    }

    /// Render stacked plot areas
    fn render_chart_stacked_mode(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("No active tab")
                        .size(self.scaled_font(20.0))
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        };

        let plot_areas = self.tabs[tab_idx].plot_areas.clone();
        let num_selected = self.tabs[tab_idx].selected_channels.len();

        if num_selected == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("chart.select_channels"))
                        .size(self.scaled_font(20.0))
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        // Track resize drag
        let mut resize_drag: Option<(usize, f32)> = None;

        // Get available height to constrain scroll area
        let max_scroll_height = ui.available_height();

        // Wrap in scroll area to allow vertical scrolling when plots don't fit
        egui::ScrollArea::vertical()
            .id_salt("stacked_plots_scroll")
            .max_height(max_scroll_height)
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                // Render each plot area
                for (plot_idx, plot_area) in plot_areas.iter().enumerate() {
                    // Skip collapsed plots (just show header)
                    if plot_area.collapsed {
                        self.render_plot_area_header_collapsed(ui, plot_area, plot_idx);
                        ui.add_space(5.0);
                        continue;
                    }

                    // Use the plot's own pixel height
                    let plot_height = plot_area.height_pixels.max(MIN_PLOT_HEIGHT);

                    // Render plot area header
                    ui.horizontal(|ui| {
                        self.render_plot_area_header(ui, plot_area, plot_idx);
                    });

                    ui.add_space(5.0);

                    // Get channels for this plot
                    let plot_channels: Vec<SelectedChannel> = plot_area
                        .channel_indices
                        .iter()
                        .filter_map(|&idx| self.tabs[tab_idx].selected_channels.get(idx).cloned())
                        .filter(|channel| !channel.hidden)
                        .collect();

                    if plot_area.channel_indices.is_empty() {
                        // Empty plot area with drop zone
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), plot_height),
                            egui::Sense::hover(),
                        );

                        // Check for dropped channel
                        if egui::DragAndDrop::has_payload_of_type::<(usize, usize)>(ui.ctx()) {
                            if let Some(payload) = response.dnd_release_payload::<(usize, usize)>()
                            {
                                if plot_area.has_capacity() {
                                    let (dropped_file_idx, dropped_channel_idx) = *payload;
                                    self.add_channel_to_plot(
                                        dropped_file_idx,
                                        dropped_channel_idx,
                                        plot_area.id,
                                    );
                                }
                            }
                        } else if egui::DragAndDrop::has_payload_of_type::<PlotChannelDragPayload>(
                            ui.ctx(),
                        ) {
                            if let Some(payload) =
                                response.dnd_release_payload::<PlotChannelDragPayload>()
                            {
                                self.move_channel_to_plot(payload.channel_idx, plot_area.id);
                            }
                        }

                        // Highlight if hovering with drag payload
                        let is_drop_target =
                            response.dnd_hover_payload::<(usize, usize)>().is_some()
                                || response
                                    .dnd_hover_payload::<PlotChannelDragPayload>()
                                    .is_some();
                        let stroke_color = if is_drop_target && plot_area.has_capacity() {
                            egui::Color32::from_rgb(71, 108, 155)
                        } else {
                            egui::Color32::from_gray(100)
                        };
                        let stroke_width = if is_drop_target { 2.0 } else { 1.0 };

                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(4),
                            egui::Stroke::new(stroke_width, stroke_color),
                            egui::StrokeKind::Outside,
                        );
                        ui.put(
                            rect,
                            egui::Label::new(
                                egui::RichText::new(
                                    if is_drop_target && plot_area.has_capacity() {
                                        "Drop channel here"
                                    } else {
                                        "No channels in this plot"
                                    },
                                )
                                .italics()
                                .color(if is_drop_target {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::GRAY
                                }),
                            ),
                        );
                    } else if plot_channels.is_empty() {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), plot_height),
                            egui::Sense::hover(),
                        );

                        if egui::DragAndDrop::has_payload_of_type::<(usize, usize)>(ui.ctx()) {
                            if let Some(payload) = response.dnd_release_payload::<(usize, usize)>()
                            {
                                if plot_area.has_capacity() {
                                    let (dropped_file_idx, dropped_channel_idx) = *payload;
                                    self.add_channel_to_plot(
                                        dropped_file_idx,
                                        dropped_channel_idx,
                                        plot_area.id,
                                    );
                                }
                            }
                        } else if egui::DragAndDrop::has_payload_of_type::<PlotChannelDragPayload>(
                            ui.ctx(),
                        ) {
                            if let Some(payload) =
                                response.dnd_release_payload::<PlotChannelDragPayload>()
                            {
                                self.move_channel_to_plot(payload.channel_idx, plot_area.id);
                            }
                        }

                        let is_drop_target =
                            response.dnd_hover_payload::<(usize, usize)>().is_some()
                                || response
                                    .dnd_hover_payload::<PlotChannelDragPayload>()
                                    .is_some();
                        let stroke_color = if is_drop_target && plot_area.has_capacity() {
                            egui::Color32::from_rgb(71, 108, 155)
                        } else {
                            egui::Color32::from_gray(80)
                        };

                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(4),
                            egui::Stroke::new(if is_drop_target { 2.0 } else { 1.0 }, stroke_color),
                            egui::StrokeKind::Outside,
                        );
                        ui.put(
                            rect,
                            egui::Label::new(
                                egui::RichText::new("All channels in this plot are hidden")
                                    .italics()
                                    .color(egui::Color32::GRAY),
                            ),
                        );
                    } else {
                        // Render the plot with drop zone support
                        self.render_single_plot(ui, &plot_channels, plot_area.id, plot_height);
                    }

                    ui.add_space(5.0);

                    // Resize handle (except after last plot)
                    if plot_idx < plot_areas.len() - 1 {
                        let handle_response = self.render_resize_handle(ui);
                        if handle_response.dragged() {
                            let delta_pixels = handle_response.drag_delta().y;
                            resize_drag = Some((plot_idx, delta_pixels));
                        }
                    }
                }

                // Apply resize if drag occurred
                if let Some((plot_idx, delta_pixels)) = resize_drag {
                    self.adjust_plot_heights(plot_idx, delta_pixels);
                }
            });
    }

    /// Render a single plot within a plot area
    fn render_single_plot(
        &mut self,
        ui: &mut egui::Ui,
        channels: &[SelectedChannel],
        plot_area_id: usize,
        height: f32,
    ) {
        let cursor_time = self.get_cursor_time();
        let cursor_tracking = self.cursor_tracking;
        let chart_panned = self.get_chart_panned();
        let view_window = self.get_current_view_window();
        let time_range = self.get_time_range();
        let chart_interacted = self.get_chart_interacted();
        let initial_view_seconds = self.initial_view_seconds;
        let scroll_delta_y = chart_scroll_delta_y(ui);

        // Compute viewport-aware downsampled + normalized points for this plot area.
        let shared_viewport = self.active_tab.and_then(|tab_idx| {
            self.chart_last_x_bounds
                .get(&(tab_idx, STACKED_SHARED_X_BOUNDS_ID))
                .copied()
        });
        let mut viewport = shared_viewport
            .or_else(|| {
                self.active_tab.and_then(|tab_idx| {
                    self.chart_last_x_bounds
                        .get(&(tab_idx, plot_area_id))
                        .copied()
                })
            })
            .or_else(|| {
                initial_chart_x_bounds(
                    time_range,
                    cursor_time,
                    cursor_tracking,
                    chart_panned,
                    view_window,
                    chart_interacted,
                    initial_view_seconds,
                )
            });
        let mut input_panned = false;
        let mut input_zoomed = false;
        if let Some(bounds) = viewport {
            let (next_bounds, panned, zoomed) =
                apply_chart_input_to_bounds(ui, bounds, time_range, scroll_delta_y);
            viewport = Some(next_bounds);
            input_panned = panned;
            input_zoomed = zoomed;
        }
        let chart_points: Vec<Option<Vec<[f64; 2]>>> = channels
            .iter()
            .map(|selected| {
                self.compute_viewport_points(selected.file_index, selected.channel_index, viewport)
            })
            .collect();
        // Prepare data for plot
        let use_normalization = self.field_normalization;
        let custom_mappings = &self.custom_normalizations;
        let chart_points = &chart_points;
        let files = &self.files;
        let color_blind_mode = self.color_blind_mode;
        let theme = self.theme();
        let chart_palette = if color_blind_mode {
            COLORBLIND_COLORS
        } else {
            theme.chart.as_slice()
        };
        let playhead_color = theme.color(theme.playhead);
        let jump_to_time = self.get_jump_to_time();
        let values_follow_cursor = self.values_follow_cursor;

        // Fixed Y bounds
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;

        let show_grid = self.show_grid;
        let grid_color = grid_color_with_opacity(ui, self.grid_opacity);

        let zooming = input_zoomed || ui.input(|i| i.zoom_delta() != 1.0);

        // Build plot with fixed height
        let default_x_bounds = viewport
            .or(time_range)
            .filter(|(min, max)| max > min)
            .unwrap_or((0.0, 1.0));
        let plot = Plot::new(format!("plot_{}", plot_area_id))
            .height(height)
            .y_axis_label("")
            .show_axes([true, false])
            .legend(egui_plot::Legend::default())
            .show_grid([show_grid, show_grid])
            .grid_color(grid_color)
            .default_x_bounds(default_x_bounds.0, default_x_bounds.1)
            .default_y_bounds(Y_MIN, Y_MAX)
            .auto_bounds([false, false])
            .allow_zoom([false, false])
            .allow_axis_zoom_drag([false, false])
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .allow_drag([false, false])
            .allow_scroll([false, false]);

        let response = plot.show(ui, |plot_ui| {
            // Get current bounds
            let current_bounds = plot_ui.plot_bounds();
            let mut x_min = current_bounds.min()[0];
            let mut x_max = current_bounds.max()[0];

            if let Some((viewport_min, viewport_max)) = viewport {
                x_min = viewport_min;
                x_max = viewport_max;
            }

            // Handle jump-to-time request
            if let (Some(jump_time), Some((min_t, max_t))) = (jump_to_time, time_range) {
                let current_width = (x_max - x_min).max(view_window);
                let half_width = current_width / 2.0;
                x_min = (jump_time - half_width).max(min_t);
                x_max = (jump_time + half_width).min(max_t);
                if x_max - x_min < current_width {
                    if x_min == min_t {
                        x_max = (min_t + current_width).min(max_t);
                    } else {
                        x_min = (max_t - current_width).max(min_t);
                    }
                }
            } else if cursor_tracking && !chart_panned && !zooming {
                if let (Some(cursor), Some((min_t, max_t))) = (cursor_time, time_range) {
                    let data_width = max_t - min_t;
                    let window = view_window.min(data_width).max(MIN_ZOOM_WINDOW_SECONDS);
                    let half_window = window / 2.0;
                    x_min = cursor - half_window;
                    x_max = cursor + half_window;

                    if x_min < min_t {
                        x_min = min_t;
                        x_max = min_t + window;
                    }
                    if x_max > max_t {
                        x_max = max_t;
                        x_min = max_t - window;
                    }
                }
            } else if let Some((min_t, max_t)) = time_range {
                let data_width = max_t - min_t;

                if !chart_interacted && !zooming && data_width > initial_view_seconds {
                    if let Some(cursor) = cursor_time {
                        let half_window = initial_view_seconds / 2.0;
                        x_min = (cursor - half_window).max(min_t);
                        x_max = (cursor + half_window).min(max_t);
                        if x_max - x_min < initial_view_seconds {
                            if x_min == min_t {
                                x_max = (min_t + initial_view_seconds).min(max_t);
                            } else {
                                x_min = (max_t - initial_view_seconds).max(min_t);
                            }
                        }
                    } else {
                        x_min = min_t;
                        x_max = min_t + initial_view_seconds;
                    }
                } else {
                    let current_width = x_max - x_min;

                    if current_width > data_width {
                        x_min = min_t;
                        x_max = max_t;
                    } else {
                        if x_min < min_t {
                            x_min = min_t;
                            x_max = min_t + current_width;
                        }
                        if x_max > max_t {
                            x_max = max_t;
                            x_min = max_t - current_width;
                        }
                    }
                }
            }

            // Set bounds
            let new_bounds = PlotBounds::from_min_max([x_min, Y_MIN], [x_max, Y_MAX]);
            plot_ui.set_plot_bounds(new_bounds);

            let pointer_plot_pos = plot_ui.pointer_coordinate();
            let pointer_screen_pos = plot_ui
                .ctx()
                .input(|i| i.pointer.latest_pos())
                .filter(|pos| plot_ui.transform().frame().contains(*pos));
            let hover_time = pointer_plot_pos.map(|pos| pos.x);
            let mouse_record = pointer_screen_pos
                .and_then(|_| hover_time.and_then(|t| self.find_record_at_time(t)));
            let record = mouse_record.or(self.get_cursor_record());

            let mut value_view_items: ChartValueItems = Vec::new();
            let palette = chart_palette;

            let line_names: Vec<String> = channels
                .iter()
                .map(|selected| {
                    let original_name = selected.channel.name();
                    let base_name = if use_normalization {
                        normalize_channel_name_with_custom(&original_name, Some(custom_mappings))
                    } else {
                        original_name
                    };
                    if let Some(record) = record {
                        if let Some(value) = self.get_value_at_record(
                            selected.file_index,
                            selected.channel_index,
                            record,
                        ) {
                            let source_unit = selected.channel.unit();
                            let (converted_value, display_unit) =
                                self.unit_preferences.convert_value(value, source_unit);
                            let value_label = if display_unit.is_empty() {
                                format!("{}: {:.2}", base_name, converted_value)
                            } else {
                                format!("{}: {:.2} {}", base_name, converted_value, display_unit)
                            };

                            if values_follow_cursor && mouse_record.is_some() {
                                let color = palette[selected.color_index % palette.len()];
                                value_view_items.push((
                                    value_label.clone(),
                                    egui::Color32::from_rgb(color[0], color[1], color[2]),
                                ));
                            }
                            value_label
                        } else {
                            base_name
                        }
                    } else {
                        base_name
                    }
                })
                .collect();

            // Draw channel lines
            for (i, selected) in channels.iter().enumerate() {
                if selected.file_index >= files.len() {
                    continue;
                }

                if let Some(points) = chart_points.get(i).and_then(|p| p.as_ref()) {
                    let plot_points: PlotPoints = points.iter().copied().collect();
                    let color = palette[selected.color_index % palette.len()];
                    let name = &line_names[i];

                    plot_ui.line(
                        Line::new(name.clone(), plot_points)
                            .color(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .width(1.5),
                    );
                }
            }

            // Draw cursor line
            if let Some(time) = cursor_time {
                plot_ui.vline(
                    VLine::new("Playhead", time)
                        .color(playhead_color)
                        .width(2.0),
                );
            }

            if !value_view_items.is_empty() {
                let frame = *plot_ui.transform().frame();
                if values_follow_cursor {
                    if let Some(pointer_screen_pos) = pointer_screen_pos {
                        let place_right = pointer_screen_pos.x + 180.0 < frame.right();
                        render_chart_value_view(
                            plot_ui.ctx(),
                            egui::Id::new(("plot_mouse_value_view", plot_area_id)),
                            pointer_screen_pos
                                + egui::vec2(if place_right { 14.0 } else { -14.0 }, 14.0),
                            if place_right {
                                egui::Align2::LEFT_TOP
                            } else {
                                egui::Align2::RIGHT_TOP
                            },
                            &value_view_items,
                        );
                    }
                }
            }

            (plot_ui.pointer_coordinate(), (x_min, x_max))
        });

        let x_bounds = response.inner.1;
        let viewport_changed = input_panned || input_zoomed;

        // Save the bounds so the next frame's render and downsample match the
        // visible viewport.
        if let Some(tab_idx) = self.active_tab {
            self.chart_last_x_bounds
                .insert((tab_idx, plot_area_id), x_bounds);
            self.chart_last_x_bounds
                .insert((tab_idx, STACKED_SHARED_X_BOUNDS_ID), x_bounds);
        }

        // Detect interaction
        let hovered_scroll_zoom = input_zoomed;
        let zooming = ui.input(|i| i.zoom_delta() != 1.0)
            || ui.input(|i| i.smooth_scroll_delta.x != 0.0)
            || hovered_scroll_zoom;
        if response.response.dragged()
            || response.response.drag_started()
            || zooming
            || viewport_changed
        {
            self.set_chart_interacted(true);
        }
        if input_panned
            || response.response.dragged()
            || response.response.drag_started()
            || (input_zoomed && cursor_tracking)
        {
            self.set_chart_panned(true);
        } else if (zooming || input_zoomed) && !cursor_tracking {
            self.set_chart_panned(false);
        }
        if viewport_changed {
            ui.ctx().request_repaint();
        }

        // Clear jump-to-time
        if self.get_jump_to_time().is_some() {
            self.clear_jump_to_time();
            self.set_chart_interacted(true);
        }

        // Handle click
        if response.response.clicked() {
            if let Some(pos) = response.inner.0 {
                let clicked_time = pos.x;
                if let Some((min, max)) = self.get_time_range() {
                    self.is_playing = false;
                    self.last_frame_time = None;
                    let clamped_time = clicked_time.clamp(min, max);
                    self.set_cursor_time(Some(clamped_time));
                    let record = self.find_record_at_time(clamped_time);
                    self.set_cursor_record(record);
                    ui.ctx().request_repaint();
                }
            }
        }

        // Handle dropped channel on the plot. Guard by payload type because
        // `dnd_release_payload` consumes the active payload even on a type mismatch.
        if egui::DragAndDrop::has_payload_of_type::<(usize, usize)>(ui.ctx()) {
            if let Some(payload) = response.response.dnd_release_payload::<(usize, usize)>() {
                let (dropped_file_idx, dropped_channel_idx) = *payload;
                self.add_channel_to_plot(dropped_file_idx, dropped_channel_idx, plot_area_id);
            }
        } else if egui::DragAndDrop::has_payload_of_type::<PlotChannelDragPayload>(ui.ctx()) {
            if let Some(payload) = response
                .response
                .dnd_release_payload::<PlotChannelDragPayload>()
            {
                self.move_channel_to_plot(payload.channel_idx, plot_area_id);
            }
        }

        // Highlight plot when hovering with drag payload
        if response
            .response
            .dnd_hover_payload::<(usize, usize)>()
            .is_some()
            || response
                .response
                .dnd_hover_payload::<PlotChannelDragPayload>()
                .is_some()
        {
            ui.painter().rect_stroke(
                response.response.rect,
                egui::CornerRadius::same(4),
                egui::Stroke::new(3.0, egui::Color32::from_rgb(71, 108, 155)),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Render plot area header with title and controls
    fn render_plot_area_header(
        &mut self,
        ui: &mut egui::Ui,
        plot_area: &PlotArea,
        plot_idx: usize,
    ) {
        let font_14 = self.scaled_font(14.0);

        // Collapse/expand icon (custom drawn triangle)
        let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
        let center = rect.center();
        let color = if response.hovered() {
            egui::Color32::WHITE
        } else {
            egui::Color32::LIGHT_GRAY
        };

        if plot_area.collapsed {
            crate::ui::icons::draw_triangle_right(ui, center, 12.0, color);
        } else {
            crate::ui::icons::draw_triangle_down(ui, center, 12.0, color);
        }

        if response.clicked() {
            if let Some(tab_idx) = self.active_tab {
                self.tabs[tab_idx].plot_areas[plot_idx].collapsed = !plot_area.collapsed;
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Plot title
        ui.label(egui::RichText::new(&plot_area.name).strong().size(font_14));

        // Channel count
        ui.label(
            egui::RichText::new(format!(
                "({}/{})",
                plot_area.channel_count(),
                10 // MAX_CHANNELS_PER_PLOT
            ))
            .color(egui::Color32::GRAY)
            .size(font_14),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Delete button
            if ui.button("🗑").on_hover_text("Delete plot area").clicked() {
                self.delete_plot_area(plot_area.id);
            }
        });
    }

    /// Render collapsed plot area header
    fn render_plot_area_header_collapsed(
        &mut self,
        ui: &mut egui::Ui,
        plot_area: &PlotArea,
        plot_idx: usize,
    ) {
        ui.horizontal(|ui| {
            // Expand icon (custom drawn triangle)
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
            let center = rect.center();
            let color = if response.hovered() {
                egui::Color32::WHITE
            } else {
                egui::Color32::GRAY
            };

            crate::ui::icons::draw_triangle_right(ui, center, 12.0, color);

            if response.clicked() {
                if let Some(tab_idx) = self.active_tab {
                    self.tabs[tab_idx].plot_areas[plot_idx].collapsed = false;
                }
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label(egui::RichText::new(&plot_area.name).color(egui::Color32::GRAY));
        });
    }

    /// Render resize handle between plots
    fn render_resize_handle(&self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), PLOT_RESIZE_HANDLE_HEIGHT),
            egui::Sense::drag(),
        );

        // Visual indicator
        let color = if response.hovered() || response.dragged() {
            egui::Color32::from_rgb(100, 150, 255)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };

        ui.painter().rect_filled(rect, 2.0, color);

        // Change cursor on hover
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        response
    }

    /// Format time in seconds to a human-readable string (h:mm:ss.xxx or m:ss.xxx or s.xxx)
    pub fn format_time(seconds: f64) -> String {
        let total_seconds = seconds.abs();
        let hours = (total_seconds / 3600.0).floor() as u32;
        let minutes = ((total_seconds % 3600.0) / 60.0).floor() as u32;
        let secs = total_seconds % 60.0;

        let sign = if seconds < 0.0 { "-" } else { "" };

        if hours > 0 {
            // h:mm:ss.xxx format
            format!("{}{}:{:02}:{:06.3}", sign, hours, minutes, secs)
        } else if minutes > 0 {
            // m:ss.xxx format
            format!("{}{}:{:06.3}", sign, minutes, secs)
        } else {
            // s.xxxs format
            format!("{}{:.3}s", sign, secs)
        }
    }

    /// Compute the points to plot for one channel, sliced to the currently
    /// visible viewport before LTTB-downsampling. Y is normalized to [0, 1]
    /// against the channel's full-range min/max so heights stay stable when
    /// the user pans or zooms. `viewport` is the previous frame's X bounds;
    /// when `None` (e.g., first frame after load) the full data range is used.
    fn compute_viewport_points(
        &mut self,
        file_index: usize,
        channel_index: usize,
        viewport: Option<(f64, f64)>,
    ) -> Option<Vec<[f64; 2]>> {
        // Resolve min/max first so the mutable borrow on the cache ends before
        // we take immutable borrows on the channel data below.
        let (min_y, max_y) = self
            .get_channel_min_max(file_index, channel_index)
            .unwrap_or((0.0, 1.0));

        let file = self.files.get(file_index)?;
        let times = file.log.get_times_as_f64();
        let data = self.get_channel_data_ref(file_index, channel_index);
        if times.is_empty() || times.len() != data.len() {
            return None;
        }

        let full_lttb = || Self::downsample_lttb(times, data, MAX_CHART_POINTS);
        let downsampled = match viewport {
            Some((vmin, vmax)) if vmax > vmin => {
                // Anchored min/max-per-bucket downsampling. Bucket
                // boundaries are at multiples of `bucket_size` from t=0,
                // so during cursor-tracked playback samples slide through
                // a fixed grid instead of being re-bucketed every frame.
                // Without this anchoring, LTTB-by-index re-selects a
                // different "best peak" per frame and the curve jitters
                // at far zoom-out.
                let pad = (vmax - vmin) * 0.1;
                let padded_span = (vmax - vmin) + 2.0 * pad;
                let n_buckets = (MAX_CHART_POINTS / 2).max(1);
                let bucket_size = padded_span / n_buckets as f64;
                if bucket_size <= 0.0 {
                    full_lttb()
                } else {
                    let raw_lo = vmin - pad;
                    let k_lo = (raw_lo / bucket_size).floor() as i64;
                    let mut points: Vec<[f64; 2]> = Vec::with_capacity(MAX_CHART_POINTS);
                    let mut idx = times.partition_point(|&t| t < k_lo as f64 * bucket_size);
                    for k in 0..n_buckets as i64 {
                        let bucket_end = (k_lo + k + 1) as f64 * bucket_size;
                        let mut end_idx = idx;
                        while end_idx < times.len() && times[end_idx] < bucket_end {
                            end_idx += 1;
                        }
                        if end_idx > idx {
                            let mut min_i = idx;
                            let mut max_i = idx;
                            for i in idx..end_idx {
                                if data[i] < data[min_i] {
                                    min_i = i;
                                }
                                if data[i] > data[max_i] {
                                    max_i = i;
                                }
                            }
                            if min_i == max_i {
                                points.push([times[min_i], data[min_i]]);
                            } else if min_i < max_i {
                                points.push([times[min_i], data[min_i]]);
                                points.push([times[max_i], data[max_i]]);
                            } else {
                                points.push([times[max_i], data[max_i]]);
                                points.push([times[min_i], data[min_i]]);
                            }
                        }
                        idx = end_idx;
                    }
                    points
                }
            }
            _ => full_lttb(),
        };

        let range = (max_y - min_y).abs();
        // Constant channels (range ≈ 0) get parked at the middle of the
        // overlay strip so they remain visible instead of pinning to the
        // bottom edge — matches the prior `normalize_points` behavior.
        if range < f64::EPSILON {
            return Some(downsampled.into_iter().map(|p| [p[0], 0.5]).collect());
        }
        Some(
            downsampled
                .into_iter()
                .map(|p| [p[0], (p[1] - min_y) / range])
                .collect(),
        )
    }

    /// Normalize values to 0-1 range for overlay display
    pub fn normalize_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        if points.is_empty() {
            return Vec::new();
        }

        // Find min and max Y values
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for point in points {
            min_y = min_y.min(point[1]);
            max_y = max_y.max(point[1]);
        }

        // Handle case where all values are the same
        let range = max_y - min_y;
        if range.abs() < f64::EPSILON {
            // All values are the same, put at 0.5
            return points.iter().map(|p| [p[0], 0.5]).collect();
        }

        // Normalize to 0-1 range
        points
            .iter()
            .map(|p| [p[0], (p[1] - min_y) / range])
            .collect()
    }

    /// Downsample data using the LTTB (Largest Triangle Three Buckets) algorithm.
    /// This preserves visual characteristics while reducing point count for performance.
    pub fn downsample_lttb(times: &[f64], values: &[f64], target_points: usize) -> Vec<[f64; 2]> {
        let n = times.len();

        if n <= target_points || target_points < 3 {
            // No downsampling needed
            return times
                .iter()
                .zip(values.iter())
                .map(|(t, v)| [*t, *v])
                .collect();
        }

        let mut result = Vec::with_capacity(target_points);

        // Always include first point
        result.push([times[0], values[0]]);

        // Bucket size
        let bucket_size = (n - 2) as f64 / (target_points - 2) as f64;

        let mut a_index = 0usize;

        for i in 0..(target_points - 2) {
            // Calculate bucket range
            let bucket_start = ((i as f64 + 1.0) * bucket_size).floor() as usize + 1;
            let bucket_end = (((i + 2) as f64) * bucket_size).floor() as usize + 1;
            let bucket_end = bucket_end.min(n - 1);

            // Calculate average point for next bucket (for triangle calculation)
            let next_bucket_start = bucket_end;
            let next_bucket_end = (((i + 3) as f64) * bucket_size).floor() as usize + 1;
            let next_bucket_end = next_bucket_end.min(n);

            let (avg_x, avg_y) = if next_bucket_start < next_bucket_end {
                let count = (next_bucket_end - next_bucket_start) as f64;
                let sum_x: f64 = times[next_bucket_start..next_bucket_end].iter().sum();
                let sum_y: f64 = values[next_bucket_start..next_bucket_end].iter().sum();
                (sum_x / count, sum_y / count)
            } else {
                (times[n - 1], values[n - 1])
            };

            // Find point in current bucket with largest triangle area
            let mut max_area = -1.0f64;
            let mut max_index = bucket_start;

            let a_x = times[a_index];
            let a_y = values[a_index];

            for j in bucket_start..bucket_end {
                // Calculate triangle area
                let area =
                    ((a_x - avg_x) * (values[j] - a_y) - (a_x - times[j]) * (avg_y - a_y)).abs();

                if area > max_area {
                    max_area = area;
                    max_index = j;
                }
            }

            result.push([times[max_index], values[max_index]]);
            a_index = max_index;
        }

        // Always include last point
        result.push([times[n - 1], values[n - 1]]);

        result
    }
}

/// Build a grid color matching the active theme but with a user-controlled
/// alpha override. The base RGB comes from `Visuals::text_color`, which is
/// what egui_plot uses by default; we just substitute the alpha so the
/// distance-based fade still applies on top.
fn grid_color_with_opacity(ui: &egui::Ui, alpha: u8) -> egui::Color32 {
    let c = ui.visuals().text_color();
    egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}
