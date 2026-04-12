use crate::{
    app::{
        diagnostics::{AppDiagnostic, DiagnosticCategory},
        TaxelApp,
    },
    domain::ReportStatus,
    load_xml,
};
use eframe::egui::{self};
use eric_sdk::ErrorCode;
use std::{
    fs::{self},
    path::PathBuf,
    sync::mpsc,
    thread,
};
use taxel::TAXONOMY_YEAR_TO_VERSION;
use xbrl_rs::TaxonomySet;

/// Loads an XBRL instance document from the specified path and updates the app
/// state. The load runs on a background thread to keep the UI responsive.
pub fn load_report(app: &mut TaxelApp, path: PathBuf, ctx: egui::Context) {
    app.selected_tab = 0;
    app.report = None;
    app.report_path = Some(path.clone());
    app.diagnostics.clear();
    app.editing_section = None;
    app.edit_snapshot.clear();
    app.report_status = ReportStatus::Draft;

    let (tx, rx) = mpsc::channel();
    app.loading = Some(rx);

    thread::spawn(move || {
        let table = load_xml(&path);
        let _ = tx.send(table);
        ctx.request_repaint();
    });
}

/// Validates the imported report and reports any issues in the diagnostics
/// panel.
pub fn validate_report(app: &mut TaxelApp) {
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
pub fn send_report(app: &mut TaxelApp) {
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
