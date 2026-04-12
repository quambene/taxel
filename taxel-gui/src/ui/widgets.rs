use eframe::{
    self,
    egui::{
        vec2, Align2, Button, Color32, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui,
        Vec2, Window,
    },
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
            Stroke::new(1.0, Color32::from_gray(200)),
            StrokeKind::Middle,
        );
    }

    response
}

/// Draws a modal dialog warning about unsaved changes, with "Stay" and
/// "Continue" buttons.
pub fn draw_unsaved_changes_modal(ui: &mut Ui, stay: &mut bool, continue_nav: &mut bool) {
    Window::new("Unsaved changes")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.label("Switching sections will discard unsaved changes.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Stay").clicked() {
                    *stay = true;
                }
                if ui.button("Continue").clicked() {
                    *continue_nav = true;
                }
            });
        });
}
