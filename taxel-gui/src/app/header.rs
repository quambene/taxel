use super::{IssueSeverity, TaxelApp};
use crate::app::{error_panel::WARNING_COLOR, AppIssue};
use eframe::egui::{
    text::LayoutJob, vec2, Align, Button, Color32, Layout, Shape, TextEdit, TextFormat, Ui,
};
use rfd::FileDialog;
use std::{
    fs::{self},
    sync::mpsc,
    thread,
};
use taxel_gui::load_xml;

/// Draws the header panel of the application, including the "Import report"
/// button, the "Clear report" button, any error messages, and the language
/// selector tabs.
pub(super) fn draw_header(app: &mut TaxelApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        if ui.button("Import report").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("XML", &["xml"])
                .add_filter("All", &["*"])
                .pick_file()
            {
                import_report(app, path, ui.ctx().clone());
            }
        }

        if app.loading.is_some() {
            ui.spinner();
            ui.label("Loading…");
        }

        if app.table.is_some() && ui.button("Clear report").clicked() {
            app.table = None;
            app.report_path = None;
            app.issues.clear();
            app.show_error_panel = false;
        }

        if app.table.is_some() && ui.button("Validate report").clicked() {
            validate_report(app);
        }

        if app.table.is_some() && ui.button("Send report").clicked() {
            send_report(app);
        }

        draw_error_summary(app, ui);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            draw_language_toolbar(ui, &mut app.lang);

            ui.separator();

            draw_dark_mode_toggle(ui, &mut app.dark_mode);

            ui.separator();

            draw_zoom_toolbar(ui, &mut app.zoom_input);
        });
    });
}

/// Loads an XBRL instance document from the specified path and updates the app
/// state. The load runs on a background thread to keep the UI responsive.
fn import_report(app: &mut TaxelApp, path: std::path::PathBuf, ctx: eframe::egui::Context) {
    app.selected_tab = 0;
    app.table = None;
    app.report_path = Some(path.clone());
    app.issues.clear();
    app.show_error_panel = false;
    app.editing_section = None;
    app.edit_snapshot.clear();

    let (tx, rx) = mpsc::channel();
    app.loading = Some(rx);

    thread::spawn(move || {
        let table = load_xml(&path);
        let _ = tx.send(table);
        ctx.request_repaint();
    });
}

/// Reads the XML from `report_path`.
fn read_report_xml(app: &mut TaxelApp) -> Option<String> {
    let path = app.report_path.as_ref()?;

    match fs::read_to_string(path) {
        Ok(xml) => Some(xml),
        Err(err) => {
            app.issues.push(AppIssue {
                severity: IssueSeverity::Error,
                message: format!("Failed to read report file: {err}"),
            });
            app.show_error_panel = true;
            None
        }
    }
}

// TODO: determine taxonomy version
/// Validates the imported report and reports any issues in the diagnostics
/// panel.
fn validate_report(app: &mut TaxelApp) {
    app.issues.clear();

    let Some(xml) = read_report_xml(app) else {
        return;
    };
    let Some(eric) = &app.eric else { return };

    let taxonomy_type = "Bilanz";
    let taxonomy_version = "6.5";

    match eric.validate(xml, taxonomy_type, taxonomy_version, None) {
        Ok(response) => app.issues.push(AppIssue {
            severity: IssueSeverity::Error,
            message: format!(
                "Validation failed with error code {}: {}",
                response.error_code, response.validation_response
            ),
        }),
        Err(err) => app.issues.push(AppIssue {
            severity: IssueSeverity::Error,
            message: format!("Validation error: {err}"),
        }),
    }

    app.show_error_panel = !app.issues.is_empty();
}

// TODO: provide certifcate path and password
// TODO: determine taxonomy version
/// Sends the imported report and reports the server response in the diagnostics
/// panel.
fn send_report(app: &mut TaxelApp) {
    app.issues.clear();

    let Some(xml) = read_report_xml(app) else {
        return;
    };
    let Some(eric) = &app.eric else { return };

    let taxonomy_type = "Bilanz";
    let taxonomy_version = "6.5";

    match eric.send(xml, taxonomy_type, taxonomy_version, None) {
        Ok(response) => app.issues.push(AppIssue {
            severity: IssueSeverity::Error,
            message: format!(
                "Send failed with error code {}: {}",
                response.error_code, response.server_response
            ),
        }),
        Err(err) => app.issues.push(AppIssue {
            severity: IssueSeverity::Error,
            message: format!("Send error: {err}"),
        }),
    }

    app.show_error_panel = !app.issues.is_empty();
}

/// Draws a summary of errors and warnings in the header. Clicking on the
/// summary toggles the visibility of the detailed diagnostics panel.
fn draw_error_summary(app: &mut TaxelApp, ui: &mut Ui) {
    let error_count = app
        .issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .count();
    let warning_count = app
        .issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Warning)
        .count();

    if error_count > 0 || warning_count > 0 {
        ui.separator();

        let mut job = LayoutJob::default();
        job.append(
            &format!("errors: {error_count}"),
            0.0,
            TextFormat {
                color: Color32::RED,
                ..Default::default()
            },
        );
        job.append(
            &format!("  warnings: {warning_count}"),
            0.0,
            TextFormat {
                color: WARNING_COLOR,
                ..Default::default()
            },
        );

        let bg_idx = ui.painter().add(Shape::Noop);
        let response = ui.add(Button::new(job).frame(false));
        if response.hovered() {
            ui.painter().set(
                bg_idx,
                Shape::rect_filled(
                    response.rect,
                    ui.visuals().widgets.hovered.corner_radius,
                    ui.visuals().widgets.hovered.weak_bg_fill,
                ),
            );
        }

        if response.clicked() {
            app.show_error_panel = !app.show_error_panel;
        }
    }
}

/// Draw the zoom controls: `[+] [100%] [-]`.
fn draw_zoom_toolbar(ui: &mut Ui, zoom_input: &mut String) {
    let zoom = ui.ctx().zoom_factor();

    if ui
        .add(Button::new("−").min_size(vec2(24.0, 24.0)))
        .clicked()
    {
        let new_zoom = (zoom - 0.1).max(0.5);
        ui.ctx().set_zoom_factor(new_zoom);
        *zoom_input = format!("{}", (new_zoom * 100.0).round() as u32);
    }

    ui.label("%");

    let response = ui.add(
        TextEdit::singleline(zoom_input)
            .desired_width(35.0)
            .horizontal_align(Align::Center),
    );

    if response.lost_focus() {
        if let Ok(percent) = zoom_input.trim().parse::<u32>() {
            let clamped = percent.clamp(50, 400);
            ui.ctx().set_zoom_factor(clamped as f32 / 100.0);
            *zoom_input = format!("{}", clamped);
        } else {
            *zoom_input = format!("{}", (zoom * 100.0).round() as u32);
        }
    } else if !response.has_focus() {
        *zoom_input = format!("{}", (zoom * 100.0).round() as u32);
    }

    if ui
        .add(Button::new("+").min_size(vec2(24.0, 24.0)))
        .clicked()
    {
        let new_zoom = (zoom + 0.1).min(4.0);
        ui.ctx().set_zoom_factor(new_zoom);
        *zoom_input = format!("{}", (new_zoom * 100.0).round() as u32);
    }
}

/// Draw the dark mode toggle button (☀ / ☾).
fn draw_dark_mode_toggle(ui: &mut Ui, dark_mode: &mut bool) {
    // Show sun icon in dark mode (to switch to light) and moon icon in light
    // mode (to switch to dark).
    let icon = if *dark_mode { "\u{2600}" } else { "\u{1F319}" };

    let tooltip = if *dark_mode {
        "Switch to light mode"
    } else {
        "Switch to dark mode"
    };

    if ui
        .add(Button::new(icon).min_size(vec2(24.0, 24.0)))
        .on_hover_text(tooltip)
        .clicked()
    {
        *dark_mode = !*dark_mode;
    }
}

/// Draw the language selector tabs ("en", "de").
fn draw_language_toolbar(ui: &mut Ui, selected_lang: &mut String) {
    for (lang, tooltip) in [("de", "German"), ("en", "English")] {
        if ui
            .selectable_label(*selected_lang == lang, lang)
            .on_hover_text(tooltip)
            .clicked()
            && *selected_lang != lang
        {
            *selected_lang = lang.to_string();
        }
    }
}
