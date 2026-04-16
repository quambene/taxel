use crate::{
    app::{APP_NAME, APP_VERSION},
    domain::{ReportMeta, ReportStatus},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};
use taxel::{ElsterReport, TEST_MARKER};
use uuid::Uuid;

/// The file name of the manifest that stores report metadata. This manifest is
/// stored in the application's directory.
const CREATION_MANIFEST_FILE: &str = "reports.json";

/// Persisted metadata for a report in the manifest file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportManifestEntry {
    /// The creation date as a unix timestamp.
    pub created: i64,
    /// The lifecycle status of the report.
    #[serde(default)]
    pub status: ReportStatus,
}

/// The `ReportStore` struct manages the list of created or imported reports,
/// including their metadata and creation dates.
pub struct ReportStore;

impl ReportStore {
    /// Loads the list of imported reports from the filesystem, extracting metadata
    /// for display. The list is sorted by creation date, newest first. This
    /// method is called when the application starts and whenever the report
    /// list is refreshed.
    pub fn load() -> Result<Vec<ReportMeta>> {
        let reports_dir = reports_dir()?;

        create_reports_dir_if_not_exists(&reports_dir)?;

        let manifest = load_report_manifest()?;

        let report_meta = manifest
            .into_iter()
            .map(|(path, entry)| ReportMeta {
                path: PathBuf::from(path),
                created: entry.created,
                status: entry.status,
            })
            .collect();

        Ok(report_meta)
    }
}

/// Imports a report file into the application's reports directory.
///
/// The source XML is parsed as an [`taxel::elster::ElsterReport`], modified,
/// and serialized back to XML. The original file is never modified.
pub fn copy_report(path: &Path) -> Result<PathBuf> {
    let reports_dir = reports_dir()?;

    create_reports_dir_if_not_exists(&reports_dir)?;

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

/// Loads the report manifest. Supports both the current object format and the
/// legacy format where values were plain unix timestamps.
fn load_report_manifest() -> Result<HashMap<String, ReportManifestEntry>> {
    let reports_dir = reports_dir()?;

    create_reports_dir_if_not_exists(&reports_dir)?;

    let manifest_path = reports_dir.join(CREATION_MANIFEST_FILE);

    if !manifest_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "Failed to read reports manifest: {}",
            manifest_path.display()
        )
    })?;

    if content.trim().is_empty() {
        return Ok(HashMap::new());
    }

    if let Ok(entries) = serde_json::from_str::<HashMap<String, ReportManifestEntry>>(&content) {
        return Ok(entries);
    }

    Err(anyhow::anyhow!(
        "Failed to parse reports manifest JSON: {}",
        manifest_path.display()
    ))
}

/// Saves the report manifest as JSON.
pub fn save_report_manifest(manifest: &HashMap<String, ReportManifestEntry>) -> Result<()> {
    let reports_dir = reports_dir()?;

    create_reports_dir_if_not_exists(&reports_dir)?;

    let manifest_path = reports_dir.join(CREATION_MANIFEST_FILE);
    let temp_path = reports_dir.join(format!("{CREATION_MANIFEST_FILE}.tmp"));
    let json =
        serde_json::to_string_pretty(manifest).context("Failed to serialize reports manifest")?;

    fs::write(&temp_path, json).with_context(|| {
        format!(
            "Failed to write temporary reports manifest: {}",
            temp_path.display()
        )
    })?;

    fs::rename(&temp_path, &manifest_path).with_context(|| {
        format!(
            "Failed to move temporary reports manifest to {}",
            manifest_path.display()
        )
    })?;

    Ok(())
}

pub fn create_reports_dir_if_not_exists(reports_dir: &Path) -> Result<()> {
    if !reports_dir.exists() {
        fs::create_dir_all(reports_dir).with_context(|| {
            format!(
                "Failed to create reports directory: {}",
                reports_dir.display()
            )
        })?;
    }

    Ok(())
}

/// Determines the path to the application's reports directory, which is
/// typically located in the user's data directory.
fn reports_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join("taxel").join("reports"))
        .context("Could not determine data directory")
}
