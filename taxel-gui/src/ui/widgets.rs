use eframe::{
    self,
    egui::{vec2, Button, Color32, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui},
};

/// A small clickable triangle button: points right when collapsed, down when expanded.
/// Painted directly rather than using a Unicode glyph to avoid font coverage issues.
pub fn triangle_button(ui: &mut Ui, collapsed: bool) -> Response {
    let size = vec2(12.0, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let color = ui.visuals().text_color();
        let center = rect.center();
        let r = 4.0_f32;
        let points = if collapsed {
            vec![
                center + vec2(-r * 0.6, -r),
                center + vec2(-r * 0.6, r),
                center + vec2(r, 0.0),
            ]
        } else {
            vec![
                center + vec2(-r, -r * 0.6),
                center + vec2(r, -r * 0.6),
                center + vec2(0.0, r),
            ]
        };
        ui.painter()
            .add(Shape::convex_polygon(points, color, Stroke::NONE));
    }

    response
}

/// A custom button style with a dark background and white text, which also has a hover effect.
pub fn draw_dark_button(ui: &mut Ui, label: &str) -> Response {
    let button = Button::new(RichText::new(label).color(Color32::WHITE).strong())
        .fill(Color32::from_gray(100));
    let response = ui.add(button);

    if response.hovered() {
        let cr = ui.visuals().widgets.inactive.corner_radius;
        ui.painter()
            .rect_filled(response.rect, cr, Color32::from_white_alpha(25));
        ui.painter().rect_stroke(
            response.rect,
            cr,
            Stroke::new(1.0_f32, Color32::from_gray(200)),
            StrokeKind::Middle,
        );
    }

    response
}
