use crate::{
    app::{
        diagnostics::{AppDiagnostic, DiagnosticCategory},
        TaxelApp,
    },
    domain::{Report, ReportStatus},
};
use anyhow::Context;
use eframe::egui::{self};
use eric_sdk::ErrorCode;
use log::debug;
use std::{
    fs::{self},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};
use taxel::TAXONOMY_YEAR_TO_VERSION;
use xbrl_rs::{InstanceDocument, TaxonomySet};

/// Loads an XBRL instance document from the specified path and updates the app
/// state. The load runs on a background thread to keep the UI responsive.
pub fn load_report(app: &mut TaxelApp, path: PathBuf, ctx: egui::Context) {
    app.selected_tab = 0;
    app.report = None;
    app.diagnostics.clear();
    app.editing_section = None;
    app.edit_snapshot.clear();

    let (tx, rx) = mpsc::channel();
    app.loading = Some(rx);

    thread::spawn(move || {
        let report = load_instance_document(&path);
        let _ = tx.send(report);
        ctx.request_repaint();
    });
}

/// Loads an XBRL instance document from the specified path, discovers the
/// referenced taxonomies, and populates the fact table with the extracted
/// facts.
fn load_instance_document(
    path: &Path,
) -> Result<(TaxonomySet, InstanceDocument, Report), anyhow::Error> {
    debug!("Read xml file: {}", path.display());

    let instance = InstanceDocument::from_file(path)?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("missing path to taxonomies")?
        .join("test_data/taxonomies");
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let mut report = Report::new(path.to_path_buf());

    report.populate(view, &item_facts);

    Ok((taxonomy, instance, report))
}

/// Validates the imported report and reports any issues in the diagnostics
/// panel.
pub fn validate_report(app: &mut TaxelApp) {
    clear_diagnostics_by_category(app, DiagnosticCategory::Validation);

    let Some(report) = &mut app.report else {
        return;
    };

    report.status = ReportStatus::Draft;
    app.report_list
        .set_report_status(&report.path, ReportStatus::Draft, &mut app.diagnostics);

    let Some(xml) = read_report(
        &report.path,
        &mut app.diagnostics,
        &mut app.show_error_panel,
    ) else {
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
                if let Some(report) = &mut app.report {
                    report.status = ReportStatus::Validated;
                    app.report_list.set_report_status(
                        &report.path,
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

    let Some(report) = &mut app.report else {
        return;
    };

    if report.status != ReportStatus::Validated {
        app.diagnostics.push(AppDiagnostic::new_error(
            DiagnosticCategory::Send,
            "Validate the report first and make sure that all errors are resolved".to_string(),
        ));
        app.show_error_panel = true;
        return;
    }

    let Some(xml) = read_report(
        &report.path,
        &mut app.diagnostics,
        &mut app.show_error_panel,
    ) else {
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
                report.status = ReportStatus::Sent;

                if let Some(report) = &app.report {
                    app.report_list.set_report_status(
                        &report.path,
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
fn read_report(
    report_path: &Path,
    diagnostics: &mut Vec<AppDiagnostic>,
    show_error_panel: &mut bool,
) -> Option<String> {
    match fs::read_to_string(report_path) {
        Ok(xml) => Some(xml),
        Err(err) => {
            diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to read report file: {err}"),
            ));
            *show_error_panel = true;
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
