use crate::{
    app::{
        diagnostics::{AppDiagnostic, DiagnosticCategory},
        SectionState, TaxelApp, APP_NAME, APP_VERSION, VENDOR_ID,
    },
    domain::{
        create_instance_document, extract_period, update_instance_document, FactValue, Report,
        ReportStatus, UpdateOutcome,
    },
    infrastructure::report_store::reports_dir,
};
use anyhow::Context;
use chrono::{Datelike, Utc};
use eframe::egui::{self};
use eric_sdk::ErrorCode;
use log::debug;
use std::{
    collections::HashSet,
    env,
    fs::{self},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::SystemTime,
};
use taxel::{
    elster::Submitter, ElsterReport, TaxonomyType, COMPANY_TAX_NUMBER, COMPANY_TAX_NUMBER_PARENT,
    GCD_ROLE_URI, REQUIRED_GCD_FACTS, TAXONOMY_DATE_TO_VERSION, TAXONOMY_VERSION_TO_DATE,
    TEST_MARKER,
};
use uuid::Uuid;
use xbrl_rs::{InstanceDocument, TaxonomyLoader, TaxonomySet};

/// Discriminates how a report reaches the background load thread.
#[derive(Clone)]
pub enum LoadKind {
    /// Re-opening an already-registered report from the manifest. Produces
    /// `LoadOutcome::Ready`.
    Open(PathBuf),
    /// A freshly copied-in import. Produces `LoadOutcome::Created` so it gets
    /// registered.
    Import(PathBuf),
    /// A freshly created blank report. Produces `LoadOutcome::Created` so it gets
    /// registered.
    Create(NewReportForm),
}

/// The result of a background load operation.
#[allow(clippy::large_enum_variant)]
pub enum LoadOutcome {
    /// The report was loaded successfully. Already registered in the manifest.
    Ready(TaxonomySet, InstanceDocument, Report, ElsterReport),
    /// Like `Ready`, but the report was freshly imported.
    Imported(TaxonomySet, InstanceDocument, Report, ElsterReport),
    /// Like `Ready`, but the report was freshly created.
    Created(TaxonomySet, InstanceDocument, Report, ElsterReport),
    /// Taxonomies are missing from disk; the user must confirm before downloading.
    NeedsDownload,
}

/// Form data for creating a new blank report.
#[derive(Debug, Clone)]
pub struct NewReportForm {
    /// Reporting period start date in `YYYY-MM-DD` format.
    pub start_date: String,
    /// Reporting period end date (balance sheet date) in `YYYY-MM-DD` format.
    pub end_date: String,
    /// eBilanz taxonomy version, e.g. `"6.5"`.
    pub taxonomy_version: String,
    /// Which taxonomy module to use.
    pub taxonomy_type: TaxonomyType,
    /// Short codes of report elements selected by the user, e.g. `"B"`, `"GuV"`.
    pub selected_elements: HashSet<String>,
}

impl Default for NewReportForm {
    fn default() -> Self {
        let year = Utc::now().year();

        let taxonomy_version = TAXONOMY_VERSION_TO_DATE
            .keys()
            .max_by_key(|version| *version)
            .unwrap_or(&"6.9")
            .to_owned()
            .to_owned();

        Self {
            start_date: format!("{year}-01-01"),
            end_date: format!("{year}-12-31"),
            taxonomy_version,
            taxonomy_type: TaxonomyType::CoreFiscal,
            selected_elements: HashSet::from(["B".to_string(), "GuV".to_string()]),
        }
    }
}

/// Starts a background load based on `kind`, updating app state and spawning
/// the worker thread. `pending_load_kind` is retained so the load can be
/// re-triggered after the user confirms a taxonomy download.
pub fn start_load(app: &mut TaxelApp, kind: LoadKind, ctx: egui::Context, allow_download: bool) {
    app.selected_tab = 0;
    app.diagnostics.clear();
    app.editing_section = None;
    app.edit_snapshot.clear();
    app.pending_load_kind = Some(kind.clone());

    let (tx, rx) = mpsc::channel();
    app.loading = Some(rx);

    thread::spawn(move || {
        let _ = tx.send(load_report(kind, allow_download));
        ctx.request_repaint();
    });
}

/// Dispatches the background work based on `LoadKind`. For imports,
/// `LoadOutcome::Imported` is returned so the caller can distinguish it from a
/// regular open and register the report in the manifest.
fn load_report(kind: LoadKind, allow_download: bool) -> Result<LoadOutcome, anyhow::Error> {
    match kind {
        LoadKind::Open(path) => load_instance_document_and_taxonomy(&path, allow_download),
        LoadKind::Import(path) => {
            match load_instance_document_and_taxonomy(&path, allow_download)? {
                LoadOutcome::Ready(taxonomy, instance, report, elster_report) => Ok(
                    LoadOutcome::Imported(taxonomy, instance, report, elster_report),
                ),
                other => Ok(other),
            }
        }
        LoadKind::Create(form) => load_taxonomy_and_create_instance_document(form, allow_download),
    }
}

/// Polls the background XML load result and updates the app state accordingly.
pub fn poll_load_result(app: &mut TaxelApp) {
    if let Some(rx) = &app.loading {
        match rx.try_recv() {
            Ok(Ok(LoadOutcome::Ready(taxonomy, instance, report, elster_report))) => {
                finish_load(app, taxonomy, instance, report, elster_report);
            }
            Ok(Ok(LoadOutcome::Created(taxonomy, instance, report, elster_report)))
            | Ok(Ok(LoadOutcome::Imported(taxonomy, instance, report, elster_report))) => {
                let (start_date, end_date) = extract_period(&instance)
                    .map(|(start, end)| (Some(start), Some(end)))
                    .unwrap_or((None, None));
                let taxonomy_version =
                    extract_taxonomy_version_from_schema_refs(instance.schema_refs())
                        .map(|v| v.to_owned());

                app.upsert_report(
                    &report.path,
                    Some(report.taxonomy_type.clone()),
                    taxonomy_version,
                    start_date,
                    end_date,
                );
                app.refresh_reports();

                finish_load(app, taxonomy, instance, report, elster_report);
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
                app.pending_load_kind = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                app.loading = None;
            }
        }
    }
}

/// Imports a report from an arbitrary file path by copying it to the app's
/// managed reports directory and then loading it from there. This ensures that
/// the app has full control over the report file and can reliably persist
/// changes.
pub fn import_report(app: &mut TaxelApp, path: PathBuf, ctx: &egui::Context) {
    let vendor_id = env::var("VENDOR_ID").unwrap_or_else(|_| VENDOR_ID.to_string());

    match parse_and_copy_report(&path, vendor_id) {
        Ok(copied_path) => {
            start_load(app, LoadKind::Import(copied_path), ctx.clone(), false);
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

/// Parses and copies the report.
///
/// The source XML is parsed as an [`taxel::elster::ElsterReport`], modified,
/// and serialized back to XML. The original file is never modified.
fn parse_and_copy_report(path: &Path, vendor_id: String) -> Result<PathBuf, anyhow::Error> {
    let reports_dir = reports_dir()?;

    let xml = fs::read_to_string(path)
        .with_context(|| format!("Failed to read report from {}", path.display()))?;

    let mut report = ElsterReport::parse(&xml)
        .with_context(|| format!("Failed to parse ElsterReport from {}", path.display()))?;

    report.transfer_header.manufacturer_id = vendor_id;

    if let Some(payload_block) = report.data_section.payload_blocks.get_mut(0) {
        if let Some(manufacturer) = payload_block.payload_header.manufacturer.as_mut() {
            manufacturer.product_name = APP_NAME.to_owned();
            manufacturer.product_version = APP_VERSION.to_owned();
        }
    }

    let serialized = report
        .to_xml()
        .context("Failed to serialize ElsterReport")?;

    let uuid = Uuid::new_v4();
    let dest = reports_dir.join(format!("ebilanz_{uuid}.xml"));

    fs::write(&dest, serialized)
        .with_context(|| format!("Failed to write report to {}", dest.display()))?;

    Ok(dest)
}

/// Loads an XBRL instance document from the specified path, discovers the
/// referenced taxonomies, and populates the fact table with the extracted
/// facts.
fn load_instance_document_and_taxonomy(
    path: &Path,
    allow_download: bool,
) -> Result<LoadOutcome, anyhow::Error> {
    debug!("Read xml file: {}", path.display());

    let instance = InstanceDocument::from_file(path)?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let schema_ref_paths = instance.schema_ref_paths();

    let Some(taxonomy) = load_taxonomies(schema_refs, &schema_ref_paths, allow_download)? else {
        return Ok(LoadOutcome::NeedsDownload);
    };

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let taxonomy_type = TaxonomyType::from_schema_refs(instance.schema_refs()).unwrap_or_default();
    let mut report = Report::new(path.to_path_buf(), taxonomy_type);

    report.populate(view, &item_facts, &taxonomy);

    let xml = fs::read_to_string(path)
        .with_context(|| format!("Failed to read Elster XML from {}", path.display()))?;
    let elster_report = ElsterReport::parse(&xml)
        .with_context(|| format!("Failed to parse ElsterReport from {}", path.display()))?;

    Ok(LoadOutcome::Ready(
        taxonomy,
        instance,
        report,
        elster_report,
    ))
}

/// Creates a new XBRL instance document based on the provided form data,
/// populates the fact table, and writes the report to disk. If `allow_download`
/// is false and the required taxonomies are missing,
/// `LoadOutcome::NeedsDownload` is returned so the UI can ask for confirmation
/// before downloading.
fn load_taxonomy_and_create_instance_document(
    form: NewReportForm,
    allow_download: bool,
) -> Result<LoadOutcome, anyhow::Error> {
    let vendor_id = env::var("VENDOR_ID").unwrap_or_else(|_| VENDOR_ID.to_string());
    let taxonomy_date = TAXONOMY_VERSION_TO_DATE
        .get(form.taxonomy_version.as_str())
        .with_context(|| {
            format!(
                "No taxonomy date known for version {}. Supported: 6.4–6.9.",
                form.taxonomy_version
            )
        })?;

    let schema_refs = form.taxonomy_type.schema_refs(taxonomy_date);
    let namespace_prefix = form.taxonomy_type.namespace_prefix();
    let namespace_uri = form.taxonomy_type.namespace_uri(taxonomy_date);
    let schema_ref_paths: Vec<String> = schema_refs
        .iter()
        .filter_map(|url| url.split("/taxonomies/").nth(1).map(str::to_string))
        .collect();

    let Some(taxonomy) = load_taxonomies(
        schema_refs,
        &schema_ref_paths
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<&str>>(),
        allow_download,
    )?
    else {
        return Ok(LoadOutcome::NeedsDownload);
    };

    let instance = create_instance_document(
        &form.start_date,
        &form.end_date,
        namespace_prefix,
        &namespace_uri,
        taxonomy_date,
        &taxonomy,
    )?;

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let dest = new_report_path()?;
    let mut report = Report::new(dest.clone(), form.taxonomy_type.clone());

    report.populate(view, &item_facts, &taxonomy);

    // Serialize and wrap in an Elster envelope, then write to disk.
    let mut xbrl_bytes: Vec<u8> = Vec::new();
    instance.to_writer(&mut xbrl_bytes)?;

    let balance_date: u32 = form.end_date.replace('-', "").parse()?;

    // TODO: overwrite manufacturer id, recipient id, recipient value,
    // ebilanz_version, and test_marker.
    let mut elster = ElsterReport::new(
        vendor_id,
        Submitter::default(),
        "",
        "",
        balance_date,
        Some(TEST_MARKER),
    );
    elster.set_payload_xbrl(xbrl_bytes);
    let xml = elster.to_xml()?;

    fs::write(&dest, &xml)
        .with_context(|| format!("Failed to write new report to {}", dest.display()))?;

    Ok(LoadOutcome::Created(taxonomy, instance, report, elster))
}

/// Loads the taxonomies required for the given schema refs. If they are missing
/// and `allow_download` is false, returns Ok(None) so the UI can ask for
/// confirmation before downloading.
fn load_taxonomies(
    schema_refs: Vec<String>,
    schema_ref_paths: &[&str],
    allow_download: bool,
) -> Result<Option<TaxonomySet>, anyhow::Error> {
    let taxonomy_dir = taxonomy_dir()?;
    let loader = TaxonomyLoader::new()?;

    let taxonomies_missing = schema_ref_paths
        .iter()
        .any(|path| !taxonomy_dir.join(path).exists());

    if taxonomies_missing {
        if !allow_download {
            return Ok(None);
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

    Ok(Some(taxonomy))
}

/// Applies a successfully loaded or created report to the app state.
fn finish_load(
    app: &mut TaxelApp,
    taxonomy: TaxonomySet,
    instance: InstanceDocument,
    mut report: Report,
    elster_report: ElsterReport,
) {
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
        .find(|overview| overview.path == report.path)
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
    app.elster_report = Some(elster_report);
    app.loading = None;
    app.pending_load_kind = None;
}

fn new_report_path() -> Result<PathBuf, anyhow::Error> {
    let reports_dir = dirs::data_dir()
        .map(|dir| dir.join("taxel").join("reports"))
        .context("Could not determine data directory")?;

    if !reports_dir.exists() {
        fs::create_dir_all(&reports_dir).with_context(|| {
            format!(
                "Failed to create reports directory: {}",
                reports_dir.display()
            )
        })?;
    }

    Ok(reports_dir.join(format!("ebilanz_{}.xml", Uuid::new_v4())))
}

/// Edits the currently loaded report by enabling edit mode for the selected
/// section and taking a snapshot of the current values for that section.
pub fn edit_report(app: &mut TaxelApp) {
    if let Some(table) = &app.report {
        if let Some(section) = table.sections.get(app.selected_tab) {
            app.edit_snapshot = section.rows.iter().map(|r| r.value.clone()).collect();
        }
    }
    app.editing_section = Some(app.selected_tab);

    // If the report was previously validated, mark it
    // as draft again since it has unsaved changes now.
    if let Some(report) = &app.report {
        app.report_list
            .set_report_status(&report.path, ReportStatus::Draft);
        app.report_list.save(&mut app.diagnostics);
    }
}

/// Cancels the edit by restoring the values from the snapshot and exiting edit
/// mode.
pub fn cancel_edit(app: &mut TaxelApp) {
    let editing_tab = app.editing_section.unwrap_or(app.selected_tab);

    if let Some(table) = &mut app.report {
        if let Some(section) = table.sections.get_mut(editing_tab) {
            for (row, value) in section.rows.iter_mut().zip(app.edit_snapshot.iter()) {
                row.value = value.clone();
            }
        }
    }
    app.editing_section = None;
    app.edit_snapshot.clear();
}

/// Saves the currently loaded report by writing the modified XBRL instance
/// document back to the original file path.
pub fn save_report(app: &mut TaxelApp) {
    let editing_tab = app.editing_section.unwrap_or(app.selected_tab);

    let mut update_outcome = UpdateOutcome::NoChange;

    if let (Some(report), Some(instance)) = (&app.report, &mut app.instance_document) {
        if let Some(section) = report.sections.get(editing_tab) {
            // Only write back rows whose value differs from the pre-edit
            // snapshot. The same fact can appear in multiple rows (same concept
            // at multiple positions in the presentation tree) but we only need
            // to update it once.
            for (i, row) in section.rows.iter().enumerate() {
                let unchanged = app
                    .edit_snapshot
                    .get(i)
                    .is_some_and(|snapshot| snapshot == &row.value);

                if unchanged {
                    continue;
                }

                let snapshot = app.edit_snapshot.get(i).map(|s| s as &FactValue);

                match update_instance_document(
                    instance,
                    &row.value,
                    snapshot,
                    row.fact_index,
                    &row.concept,
                    app.taxonomy.as_ref(),
                ) {
                    Ok(outcome) => update_outcome = outcome,
                    Err(err) => {
                        app.diagnostics.push(AppDiagnostic::new_error(
                            DiagnosticCategory::App,
                            format!("{err}"),
                        ));
                    }
                }
            }
        }

        // Sync GCD facts to ElsterReport metadata and XBRL contexts.
        if let Some(elster) = &mut app.elster_report {
            report.sync_gcd_to_elster(instance, elster);
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
                let result = app
                    .elster_report
                    .as_mut()
                    .context("No Elster report in app state")
                    .and_then(|elster| {
                        elster.set_payload_xbrl(xbrl_bytes);
                        elster.to_xml().context("Failed to serialize Elster report")
                    })
                    .and_then(|xml| {
                        fs::write(&report.path, xml).context("Failed to write report to disk")
                    });

                match result {
                    Ok(()) => {
                        let now = SystemTime::now();

                        if let Some((start_date, end_date)) = extract_period(instance) {
                            app.report_list.set_period(
                                &report.path,
                                Some(start_date),
                                Some(end_date),
                            );
                        }

                        app.report_list.set_timestamp(&report.path, now);
                        app.report_list.save(&mut app.diagnostics);
                    }
                    Err(err) => {
                        app.diagnostics.push(AppDiagnostic::new_error(
                            DiagnosticCategory::App,
                            format!("Failed to save report: {err}"),
                        ));
                    }
                }
            }
        }
    }

    // Rebuild the fact table after a tuple child switch so the new element
    // names and row structure are reflected in the UI.
    if update_outcome == UpdateOutcome::Rebuild {
        if let (Some(taxonomy), Some(instance), Some(report)) =
            (&app.taxonomy, &app.instance_document, &mut app.report)
        {
            let view = instance.view(taxonomy);
            let item_facts = instance.item_facts();
            report.populate(view, &item_facts, taxonomy);
        }
    }

    app.editing_section = None;
    app.edit_snapshot.clear();
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
        .set_report_status(&report.path, ReportStatus::Draft);
    app.report_list.save(&mut app.diagnostics);

    serialize_and_validate_report(app)
        .map_err(|err| {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::Validation,
                err.to_string(),
            ));
        })
        .ok();
}

/// Serializes the current XBRL instance document, wraps it in an Elster
/// envelope, and sends it to ERiC for validation.
fn serialize_and_validate_report(app: &mut TaxelApp) -> Result<(), anyhow::Error> {
    let instance = app
        .instance_document
        .as_ref()
        .context("Failed to get instance document from app state")?;
    let mut xbrl_bytes = Vec::new();

    instance
        .to_writer(&mut xbrl_bytes)
        .context("Failed to serialize XBRL instance")?;

    let Some(taxonomy_version) = extract_taxonomy_version(&app.taxonomy) else {
        app.diagnostics.push(AppDiagnostic::taxonomy_version_error(
            DiagnosticCategory::Validation,
        ));
        app.show_error_panel = true;
        return Ok(());
    };

    let elster = app
        .elster_report
        .as_mut()
        .context("No Elster report in app state")?;

    elster.set_payload_xbrl(xbrl_bytes);

    let xml = elster
        .to_xml()
        .context("Failed to serialize Elster report")?;

    if let Some(report) = &app.report {
        for &concept in REQUIRED_GCD_FACTS {
            // The "companyId.ST13" concept can occur multiple times in the
            // report under different parent concepts. We only want to check for
            // its existence once.
            let parent = if concept == COMPANY_TAX_NUMBER {
                Some(COMPANY_TAX_NUMBER_PARENT)
            } else {
                None
            };

            if report
                .find_in_section(GCD_ROLE_URI, concept, parent)
                .is_none()
            {
                app.diagnostics.push(AppDiagnostic::new_missing_fact_value(
                    DiagnosticCategory::Validation,
                    concept,
                ));
            }
        }
    }

    if app.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Validation && diagnostic.fact.is_some()
    }) {
        return Ok(());
    }

    let eric = app.eric.as_ref().context("Failed to get Eric")?;

    match eric.validate(xml, "Bilanz", taxonomy_version, None) {
        Ok(response) => {
            if response.error_code == ErrorCode::ERIC_OK as i32 {
                if let Some(report) = &mut app.report {
                    report.status = ReportStatus::Validated;
                    app.report_list
                        .set_report_status(&report.path, ReportStatus::Validated);
                    app.report_list.save(&mut app.diagnostics);
                }

                app.diagnostics.push(AppDiagnostic::new_success(
                    DiagnosticCategory::Validation,
                    format!(
                        "Validation completed successfully\n{}",
                        response.validation_response
                    ),
                ));
            } else {
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

    Ok(())
}

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
                    app.report_list
                        .set_report_status(&report.path, ReportStatus::Sent);
                    app.report_list.save(&mut app.diagnostics);
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
            app.elster_report = None;
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
/// contain a date like "2024-06-30". We extract the date and map it to
/// the corresponding version string expected by ERiC.
fn extract_taxonomy_version(taxonomy: &Option<TaxonomySet>) -> Option<&&str> {
    taxonomy
        .as_ref()
        .and_then(|taxonomy| taxonomy.date())
        .and_then(|date| TAXONOMY_DATE_TO_VERSION.get(date))
}

/// Determines the taxonomy version string (e.g. `"6.8"`) from a set of
/// schema ref URLs by reverse-looking up the date embedded in the URL.
pub fn extract_taxonomy_version_from_schema_refs(schema_refs: &[String]) -> Option<&'static str> {
    schema_refs.iter().find_map(|schema_ref| {
        TAXONOMY_VERSION_TO_DATE
            .iter()
            .find(|(_, date)| schema_ref.contains(*date))
            .map(|(version, _)| *version)
    })
}

/// Returns the path to the application's taxonomy directory, which is located
/// in the user's data directory.
fn taxonomy_dir() -> Result<PathBuf, anyhow::Error> {
    dirs::data_dir()
        .map(|dir| dir.join("taxel").join("taxonomies"))
        .context("Could not determine data directory")
}
