use eframe::egui::{self, Color32, Ui};

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

/// A custom button style with a dark background and white text, which also has a hover effect.
pub fn draw_dark_button(ui: &mut Ui, label: &str) -> egui::Response {
    let button = egui::Button::new(egui::RichText::new(label).color(Color32::WHITE).strong())
        .fill(Color32::from_gray(100));
    let response = ui.add(button);

    if response.hovered() {
        let cr = ui.visuals().widgets.inactive.corner_radius;
        ui.painter()
            .rect_filled(response.rect, cr, Color32::from_white_alpha(25));
        ui.painter().rect_stroke(
            response.rect,
            cr,
            egui::Stroke::new(1.0, Color32::from_gray(200)),
            egui::StrokeKind::Middle,
        );
    }

    response
}

/// Draws a modal dialog warning about unsaved changes, with "Stay" and
/// "Continue" buttons.
pub fn draw_unsaved_changes_modal(ui: &mut Ui, stay: &mut bool, continue_nav: &mut bool) {
    egui::Window::new("Unsaved changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
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
