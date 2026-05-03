use crate::app::{AppDiagnostic, DiagnosticLevel};
use eframe::egui::{
    vec2, Align, Button, Color32, CursorIcon, Frame, InnerResponse, Layout, Margin, Panel,
    ScrollArea, Sense, Ui,
};

pub const WARNING_COLOR: Color32 = Color32::from_rgb(180, 120, 0);
pub const ERROR_COLOR: Color32 = Color32::RED;
pub const SUCCESS_COLOR: Color32 = Color32::from_rgb(34, 139, 34);

/// Draws a bottom diagnostics panel. Rows that carry a `fact` are clickable;
/// clicking one returns the fact name so the caller can navigate to it.
pub fn draw_error_panel(
    ctx: &mut Ui,
    diagnostics: &[AppDiagnostic],
    show_error_panel: &mut bool,
) -> Option<String> {
    Panel::bottom("error_panel")
        .resizable(true)
        .default_size(400.0)
        .show_inside(ctx, |ui| {
            ui.add_space(6.0);

            if draw_panel_header(ui, diagnostics) {
                *show_error_panel = false;
            }

            ui.separator();

            let available = ui.available_size();
            ui.allocate_ui_with_layout(available, Layout::top_down(Align::Min), |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if diagnostics.is_empty() {
                            ui.weak("No diagnostics.");
                            return None;
                        }

                        let mut clicked_fact = None;

                        for (i, diagnostic) in diagnostics.iter().enumerate().rev() {
                            let (tag, color) = match diagnostic.level {
                                DiagnosticLevel::Error => ("Error", ERROR_COLOR),
                                DiagnosticLevel::Warning => ("Warning", WARNING_COLOR),
                                DiagnosticLevel::Success => ("Success", SUCCESS_COLOR),
                            };

                            let row = draw_error_row(ui, diagnostic, color, tag);

                            let response = ui
                                .interact(
                                    row.response.rect,
                                    ui.id().with(("diag_nav", i)),
                                    Sense::click(),
                                )
                                .on_hover_cursor(CursorIcon::PointingHand);

                            if response.is_pointer_button_down_on() {
                                ui.painter().rect_filled(
                                    row.response.rect,
                                    2.0,
                                    ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                                );
                            } else if response.hovered() {
                                ui.painter().rect_filled(
                                    row.response.rect,
                                    2.0,
                                    ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.25),
                                );
                            }

                            if response.clicked() {
                                clicked_fact = diagnostic.fact.clone();
                            }
                        }

                        clicked_fact
                    })
                    .inner
            })
            .inner
        })
        .inner
}

/// Draws a single row in the diagnostics panel, with a colored tag and message.
/// If the diagnostic has an associated fact, it is included in the message.
/// Returns the response of the row for interaction handling.
fn draw_error_row(
    ui: &mut Ui,
    diagnostic: &AppDiagnostic,
    color: Color32,
    tag: &str,
) -> InnerResponse<()> {
    Frame::new()
        .inner_margin(Margin::symmetric(2, 1))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(color, format!("[{tag}]"));

                if let Some(fact) = &diagnostic.fact {
                    ui.label(format!("{}: {}", diagnostic.message, fact));
                } else {
                    ui.label(&diagnostic.message);
                }
            });
        })
}

/// Draws the header of the diagnostics panel, showing a summary of errors and
/// warnings, and a close button. Returns `true` if the close button was
/// clicked.
fn draw_panel_header(ui: &mut Ui, diagnostics: &[AppDiagnostic]) -> bool {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Warning)
        .count();
    let successes = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Success)
        .count();

    let mut close = false;

    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), ui.spacing().interact_size.y),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.label("Diagnostics");
            ui.label(format!("errors: {errors}"));
            ui.label(format!("warnings: {warnings}"));
            ui.label(format!("success: {successes}"));
            draw_close_button(ui, &mut close);
        },
    );

    close
}

fn draw_close_button(ui: &mut Ui, close: &mut bool) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.add(Button::new("\u{00D7}")).clicked() {
            *close = true;
        }
    });
}
