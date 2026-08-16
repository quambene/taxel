use crate::{
    app::{
        diagnostics::{AppDiagnostic, DiagnosticCategory},
        SectionState, TaxelApp, APP_NAME, APP_VERSION, VENDOR_ID,
    },
    domain::{
        update_instance_document, write_calculated_values_to_instance, FactValue, Report,
        ReportStatus, UpdateOutcome,
    },
    infrastructure::report_store::{confirmations_dir, reports_dir},
};
use anyhow::Context;
use chrono::{Datelike, NaiveDate, Utc};
use eframe::egui::{self};
use log::{debug, info};
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
    active_roles, create_instance_document, elster::Submitter, extract_fact_name, extract_period,
    load_taxonomies, remove_forbidden_facts, remove_trade_accounting_facts,
    restore_required_nil_tuple_children, taxonomy_dir, taxonomy_version_from_schema_refs,
    ElsterReport, TaxonomyType, BASELINE_ROLE_URIS, CLOSING_DATE, COMPANY_TAX_NUMBER,
    COMPANY_TAX_NUMBER_PARENT, GCD_ROLE_URI, REPORT_ELEMENT_PREFIX, REQUIRED_GCD_FACTS,
    TAXONOMY_DATE_TO_VERSION, TAXONOMY_VERSION_TO_DATE,
};
use uuid::Uuid;
use xbrl_rs::{InstanceDocument, RoleUri, TaxonomySet};

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

/// All state associated with a currently loaded report.
pub struct LoadedReport {
    pub taxonomy: TaxonomySet,
    pub instance: InstanceDocument,
    pub elster: ElsterReport,
    pub report: Report,
    /// The report's lifecycle status. Lives here rather than on `Report`
    /// (shared with `taxel-cli`, which has no persisted report status to
    /// track) since `taxel-gui` is the only consumer that needs it.
    pub status: ReportStatus,
}

/// The result of a background load operation.
#[allow(clippy::large_enum_variant)]
pub enum LoadOutcome {
    /// The report was loaded successfully. Already registered in the manifest.
    Ready(LoadedReport),
    /// Like `Ready`, but the report was freshly imported.
    Imported(LoadedReport),
    /// Like `Ready`, but the report was freshly created.
    Created(LoadedReport),
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
                LoadOutcome::Ready(loaded) => Ok(LoadOutcome::Imported(loaded)),
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
            Ok(Ok(LoadOutcome::Ready(loaded))) => {
                finish_load(app, loaded);
            }
            Ok(Ok(LoadOutcome::Created(loaded))) | Ok(Ok(LoadOutcome::Imported(loaded))) => {
                let (start_date, end_date) = extract_period(&loaded.instance)
                    .map(|(start, end)| (Some(start), Some(end)))
                    .unwrap_or((None, None));
                let taxonomy_version =
                    taxonomy_version_from_schema_refs(loaded.instance.schema_refs())
                        .map(|v| v.to_owned());

                app.upsert_report(
                    &loaded.report.path,
                    Some(loaded.report.taxonomy_type.clone()),
                    taxonomy_version,
                    start_date,
                    end_date,
                );
                app.refresh_reports();

                finish_load(app, loaded);
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
                app.show_diagnostics_panel = true;
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
            app.show_diagnostics_panel = true;
        }
    }
}

/// Parses and copies the report.
///
/// The source report is parsed as an [`taxel::elster::ElsterReport`], modified,
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

/// Imports fact values from a source XML into the currently opened report.
///
/// The import updates both the in-memory `InstanceDocument` and the visible
/// fact rows. Changes are not written to disk here; the user must explicitly
/// click "Save" to persist them.
pub fn import_values(app: &mut TaxelApp) {
    match read_and_import_values(app) {
        Ok((matched_count, imported_count, source_path)) => {
            app.diagnostics.push(AppDiagnostic::new_success(
            DiagnosticCategory::Import,
            format!(
                "Imported values from {} (matched facts: {matched_count}, updated facts: {imported_count}). Changes are pending and will be written only after Save.",
                source_path.display()
            ),
        ));
            app.show_diagnostics_panel = true;
        }
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::Import,
                format!("Failed to import values: {err}"),
            ));
            app.show_diagnostics_panel = true;
        }
    }
}

/// Reads the source XML, extracts fact values, and applies them to the
/// currently loaded report and instance document.
fn read_and_import_values(app: &mut TaxelApp) -> Result<(usize, usize, PathBuf), anyhow::Error> {
    let Some(source_path) = app.import_values_path.clone() else {
        return Err(anyhow::anyhow!("No XML file selected for value import"));
    };

    // Ensure the UI stays in edit mode after applying imported values so the
    // user can confirm with Save.
    if app.editing_section.is_none() {
        edit_report(app);
    }

    let source_instance =
        InstanceDocument::from_file(&source_path).context("Failed to parse source XML")?;
    let source_schema_refs = source_instance.schema_refs().to_vec();
    let source_schema_ref_paths = source_instance.schema_ref_paths();
    let source_taxonomy =
        load_taxonomies(source_schema_refs, &source_schema_ref_paths, false, &taxonomy_dir()?)?
        .context("Source taxonomy not available on disk. Open the source report first so its taxonomy is downloaded.")?;

    let Some(loaded) = app.loaded.as_mut() else {
        return Err(anyhow::anyhow!(
            "Cannot import values without a loaded report"
        ));
    };

    let mut source_report = Report::new(
        source_path.to_path_buf(),
        loaded.report.taxonomy_type.clone(),
    );
    let source_view = source_instance.view(&source_taxonomy);
    let source_item_facts = source_instance.item_facts();
    source_report.populate(source_view, &source_item_facts, &source_taxonomy);

    // Report-element selections are never imported: only sections already
    // enabled in the target report receive values. Enabling a source's
    // section requires a structural rebuild, which is left to the user via
    // the existing checkbox toggle in the GCD tab.
    let (matched_count, imported_count) = loaded.report.apply_imported_values(
        &source_report,
        &source_item_facts,
        &mut loaded.instance,
        &loaded.taxonomy,
        false,
    );

    restore_required_nil_tuple_children(&mut loaded.instance, &loaded.taxonomy);

    Ok((matched_count, imported_count, source_path))
}

/// Exports the currently displayed fact values to a semicolon-delimited CSV
/// file.
///
/// Columns are: Section, ID (concept), Depth, Name (label in the active UI
/// language), Value, Unit, and Context. Only sections selected in the GCD
/// section (i.e. not disabled) are exported. Within those sections, abstract
/// parent rows are always included (they carry no value but provide the
/// section/tree structure); other rows are only included if they have a
/// non-empty value. This only reads already-in-memory report data, so it
/// runs synchronously on the UI thread.
pub fn export_values(app: &mut TaxelApp, dest: PathBuf) {
    match write_values_csv(app, &dest) {
        Ok(row_count) => {
            app.diagnostics.push(AppDiagnostic::new_success(
                DiagnosticCategory::Export,
                format!("Exported {row_count} fact values to {}", dest.display()),
            ));
            app.show_diagnostics_panel = true;
        }
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::Export,
                format!("Failed to export values: {err}"),
            ));
            app.show_diagnostics_panel = true;
        }
    }
}

/// Writes fact rows across all GCD-selected (non-disabled) sections of the
/// loaded report to `dest` as semicolon-delimited CSV, returning the number
/// of rows written. Abstract rows are always kept; other rows are kept only
/// if their value is non-empty.
fn write_values_csv(app: &TaxelApp, dest: &Path) -> Result<usize, anyhow::Error> {
    let loaded = app
        .loaded
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Cannot export values without a loaded report"))?;
    let lang = &app.settings.lang;

    let mut writer = csv::WriterBuilder::new().delimiter(b';').from_path(dest)?;
    writer.write_record(["Section", "ID", "Depth", "Name", "Value", "Unit", "Context"])?;

    let mut row_count = 0;

    for section in &loaded.report.sections {
        if section.disabled {
            continue;
        }

        let section_name = section
            .labels
            .get(lang)
            .map(String::as_str)
            .unwrap_or(section.role.as_str());

        for row in &section.rows {
            let value = fact_value_to_string(&row.value, lang);

            if !row.is_abstract && value.is_empty() {
                continue;
            }

            let name = row.labels.get(lang).map(String::as_str).unwrap_or("");
            writer.write_record([
                section_name,
                row.concept.as_str(),
                row.depth.to_string().as_str(),
                name,
                value.as_str(),
                row.unit.as_deref().unwrap_or(""),
                row.context.as_str(),
            ])?;
            row_count += 1;
        }
    }

    writer.flush()?;

    Ok(row_count)
}

/// Renders a `FactValue` the same way the fact table displays it, for CSV
/// export.
fn fact_value_to_string(value: &FactValue, lang: &str) -> String {
    match value {
        FactValue::Text(text) => text.clone(),
        FactValue::Checkbox(checked) => checked.to_string(),
        FactValue::Dropdown { selected, options } => options
            .iter()
            .find(|(key, _)| key == selected)
            .and_then(|(_, labels)| labels.get(lang))
            .cloned()
            .unwrap_or_else(|| selected.clone()),
        FactValue::BooleanDropdown(selected) => selected.clone(),
        FactValue::Decimal { raw, .. } => raw.clone(),
        FactValue::Integer(text) => text.clone(),
        FactValue::Date { raw, .. } => raw.clone(),
    }
}

/// Resets `elster_report`'s test marker to match the current `TEST_MARKER`
/// environment variable.
fn sync_test_marker(elster_report: &mut ElsterReport, path: &Path) -> Result<(), anyhow::Error> {
    let current_test_marker = env::var("TEST_MARKER").ok();

    if current_test_marker != elster_report.transfer_header.test_marker {
        elster_report.transfer_header.test_marker = current_test_marker;

        let xml = elster_report
            .to_xml()
            .context("Failed to serialize ElsterReport")?;

        fs::write(path, xml)
            .with_context(|| format!("Failed to write report to {}", path.display()))?;
    }

    if let Some(test_marker) = &elster_report.transfer_header.test_marker {
        info!("Using test marker {test_marker}");
    }

    Ok(())
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

    let Some(taxonomy) =
        load_taxonomies(schema_refs, &schema_ref_paths, allow_download, &taxonomy_dir()?)?
    else {
        return Ok(LoadOutcome::NeedsDownload);
    };

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let taxonomy_type = TaxonomyType::from_schema_refs(instance.schema_refs()).unwrap_or_default();
    let mut report = Report::new(path.to_path_buf(), taxonomy_type);

    report.populate(view, &item_facts, &taxonomy);

    let xml = fs::read_to_string(path)
        .with_context(|| format!("Failed to read Elster XML from {}", path.display()))?;
    let mut elster_report = ElsterReport::parse(&xml)
        .with_context(|| format!("Failed to parse ElsterReport from {}", path.display()))?;

    sync_test_marker(&mut elster_report, path)?;

    Ok(LoadOutcome::Ready(LoadedReport {
        taxonomy,
        instance,
        report,
        elster: elster_report,
        status: ReportStatus::Draft,
    }))
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
    let test_marker = env::var("TEST_MARKER").ok();

    if let Some(test_marker) = &test_marker {
        info!("Using test marker {test_marker}");
    }

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
    let schema_ref_paths = taxel::schema_ref_paths(&schema_refs);

    let Some(taxonomy) = load_taxonomies(
        schema_refs,
        &schema_ref_paths
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<&str>>(),
        allow_download,
        &taxonomy_dir()?,
    )?
    else {
        return Ok(LoadOutcome::NeedsDownload);
    };

    let baseline_roles: Vec<RoleUri> = BASELINE_ROLE_URIS
        .iter()
        .map(|&uri| RoleUri::from(uri))
        .collect();

    let mut instance = create_instance_document(
        &form.start_date,
        &form.end_date,
        namespace_prefix,
        &namespace_uri,
        taxonomy_date,
        &taxonomy,
        &baseline_roles,
    )?;

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let dest = new_report_path()?;
    let mut report = Report::new(dest.clone(), form.taxonomy_type.clone());

    report.populate(view, &item_facts, &taxonomy);
    report.initialize_period_dates(&mut instance, &form.start_date, &form.end_date);

    // Serialize and wrap in an Elster envelope, then write to disk.
    let mut xbrl_bytes: Vec<u8> = Vec::new();
    instance.to_writer(&mut xbrl_bytes)?;

    let balance_date: u32 = form.end_date.replace('-', "").parse()?;

    let mut elster = ElsterReport::new(
        vendor_id,
        Submitter::default(),
        "",
        "",
        balance_date,
        test_marker,
    );
    elster.set_payload_xbrl(xbrl_bytes);
    let xml = elster.to_xml()?;

    fs::write(&dest, &xml)
        .with_context(|| format!("Failed to write new report to {}", dest.display()))?;

    Ok(LoadOutcome::Created(LoadedReport {
        taxonomy,
        instance,
        report,
        elster,
        status: ReportStatus::Draft,
    }))
}

/// Applies a successfully loaded or created report to the app state.
fn finish_load(app: &mut TaxelApp, mut loaded: LoadedReport) {
    for missing_role in &loaded.report.role_mapping_errors {
        app.diagnostics.push(AppDiagnostic::new_warning(
            DiagnosticCategory::Import,
            format!("Missing report-element mapping for role URI: {missing_role}"),
        ));
    }

    if let Some(overview) = app
        .report_list
        .reports()
        .iter()
        .find(|overview| overview.path == loaded.report.path)
    {
        loaded.status = overview.report_status;
    }

    app.section_states = loaded
        .report
        .sections
        .iter()
        .map(|_| SectionState::default())
        .collect();
    app.loaded = Some(loaded);
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
    if let Some(loaded) = &app.loaded {
        if let Some(section) = loaded.report.sections.get(app.selected_tab) {
            app.edit_snapshot = section.rows.iter().map(|r| r.value.clone()).collect();
        }
    }
    app.editing_section = Some(app.selected_tab);

    // If the report was previously validated, mark it
    // as draft again since it has unsaved changes now.
    if let Some(loaded) = &app.loaded {
        app.report_list
            .set_report_status(&loaded.report.path, ReportStatus::Draft);
        app.report_list.save(&mut app.diagnostics);
    }
}

/// Cancels the edit by restoring the values from the snapshot and exiting edit
/// mode.
pub fn cancel_edit(app: &mut TaxelApp) {
    let editing_tab = app.editing_section.unwrap_or(app.selected_tab);

    if let Some(loaded) = &mut app.loaded {
        if let Some(section) = loaded.report.sections.get_mut(editing_tab) {
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

    if let Some(loaded) = app.loaded.as_mut() {
        if let Some(section) = loaded.report.sections.get(editing_tab) {
            // Reject the save if any typed value is syntactically invalid.
            let invalid_facts: Vec<_> = section
                .rows
                .iter()
                .filter(|row| !row.value.is_type_valid())
                .map(|row| (&row.concept, &row.value))
                .collect();

            if !invalid_facts.is_empty() {
                for invalid_fact in &invalid_facts {
                    match &invalid_fact.1 {
                        FactValue::Decimal { .. } => {
                            app.diagnostics.push(AppDiagnostic::new_error_with_fact(
                                DiagnosticCategory::Validation,
                                format!(
                                    "Invalid decimal value for fact '{}'. 2 decimals expected.",
                                    invalid_fact.0
                                ),
                                invalid_fact.0,
                            ));
                        }
                        FactValue::Date { .. } => {
                            app.diagnostics.push(AppDiagnostic::new_error_with_fact(
                                DiagnosticCategory::Validation,
                                format!(
                                    "Invalid date value for fact '{}'. Format YYYY-MM-DD expected.",
                                    invalid_fact.0
                                ),
                                invalid_fact.0,
                            ));
                        }
                        _ => {
                            app.diagnostics.push(AppDiagnostic::new_error_with_fact(
                                DiagnosticCategory::Validation,
                                format!("Invalid value for fact '{}'", invalid_fact.0),
                                invalid_fact.0,
                            ));
                        }
                    }
                }

                return;
            }

            let instance = &mut loaded.instance;
            let taxonomy = &loaded.taxonomy;

            let mut apply_row_update =
                |i: usize, row: &crate::domain::FactRow| -> Result<(), anyhow::Error> {
                    let unchanged = app
                        .edit_snapshot
                        .get(i)
                        .is_some_and(|snapshot| snapshot == &row.value);

                    if unchanged {
                        return Ok(());
                    }

                    let snapshot = app
                        .edit_snapshot
                        .get(i)
                        .map(|fact_value| fact_value as &FactValue);
                    let outcome = update_instance_document(
                        instance,
                        &row.value,
                        snapshot,
                        row.fact_index,
                        row.parent_concept.as_deref(),
                        &row.concept,
                        Some(taxonomy),
                    )?;

                    if outcome == UpdateOutcome::Rebuild {
                        update_outcome = UpdateOutcome::Rebuild;
                    }

                    Ok(())
                };

            // Apply non-structural updates first because they rely on stable
            // fact_index mappings. Tuple structural updates (dropdown and
            // tuple-child checkboxes) can reorder item_facts.
            for (i, row) in section.rows.iter().enumerate() {
                let is_structural = matches!(row.value, FactValue::Dropdown { .. })
                    || matches!(row.value, FactValue::Checkbox(_) if row.fact_index.is_none());

                if is_structural {
                    continue;
                }

                if let Err(err) = apply_row_update(i, row) {
                    app.diagnostics.push(AppDiagnostic::new_error(
                        DiagnosticCategory::App,
                        format!("{err}"),
                    ));
                }
            }

            for (i, row) in section.rows.iter().enumerate() {
                let is_structural = matches!(row.value, FactValue::Dropdown { .. })
                    || matches!(row.value, FactValue::Checkbox(_) if row.fact_index.is_none());

                if !is_structural {
                    continue;
                }

                if let Err(err) = apply_row_update(i, row) {
                    app.diagnostics.push(AppDiagnostic::new_error(
                        DiagnosticCategory::App,
                        format!("{err}"),
                    ));
                }
            }
        }
    }

    if handle_consolidation_range_change(app, editing_tab) == UpdateOutcome::Rebuild {
        update_outcome = UpdateOutcome::Rebuild;
    }

    if handle_report_element_change(app, editing_tab) == UpdateOutcome::Rebuild {
        update_outcome = UpdateOutcome::Rebuild;
    }

    if handle_end_date_change(app, editing_tab) == UpdateOutcome::Rebuild {
        update_outcome = UpdateOutcome::Rebuild;
    }

    if let Some(loaded) = app.loaded.as_mut() {
        // Sync GCD facts to ElsterReport metadata and XBRL contexts.
        loaded
            .report
            .sync_gcd_to_elster(&mut loaded.instance, &mut loaded.elster);

        // Serialize the updated InstanceDocument to bytes.
        let mut xbrl_bytes: Vec<u8> = Vec::new();

        match loaded.instance.to_writer(&mut xbrl_bytes) {
            Err(err) => {
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::App,
                    format!("Failed to serialize XBRL instance: {err}"),
                ));
            }
            Ok(()) => {
                loaded.elster.set_payload_xbrl(xbrl_bytes);

                let result = loaded
                    .elster
                    .to_xml()
                    .context("Failed to serialize Elster report")
                    .and_then(|xml| {
                        fs::write(&loaded.report.path, xml)
                            .context("Failed to write report to disk")
                    });

                match result {
                    Ok(()) => {
                        let now = SystemTime::now();

                        if let Some((start_date, end_date)) = extract_period(&loaded.instance) {
                            app.report_list.set_period(
                                &loaded.report.path,
                                Some(start_date),
                                Some(end_date),
                            );
                        }

                        app.report_list.set_timestamp(&loaded.report.path, now);
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
        if let Some(loaded) = app.loaded.as_mut() {
            let view = loaded.instance.view(&loaded.taxonomy);
            let item_facts = loaded.instance.item_facts();
            loaded.report.populate(view, &item_facts, &loaded.taxonomy);
        }
    }

    app.editing_section = None;
    app.edit_snapshot.clear();
}

/// Saves the currently loaded report to a new location chosen by the user,
/// moves the previous file to the OS trash, and updates the report's path in
/// memory and in the manifest so subsequent saves go to the new location.
pub fn save_report_as(app: &mut TaxelApp, dest: PathBuf) {
    let Some(loaded) = app.loaded.as_ref() else {
        return;
    };

    let old_path = loaded.report.path.clone();

    if dest == old_path {
        return;
    }

    let result = loaded
        .elster
        .to_xml()
        .context("Failed to serialize Elster report")
        .and_then(|xml| fs::write(&dest, xml).context("Failed to write report to disk"));

    match result {
        Ok(()) => {
            if old_path.exists() {
                if let Err(err) = trash::delete(&old_path) {
                    app.diagnostics.push(AppDiagnostic::new_error(
                        DiagnosticCategory::App,
                        format!(
                            "Report saved to new location, but failed to move the previous file to trash: {err}"
                        ),
                    ));
                }
            }

            if let Some(loaded) = app.loaded.as_mut() {
                loaded.report.path = dest.clone();
            }

            app.report_list
                .rename_report(&old_path, dest.clone(), &mut app.diagnostics);

            app.diagnostics.push(AppDiagnostic::new_success(
                DiagnosticCategory::App,
                format!("Report saved to {}", dest.display()),
            ));
        }
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to save report to new location: {err}"),
            ));
        }
    }

    app.show_diagnostics_panel = true;
}

/// Detects if any row whose concept matches `is_target` changed vs the snapshot,
/// then calls `rebuild`. Returns `Rebuild` on success or `NoChange` (with a
/// diagnostic) on error. Shared by both structural-change handlers below.
fn handle_structural_change(
    app: &mut TaxelApp,
    editing_tab: usize,
    is_target: impl Fn(&str) -> bool,
    rebuild: impl FnOnce(&mut TaxelApp) -> Result<(), anyhow::Error>,
) -> UpdateOutcome {
    let changed = app
        .loaded
        .as_ref()
        .and_then(|loaded| loaded.report.sections.get(editing_tab))
        .is_some_and(|section| {
            section.rows.iter().enumerate().any(|(i, row)| {
                is_target(&row.concept)
                    && app
                        .edit_snapshot
                        .get(i)
                        .is_some_and(|fact_value| fact_value != &row.value)
            })
        });

    if changed {
        match rebuild(app) {
            Ok(()) => UpdateOutcome::Rebuild,
            Err(err) => {
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::App,
                    format!("{err}"),
                ));
                UpdateOutcome::NoChange
            }
        }
    } else {
        UpdateOutcome::NoChange
    }
}

/// Detects changes to the consolidation range dropdown and rebuilds the
/// instance if the selection changed to or from "EA". This is necessary because
/// the consolidation range determines which concepts are valid for the report
/// and changing it can cause structural changes to the fact table (e.g. new
/// fact rows being added or removed).
fn handle_consolidation_range_change(app: &mut TaxelApp, editing_tab: usize) -> UpdateOutcome {
    const CONSOLIDATION_RANGE: &str = "genInfo.report.id.consolidationRange";
    const EA_VALUE: &str = "genInfo.report.id.consolidationRange.consolidationRange.EA";

    let ea_selected = app
        .loaded
        .as_ref()
        .and_then(|loaded| loaded.report.sections.get(editing_tab))
        .is_some_and(|section| {
            section.rows.iter().any(|row| {
                row.concept == CONSOLIDATION_RANGE
                    && matches!(&row.value, FactValue::Dropdown { selected, .. } if selected == EA_VALUE)
            })
        });

    handle_structural_change(
        app,
        editing_tab,
        |concept| concept == CONSOLIDATION_RANGE,
        |app| rebuild_instance(app, ea_selected),
    )
}

fn handle_report_element_change(app: &mut TaxelApp, editing_tab: usize) -> UpdateOutcome {
    handle_structural_change(
        app,
        editing_tab,
        |concept| concept.starts_with(REPORT_ELEMENT_PREFIX),
        |app| rebuild_instance(app, false),
    )
}

/// Detects changes to `balSheetClosingDate` and re-runs `remove_forbidden_facts`
/// with the new date. Returns `Rebuild` when facts were actually removed so the
/// UI reflects the trimmed instance.
fn handle_end_date_change(app: &mut TaxelApp, editing_tab: usize) -> UpdateOutcome {
    let changed = app
        .loaded
        .as_ref()
        .and_then(|loaded| loaded.report.sections.get(editing_tab))
        .is_some_and(|section| {
            section.rows.iter().enumerate().any(|(i, row)| {
                row.concept == CLOSING_DATE
                    && app
                        .edit_snapshot
                        .get(i)
                        .is_some_and(|fact_value| fact_value != &row.value)
            })
        });

    if !changed {
        return UpdateOutcome::NoChange;
    }

    let new_end_date_str = app
        .loaded
        .as_ref()
        .and_then(|loaded| loaded.report.sections.get(editing_tab))
        .and_then(|section| section.rows.iter().find(|row| row.concept == CLOSING_DATE))
        .and_then(|row| match &row.value {
            FactValue::Text(text) if !text.is_empty() => Some(text.clone()),
            _ => None,
        });

    let Some(end_date_str) = new_end_date_str else {
        return UpdateOutcome::NoChange;
    };

    let Ok(end_date) = NaiveDate::parse_from_str(&end_date_str, "%Y-%m-%d") else {
        return UpdateOutcome::NoChange;
    };

    let Some(loaded) = app.loaded.as_mut() else {
        return UpdateOutcome::NoChange;
    };

    let facts_before = loaded.instance.facts().len();

    remove_forbidden_facts(&mut loaded.instance, &loaded.taxonomy, &end_date);

    let facts_after = loaded.instance.facts().len();

    if facts_after < facts_before {
        debug!(
            "balSheetClosingDate changed: removed {} forbidden facts",
            facts_before - facts_after
        );
        UpdateOutcome::Rebuild
    } else {
        UpdateOutcome::NoChange
    }
}

/// Rebuilds the instance document and report from scratch, importing values
/// from the current instance. Used by both `handle_consolidation_range_change`
/// and `handle_report_element_change`. When `remove_trade_accounting` is true,
/// facts forbidden for handelsrechtlicher Einzelabschluss are stripped.
///
/// `pub` (rather than the private visibility every other helper in this
/// module has) solely so `lib.rs` can narrowly re-export it for
/// `tests/rebuild_instance.rs`. The enclosing `app::report` module stays
/// `pub(crate)`, so this specific `lib.rs` re-export is the only path that
/// reaches it from outside the crate — not meant to be a stable public API.
pub fn rebuild_instance(
    app: &mut TaxelApp,
    remove_trade_accounting: bool,
) -> Result<(), anyhow::Error> {
    let loaded = app
        .loaded
        .as_ref()
        .context("No loaded report in app state")?;
    let source_instance = loaded.instance.clone();
    let report_path = loaded.report.path.clone();
    let taxonomy_type = loaded.report.taxonomy_type.clone();
    let taxonomy_version = taxonomy_version_from_schema_refs(source_instance.schema_refs());
    let taxonomy_date =
        taxonomy_version.and_then(|version| TAXONOMY_VERSION_TO_DATE.get(version).copied());
    let (start_date, end_date) = extract_period(&source_instance).unwrap_or_default();
    let namespace_prefix = taxonomy_type.namespace_prefix();
    let namespace_uri = taxonomy_date
        .map(|date| taxonomy_type.namespace_uri(date))
        .unwrap_or_default();
    let roles = active_roles(&source_instance);

    let mut source_report = Report::new(report_path.clone(), taxonomy_type.clone());
    {
        let taxonomy = &loaded.taxonomy;
        let source_item_facts = source_instance.item_facts();
        let source_view = source_instance.view(taxonomy);
        source_report.populate(source_view, &source_item_facts, taxonomy);
    }

    let mut fresh_instance = {
        let taxonomy = &app
            .loaded
            .as_ref()
            .context("No loaded report in app state")?
            .taxonomy;
        create_instance_document(
            &start_date,
            &end_date,
            namespace_prefix,
            &namespace_uri,
            taxonomy_date.unwrap_or(""),
            taxonomy,
            &roles,
        )?
    };

    if remove_trade_accounting {
        let taxonomy = &app
            .loaded
            .as_ref()
            .context("No loaded report in app state")?
            .taxonomy;
        remove_trade_accounting_facts(&mut fresh_instance, taxonomy);
    }

    let mut fresh_report = Report::new(report_path, taxonomy_type);

    {
        let taxonomy = &app
            .loaded
            .as_ref()
            .context("No loaded report in app state")?
            .taxonomy;
        let source_item_facts = source_instance.item_facts();

        {
            let fresh_view = fresh_instance.view(taxonomy);
            let fresh_item_facts = fresh_instance.item_facts();
            fresh_report.populate(fresh_view, &fresh_item_facts, taxonomy);
        }

        let (matched, imported) = fresh_report.apply_imported_values(
            &source_report,
            &source_item_facts,
            &mut fresh_instance,
            taxonomy,
            true,
        );
        debug!("Rebuild import: matched={matched}, imported={imported}");

        restore_required_nil_tuple_children(&mut fresh_instance, taxonomy);

        // `apply_imported_values` may have copied a stale total from the
        // source report for a calculated concept (e.g. if its children were
        // reduced by `remove_trade_accounting_facts`). This path persists
        // straight into `fresh_instance` without going through the normal
        // save_report snapshot-diff, so totals must be reconciled here.
        fresh_report.recompute_calculated_values();
        for section in &fresh_report.sections {
            write_calculated_values_to_instance(section, &mut fresh_instance);
        }
    }

    {
        let taxonomy = &app
            .loaded
            .as_ref()
            .context("No loaded report in app state")?
            .taxonomy;
        for section in &fresh_report.sections {
            for row in &section.rows {
                if let FactValue::Dropdown { selected, .. } = &row.value {
                    if !selected.is_empty() {
                        let snapshot_val = FactValue::Dropdown {
                            selected: String::new(),
                            options: vec![],
                        };
                        let _ = update_instance_document(
                            &mut fresh_instance,
                            &row.value,
                            Some(&snapshot_val),
                            row.fact_index,
                            row.parent_concept.as_deref(),
                            &row.concept,
                            Some(taxonomy),
                        );
                    }
                }
            }
        }
    }

    {
        let taxonomy = &app
            .loaded
            .as_ref()
            .context("No loaded report in app state")?
            .taxonomy;

        // Re-run legal-form and date-based removal now that company data has
        // been imported into `fresh_instance`. The earlier call inside
        // `create_instance_document` ran on a still-blank instance, where
        // `entity_legal_forms` is always empty, so `should_remove_fact`
        // skips legal-form filtering entirely (fails safe) — this second
        // pass is the only place it actually applies during a rebuild.
        // Doesn't help STU-based removal, which has the opposite polarity
        // (removes rather than skips on the blank pass, so the damage is
        // already done and can't be undone here).
        let end_date_parsed = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
            .with_context(|| format!("Failed to parse end date '{end_date}'"))?;
        remove_forbidden_facts(&mut fresh_instance, taxonomy, &end_date_parsed);
        restore_required_nil_tuple_children(&mut fresh_instance, taxonomy);

        let fresh_view = fresh_instance.view(taxonomy);
        let fresh_item_facts = fresh_instance.item_facts();
        fresh_report.populate(fresh_view, &fresh_item_facts, taxonomy);
        fresh_report.recompute_calculated_values();

        for section in &fresh_report.sections {
            write_calculated_values_to_instance(section, &mut fresh_instance);
        }
    }

    if let Some(loaded) = app.loaded.as_mut() {
        loaded.instance = fresh_instance;
        loaded.report = fresh_report;
    }

    Ok(())
}

/// Validates the imported report and reports any issues in the diagnostics
/// panel.
pub fn validate_report(app: &mut TaxelApp) {
    clear_diagnostics_by_category(app, DiagnosticCategory::Validation);

    let Some(loaded) = app.loaded.as_mut() else {
        return;
    };

    loaded.status = ReportStatus::Draft;
    app.report_list
        .set_report_status(&loaded.report.path, ReportStatus::Draft);
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
    let loaded = app
        .loaded
        .as_mut()
        .context("Failed to get loaded report from app state")?;

    let mut xbrl_bytes = Vec::new();
    loaded
        .instance
        .to_writer(&mut xbrl_bytes)
        .context("Failed to serialize XBRL instance")?;

    let taxonomy_version = loaded
        .taxonomy
        .date()
        .and_then(|date| TAXONOMY_DATE_TO_VERSION.get(date));

    let Some(taxonomy_version) = taxonomy_version else {
        app.diagnostics.push(AppDiagnostic::taxonomy_version_error(
            DiagnosticCategory::Validation,
        ));
        app.show_diagnostics_panel = true;
        return Ok(());
    };

    let loaded = app
        .loaded
        .as_mut()
        .context("No loaded report in app state")?;
    loaded.elster.set_payload_xbrl(xbrl_bytes);
    let xml = loaded
        .elster
        .to_xml()
        .context("Failed to serialize Elster report")?;

    for &concept in REQUIRED_GCD_FACTS {
        // The "companyId.ST13" concept can occur multiple times in the
        // report under different parent concepts. We only want to check for
        // its existence once.
        let parent = if concept == COMPANY_TAX_NUMBER {
            Some(COMPANY_TAX_NUMBER_PARENT)
        } else {
            None
        };

        if loaded
            .report
            .find_in_section(GCD_ROLE_URI, concept, parent)
            .is_none()
        {
            app.diagnostics.push(AppDiagnostic::new_missing_fact_value(
                DiagnosticCategory::Validation,
                concept,
            ));
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
            if let Some(loaded) = app.loaded.as_mut() {
                loaded.status = ReportStatus::Validated;
                app.report_list
                    .set_report_status(&loaded.report.path, ReportStatus::Validated);
                app.report_list.save(&mut app.diagnostics);
            }

            app.diagnostics.push(AppDiagnostic::new_success(
                DiagnosticCategory::Validation,
                format!(
                    "Validation completed successfully\n{}",
                    response.validation_response()
                ),
            ));
        }
        Err(err) => {
            // Display raw xml error in debug mode only.
            if cfg!(debug_assertions) {
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::Validation,
                    format!(
                        "Full validation error: {err}\n{}",
                        err.validation_response().unwrap_or_default()
                    ),
                ));
            }

            if let Some(validation_report) = err
                .validation_report()
                .context("Failed to extract validation report from error")?
            {
                for issue in validation_report.issues {
                    if let (Some(error_code), Some(error_text)) = (issue.error_code, issue.text) {
                        let message = if let Some(rule_name) = &issue.rule_name {
                            let rule_suffix = rule_name.rsplit('/').next().unwrap_or(rule_name);

                            format!("Error code ({error_code}): {error_text}\nRule: {rule_suffix}")
                        } else {
                            format!("Error code ({error_code}): {error_text}")
                        };

                        if let Some(field_identifier) = issue.field_identifier {
                            let fact_name = extract_fact_name(&field_identifier);
                            app.diagnostics.push(AppDiagnostic::new_error_with_fact(
                                DiagnosticCategory::Validation,
                                message,
                                fact_name,
                            ));
                        } else {
                            app.diagnostics.push(AppDiagnostic::new_error(
                                DiagnosticCategory::Validation,
                                message,
                            ));
                        }
                    }
                }
            }
        }
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

    let Some(loaded) = app.loaded.as_ref() else {
        return;
    };

    if loaded.status != ReportStatus::Validated {
        app.diagnostics.push(AppDiagnostic::new_error(
            DiagnosticCategory::Send,
            "Validate the report first and make sure that all errors are resolved".to_string(),
        ));
        app.show_diagnostics_panel = true;
        return;
    }

    let report_path = loaded.report.path.clone();
    let xml_result = loaded
        .elster
        .to_xml()
        .context("Failed to serialize Elster report");

    let xml = match xml_result {
        Ok(xml) => xml,
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::Send,
                err.to_string(),
            ));
            app.show_diagnostics_panel = true;
            return;
        }
    };

    let Some(eric) = &app.eric else {
        return;
    };

    let taxonomy_version = app
        .loaded
        .as_ref()
        .and_then(|loaded| loaded.taxonomy.date())
        .and_then(|date| TAXONOMY_DATE_TO_VERSION.get(date));

    let Some(taxonomy_version) = taxonomy_version else {
        app.diagnostics.push(AppDiagnostic::taxonomy_version_error(
            DiagnosticCategory::Send,
        ));
        app.show_diagnostics_panel = true;
        return;
    };

    let certificate_path = certificate_path.clone();
    let password = password.clone();

    let confirmations_dir = match confirmations_dir() {
        Ok(dir) => dir,
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::Send,
                format!("Failed to prepare confirmations directory: {err}"),
            ));
            app.show_diagnostics_panel = true;
            return;
        }
    };

    let report_stem = report_path
        .file_stem()
        .and_then(|file_stem| file_stem.to_str())
        .unwrap_or("confirmation");
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let pdf_filename = format!("{report_stem}_{timestamp}.pdf");
    let pdf_path = confirmations_dir.join(&pdf_filename);
    let pdf_path_str = match pdf_path.to_str() {
        Some(pdf_path) => pdf_path,
        None => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::Send,
                "Failed to convert PDF path to string".to_string(),
            ));
            app.show_diagnostics_panel = true;
            return;
        }
    };

    match eric.send(
        xml,
        "Bilanz",
        taxonomy_version,
        &certificate_path,
        &password,
        Some(pdf_path_str),
    ) {
        Ok(response) => {
            if let Some(loaded) = app.loaded.as_mut() {
                loaded.status = ReportStatus::Sent;
                app.report_list
                    .set_report_status(&loaded.report.path, ReportStatus::Sent);
                app.report_list.save(&mut app.diagnostics);
            }

            app.diagnostics.push(AppDiagnostic::new_success(
                DiagnosticCategory::Send,
                format!(
                    "Send completed successfully\nConfirmation PDF saved to: {}\n{}",
                    pdf_path.display(),
                    response.payload.server_response
                ),
            ));

            if let Err(err) = open::that(&pdf_path) {
                app.diagnostics.push(AppDiagnostic::new_warning(
                    DiagnosticCategory::Send,
                    format!("Confirmation PDF saved, but could not be opened automatically: {err}"),
                ));
            }
        }
        Err(err) => {
            let validation_error = err.validation_response().unwrap_or_default();
            let server_error = err.server_response().unwrap_or_default();
            let mut message = format!("Send error: {err}");

            if !validation_error.is_empty() {
                message.push_str(&format!("\nValidation error: {validation_error}"));
            }

            if !server_error.is_empty() {
                message.push_str(&format!("\nServer error: {server_error}"));
            }

            app.diagnostics
                .push(AppDiagnostic::new_error(DiagnosticCategory::Send, message));
        }
    }

    app.show_diagnostics_panel = true;
}

/// Moves the currently loaded report to the operating system trash and resets
/// the app state to the home screen.
pub fn delete_report(app: &mut TaxelApp) {
    let Some(loaded) = &app.loaded else {
        return;
    };
    let path = loaded.report.path.clone();

    match trash::delete(&path) {
        Ok(()) => {
            app.report_list.remove_report(&path, &mut app.diagnostics);
            app.loaded = None;
            app.selected_tab = 0;
            app.search.results.clear();
            app.search.scroll_to_row = None;
            app.search.row_highlight = None;
            app.loading = None;
            app.diagnostics.clear();
            app.show_diagnostics_panel = true;
            app.editing_section = None;
            app.edit_snapshot.clear();
        }
        Err(err) => {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to move report to trash: {err}"),
            ));
            app.show_diagnostics_panel = true;
        }
    }
}

/// Removes a report entry from the report list without requiring it to be
/// loaded first. Used for entries whose file can no longer be found (e.g. it
/// was moved or deleted outside the app) as well as entries whose file still
/// exists, in which case it is moved to the OS trash first.
pub fn remove_report_from_list(app: &mut TaxelApp, path: PathBuf) {
    if path.exists() {
        if let Err(err) = trash::delete(&path) {
            app.diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to move report to trash: {err}"),
            ));
            app.show_diagnostics_panel = true;
            return;
        }
    }

    app.report_list.remove_report(&path, &mut app.diagnostics);
    app.show_diagnostics_panel = true;
}

/// Clears all diagnostics of the specified category from the app state. This is
/// used to clear old validation errors when re-validating, or to clear old send
/// errors when re-sending the report, while keeping other diagnostics (like app
/// errors).
fn clear_diagnostics_by_category(app: &mut TaxelApp, category: DiagnosticCategory) {
    app.diagnostics
        .retain(|diagnostic| diagnostic.category != category);
}
