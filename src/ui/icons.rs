//! Custom icon drawing utilities.

use eframe::egui;

/// Draw a right-pointing triangle (for collapsed state)
pub fn draw_triangle_right(ui: &mut egui::Ui, center: egui::Pos2, size: f32, color: egui::Color32) {
    let painter = ui.painter();
    let half = size / 2.0;
    let points = [
        egui::pos2(center.x - half * 0.3, center.y - half),
        egui::pos2(center.x + half * 0.6, center.y),
        egui::pos2(center.x - half * 0.3, center.y + half),
    ];
    painter.add(egui::Shape::convex_polygon(
        points.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
}

/// Draw a down-pointing triangle (for expanded state)
pub fn draw_triangle_down(ui: &mut egui::Ui, center: egui::Pos2, size: f32, color: egui::Color32) {
    let painter = ui.painter();
    let half = size / 2.0;
    let points = [
        egui::pos2(center.x - half, center.y - half * 0.3),
        egui::pos2(center.x + half, center.y - half * 0.3),
        egui::pos2(center.x, center.y + half * 0.6),
    ];
    painter.add(egui::Shape::convex_polygon(
        points.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
}

/// Draw an upload icon (circle with upward arrow)
pub fn draw_upload_icon(ui: &mut egui::Ui, center: egui::Pos2, size: f32, color: egui::Color32) {
    let painter = ui.painter();
    let radius = size / 2.0;

    // Draw circle outline
    painter.circle_stroke(center, radius, egui::Stroke::new(2.0, color));

    // Draw arrow pointing up
    let arrow_size = size * 0.35;
    let arrow_top = egui::pos2(center.x, center.y - arrow_size * 0.6);
    let arrow_bottom = egui::pos2(center.x, center.y + arrow_size * 0.4);

    // Arrow shaft
    painter.line_segment([arrow_bottom, arrow_top], egui::Stroke::new(2.0, color));

    // Arrow head
    let head_size = arrow_size * 0.4;
    painter.line_segment(
        [
            arrow_top,
            egui::pos2(arrow_top.x - head_size, arrow_top.y + head_size),
        ],
        egui::Stroke::new(2.0, color),
    );
    painter.line_segment(
        [
            arrow_top,
            egui::pos2(arrow_top.x + head_size, arrow_top.y + head_size),
        ],
        egui::Stroke::new(2.0, color),
    );
}

/// Draw an eye icon, optionally slashed for hidden state.
pub fn draw_eye_icon(
    ui: &mut egui::Ui,
    center: egui::Pos2,
    size: f32,
    color: egui::Color32,
    slashed: bool,
) {
    let painter = ui.painter();
    let half_width = size * 0.48;
    let half_height = size * 0.26;
    let stroke = egui::Stroke::new(1.6, color);

    let left = egui::pos2(center.x - half_width, center.y);
    let right = egui::pos2(center.x + half_width, center.y);
    let top = egui::pos2(center.x, center.y - half_height);
    let bottom = egui::pos2(center.x, center.y + half_height);

    painter.add(egui::Shape::line(vec![left, top, right], stroke));
    painter.add(egui::Shape::line(vec![left, bottom, right], stroke));
    painter.circle_stroke(center, size * 0.14, stroke);

    if slashed {
        let slash_offset = size * 0.38;
        painter.line_segment(
            [
                egui::pos2(center.x - slash_offset, center.y + slash_offset),
                egui::pos2(center.x + slash_offset, center.y - slash_offset),
            ],
            egui::Stroke::new(2.0, color),
        );
    }
}
