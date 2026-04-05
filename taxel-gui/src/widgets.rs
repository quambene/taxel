use eframe::egui::{self, Ui};

/// A small clickable triangle button: points right when collapsed, down when expanded.
/// Painted directly rather than using a Unicode glyph to avoid font coverage issues.
pub fn triangle_button(ui: &mut Ui, collapsed: bool) -> egui::Response {
    let size = egui::vec2(12.0, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let color = ui.visuals().text_color();
        let center = rect.center();
        let r = 4.0_f32;
        let points = if collapsed {
            vec![
                center + egui::vec2(-r * 0.6, -r),
                center + egui::vec2(-r * 0.6, r),
                center + egui::vec2(r, 0.0),
            ]
        } else {
            vec![
                center + egui::vec2(-r, -r * 0.6),
                center + egui::vec2(r, -r * 0.6),
                center + egui::vec2(0.0, r),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            color,
            egui::Stroke::NONE,
        ));
    }

    response
}
