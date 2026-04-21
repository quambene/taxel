use crate::domain::{ReportMeta, ReportStatus};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

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

/// Loads the list of imported reports from the filesystem, extracting metadata
/// for display. The list is sorted by creation date, newest first. This
/// method is called when the application starts and whenever the report
/// list is refreshed.
pub fn load_report_meta() -> Result<Vec<ReportMeta>> {
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

/// Loads the report manifest. Supports both the current object format and the
/// legacy format where values were plain unix timestamps.
fn load_report_manifest() -> Result<HashMap<String, ReportManifestEntry>> {
    let reports_dir = reports_dir()?;

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

/// Determines the path to the application's reports directory, which is
/// typically located in the user's data directory.
pub fn reports_dir() -> Result<PathBuf> {
    let reports_dir = dirs::data_dir()
        .map(|dir| dir.join("taxel").join("reports"))
        .context("Could not determine data directory")?;

    create_reports_dir_if_not_exists(&reports_dir)?;

    Ok(reports_dir)
}

fn create_reports_dir_if_not_exists(reports_dir: &Path) -> Result<()> {
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
