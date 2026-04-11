use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use uuid::Uuid;

/// The `ReportSummary` struct represents a summary of a created or imported
/// report.
#[derive(Clone)]
pub struct ReportSummary {
    pub path: PathBuf,
    pub display_name: String,
    pub created_date: String,
    pub created_unix: i64,
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
            let created_unix = system_time_to_unix_seconds(modified_at);
            let display_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown.xml")
                .to_string();

            summaries.push(ReportSummary {
                path,
                display_name,
                created_date: format_date(created_unix),
                created_unix,
            });
        }

        summaries.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));

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

fn reports_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join("taxel").join("reports"))
        .context("Could not determine data directory")
}
