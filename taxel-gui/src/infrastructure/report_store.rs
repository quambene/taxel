use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use uuid::Uuid;

const CREATION_MANIFEST_FILE: &str = "reports.json";

/// The lifecycle status of a report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReportStatus {
    #[default]
    Draft,
    Validated,
    Sent,
}

impl ReportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReportStatus::Draft => "Draft",
            ReportStatus::Validated => "Validated",
            ReportStatus::Sent => "Sent",
        }
    }
}

/// Persisted metadata for a report in the manifest file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportManifestEntry {
    /// The creation date as a unix timestamp.
    pub created: i64,
    /// The lifecycle status of the report.
    #[serde(default)]
    pub status: ReportStatus,
}

/// The `ReportSummary` struct represents a summary of a created or imported
/// report.
#[derive(Clone)]
pub struct ReportSummary {
    /// The file path to the report XML file.
    pub path: PathBuf,
    /// The display name of the report, typically derived from the file name.
    pub display_name: String,
    /// The creation date as a formatted string for display in the UI.
    pub created_date: String,
    /// The creation date as a unix timestamp for sorting and comparison.
    pub created: i64,
    /// The lifecycle status of the report, used for display and filtering in
    /// the UI.
    pub report_status: ReportStatus,
}

/// The `ReportStore` struct manages the list of created or imported reports,
/// including their metadata and creation dates.
pub struct ReportStore {
    pub report_list: Vec<ReportSummary>,
}

impl ReportStore {
    pub fn new(report_list: Vec<ReportSummary>) -> Self {
        Self { report_list }
    }

    /// Loads the list of imported reports from the filesystem, extracting metadata
    /// for display. The list is sorted by creation date, newest first. This
    /// method is called when the application starts and whenever the report
    /// list is refreshed.
    pub fn load_reports() -> Result<Self> {
        let reports_dir = reports_dir()?;

        create_reports_dir_if_not_exists(&reports_dir)?;

        let mut summaries = Vec::new();

        for entry in fs::read_dir(&reports_dir).with_context(|| {
            format!(
                "Failed to read reports directory: {}",
                reports_dir.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if path.extension().and_then(|ext| ext.to_str()) != Some("xml") {
                continue;
            }

            let metadata = entry.metadata().with_context(|| {
                format!(
                    "Failed to read metadata for report file: {}",
                    path.display()
                )
            })?;

            let modified_at = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let created = system_time_to_unix_seconds(modified_at);
            let display_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown.xml")
                .to_string();

            summaries.push(ReportSummary {
                path,
                display_name,
                created_date: format_date(created),
                created,
                report_status: ReportStatus::Draft,
            });
        }

        summaries.sort_by(|a, b| b.created.cmp(&a.created));

        let report_store = Self::new(summaries);

        Ok(report_store)
    }
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

/// Copies a report file to the application's reports directory, assigning it a
/// unique name based on a UUID. This is used when importing a report, ensuring
/// that the original file remains unchanged and that the imported report is
/// stored in a consistent location for the application to manage.
pub fn copy_report(path: &Path) -> Result<PathBuf> {
    let reports_dir = reports_dir()?;

    create_reports_dir_if_not_exists(&reports_dir)?;

    let uuid = Uuid::new_v4();
    let copied_path = reports_dir.join(format!("ebilanz_{uuid}.xml"));

    fs::copy(path, &copied_path).with_context(|| {
        format!(
            "Failed to copy report from {} to {}",
            path.display(),
            copied_path.display()
        )
    })?;

    Ok(copied_path)
}

pub fn system_time_to_unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub fn format_date(unix_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(unix_seconds, 0)
        .map(|utc| utc.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Loads the report manifest. Supports both the current object format and the
/// legacy format where values were plain unix timestamps.
pub fn load_report_manifest() -> Result<HashMap<String, ReportManifestEntry>> {
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

fn reports_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join("taxel").join("reports"))
        .context("Could not determine data directory")
}
