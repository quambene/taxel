use eframe::egui::{vec2, Align, Button, Color32, Layout, Panel, ScrollArea, Ui};

pub const WARNING_COLOR: Color32 = Color32::from_rgb(180, 120, 0);
pub const ERROR_COLOR: Color32 = Color32::RED;

/// Indicates the issue severity for diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IssueSeverity {
    Error,
    Warning,
}

/// Collects all information about an error or warning to display in the
/// diagnostics panel and error summary in the header.
#[derive(Clone, Debug)]
pub(super) struct AppIssue {
    pub(super) severity: IssueSeverity,
    pub(super) message: String,
}

impl AppIssue {
    pub fn new_warning(message: String) -> Self {
        AppIssue {
            severity: IssueSeverity::Warning,
            message,
        }
    }

    pub fn new_error(message: String) -> Self {
        AppIssue {
            severity: IssueSeverity::Error,
            message,
        }
    }

    pub fn taxonomy_version_error() -> Self {
        AppIssue::new_error("Failed to determine taxonomy version".to_string())
    }
}

/// Draws a bottom diagnostics panel with detailed error and warning messages.
pub(super) fn draw_error_panel(ctx: &mut Ui, issues: &[AppIssue], show_error_panel: &mut bool) {
    Panel::bottom("error_panel")
        .resizable(true)
        .default_size(400.0)
        .show_inside(ctx, |ui| {
            ui.add_space(6.0);

            if draw_panel_header(ui, issues) {
                *show_error_panel = false;
            }

            ui.separator();

            let available = ui.available_size();
            ui.allocate_ui_with_layout(available, Layout::top_down(Align::Min), |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if issues.is_empty() {
                            ui.weak("No issues.");
                            return;
                        }

                        for issue in issues {
                            let (tag, color) = match issue.severity {
                                IssueSeverity::Error => ("Error", ERROR_COLOR),
                                IssueSeverity::Warning => ("Warning", WARNING_COLOR),
                            };

                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(color, format!("[{tag}]"));
                                ui.label(&issue.message);
                            });
                        }
                    });
            });
        });
}

/// Draws the header of the diagnostics panel, showing a summary of errors and
/// warnings, and a close button. Returns `true` if the close button was
/// clicked.
fn draw_panel_header(ui: &mut Ui, issues: &[AppIssue]) -> bool {
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Warning)
        .count();

    let mut close = false;

    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), ui.spacing().interact_size.y),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.strong("Diagnostics");
            ui.label(format!("errors: {errors}, warnings: {warnings}"));
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
