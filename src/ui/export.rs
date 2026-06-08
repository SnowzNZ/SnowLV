//! Chart export functionality (PNG, PDF).

use printpdf::*;
use rust_i18n::t;

// Use fully qualified path to disambiguate from printpdf's image module
use ::image::{Rgba, RgbaImage};

use crate::analytics;
use crate::app::SnowLVApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::state::{HistogramMode, SelectedChannel};

struct ExportChannelSeries {
    name: String,
    unit: String,
    color: [u8; 3],
    points: Vec<(f64, f64)>,
    min: f64,
    max: f64,
    peak: (f64, f64),
}

/// Helper to push text ops into a Vec<Op>
fn push_text(ops: &mut Vec<Op>, text: &str, size: f32, x: Mm, y: Mm, font: &PdfFontHandle) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFont {
        font: font.clone(),
        size: Pt(size),
    });
    ops.push(Op::SetTextCursor {
        pos: Point::new(x, y),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}

/// Helper to push a filled rectangle (polygon) into ops
fn push_filled_rect(ops: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32) {
    let rect = Polygon {
        rings: vec![PolygonRing {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x + w), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x + w), Mm(y + h)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x), Mm(y + h)),
                    bezier: false,
                },
            ],
        }],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    };
    ops.push(Op::DrawPolygon { polygon: rect });
}

/// Helper to push a closed line (border) into ops
fn push_closed_line(ops: &mut Vec<Op>, points: &[(f32, f32)]) {
    let line = Line {
        points: points
            .iter()
            .map(|&(x, y)| LinePoint {
                p: Point::new(Mm(x), Mm(y)),
                bezier: false,
            })
            .collect(),
        is_closed: true,
    };
    ops.push(Op::DrawLine { line });
}

/// Helper to push an open line into ops
fn push_open_line(ops: &mut Vec<Op>, points: &[(f32, f32)]) {
    let line = Line {
        points: points
            .iter()
            .map(|&(x, y)| LinePoint {
                p: Point::new(Mm(x), Mm(y)),
                bezier: false,
            })
            .collect(),
        is_closed: false,
    };
    ops.push(Op::DrawLine { line });
}

fn selected_channel_display_name(app: &SnowLVApp, selected: &SelectedChannel) -> String {
    let channel_name = app.get_channel_name(selected.file_index, selected.channel_index);
    if app.field_normalization {
        normalize_channel_name_with_custom(&channel_name, Some(&app.custom_normalizations))
    } else {
        channel_name
    }
}

fn format_export_value(value: f64, unit: &str) -> String {
    let abs = value.abs();
    let decimals = if abs >= 100.0 {
        0
    } else if abs >= 10.0 {
        1
    } else {
        2
    };

    if unit.is_empty() {
        format!("{:.*}", decimals, value)
    } else {
        format!("{:.*} {}", decimals, value, unit)
    }
}

fn draw_filled_rect(
    img: &mut RgbaImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    color: Rgba<u8>,
) {
    let (img_width, img_height) = img.dimensions();
    let right = left.saturating_add(width).min(img_width);
    let bottom = top.saturating_add(height).min(img_height);
    for y in top..bottom {
        for x in left..right {
            img.put_pixel(x, y, color);
        }
    }
}

fn draw_filled_circle(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
    let (width, height) = img.dimensions();
    let r2 = radius * radius;
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r2 && x >= 0 && y >= 0 && x < width as i32 && y < height as i32
            {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    let (width, height) = img.dimensions();
    if x >= width || y >= height {
        return;
    }

    let alpha = color[3] as f32 / 255.0;
    if alpha <= 0.0 {
        return;
    }

    let base = *img.get_pixel(x, y);
    let inv_alpha = 1.0 - alpha;
    img.put_pixel(
        x,
        y,
        Rgba([
            (color[0] as f32 * alpha + base[0] as f32 * inv_alpha).round() as u8,
            (color[1] as f32 * alpha + base[1] as f32 * inv_alpha).round() as u8,
            (color[2] as f32 * alpha + base[2] as f32 * inv_alpha).round() as u8,
            255,
        ]),
    );
}

fn draw_chart_grid(img: &mut RgbaImage, left: u32, right: u32, top: u32, bottom: u32, opacity: u8) {
    if opacity == 0 {
        return;
    }

    let minor_alpha = ((opacity as u16 * 45) / 100).max(1) as u8;
    let major_alpha = opacity;
    let minor_color = Rgba([115, 123, 140, minor_alpha]);
    let major_color = Rgba([145, 153, 170, major_alpha]);
    let major_vertical_lines = 16u32;
    let major_horizontal_lines = 10u32;
    let minor_per_major = 5u32;
    let minor_vertical_lines = major_vertical_lines * minor_per_major;
    let minor_horizontal_lines = major_horizontal_lines * minor_per_major;

    for i in 1..minor_vertical_lines {
        if i % minor_per_major == 0 {
            continue;
        }
        let x =
            left + ((right - left) as f32 * i as f32 / minor_vertical_lines as f32).round() as u32;
        for y in top..bottom {
            blend_pixel(img, x, y, minor_color);
        }
    }

    for i in 1..minor_horizontal_lines {
        if i % minor_per_major == 0 {
            continue;
        }
        let y =
            top + ((bottom - top) as f32 * i as f32 / minor_horizontal_lines as f32).round() as u32;
        for x in left..right {
            blend_pixel(img, x, y, minor_color);
        }
    }

    for i in 1..major_vertical_lines {
        let x =
            left + ((right - left) as f32 * i as f32 / major_vertical_lines as f32).round() as u32;
        for y in top..bottom {
            blend_pixel(img, x, y, major_color);
        }
    }

    for i in 1..major_horizontal_lines {
        let y =
            top + ((bottom - top) as f32 * i as f32 / major_horizontal_lines as f32).round() as u32;
        for x in left..right {
            blend_pixel(img, x, y, major_color);
        }
    }
}

fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 6 * scale
}

fn rects_overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
}

fn clamp_label_rect(
    mut label_x: i32,
    mut label_y: i32,
    label_width: i32,
    label_height: i32,
    bounds: (i32, i32, i32, i32),
) -> (i32, i32, (i32, i32, i32, i32)) {
    let (left, top, right, bottom) = bounds;
    label_x = label_x.clamp(left + 6, right - label_width - 6);
    label_y = label_y.clamp(top + 5, bottom - label_height - 5);

    (
        label_x,
        label_y,
        (
            label_x - 6,
            label_y - 5,
            label_x + label_width + 6,
            label_y + label_height + 5,
        ),
    )
}

fn choose_peak_label_position(
    peak_x: i32,
    peak_y: i32,
    label_width: u32,
    label_height: u32,
    bounds: (i32, i32, i32, i32),
    used_rects: &mut Vec<(i32, i32, i32, i32)>,
) -> (i32, i32) {
    let label_width = label_width as i32;
    let label_height = label_height as i32;
    let mut candidates = Vec::new();

    for distance in [14, 34, 54, 74, 94, 114] {
        candidates.extend([
            (peak_x + distance, peak_y - label_height / 2),
            (peak_x - label_width - distance, peak_y - label_height / 2),
            (peak_x + distance, peak_y - label_height - 8),
            (peak_x - label_width - distance, peak_y - label_height - 8),
            (peak_x + distance, peak_y + 8),
            (peak_x - label_width - distance, peak_y + 8),
            (peak_x - label_width / 2, peak_y - label_height - distance),
            (peak_x - label_width / 2, peak_y + distance),
        ]);
    }

    let mut fallback = (peak_x + 18, peak_y - label_height - 12);
    let mut fallback_rect = (0, 0, 0, 0);

    for (candidate_x, candidate_y) in candidates {
        let (label_x, label_y, rect) =
            clamp_label_rect(candidate_x, candidate_y, label_width, label_height, bounds);
        fallback = (label_x, label_y);
        fallback_rect = rect;

        if !used_rects.iter().any(|used| rects_overlap(rect, *used)) {
            used_rects.push(rect);
            return (label_x, label_y);
        }
    }

    used_rects.push(fallback_rect);
    fallback
}

fn draw_text(img: &mut RgbaImage, text: &str, x: i32, y: i32, scale: u32, color: Rgba<u8>) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_char(img, ch, cursor_x, y, scale, color);
        cursor_x += (6 * scale) as i32;
    }
}

fn draw_text_blended(img: &mut RgbaImage, text: &str, x: i32, y: i32, scale: u32, color: Rgba<u8>) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_char_blended(img, ch, cursor_x, y, scale, color);
        cursor_x += (6 * scale) as i32;
    }
}

fn draw_char(img: &mut RgbaImage, ch: char, x: i32, y: i32, scale: u32, color: Rgba<u8>) {
    let glyph = glyph_rows(ch);
    let (width, height) = img.dimensions();
    for (row_idx, row) in glyph.iter().enumerate() {
        for (col_idx, pixel) in row.as_bytes().iter().enumerate() {
            if *pixel != b'1' {
                continue;
            }
            let px = x + col_idx as i32 * scale as i32;
            let py = y + row_idx as i32 * scale as i32;
            for sy in 0..scale as i32 {
                for sx in 0..scale as i32 {
                    let tx = px + sx;
                    let ty = py + sy;
                    if tx >= 0 && ty >= 0 && tx < width as i32 && ty < height as i32 {
                        img.put_pixel(tx as u32, ty as u32, color);
                    }
                }
            }
        }
    }
}

fn draw_char_blended(img: &mut RgbaImage, ch: char, x: i32, y: i32, scale: u32, color: Rgba<u8>) {
    let glyph = glyph_rows(ch);
    let (width, height) = img.dimensions();
    for (row_idx, row) in glyph.iter().enumerate() {
        for (col_idx, pixel) in row.as_bytes().iter().enumerate() {
            if *pixel != b'1' {
                continue;
            }
            let px = x + col_idx as i32 * scale as i32;
            let py = y + row_idx as i32 * scale as i32;
            for sy in 0..scale as i32 {
                for sx in 0..scale as i32 {
                    let tx = px + sx;
                    let ty = py + sy;
                    if tx >= 0 && ty >= 0 && tx < width as i32 && ty < height as i32 {
                        blend_pixel(img, tx as u32, ty as u32, color);
                    }
                }
            }
        }
    }
}

fn glyph_rows(ch: char) -> [&'static str; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'B' => [
            "11110", "10001", "10001", "11110", "10001", "10001", "11110",
        ],
        'C' => [
            "01111", "10000", "10000", "10000", "10000", "10000", "01111",
        ],
        'D' => [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
        'E' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
        'F' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "10000",
        ],
        'G' => [
            "01111", "10000", "10000", "10111", "10001", "10001", "01111",
        ],
        'H' => [
            "10001", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'I' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "11111",
        ],
        'J' => [
            "00111", "00010", "00010", "00010", "10010", "10010", "01100",
        ],
        'K' => [
            "10001", "10010", "10100", "11000", "10100", "10010", "10001",
        ],
        'L' => [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
        'M' => [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
        'N' => [
            "10001", "11001", "10101", "10011", "10001", "10001", "10001",
        ],
        'O' => [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'P' => [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
        'Q' => [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
        'R' => [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
        'S' => [
            "01111", "10000", "10000", "01110", "00001", "00001", "11110",
        ],
        'T' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
        'U' => [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'V' => [
            "10001", "10001", "10001", "10001", "10001", "01010", "00100",
        ],
        'W' => [
            "10001", "10001", "10001", "10101", "10101", "10101", "01010",
        ],
        'X' => [
            "10001", "10001", "01010", "00100", "01010", "10001", "10001",
        ],
        'Y' => [
            "10001", "10001", "01010", "00100", "00100", "00100", "00100",
        ],
        'Z' => [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
        '0' => [
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
        '1' => [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
        '3' => [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        '4' => [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
        '5' => [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
        '6' => [
            "01110", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => [
            "01110", "10001", "10001", "01111", "00001", "00001", "01110",
        ],
        '.' => [
            "00000", "00000", "00000", "00000", "00000", "01100", "01100",
        ],
        '-' => [
            "00000", "00000", "00000", "11111", "00000", "00000", "00000",
        ],
        '_' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "11111",
        ],
        '/' => [
            "00001", "00010", "00010", "00100", "01000", "01000", "10000",
        ],
        '%' => [
            "11001", "11010", "00010", "00100", "01000", "01011", "10011",
        ],
        ':' => [
            "00000", "01100", "01100", "00000", "01100", "01100", "00000",
        ],
        '(' => [
            "00010", "00100", "01000", "01000", "01000", "00100", "00010",
        ],
        ')' => [
            "01000", "00100", "00010", "00010", "00010", "00100", "01000",
        ],
        ' ' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "00000",
        ],
        _ => [
            "11111", "10001", "00010", "00100", "00100", "00000", "00100",
        ],
    }
}

impl SnowLVApp {
    fn collect_export_channel_series(
        &self,
        min_time: f64,
        max_time: f64,
    ) -> Vec<ExportChannelSeries> {
        self.get_selected_channels()
            .iter()
            .filter(|selected| !selected.hidden)
            .filter_map(|selected| {
                let file = self.files.get(selected.file_index)?;
                let times = file.log.get_times_as_f64();
                let data = self.get_channel_data(selected.file_index, selected.channel_index);
                if times.is_empty() || data.is_empty() || times.len() != data.len() {
                    return None;
                }

                let source_unit = selected.channel.unit();
                let mut points = Vec::new();
                let mut min = f64::INFINITY;
                let mut max = f64::NEG_INFINITY;
                let mut peak: Option<(f64, f64)> = None;

                for (&time, &raw_value) in times.iter().zip(data.iter()) {
                    if time < min_time || time > max_time {
                        continue;
                    }

                    let (value, _) = self.unit_preferences.convert_value(raw_value, source_unit);
                    if !value.is_finite() {
                        continue;
                    }

                    min = min.min(value);
                    max = max.max(value);
                    if peak.is_none_or(|(_, peak_value)| value > peak_value) {
                        peak = Some((time, value));
                    }
                    points.push((time, value));
                }

                let peak = peak?;
                let (_, display_unit) = self.unit_preferences.convert_value(0.0, source_unit);

                Some(ExportChannelSeries {
                    name: selected_channel_display_name(self, selected),
                    unit: display_unit.to_string(),
                    color: self.get_channel_color(selected.color_index),
                    points,
                    min,
                    max,
                    peak,
                })
            })
            .collect()
    }

    /// Export the current chart view as PNG
    pub fn export_chart_png(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("snowlv_chart.png")
            .save_file()
        else {
            return;
        };

        // Create a simple chart representation as image
        match self.render_chart_to_png(&path) {
            Ok(_) => {
                analytics::track_export("png");
                self.show_toast_success(&t!("toast.export_png_success"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Export the current chart view as PDF
    pub fn export_chart_pdf(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name("snowlv_chart.pdf")
            .save_file()
        else {
            return;
        };

        match self.render_chart_to_pdf(&path) {
            Ok(_) => {
                analytics::track_export("pdf");
                self.show_toast_success(&t!("toast.export_pdf_success"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Render chart data to PNG file
    fn render_chart_to_png(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let width = 1920u32;
        let height = 1080u32;

        // Create image buffer
        let mut imgbuf = RgbaImage::new(width, height);

        // Fill with dark background
        for pixel in imgbuf.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        // Draw chart area background
        let chart_left = 80u32;
        let chart_right = width - 40;
        let chart_top = 60u32;
        let chart_bottom = height - 115;

        for y in chart_top..chart_bottom {
            for x in chart_left..chart_right {
                imgbuf.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            }
        }
        if self.show_grid {
            draw_chart_grid(
                &mut imgbuf,
                chart_left,
                chart_right,
                chart_top,
                chart_bottom,
                self.grid_opacity,
            );
        }

        // Get time range
        let Some((min_time, max_time)) = self.time_range else {
            return Err("No time range available".into());
        };

        let time_span = max_time - min_time;
        if time_span <= 0.0 {
            return Err("Invalid time range".into());
        }

        let chart_width = (chart_right - chart_left) as f64;
        let chart_height = (chart_bottom - chart_top) as f64;
        let series = self.collect_export_channel_series(min_time, max_time);
        let mut peak_label_rects = Vec::new();

        // Draw each channel
        for channel in &series {
            let pixel_color = Rgba([channel.color[0], channel.color[1], channel.color[2], 255]);
            let data_range = if (channel.max - channel.min).abs() < 0.0001 {
                1.0
            } else {
                channel.max - channel.min
            };

            // Draw data points as lines
            let mut prev_x: Option<u32> = None;
            let mut prev_y: Option<u32> = None;

            for &(time, value) in &channel.points {
                let x_ratio = (time - min_time) / time_span;
                let y_ratio = (value - channel.min) / data_range;

                let x = chart_left + (x_ratio * chart_width) as u32;
                let y = chart_bottom - (y_ratio * chart_height) as u32;

                // Draw line from previous point
                if let (Some(px), Some(py)) = (prev_x, prev_y) {
                    draw_line(&mut imgbuf, px, py, x, y, pixel_color);
                }

                prev_x = Some(x);
                prev_y = Some(y);
            }

            let (peak_time, peak_value) = channel.peak;
            let peak_x = chart_left + (((peak_time - min_time) / time_span) * chart_width) as u32;
            let peak_y =
                chart_bottom - (((peak_value - channel.min) / data_range) * chart_height) as u32;
            draw_filled_circle(&mut imgbuf, peak_x as i32, peak_y as i32, 6, pixel_color);

            let label = format_export_value(peak_value, &channel.unit);
            let label_scale = 2;
            let label_width = text_width(&label, label_scale);
            let label_height = 7 * label_scale;
            let (label_x, label_y) = choose_peak_label_position(
                peak_x as i32,
                peak_y as i32,
                label_width,
                label_height,
                (
                    chart_left as i32,
                    chart_top as i32,
                    chart_right as i32,
                    chart_bottom as i32,
                ),
                &mut peak_label_rects,
            );
            let leader_x = if label_x > peak_x as i32 {
                label_x - 6
            } else {
                label_x + label_width as i32 + 6
            };
            let leader_y = label_y + label_height as i32 / 2;
            draw_line(
                &mut imgbuf,
                peak_x,
                peak_y,
                leader_x.clamp(chart_left as i32, chart_right as i32) as u32,
                leader_y.clamp(chart_top as i32, chart_bottom as i32) as u32,
                pixel_color,
            );
            draw_filled_rect(
                &mut imgbuf,
                (label_x - 4).max(0) as u32,
                (label_y - 4).max(0) as u32,
                label_width + 8,
                label_height + 8,
                Rgba([18, 18, 18, 205]),
            );
            draw_text(
                &mut imgbuf,
                &label,
                label_x,
                label_y,
                label_scale,
                pixel_color,
            );
        }

        // Draw bottom key/legend.
        let legend_top = chart_bottom + 24;
        let mut legend_x = chart_left;
        let legend_text_color = Rgba([230, 230, 230, 255]);
        let legend_scale = 2;
        for channel in &series {
            let marker_color = Rgba([channel.color[0], channel.color[1], channel.color[2], 255]);
            draw_filled_circle(
                &mut imgbuf,
                legend_x as i32 + 7,
                legend_top as i32 + 8,
                7,
                marker_color,
            );
            let label = channel.name.to_ascii_uppercase();
            draw_text(
                &mut imgbuf,
                &label,
                legend_x as i32 + 20,
                legend_top as i32 + 1,
                legend_scale,
                legend_text_color,
            );

            legend_x += text_width(&label, legend_scale) + 42;
            if legend_x > chart_right.saturating_sub(120) {
                break;
            }
        }

        let watermark = "GENERATED BY SNOWLV";
        let watermark_width = text_width(watermark, 2);
        draw_text_blended(
            &mut imgbuf,
            watermark,
            (width - watermark_width - 40) as i32,
            (height - 34) as i32,
            2,
            Rgba([230, 230, 230, 75]),
        );

        // Save the image
        imgbuf.save(path)?;

        Ok(())
    }

    /// Render chart data to PDF file
    fn render_chart_to_pdf(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut ops: Vec<Op> = Vec::new();

        let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
        let font_regular = PdfFontHandle::Builtin(BuiltinFont::Helvetica);

        // Get time range
        let Some((min_time, max_time)) = self.time_range else {
            return Err("No time range available".into());
        };

        let time_span = max_time - min_time;
        if time_span <= 0.0 {
            return Err("Invalid time range".into());
        }

        // Chart dimensions in mm (A4 landscape with margins)
        let margin: f64 = 20.0;
        let chart_left: f64 = margin;
        let chart_right: f64 = 297.0 - margin;
        let chart_bottom: f64 = margin + 20.0; // Leave room for time labels
        let chart_top: f64 = 210.0 - margin - 30.0; // Leave room for title

        let chart_width: f64 = chart_right - chart_left;
        let chart_height: f64 = chart_top - chart_bottom;
        let series = self.collect_export_channel_series(min_time, max_time);

        // Draw title
        push_text(
            &mut ops,
            "SnowLV Chart Export",
            16.0,
            Mm(margin as f32),
            Mm(200.0),
            &font_bold,
        );

        // Draw subtitle with file info
        if let Some(file) = self.files.first() {
            let subtitle = format!(
                "{} | {} channels selected | Time: {:.1}s - {:.1}s",
                file.name,
                self.get_selected_channels()
                    .iter()
                    .filter(|channel| !channel.hidden)
                    .count(),
                min_time,
                max_time
            );
            push_text(
                &mut ops,
                &subtitle,
                10.0,
                Mm(margin as f32),
                Mm(192.0),
                &font_regular,
            );
        }

        // Draw chart border
        let border_color = Color::Rgb(Rgb::new(0.3, 0.3, 0.3, None));
        ops.push(Op::SetOutlineColor { col: border_color });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

        push_closed_line(
            &mut ops,
            &[
                (chart_left as f32, chart_bottom as f32),
                (chart_right as f32, chart_bottom as f32),
                (chart_right as f32, chart_top as f32),
                (chart_left as f32, chart_top as f32),
            ],
        );

        // Draw each channel
        for channel in &series {
            let color_rgb = channel.color;
            let line_color = Color::Rgb(Rgb::new(
                color_rgb[0] as f32 / 255.0,
                color_rgb[1] as f32 / 255.0,
                color_rgb[2] as f32 / 255.0,
                None,
            ));

            ops.push(Op::SetOutlineColor {
                col: line_color.clone(),
            });
            ops.push(Op::SetOutlineThickness { pt: Pt(0.75) });

            let data_range = if (channel.max - channel.min).abs() < 0.0001 {
                1.0
            } else {
                channel.max - channel.min
            };

            // Build line points (downsample for PDF)
            let mut points: Vec<LinePoint> = Vec::new();
            let step = (channel.points.len() / 500).max(1); // Max ~500 points per channel

            for (i, &(time, value)) in channel.points.iter().enumerate() {
                if i % step != 0 {
                    continue;
                }

                let x_ratio = (time - min_time) / time_span;
                let y_ratio = (value - channel.min) / data_range;

                let x = chart_left + x_ratio * chart_width;
                let y = chart_bottom + y_ratio * chart_height;

                points.push(LinePoint {
                    p: Point::new(Mm(x as f32), Mm(y as f32)),
                    bezier: false,
                });
            }

            if points.len() >= 2 {
                let line = Line {
                    points,
                    is_closed: false,
                };
                ops.push(Op::DrawLine { line });
            }

            let (peak_time, peak_value) = channel.peak;
            let peak_x = chart_left + ((peak_time - min_time) / time_span) * chart_width;
            let peak_y = chart_bottom + ((peak_value - channel.min) / data_range) * chart_height;
            ops.push(Op::SetFillColor {
                col: line_color.clone(),
            });
            push_filled_rect(
                &mut ops,
                (peak_x - 1.6) as f32,
                (peak_y - 1.6) as f32,
                3.2,
                3.2,
            );
            ops.push(Op::SetFillColor {
                col: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
            });
            push_text(
                &mut ops,
                &format_export_value(peak_value, &channel.unit),
                8.0,
                Mm((peak_x + 2.5).min(chart_right - 24.0) as f32),
                Mm((peak_y + 3.0).min(chart_top - 3.0) as f32),
                &font_bold,
            );
        }

        // Draw legend
        let legend_y = chart_bottom - 12.0;
        let mut legend_x = chart_left;

        for channel in &series {
            let color_rgb = channel.color;
            let text_color = Color::Rgb(Rgb::new(
                color_rgb[0] as f32 / 255.0,
                color_rgb[1] as f32 / 255.0,
                color_rgb[2] as f32 / 255.0,
                None,
            ));

            ops.push(Op::SetFillColor {
                col: text_color.clone(),
            });
            push_filled_rect(&mut ops, legend_x as f32, (legend_y - 1.0) as f32, 3.0, 3.0);
            ops.push(Op::SetFillColor { col: text_color });
            push_text(
                &mut ops,
                &channel.name,
                8.0,
                Mm((legend_x + 5.0) as f32),
                Mm(legend_y as f32),
                &font_regular,
            );

            legend_x += 40.0;
            if legend_x > chart_right - 40.0 {
                break; // Don't overflow
            }
        }

        // Build page and save PDF
        let page = PdfPage::new(Mm(297.0), Mm(210.0), ops);
        let mut doc = PdfDocument::new("SnowLV Chart Export");
        doc.with_pages(vec![page]);
        let mut warnings = Vec::new();
        let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
        std::fs::write(path, pdf_bytes)?;

        Ok(())
    }

    /// Export the current histogram view as PNG
    pub fn export_histogram_png(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("snowlv_histogram.png")
            .save_file()
        else {
            return;
        };

        match self.render_histogram_to_png(&path) {
            Ok(_) => {
                analytics::track_export("histogram_png");
                self.show_toast_success(&t!("toast.histogram_exported_png"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Export the current scatter plot view as PNG
    pub fn export_scatter_plot_png(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("snowlv_scatter_plot.png")
            .save_file()
        else {
            return;
        };

        match self.render_scatter_plot_to_png(&path) {
            Ok(_) => {
                analytics::track_export("scatter_plot_png");
                self.show_toast_success(&t!("toast.scatter_exported_png"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Export the current scatter plot view as PDF
    pub fn export_scatter_plot_pdf(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name("snowlv_scatter_plot.pdf")
            .save_file()
        else {
            return;
        };

        match self.render_scatter_plot_to_pdf(&path) {
            Ok(_) => {
                analytics::track_export("scatter_plot_pdf");
                self.show_toast_success(&t!("toast.scatter_exported_pdf"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Export the current histogram view as PDF
    pub fn export_histogram_pdf(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name("snowlv_histogram.pdf")
            .save_file()
        else {
            return;
        };

        match self.render_histogram_to_pdf(&path) {
            Ok(_) => {
                analytics::track_export("histogram_pdf");
                self.show_toast_success(&t!("toast.histogram_exported_pdf"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Render histogram to PDF file
    fn render_histogram_to_pdf(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get tab and file data
        let tab_idx = self.active_tab.ok_or("No active tab")?;
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let file = &self.files[file_idx];
        let mode = config.mode;
        let (grid_cols, grid_rows) = config.effective_grid_size();

        let x_idx = config.x_channel.ok_or("X axis not selected")?;
        let y_idx = config.y_channel.ok_or("Y axis not selected")?;
        let z_idx = if mode == HistogramMode::AverageZ {
            config
                .z_channel
                .ok_or("Z axis not selected for Average mode")?
        } else {
            0 // unused
        };

        // Get channel data (handles both regular and computed channels)
        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);
        let z_data = if mode == HistogramMode::AverageZ {
            Some(self.get_channel_data(file_idx, z_idx))
        } else {
            None
        };

        if x_data.is_empty() || y_data.is_empty() {
            return Err("No data available".into());
        }

        // Calculate data bounds
        let x_min = x_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_max = x_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let y_min = y_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let y_max = y_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let x_range = if (x_max - x_min).abs() < f64::EPSILON {
            1.0
        } else {
            x_max - x_min
        };
        let y_range = if (y_max - y_min).abs() < f64::EPSILON {
            1.0
        } else {
            y_max - y_min
        };

        // Build histogram grid
        let mut hit_counts = vec![vec![0u32; grid_cols]; grid_rows];
        let mut z_sums = vec![vec![0.0f64; grid_cols]; grid_rows];

        for i in 0..x_data.len() {
            let x_bin = (((x_data[i] - x_min) / x_range) * (grid_cols - 1) as f64).round() as usize;
            let y_bin = (((y_data[i] - y_min) / y_range) * (grid_rows - 1) as f64).round() as usize;
            let x_bin = x_bin.min(grid_cols - 1);
            let y_bin = y_bin.min(grid_rows - 1);

            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
            }
        }

        // Calculate cell values and find min/max for color scaling
        let mut cell_values = vec![vec![None::<f64>; grid_cols]; grid_rows];
        let mut min_value: f64 = f64::INFINITY;
        let mut max_value: f64 = f64::NEG_INFINITY;

        for y_bin in 0..grid_rows {
            for x_bin in 0..grid_cols {
                let hits = hit_counts[y_bin][x_bin];
                if hits > 0 {
                    let value = match mode {
                        HistogramMode::HitCount => hits as f64,
                        HistogramMode::AverageZ => z_sums[y_bin][x_bin] / hits as f64,
                    };
                    cell_values[y_bin][x_bin] = Some(value);
                    min_value = min_value.min(value);
                    max_value = max_value.max(value);
                }
            }
        }

        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        // Get channel names (handles both regular and computed channels)
        let x_name = self.get_channel_name(file_idx, x_idx);
        let y_name = self.get_channel_name(file_idx, y_idx);
        let z_name = if mode == HistogramMode::AverageZ {
            self.get_channel_name(file_idx, z_idx)
        } else {
            "Hit Count".to_string()
        };

        let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
        let font_regular = PdfFontHandle::Builtin(BuiltinFont::Helvetica);

        let mut ops: Vec<Op> = Vec::new();

        // Chart dimensions in mm (A4 landscape with margins)
        let margin: f64 = 20.0;
        let axis_margin: f64 = 25.0;
        let chart_left: f64 = margin + axis_margin;
        let chart_right: f64 = 250.0; // Leave room for legend
        let chart_bottom: f64 = margin + axis_margin;
        let chart_top: f64 = 210.0 - margin - 30.0;

        let chart_width: f64 = chart_right - chart_left;
        let chart_height: f64 = chart_top - chart_bottom;

        let cell_width = chart_width / grid_cols as f64;
        let cell_height = chart_height / grid_rows as f64;

        // Draw title
        push_text(
            &mut ops,
            "SnowLV Histogram Export",
            16.0,
            Mm(margin as f32),
            Mm(200.0),
            &font_bold,
        );

        // Draw subtitle
        let subtitle = format!(
            "{} | Grid: {}x{} | Mode: {}",
            file.name,
            grid_cols,
            grid_rows,
            if mode == HistogramMode::HitCount {
                "Hit Count"
            } else {
                "Average Z"
            }
        );
        push_text(
            &mut ops,
            &subtitle,
            10.0,
            Mm(margin as f32),
            Mm(192.0),
            &font_regular,
        );

        // Draw axis labels
        let axis_subtitle = format!("X: {} | Y: {} | Z: {}", x_name, y_name, z_name);
        push_text(
            &mut ops,
            &axis_subtitle,
            9.0,
            Mm(margin as f32),
            Mm(186.0),
            &font_regular,
        );

        // Draw histogram cells
        for y_bin in 0..grid_rows {
            for x_bin in 0..grid_cols {
                let cell_x = chart_left + x_bin as f64 * cell_width;
                let cell_y = chart_bottom + y_bin as f64 * cell_height;

                if let Some(value) = cell_values[y_bin][x_bin] {
                    // Calculate color
                    let normalized = if mode == HistogramMode::HitCount && max_value > 1.0 {
                        (value.ln() / max_value.ln()).clamp(0.0, 1.0)
                    } else {
                        ((value - min_value) / value_range).clamp(0.0, 1.0)
                    };
                    let color = Self::get_pdf_heat_color(normalized);

                    ops.push(Op::SetFillColor { col: color });

                    push_filled_rect(
                        &mut ops,
                        cell_x as f32,
                        cell_y as f32,
                        cell_width as f32,
                        cell_height as f32,
                    );

                    // Draw cell value text (only for smaller grids)
                    if grid_cols.max(grid_rows) <= 32 {
                        let text = if mode == HistogramMode::HitCount {
                            format!("{}", hit_counts[y_bin][x_bin])
                        } else {
                            format!("{:.1}", value)
                        };

                        // Calculate text color based on brightness
                        let brightness = normalized;
                        let text_color = if brightness > 0.5 {
                            Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)) // Black
                        } else {
                            Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)) // White
                        };

                        ops.push(Op::SetFillColor { col: text_color });
                        let font_size = if grid_cols.max(grid_rows) <= 16 {
                            6.0
                        } else {
                            4.0
                        };
                        push_text(
                            &mut ops,
                            &text,
                            font_size,
                            Mm((cell_x + cell_width / 2.0 - 2.0) as f32),
                            Mm((cell_y + cell_height / 2.0 - 1.0) as f32),
                            &font_regular,
                        );
                    }
                }
            }
        }

        // Draw grid lines
        let grid_color = Color::Rgb(Rgb::new(0.4, 0.4, 0.4, None));
        ops.push(Op::SetOutlineColor { col: grid_color });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.25) });

        // Vertical grid lines (X axis divisions)
        for i in 0..=grid_cols {
            let x = chart_left + i as f64 * cell_width;
            push_open_line(
                &mut ops,
                &[
                    (x as f32, chart_bottom as f32),
                    (x as f32, chart_top as f32),
                ],
            );
        }

        // Horizontal grid lines (Y axis divisions)
        for i in 0..=grid_rows {
            let y = chart_bottom + i as f64 * cell_height;
            push_open_line(
                &mut ops,
                &[
                    (chart_left as f32, y as f32),
                    (chart_right as f32, y as f32),
                ],
            );
        }

        // Draw axis value labels
        let label_color = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
        ops.push(Op::SetFillColor { col: label_color });

        // Y axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = y_min + t * y_range;
            let y_pos = chart_bottom + t * chart_height;
            push_text(
                &mut ops,
                &format!("{:.0}", value),
                7.0,
                Mm((chart_left - 12.0) as f32),
                Mm((y_pos - 1.0) as f32),
                &font_regular,
            );
        }

        // X axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = x_min + t * x_range;
            let x_pos = chart_left + t * chart_width;
            push_text(
                &mut ops,
                &format!("{:.0}", value),
                7.0,
                Mm((x_pos - 4.0) as f32),
                Mm((chart_bottom - 8.0) as f32),
                &font_regular,
            );
        }

        // Draw legend (color scale)
        let legend_left: f64 = 260.0;
        let legend_width: f64 = 15.0;
        let legend_bottom: f64 = chart_bottom;
        let legend_height: f64 = chart_height;

        // Draw legend title
        ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
        });
        let legend_title = if mode == HistogramMode::HitCount {
            "Hits"
        } else {
            "Value"
        };
        push_text(
            &mut ops,
            legend_title,
            9.0,
            Mm(legend_left as f32),
            Mm((legend_bottom + legend_height + 5.0) as f32),
            &font_bold,
        );

        // Draw color gradient bar
        let gradient_steps = 30;
        let step_height = legend_height / gradient_steps as f64;

        for i in 0..gradient_steps {
            let t = i as f64 / gradient_steps as f64;
            let color = Self::get_pdf_heat_color(t);
            ops.push(Op::SetFillColor { col: color });

            let y = legend_bottom + i as f64 * step_height;
            push_filled_rect(
                &mut ops,
                legend_left as f32,
                y as f32,
                legend_width as f32,
                (step_height + 0.5) as f32,
            );
        }

        // Draw legend min/max labels
        ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
        });
        let min_label = if mode == HistogramMode::HitCount {
            "0".to_string()
        } else {
            format!("{:.1}", min_value)
        };
        let max_label = if mode == HistogramMode::HitCount {
            format!("{:.0}", max_value)
        } else {
            format!("{:.1}", max_value)
        };

        push_text(
            &mut ops,
            &min_label,
            7.0,
            Mm((legend_left + legend_width + 3.0) as f32),
            Mm(legend_bottom as f32),
            &font_regular,
        );
        push_text(
            &mut ops,
            &max_label,
            7.0,
            Mm((legend_left + legend_width + 3.0) as f32),
            Mm((legend_bottom + legend_height - 3.0) as f32),
            &font_regular,
        );

        // Draw statistics
        let stats_y = legend_bottom - 15.0;
        push_text(
            &mut ops,
            &format!("Total Points: {}", x_data.len()),
            8.0,
            Mm(legend_left as f32),
            Mm(stats_y as f32),
            &font_regular,
        );

        // Build page and save PDF
        let page = PdfPage::new(Mm(297.0), Mm(210.0), ops);
        let mut doc = PdfDocument::new("SnowLV Histogram Export");
        doc.with_pages(vec![page]);
        let mut warnings = Vec::new();
        let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
        std::fs::write(path, pdf_bytes)?;

        Ok(())
    }

    /// Get a PDF color from the heat map gradient based on normalized value (0-1)
    fn get_pdf_heat_color(normalized: f64) -> Color {
        const HEAT_COLORS: &[[u8; 3]] = &[
            [0, 0, 80],    // Dark blue (0.0)
            [0, 0, 180],   // Blue
            [0, 100, 255], // Light blue
            [0, 200, 255], // Cyan
            [0, 255, 200], // Cyan-green
            [0, 255, 100], // Green
            [100, 255, 0], // Yellow-green
            [200, 255, 0], // Yellow
            [255, 200, 0], // Orange
            [255, 100, 0], // Red-orange
            [255, 0, 0],   // Red (1.0)
        ];

        let t = normalized.clamp(0.0, 1.0);
        let scaled = t * (HEAT_COLORS.len() - 1) as f64;
        let idx = scaled.floor() as usize;
        let frac = scaled - idx as f64;

        if idx >= HEAT_COLORS.len() - 1 {
            let c = HEAT_COLORS[HEAT_COLORS.len() - 1];
            return Color::Rgb(Rgb::new(
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                None,
            ));
        }

        let c1 = HEAT_COLORS[idx];
        let c2 = HEAT_COLORS[idx + 1];

        let r = (c1[0] as f64 + (c2[0] as f64 - c1[0] as f64) * frac) / 255.0;
        let g = (c1[1] as f64 + (c2[1] as f64 - c1[1] as f64) * frac) / 255.0;
        let b = (c1[2] as f64 + (c2[2] as f64 - c1[2] as f64) * frac) / 255.0;

        Color::Rgb(Rgb::new(r as f32, g as f32, b as f32, None))
    }

    /// Get a PNG color from the heat map gradient based on normalized value (0-1)
    fn get_png_heat_color(normalized: f64) -> Rgba<u8> {
        const HEAT_COLORS: &[[u8; 3]] = &[
            [0, 0, 80],    // Dark blue (0.0)
            [0, 0, 180],   // Blue
            [0, 100, 255], // Light blue
            [0, 200, 255], // Cyan
            [0, 255, 200], // Cyan-green
            [0, 255, 100], // Green
            [100, 255, 0], // Yellow-green
            [200, 255, 0], // Yellow
            [255, 200, 0], // Orange
            [255, 100, 0], // Red-orange
            [255, 0, 0],   // Red (1.0)
        ];

        let t = normalized.clamp(0.0, 1.0);
        let scaled = t * (HEAT_COLORS.len() - 1) as f64;
        let idx = scaled.floor() as usize;
        let frac = scaled - idx as f64;

        if idx >= HEAT_COLORS.len() - 1 {
            let c = HEAT_COLORS[HEAT_COLORS.len() - 1];
            return Rgba([c[0], c[1], c[2], 255]);
        }

        let c1 = HEAT_COLORS[idx];
        let c2 = HEAT_COLORS[idx + 1];

        let r = (c1[0] as f64 + (c2[0] as f64 - c1[0] as f64) * frac) as u8;
        let g = (c1[1] as f64 + (c2[1] as f64 - c1[1] as f64) * frac) as u8;
        let b = (c1[2] as f64 + (c2[2] as f64 - c1[2] as f64) * frac) as u8;

        Rgba([r, g, b, 255])
    }

    /// Render histogram to PNG file
    fn render_histogram_to_png(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get tab and file data
        let tab_idx = self.active_tab.ok_or("No active tab")?;
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let mode = config.mode;
        let (grid_cols, grid_rows) = config.effective_grid_size();

        let x_idx = config.x_channel.ok_or("X axis not selected")?;
        let y_idx = config.y_channel.ok_or("Y axis not selected")?;
        let z_idx = if mode == HistogramMode::AverageZ {
            config
                .z_channel
                .ok_or("Z axis not selected for Average mode")?
        } else {
            0 // unused
        };

        // Get channel data (handles both regular and computed channels)
        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);
        let z_data = if mode == HistogramMode::AverageZ {
            Some(self.get_channel_data(file_idx, z_idx))
        } else {
            None
        };

        if x_data.is_empty() || y_data.is_empty() {
            return Err("No data available".into());
        }

        // Calculate data bounds
        let x_min = x_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_max = x_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let y_min = y_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let y_max = y_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let x_range = if (x_max - x_min).abs() < f64::EPSILON {
            1.0
        } else {
            x_max - x_min
        };
        let y_range = if (y_max - y_min).abs() < f64::EPSILON {
            1.0
        } else {
            y_max - y_min
        };

        // Build histogram grid
        let mut hit_counts = vec![vec![0u32; grid_cols]; grid_rows];
        let mut z_sums = vec![vec![0.0f64; grid_cols]; grid_rows];

        for i in 0..x_data.len() {
            let x_bin = (((x_data[i] - x_min) / x_range) * (grid_cols - 1) as f64).round() as usize;
            let y_bin = (((y_data[i] - y_min) / y_range) * (grid_rows - 1) as f64).round() as usize;
            let x_bin = x_bin.min(grid_cols - 1);
            let y_bin = y_bin.min(grid_rows - 1);

            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
            }
        }

        // Calculate cell values and find min/max for color scaling
        let mut cell_values = vec![vec![None::<f64>; grid_cols]; grid_rows];
        let mut min_value: f64 = f64::INFINITY;
        let mut max_value: f64 = f64::NEG_INFINITY;

        for y_bin in 0..grid_rows {
            for x_bin in 0..grid_cols {
                let hits = hit_counts[y_bin][x_bin];
                if hits > 0 {
                    let value = match mode {
                        HistogramMode::HitCount => hits as f64,
                        HistogramMode::AverageZ => z_sums[y_bin][x_bin] / hits as f64,
                    };
                    cell_values[y_bin][x_bin] = Some(value);
                    min_value = min_value.min(value);
                    max_value = max_value.max(value);
                }
            }
        }

        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        // Image dimensions
        let width = 1920u32;
        let height = 1080u32;
        let margin = 80u32;
        let legend_width = 100u32;

        let chart_left = margin;
        let chart_right = width - margin - legend_width;
        let chart_top = margin;
        let chart_bottom = height - margin;

        let chart_width = chart_right - chart_left;
        let chart_height = chart_bottom - chart_top;
        let cell_width = chart_width as f64 / grid_cols as f64;
        let cell_height = chart_height as f64 / grid_rows as f64;

        // Create image buffer
        let mut imgbuf = RgbaImage::new(width, height);

        // Fill with dark background
        for pixel in imgbuf.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        // Draw histogram cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..grid_rows {
            for x_bin in 0..grid_cols {
                if let Some(value) = cell_values[y_bin][x_bin] {
                    // Calculate color
                    let normalized = if mode == HistogramMode::HitCount && max_value > 1.0 {
                        (value.ln() / max_value.ln()).clamp(0.0, 1.0)
                    } else {
                        ((value - min_value) / value_range).clamp(0.0, 1.0)
                    };
                    let color = Self::get_png_heat_color(normalized);

                    // Calculate cell position (Y inverted - higher values at top)
                    let cell_x = chart_left as f64 + x_bin as f64 * cell_width;
                    let cell_y = chart_bottom as f64 - (y_bin + 1) as f64 * cell_height;

                    // Fill cell
                    for py in 0..(cell_height.ceil() as u32) {
                        for px in 0..(cell_width.ceil() as u32) {
                            let x = (cell_x as u32 + px).min(width - 1);
                            let y = (cell_y as u32 + py).min(height - 1);
                            imgbuf.put_pixel(x, y, color);
                        }
                    }
                }
            }
        }

        // Draw grid lines
        let grid_color = Rgba([80, 80, 80, 255]);

        // Vertical grid lines (X axis divisions)
        for i in 0..=grid_cols {
            let x = chart_left + (i as f64 * cell_width) as u32;
            for py in chart_top..chart_bottom {
                if x < width {
                    imgbuf.put_pixel(x, py, grid_color);
                }
            }
        }

        // Horizontal grid lines (Y axis divisions)
        for i in 0..=grid_rows {
            let y = chart_top + (i as f64 * cell_height) as u32;
            for px in chart_left..chart_right {
                if y < height {
                    imgbuf.put_pixel(px, y, grid_color);
                }
            }
        }

        // Draw color scale legend
        let legend_left = width - margin - legend_width + 20;
        let legend_bar_width = 20u32;
        let legend_height = chart_height;

        for i in 0..legend_height {
            let t = i as f64 / legend_height as f64;
            let color = Self::get_png_heat_color(t);

            for px in 0..legend_bar_width {
                let x = legend_left + px;
                let y = chart_bottom - i;
                if x < width && y < height {
                    imgbuf.put_pixel(x, y, color);
                }
            }
        }

        // Save the image
        imgbuf.save(path)?;

        Ok(())
    }

    /// Render scatter plot to PNG file (exports both left and right plots)
    fn render_scatter_plot_to_png(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tab_idx = self.active_tab.ok_or("No active tab")?;

        // Image dimensions (wider to fit two plots)
        let width = 2560u32;
        let height = 1080u32;
        let margin = 60u32;
        let gap = 40u32;

        let plot_width = (width - 2 * margin - gap) / 2;
        let plot_height = height - 2 * margin;

        // Create image buffer
        let mut imgbuf = RgbaImage::new(width, height);

        // Fill with dark background
        for pixel in imgbuf.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        // Render left plot
        let left_config = &self.tabs[tab_idx].scatter_plot_state.left;
        let left_rect = (margin, margin, plot_width, plot_height);
        self.render_scatter_plot_to_image(&mut imgbuf, left_config, left_rect, tab_idx)?;

        // Render right plot
        let right_config = &self.tabs[tab_idx].scatter_plot_state.right;
        let right_rect = (margin + plot_width + gap, margin, plot_width, plot_height);
        self.render_scatter_plot_to_image(&mut imgbuf, right_config, right_rect, tab_idx)?;

        // Save the image
        imgbuf.save(path)?;

        Ok(())
    }

    /// Helper to render a single scatter plot to an image region
    fn render_scatter_plot_to_image(
        &self,
        imgbuf: &mut RgbaImage,
        config: &crate::state::ScatterPlotConfig,
        rect: (u32, u32, u32, u32),
        tab_idx: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (left, top, width, height) = rect;
        let right = left + width;
        let bottom = top + height;

        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);

        // Check if we have valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                // Draw placeholder
                let placeholder_color = Rgba([80, 80, 80, 255]);
                for y in top..bottom {
                    for x in left..right {
                        imgbuf.put_pixel(x, y, placeholder_color);
                    }
                }
                return Ok(());
            }
        };

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);

        if x_data.is_empty() || y_data.is_empty() || x_data.len() != y_data.len() {
            return Err("No data available".into());
        }

        // Calculate data bounds
        let x_min = x_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_max = x_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let y_min = y_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let y_max = y_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let x_range = if (x_max - x_min).abs() < f64::EPSILON {
            1.0
        } else {
            x_max - x_min
        };
        let y_range = if (y_max - y_min).abs() < f64::EPSILON {
            1.0
        } else {
            y_max - y_min
        };

        // Build 2D histogram (512 bins like the UI)
        const HEATMAP_BINS: usize = 512;
        let mut histogram = vec![vec![0u32; HEATMAP_BINS]; HEATMAP_BINS];
        let mut max_hits: u32 = 0;

        for (&x, &y) in x_data.iter().zip(y_data.iter()) {
            let x_bin = (((x - x_min) / x_range) * (HEATMAP_BINS - 1) as f64).round() as usize;
            let y_bin = (((y - y_min) / y_range) * (HEATMAP_BINS - 1) as f64).round() as usize;

            let x_bin = x_bin.min(HEATMAP_BINS - 1);
            let y_bin = y_bin.min(HEATMAP_BINS - 1);

            histogram[y_bin][x_bin] += 1;
            max_hits = max_hits.max(histogram[y_bin][x_bin]);
        }

        // Fill background with black
        let bg_color = Rgba([0, 0, 0, 255]);
        for y in top..bottom {
            for x in left..right {
                imgbuf.put_pixel(x, y, bg_color);
            }
        }

        let cell_width = width as f64 / HEATMAP_BINS as f64;
        let cell_height = height as f64 / HEATMAP_BINS as f64;

        // Draw heatmap cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..HEATMAP_BINS {
            for x_bin in 0..HEATMAP_BINS {
                let hits = histogram[y_bin][x_bin];
                if hits > 0 {
                    // Normalize using log scale
                    let normalized = if max_hits > 1 {
                        (hits as f64).ln() / (max_hits as f64).ln()
                    } else {
                        1.0
                    };
                    let color = Self::get_png_heat_color(normalized);

                    // Calculate cell position (Y inverted)
                    let cell_x = left as f64 + x_bin as f64 * cell_width;
                    let cell_y = bottom as f64 - (y_bin + 1) as f64 * cell_height;

                    // Fill cell
                    for py in 0..(cell_height.ceil() as u32 + 1) {
                        for px in 0..(cell_width.ceil() as u32 + 1) {
                            let x = (cell_x as u32 + px).min(right - 1);
                            let y = (cell_y as u32 + py).min(bottom - 1);
                            if x >= left && y >= top {
                                imgbuf.put_pixel(x, y, color);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Render scatter plot to PDF file
    fn render_scatter_plot_to_pdf(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tab_idx = self.active_tab.ok_or("No active tab")?;

        let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
        let font_regular = PdfFontHandle::Builtin(BuiltinFont::Helvetica);

        let mut ops: Vec<Op> = Vec::new();

        // Draw title
        push_text(
            &mut ops,
            "SnowLV Scatter Plot Export",
            16.0,
            Mm(20.0),
            Mm(200.0),
            &font_bold,
        );

        // Subtitle with file info
        if let Some(file_idx) = self.selected_file {
            if file_idx < self.files.len() {
                let file = &self.files[file_idx];
                push_text(
                    &mut ops,
                    &file.name,
                    10.0,
                    Mm(20.0),
                    Mm(192.0),
                    &font_regular,
                );
            }
        }

        // Layout: two plots side by side
        let margin: f64 = 20.0;
        let gap: f64 = 15.0;
        let plot_width: f64 = (297.0 - 2.0 * margin - gap) / 2.0;
        let plot_height: f64 = 150.0;
        let plot_top: f64 = 180.0;
        let plot_bottom: f64 = plot_top - plot_height;

        // Render left plot
        let left_config = &self.tabs[tab_idx].scatter_plot_state.left;
        let left_rect = (margin, plot_bottom, plot_width, plot_height);
        self.render_scatter_plot_to_pdf_region(
            &mut ops,
            &font_regular,
            left_config,
            left_rect,
            tab_idx,
        )?;

        // Render right plot
        let right_config = &self.tabs[tab_idx].scatter_plot_state.right;
        let right_rect = (
            margin + plot_width + gap,
            plot_bottom,
            plot_width,
            plot_height,
        );
        self.render_scatter_plot_to_pdf_region(
            &mut ops,
            &font_regular,
            right_config,
            right_rect,
            tab_idx,
        )?;

        // Build page and save PDF
        let page = PdfPage::new(Mm(297.0), Mm(210.0), ops);
        let mut doc = PdfDocument::new("SnowLV Scatter Plot Export");
        doc.with_pages(vec![page]);
        let mut warnings = Vec::new();
        let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
        std::fs::write(path, pdf_bytes)?;

        Ok(())
    }

    /// Helper to render a single scatter plot to a PDF region
    fn render_scatter_plot_to_pdf_region(
        &self,
        ops: &mut Vec<Op>,
        font: &PdfFontHandle,
        config: &crate::state::ScatterPlotConfig,
        rect: (f64, f64, f64, f64),
        tab_idx: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (left, bottom, width, height) = rect;
        let right = left + width;
        let top = bottom + height;

        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);

        // Check if we have valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                // Draw placeholder text
                push_text(
                    ops,
                    "No axes selected",
                    10.0,
                    Mm((left + width / 2.0 - 15.0) as f32),
                    Mm((bottom + height / 2.0) as f32),
                    font,
                );
                return Ok(());
            }
        };

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);

        if x_data.is_empty() || y_data.is_empty() || x_data.len() != y_data.len() {
            return Err("No data available".into());
        }

        // Get channel names for labels (handles both regular and computed channels)
        let x_name = self.get_channel_name(file_idx, x_idx);
        let y_name = self.get_channel_name(file_idx, y_idx);

        // Draw axis labels
        push_text(
            ops,
            &format!("{} vs {}", y_name, x_name),
            9.0,
            Mm(left as f32),
            Mm((top + 3.0) as f32),
            font,
        );

        // Calculate data bounds
        let x_min = x_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_max = x_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let y_min = y_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let y_max = y_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let x_range = if (x_max - x_min).abs() < f64::EPSILON {
            1.0
        } else {
            x_max - x_min
        };
        let y_range = if (y_max - y_min).abs() < f64::EPSILON {
            1.0
        } else {
            y_max - y_min
        };

        // Build 2D histogram (use smaller bins for PDF)
        const PDF_BINS: usize = 64;
        let mut histogram = vec![vec![0u32; PDF_BINS]; PDF_BINS];
        let mut max_hits: u32 = 0;

        for (&x, &y) in x_data.iter().zip(y_data.iter()) {
            let x_bin = (((x - x_min) / x_range) * (PDF_BINS - 1) as f64).round() as usize;
            let y_bin = (((y - y_min) / y_range) * (PDF_BINS - 1) as f64).round() as usize;

            let x_bin = x_bin.min(PDF_BINS - 1);
            let y_bin = y_bin.min(PDF_BINS - 1);

            histogram[y_bin][x_bin] += 1;
            max_hits = max_hits.max(histogram[y_bin][x_bin]);
        }

        let cell_width = width / PDF_BINS as f64;
        let cell_height = height / PDF_BINS as f64;

        // Draw heatmap cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..PDF_BINS {
            for x_bin in 0..PDF_BINS {
                let hits = histogram[y_bin][x_bin];
                if hits > 0 {
                    // Normalize using log scale
                    let normalized = if max_hits > 1 {
                        (hits as f64).ln() / (max_hits as f64).ln()
                    } else {
                        1.0
                    };
                    let color = Self::get_pdf_heat_color(normalized);

                    ops.push(Op::SetFillColor { col: color });

                    let cell_x = left + x_bin as f64 * cell_width;
                    let cell_y = bottom + y_bin as f64 * cell_height;

                    push_filled_rect(
                        ops,
                        cell_x as f32,
                        cell_y as f32,
                        cell_width as f32,
                        cell_height as f32,
                    );
                }
            }
        }

        // Draw border
        let border_color = Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None));
        ops.push(Op::SetOutlineColor { col: border_color });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });

        push_closed_line(
            ops,
            &[
                (left as f32, bottom as f32),
                (right as f32, bottom as f32),
                (right as f32, top as f32),
                (left as f32, top as f32),
            ],
        );

        // Draw axis labels
        let label_color = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
        ops.push(Op::SetFillColor { col: label_color });

        // X axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = x_min + t * x_range;
            let x_pos = left + t * width;
            push_text(
                ops,
                &format!("{:.0}", value),
                6.0,
                Mm((x_pos - 3.0) as f32),
                Mm((bottom - 5.0) as f32),
                font,
            );
        }

        // Y axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = y_min + t * y_range;
            let y_pos = bottom + t * height;
            push_text(
                ops,
                &format!("{:.0}", value),
                6.0,
                Mm((left - 10.0) as f32),
                Mm((y_pos - 1.0) as f32),
                font,
            );
        }

        Ok(())
    }
}

/// Draw a line between two points using Bresenham's algorithm
fn draw_line(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, color: Rgba<u8>) {
    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = -(y1 as i32 - y0 as i32).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0 as i32;
    let mut y = y0 as i32;

    let (width, height) = img.dimensions();

    loop {
        if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }

        if x == x1 as i32 && y == y1 as i32 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
