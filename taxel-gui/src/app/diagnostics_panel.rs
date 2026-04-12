use eframe::egui::{vec2, Align, Button, Color32, Layout, Panel, ScrollArea, Ui};

pub const WARNING_COLOR: Color32 = Color32::from_rgb(180, 120, 0);
pub const ERROR_COLOR: Color32 = Color32::RED;
pub const SUCCESS_COLOR: Color32 = Color32::from_rgb(34, 139, 34);

/// Indicates the level of a diagnostics message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiagnosticLevel {
    Error,
    Warning,
    Success,
}

/// Groups diagnostics by their source domain so they can be cleared
/// independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiagnosticCategory {
    /// Issues related to the application itself, such as unexpected panics or
    /// unhandled errors that should never occur during normal operation.
    App,
    /// Issues related to creating or importing a report, such as file I/O
    /// errors or parsing errors.
    Import,
    /// Issues related to validating the report against the taxonomy.
    Validation,
    /// Issues related to sending the report to the tax authority's API.
    Send,
}

/// Collects all information about a diagnostics message to display in the
/// diagnostics panel and diagnostics summary in the header.
#[derive(Clone, Debug)]
pub(super) struct AppDiagnostic {
    pub(super) level: DiagnosticLevel,
    pub(super) category: DiagnosticCategory,
    pub(super) message: String,
}

impl AppDiagnostic {
    pub fn new_warning(category: DiagnosticCategory, message: String) -> Self {
        AppDiagnostic {
            level: DiagnosticLevel::Warning,
            category,
            message,
        }
    }

    pub fn new_error(category: DiagnosticCategory, message: String) -> Self {
        AppDiagnostic {
            level: DiagnosticLevel::Error,
            category,
            message,
        }
    }

    pub fn new_success(category: DiagnosticCategory, message: String) -> Self {
        AppDiagnostic {
            level: DiagnosticLevel::Success,
            category,
            message,
        }
    }

    pub fn taxonomy_version_error(category: DiagnosticCategory) -> Self {
        AppDiagnostic::new_error(category, "Failed to determine taxonomy version".to_string())
    }
}

/// Draws a bottom diagnostics panel with detailed error and warning messages.
pub(super) fn draw_error_panel(
    ctx: &mut Ui,
    diagnostics: &[AppDiagnostic],
    show_error_panel: &mut bool,
) {
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
                            return;
                        }

                        for diagnostic in diagnostics.iter().rev() {
                            let (tag, color) = match diagnostic.level {
                                DiagnosticLevel::Error => ("Error", ERROR_COLOR),
                                DiagnosticLevel::Warning => ("Warning", WARNING_COLOR),
                                DiagnosticLevel::Success => ("Success", SUCCESS_COLOR),
                            };

                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(color, format!("[{tag}]"));
                                ui.label(&diagnostic.message);
                            });
                        }
                    });
            });
        });
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
