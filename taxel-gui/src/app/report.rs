use crate::{
    app::{
        diagnostics::{AppDiagnostic, DiagnosticCategory},
        SectionState, TaxelApp,
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
use taxel::{ElsterReport, TAXONOMY_YEAR_TO_VERSION};
use xbrl_rs::{InstanceDocument, TaxonomyLoader, TaxonomySet};

/// The result of a background load operation.
#[allow(clippy::large_enum_variant)]
pub enum LoadOutcome {
    Ready(TaxonomySet, InstanceDocument, Report),
    /// Taxonomies are missing from disk; the user must confirm before downloading.
    NeedsDownload,
}

/// Loads an XBRL instance document from the specified path and updates the app
/// state. The load runs on a background thread to keep the UI responsive.
/// If `allow_download` is false and the required taxonomies are missing,
/// `app.show_download_modal` is set so the UI can ask for confirmation.
pub fn load_report(app: &mut TaxelApp, path: PathBuf, ctx: egui::Context, allow_download: bool) {
    app.selected_tab = 0;
    app.report = None;
    app.diagnostics.clear();
    app.editing_section = None;
    app.edit_snapshot.clear();
    app.pending_download_path = Some(path.clone());

    spawn_load_thread(app, path, ctx, allow_download);
}

fn spawn_load_thread(app: &mut TaxelApp, path: PathBuf, ctx: egui::Context, allow_download: bool) {
    let (tx, rx) = mpsc::channel();
    app.loading = Some(rx);

    thread::spawn(move || {
        let result = load_instance_document(&path, allow_download);
        let _ = tx.send(result);
        ctx.request_repaint();
    });
}

/// Loads an XBRL instance document from the specified path, discovers the
/// referenced taxonomies, and populates the fact table with the extracted
/// facts.
fn load_instance_document(path: &Path, allow_download: bool) -> Result<LoadOutcome, anyhow::Error> {
    debug!("Read xml file: {}", path.display());

    let instance = InstanceDocument::from_file(path)?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let schema_ref_paths = instance.schema_ref_paths();
    let taxonomy_dir = taxonomy_dir()?;
    let loader = TaxonomyLoader::new()?;

    let taxonomies_missing = schema_ref_paths
        .iter()
        .any(|path| !taxonomy_dir.join(path).exists());

    if taxonomies_missing {
        if !allow_download {
            return Ok(LoadOutcome::NeedsDownload);
        }

        if !taxonomy_dir.exists() {
            fs::create_dir_all(&taxonomy_dir).with_context(|| {
                format!(
                    "Failed to create taxonomy directory: {}",
                    taxonomy_dir.display()
                )
            })?;
        }

        loader.download_all(&schema_refs, &taxonomy_dir)?;
    }

    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_dir)?;
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let mut report = Report::new(path.to_path_buf());

    report.populate(view, &item_facts, &taxonomy);

    Ok(LoadOutcome::Ready(taxonomy, instance, report))
}

/// Polls the background XML load result and updates the app state accordingly.
pub fn poll_load_result(app: &mut TaxelApp) {
    if let Some(rx) = &app.loading {
        match rx.try_recv() {
            Ok(Ok(LoadOutcome::Ready(taxonomy, instance, mut report))) => {
                for missing_role in &report.role_mapping_errors {
                    app.diagnostics.push(AppDiagnostic::new_warning(
                        DiagnosticCategory::Import,
                        format!("Missing report-element mapping for role URI: {missing_role}"),
                    ));
                }

                if let Some(overview) = app
                    .report_list
                    .reports()
                    .iter()
                    .find(|report_overview| report_overview.path == report.path)
                {
                    report.status = overview.report_status;
                }

                app.section_states = report
                    .sections
                    .iter()
                    .map(|_| SectionState::default())
                    .collect();
                app.report = Some(report);
                app.taxonomy = Some(taxonomy);
                app.instance_document = Some(instance);
                app.loading = None;
            }
            Ok(Ok(LoadOutcome::NeedsDownload)) => {
                app.show_download_modal = true;
                app.loading = None;
            }
            Ok(Err(err)) => {
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::Import,
                    err.to_string(),
                ));
                app.show_error_panel = true;
                app.loading = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                app.loading = None;
            }
        }
    }
}

/// Edits the currently loaded report by enabling edit mode for the selected
/// section and taking a snapshot of the current values for that section.
pub fn edit_report(app: &mut TaxelApp) {
    if let Some(table) = &app.report {
        if let Some(section) = table.sections.get(app.selected_tab) {
            app.edit_snapshot = section.rows.iter().map(|r| r.value.clone()).collect();
            app.edit_nil_snapshot = section.rows.iter().map(|row| row.is_nil).collect();
        }
    }
    app.editing_section = Some(app.selected_tab);

    // If the report was previously validated, mark it
    // as draft again since it has unsaved changes now.
    if let Some(report) = &app.report {
        app.report_list
            .set_report_status(&report.path, ReportStatus::Draft, &mut app.diagnostics);
    }
}

/// Cancels the edit by restoring the values from the snapshot and exiting edit
/// mode.
pub fn cancel_edit(app: &mut TaxelApp) {
    let editing_tab = app.editing_section.unwrap_or(app.selected_tab);

    if let Some(table) = &mut app.report {
        if let Some(section) = table.sections.get_mut(editing_tab) {
            for (row, (value, is_nil)) in section
                .rows
                .iter_mut()
                .zip(app.edit_snapshot.iter().zip(app.edit_nil_snapshot.iter()))
            {
                row.value = value.clone();
                row.is_nil = *is_nil;
            }
        }
    }
    app.editing_section = None;
    app.edit_snapshot.clear();
    app.edit_nil_snapshot.clear();
}

/// Saves the currently loaded report by writing the modified XBRL instance
/// document back to the original file path.
pub fn save_report(app: &mut TaxelApp) {
    let editing_tab = app.editing_section.unwrap_or(app.selected_tab);

    if let (Some(report), Some(instance)) = (&app.report, &mut app.instance_document) {
        if let Some(section) = report.sections.get(editing_tab) {
            // Only write back rows whose value or nil state differs from the
            // pre-edit snapshot. The same fact can appear in multiple rows but
            // we only need to update it once.
            for (i, row) in section.rows.iter().enumerate() {
                let value_unchanged = app
                    .edit_snapshot
                    .get(i)
                    .is_some_and(|snapshot| snapshot == &row.value);
                let nil_unchanged = app
                    .edit_nil_snapshot
                    .get(i)
                    .is_some_and(|snap| *snap == row.is_nil);

                if let Some(idx) = row.fact_index {
                    if !nil_unchanged {
                        instance.set_fact_nil(idx, row.is_nil);
                    } else if !value_unchanged {
                        instance.set_fact_value(idx, row.value.clone());
                    }
                }
            }
        }

        // Serialize the updated InstanceDocument to bytes.
        let mut xbrl_bytes: Vec<u8> = Vec::new();
        let serialize_result = instance.to_writer(&mut xbrl_bytes);

        match serialize_result {
            Err(err) => {
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::App,
                    format!("Failed to serialize XBRL instance: {err}"),
                ));
            }
            Ok(()) => {
                // Re-read the stored Elster XML, inject the new XBRL bytes, and
                // write the full envelope back so the Elster wrapper is preserved.
                let result = fs::read_to_string(&report.path)
                    .context("Failed to read report file")
                    .and_then(|xml| {
                        ElsterReport::parse(&xml).context("Failed to parse Elster report")
                    })
                    .and_then(|mut elster| {
                        elster.set_payload_xbrl(xbrl_bytes);
                        elster.to_xml().context("Failed to serialize Elster report")
                    })
                    .and_then(|xml| {
                        fs::write(&report.path, xml).context("Failed to write report to disk")
                    });

                if let Err(err) = result {
                    app.diagnostics.push(AppDiagnostic::new_error(
                        DiagnosticCategory::App,
                        format!("Failed to save report: {err}"),
                    ));
                }
            }
        }
    }

    app.editing_section = None;
    app.edit_snapshot.clear();
    app.edit_nil_snapshot.clear();
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

    let Some(certificate_path) = &app.send_certificate_path else {
        return;
    };
    let password = &app.send_password;

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

    match eric.send(
        xml,
        "Bilanz",
        taxonomy_version,
        certificate_path,
        password,
        None,
    ) {
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

/// Moves the currently loaded report to the operating system trash and resets
/// the app state to the home screen.
pub fn delete_report(app: &mut TaxelApp) {
    let Some(report) = &app.report else {
        return;
    };
    let path = report.path.clone();

    match trash::delete(&path) {
        Ok(()) => {
            app.report_list.remove_report(&path, &mut app.diagnostics);
            app.report = None;
            app.taxonomy = None;
            app.instance_document = None;
            app.selected_tab = 0;
            app.search.results.clear();
            app.search.scroll_to_row = None;
            app.search.row_highlight = None;
            app.loading = None;
            app.diagnostics.clear();
            app.show_error_panel = true;
            app.editing_section = None;
            app.edit_snapshot.clear();
        }
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to move report to trash: {err}"),
            ));
            app.show_error_panel = true;
        }
    }
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

/// Returns the path to the application's taxonomy directory, which is located
/// in the user's data directory.
fn taxonomy_dir() -> Result<PathBuf, anyhow::Error> {
    dirs::data_dir()
        .map(|dir| dir.join("taxel").join("taxonomies"))
        .context("Could not determine data directory")
}
