use super::TaxelApp;
use crate::app::diagnostics_panel::{
    AppDiagnostic, DiagnosticCategory, DiagnosticLevel, SUCCESS_COLOR, WARNING_COLOR,
};
use eframe::egui::{
    self, text::LayoutJob, vec2, Align, Button, Color32, Layout, Shape, TextEdit, TextFormat, Ui,
};
use eric_sdk::ErrorCode;
use rfd::FileDialog;
use std::{
    fs::{self},
    path::PathBuf,
    sync::mpsc,
    thread,
};
use taxel::TAXONOMY_YEAR_TO_VERSION;
use taxel_gui::{load_xml, report_store, report_store::ReportStatus};
use xbrl_rs::TaxonomySet;

/// Draws the header panel of the application, including the "Import report"
/// button, the "Close report" button, any error messages, and the language
/// selector tabs.
pub(super) fn draw_header(app: &mut TaxelApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        if app.table.is_none() && app.loading.is_none() && ui.button("Import report").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("XML", &["xml"])
                .add_filter("All", &["*"])
                .pick_file()
            {
                match report_store::copy_report(&path) {
                    Ok(copied_path) => {
                        app.register_imported_report(&copied_path);
                        app.refresh_imported_reports();
                        load_report(app, copied_path, ui.ctx().clone());
                    }
                    Err(err) => {
                        app.diagnostics.push(AppDiagnostic::new_error(
                            DiagnosticCategory::App,
                            format!("Failed to import report: {err}"),
                        ));
                        app.show_error_panel = true;
                    }
                }
            }
        }

        if app.loading.is_some() {
            ui.spinner();
            ui.label("Loading…");
        }

        if app.table.is_some() && ui.button("Close report").clicked() {
            app.table = None;
            app.taxonomy = None;
            app.report = None;
            app.report_path = None;
            app.selected_tab = 0;
            app.search.results.clear();
            app.search.scroll_to_row = None;
            app.search.row_highlight = None;
            app.loading = None;
            app.diagnostics.clear();
            app.show_error_panel = true;
            app.editing_section = None;
            app.edit_snapshot.clear();
            app.report_status = ReportStatus::Draft;
        }

        if app.table.is_some() && ui.button("Validate report").clicked() {
            validate_report(app);
        }

        if app.table.is_some() && ui.button("Send report").clicked() {
            send_report(app);
        }

        draw_error_summary(app, ui);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            draw_language_toolbar(ui, &mut app.settings.lang);

            ui.separator();

            draw_dark_mode_toggle(ui, &mut app.settings.dark_mode);

            ui.separator();

            draw_zoom_toolbar(ui, &mut app.settings.zoom_input);
        });
    });
}

/// Loads an XBRL instance document from the specified path and updates the app
/// state. The load runs on a background thread to keep the UI responsive.
pub(super) fn load_report(app: &mut TaxelApp, path: PathBuf, ctx: egui::Context) {
    app.selected_tab = 0;
    app.table = None;
    app.report_path = Some(path.clone());
    app.diagnostics.clear();
    app.editing_section = None;
    app.edit_snapshot.clear();
    app.report_status = app
        .report_list
        .report_status(&path)
        .unwrap_or(ReportStatus::Draft);

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
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to read report file: {err}"),
            ));
            app.show_error_panel = true;
            None
        }
    }
}

/// The taxonomy version is derived from the schemaRef URLs, which
/// contain a date like "2024-06-30". We extract the year and map it to
/// the corresponding version string expected by ERiC.
fn extract_taxonomy_version(taxonomy: &Option<TaxonomySet>) -> Option<&&str> {
    taxonomy
        .as_ref()
        .and_then(|taxonomy| taxonomy.version())
        .map(|date| date.split('-').next().unwrap_or(date))
        .and_then(|version| TAXONOMY_YEAR_TO_VERSION.get(version))
}

/// Validates the imported report and reports any issues in the diagnostics
/// panel.
fn validate_report(app: &mut TaxelApp) {
    clear_diagnostics_by_category(app, DiagnosticCategory::Validation);
    app.report_status = ReportStatus::Draft;
    if let Some(path) = app.report_path.as_ref() {
        app.report_list
            .set_report_status(path, ReportStatus::Draft, &mut app.diagnostics);
    }

    let Some(xml) = read_report_xml(app) else {
        return;
    };
    let Some(eric) = &app.eric else { return };
    let Some(taxonomy_version) = extract_taxonomy_version(&app.taxonomy) else {
        app.diagnostics.push(AppDiagnostic::taxonomy_version_error(
            DiagnosticCategory::Validation,
        ));
        app.show_error_panel = true;
        return;
    };

    match eric.validate(xml, "Bilanz", taxonomy_version, None) {
        Ok(response) => {
            if response.error_code == ErrorCode::ERIC_OK as i32 {
                app.report_status = ReportStatus::Validated;
                if let Some(path) = app.report_path.as_ref() {
                    app.report_list.set_report_status(
                        path,
                        ReportStatus::Validated,
                        &mut app.diagnostics,
                    );
                }
                app.diagnostics.push(AppDiagnostic::new_success(
                    DiagnosticCategory::Validation,
                    format!(
                        "Validation completed successfully\n{}",
                        response.validation_response
                    ),
                ));
            } else {
                // TODO: parse `validation_response` for better error messages
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::Validation,
                    format!(
                        "Validation failed with error code {}\n{}",
                        response.error_code, response.validation_response
                    ),
                ));
            }
        }
        Err(err) => app.diagnostics.push(AppDiagnostic::new_error(
            DiagnosticCategory::Validation,
            format!("Validation error: {err}"),
        )),
    }

    app.show_error_panel = true;
}

// TODO: provide certifcate path and password
/// Sends the imported report and reports the server response in the diagnostics
/// panel.
fn send_report(app: &mut TaxelApp) {
    clear_diagnostics_by_category(app, DiagnosticCategory::Send);

    if app.report_status != ReportStatus::Validated {
        app.diagnostics.push(AppDiagnostic::new_error(
            DiagnosticCategory::Send,
            "Validate the report first and make sure that all errors are resolved".to_string(),
        ));
        app.show_error_panel = true;
        return;
    }

    let Some(xml) = read_report_xml(app) else {
        return;
    };
    let Some(eric) = &app.eric else {
        return;
    };
    let Some(taxonomy_version) = extract_taxonomy_version(&app.taxonomy) else {
        app.diagnostics.push(AppDiagnostic::taxonomy_version_error(
            DiagnosticCategory::Send,
        ));
        app.show_error_panel = true;
        return;
    };

    match eric.send(xml, "Bilanz", taxonomy_version, None) {
        Ok(response) => {
            if response.error_code == ErrorCode::ERIC_OK as i32 {
                app.report_status = ReportStatus::Sent;
                if let Some(path) = app.report_path.as_ref() {
                    app.report_list.set_report_status(
                        path,
                        ReportStatus::Sent,
                        &mut app.diagnostics,
                    );
                }
                app.diagnostics.push(AppDiagnostic::new_success(
                    DiagnosticCategory::Send,
                    format!("Send completed successfully\n{}", response.server_response),
                ));
            } else {
                // TODO: parse `server_response` for better error messages
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::Send,
                    format!(
                        "Send failed with error code {}\n{}",
                        response.error_code, response.server_response
                    ),
                ))
            }
        }
        Err(err) => app.diagnostics.push(AppDiagnostic::new_error(
            DiagnosticCategory::Send,
            format!("Send error: {err}"),
        )),
    }

    app.show_error_panel = true;
}

/// Clears all diagnostics of the specified category from the app state. This is
/// used to clear old validation errors when re-validating, or to clear old send
/// errors when re-sending the report, while keeping other diagnostics (like app
/// errors).
fn clear_diagnostics_by_category(app: &mut TaxelApp, category: DiagnosticCategory) {
    app.diagnostics
        .retain(|diagnostic| diagnostic.category != category);
}

/// Draws a summary of errors and warnings in the header. Clicking on the
/// summary toggles the visibility of the detailed diagnostics panel.
fn draw_error_summary(app: &mut TaxelApp, ui: &mut Ui) {
    let error_count = app
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
        .count();
    let warning_count = app
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Warning)
        .count();
    let success_count = app
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Success)
        .count();

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
    job.append(
        &format!("  success: {success_count}"),
        0.0,
        TextFormat {
            color: SUCCESS_COLOR,
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
