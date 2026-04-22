use crate::{
    app::{
        diagnostics::{AppDiagnostic, DiagnosticCategory},
        SectionState, TaxelApp, APP_NAME, APP_VERSION,
    },
    domain::{Report, ReportStatus},
    infrastructure::report_store::reports_dir,
};
use anyhow::Context;
use chrono::{Datelike, Utc};
use eframe::egui::{self};
use eric_sdk::ErrorCode;
use log::debug;
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};
use taxel::{
    elster::Submitter, ElsterReport, TaxonomyType, GCD_ROLE_URI, REPORT_ELEMENT_TO_ROLE_URI,
    TAXONOMY_VERSION_TO_DATE, TAXONOMY_YEAR_TO_VERSION, TEST_MARKER,
};
use uuid::Uuid;
use xbrl_rs::{
    Context as XbrlContext, ContextId, EntityIdentifier, ExpandedName, InstanceDocument,
    NamespacePrefix, NamespaceUri, Period, TaxonomyLoader, TaxonomySet, Unit, UnitId,
};

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

/// The result of a background load operation.
#[allow(clippy::large_enum_variant)]
pub enum LoadOutcome {
    /// The report was loaded successfully and is ready to be displayed.
    Ready(TaxonomySet, InstanceDocument, Report),
    /// Like `Ready`, but the report was freshly created or imported and must be
    /// registered in the manifest before it appears in the report list.
    Created(TaxonomySet, InstanceDocument, Report),
    /// Taxonomies are missing from disk; the user must confirm before downloading.
    NeedsDownload,
}

/// Imports a report from an arbitrary file path by copying it to the app's
/// managed reports directory and then loading it from there. This ensures that
/// the app has full control over the report file and can reliably persist
/// changes.
pub fn import_report(app: &mut TaxelApp, path: PathBuf, ctx: &egui::Context) {
    match parse_and_copy_report(&path) {
        Ok(copied_path) => {
            app.register_report(&copied_path);
            app.refresh_reports();

            load_report(app, copied_path, ctx.clone(), false);
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
fn parse_and_copy_report(path: &Path) -> Result<PathBuf, anyhow::Error> {
    let reports_dir = reports_dir()?;

    let xml = fs::read_to_string(path)
        .with_context(|| format!("Failed to read report from {}", path.display()))?;

    let mut report = ElsterReport::parse(&xml)
        .with_context(|| format!("Failed to parse ElsterReport from {}", path.display()))?;

    if let Ok(vendor_id) = env::var("VENDOR_ID") {
        report.transfer_header.manufacturer_id = vendor_id;
    }

    if report.transfer_header.test_marker.is_none() {
        report.transfer_header.test_marker = Some(TEST_MARKER.to_string());
    }

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

    report.populate(view, &item_facts);

    Ok(LoadOutcome::Ready(taxonomy, instance, report))
}

/// Creates a new blank report from a taxonomy for the given form parameters.
/// The creation runs on a background thread; results are polled via
/// `poll_load_result`.
pub fn create_report(
    app: &mut TaxelApp,
    form: NewReportForm,
    ctx: egui::Context,
    allow_download: bool,
) {
    app.selected_tab = 0;
    app.report = None;
    app.diagnostics.clear();
    app.editing_section = None;
    app.edit_snapshot.clear();
    app.pending_new_report_form = Some(form.clone());

    let (tx, rx) = mpsc::channel();
    app.loading = Some(rx);

    thread::spawn(move || {
        let result = create_instance_document(form, allow_download);
        let _ = tx.send(result);
        ctx.request_repaint();
    });
}

/// Creates a new XBRL instance document based on the provided form data,
/// populates the fact table, and writes the report to disk. If `allow_download`
/// is false and the required taxonomies are missing,
/// `LoadOutcome::NeedsDownload` is returned so the UI can ask for confirmation
/// before downloading.
fn create_instance_document(
    form: NewReportForm,
    allow_download: bool,
) -> Result<LoadOutcome, anyhow::Error> {
    let date = TAXONOMY_VERSION_TO_DATE
        .get(form.taxonomy_version.as_str())
        .with_context(|| {
            format!(
                "No taxonomy date known for version {}. Supported: 6.4–6.9.",
                form.taxonomy_version
            )
        })?;

    let schema_refs = form.taxonomy_type.schema_refs(date);
    let namespace_prefix = form.taxonomy_type.namespace_prefix();
    let namespace_uri = form.taxonomy_type.namespace_uri(date);

    let taxonomy_dir = taxonomy_dir()?;
    let loader = TaxonomyLoader::new()?;

    let schema_ref_paths: Vec<String> = schema_refs
        .iter()
        .filter_map(|url| url.split("/taxonomies/").nth(1).map(str::to_string))
        .collect();

    let taxonomies_missing = schema_ref_paths
        .iter()
        .any(|p| !taxonomy_dir.join(p).exists());

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

    let mut namespaces: HashMap<NamespacePrefix, NamespaceUri> = [
        (
            "de-gcd",
            format!("http://www.xbrl.de/taxonomies/de-gcd-{date}"),
        ),
        ("link", "http://www.xbrl.org/2003/linkbase".to_string()),
        ("hgbref", "http://www.xbrl.de/2008/ref".to_string()),
        ("xhtml", "http://www.w3.org/1999/xhtml".to_string()),
        (
            "xsi",
            "http://www.w3.org/2001/XMLSchema-instance".to_string(),
        ),
        ("xbrli", "http://www.xbrl.org/2003/instance".to_string()),
        ("xbrldi", "http://xbrl.org/2006/xbrldi".to_string()),
        ("iso4217", "http://www.xbrl.org/2003/iso4217".to_string()),
        ("xlink", "http://www.w3.org/1999/xlink".to_string()),
        ("ref", "http://www.xbrl.org/2024/ref".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (NamespacePrefix::from(k), NamespaceUri::from(v)))
    .collect();

    namespaces.insert(
        NamespacePrefix::from(namespace_prefix),
        NamespaceUri::from(namespace_uri),
    );

    // TODO: update by tax number from GCD.
    let entity = EntityIdentifier {
        scheme: "http://www.rzf-nrw.de/Steuernummer".to_string(),
        value: "0000000000000".to_string(),
    };

    let year = form.end_date.split('-').next().unwrap_or("");

    let instant_context = XbrlContext::new(
        ContextId::from(format!("I-{year}")),
        entity.clone(),
        Period::Instant {
            date: form.end_date.clone(),
        },
    );

    let duration_context = XbrlContext::new(
        ContextId::from(format!("D-{year}")),
        entity,
        Period::Duration {
            start: form.start_date.clone(),
            end: form.end_date.clone(),
        },
    );

    let units = [
        Unit::new(
            UnitId::from("EUR"),
            vec![ExpandedName::new(
                NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
                "EUR".to_string(),
            )],
            vec![],
        ),
        Unit::new(
            UnitId::from("pure"),
            vec![ExpandedName::new(
                NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                "pure".to_string(),
            )],
            vec![],
        ),
        Unit::new(
            UnitId::from("shares"),
            vec![ExpandedName::new(
                NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                "shares".to_string(),
            )],
            vec![],
        ),
    ];

    // TODO: build instance selectively for chosen sections only.
    // Currently builds the full instance from all sections in the taxonomy.
    let instance = InstanceDocument::from_taxonomy(
        &taxonomy,
        namespaces,
        instant_context,
        duration_context,
        &units,
    );

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let dest = new_report_path()?;
    let mut report = Report::new(dest.clone());

    report.populate(view, &item_facts);

    // Filter sections to GCD + the report elements chosen by the user.
    let chosen_role_uris: HashSet<&'static str> = form
        .selected_elements
        .iter()
        .filter_map(|code| {
            let concept = format!("genInfo.report.id.reportElement.reportElements.{code}");
            REPORT_ELEMENT_TO_ROLE_URI.get(concept.as_str()).copied()
        })
        .collect();

    report.sections.retain(|section| {
        section.role == GCD_ROLE_URI || chosen_role_uris.contains(section.role.as_str())
    });

    // Serialize and wrap in an Elster envelope, then write to disk.
    let mut xbrl_bytes: Vec<u8> = Vec::new();
    instance.to_writer(&mut xbrl_bytes)?;

    let balance_date: u32 = form.end_date.replace('-', "").parse()?;

    // TODO: overwirte manufacturer id, recipient id, recipient value,
    // ebilanz_version, and test_marker.
    let mut elster = ElsterReport::new(
        "",
        Submitter::default(),
        "",
        "",
        balance_date,
        "",
        Some(TEST_MARKER),
    );
    elster.set_payload_xbrl(xbrl_bytes);
    let xml = elster.to_xml()?;

    fs::write(&dest, &xml)
        .with_context(|| format!("Failed to write new report to {}", dest.display()))?;

    Ok(LoadOutcome::Created(taxonomy, instance, report))
}

/// Polls the background XML load result and updates the app state accordingly.
pub fn poll_load_result(app: &mut TaxelApp) {
    if let Some(rx) = &app.loading {
        match rx.try_recv() {
            Ok(Ok(LoadOutcome::Ready(taxonomy, instance, report))) => {
                finish_load(app, taxonomy, instance, report);
            }
            Ok(Ok(LoadOutcome::Created(taxonomy, instance, report))) => {
                app.register_report(&report.path);
                finish_load(app, taxonomy, instance, report);
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

/// Applies a successfully loaded or created report to the app state.
fn finish_load(
    app: &mut TaxelApp,
    taxonomy: TaxonomySet,
    instance: InstanceDocument,
    mut report: Report,
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
    app.loading = None;
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
            .set_report_status(&report.path, ReportStatus::Draft, &mut app.diagnostics);
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

                if let Some(idx) = row.fact_index {
                    if row.value.is_empty() {
                        instance.set_fact_nil(idx, true);
                    } else {
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
