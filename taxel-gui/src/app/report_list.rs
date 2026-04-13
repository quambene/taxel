use crate::{
    app::{AppDiagnostic, DiagnosticCategory},
    domain::{ReportMeta, ReportStatus},
    infrastructure::report_store::{self, ReportManifestEntry, ReportStore},
};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// The `ReportSummary` struct represents a summary of a created or imported
/// report.
#[derive(Clone)]
pub struct ReportOverview {
    pub path: PathBuf,
    /// The display name of the report, typically derived from the file name.
    pub display_name: String,
    /// The creation date as a unix timestamp.
    pub unix_seconds: i64,
    /// The creation date as a formatted string for display in the UI.
    pub created_date: String,
    /// The lifecycle status of the report, used for display and filtering in
    /// the UI.
    pub report_status: ReportStatus,
}

impl TryFrom<ReportMeta> for ReportOverview {
    type Error = anyhow::Error;

    fn try_from(meta: ReportMeta) -> Result<Self> {
        let display_name = meta
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.xml")
            .to_string();

        Ok(Self {
            path: meta.path,
            display_name,
            unix_seconds: meta.created,
            created_date: format_date(meta.created),
            report_status: meta.status,
        })
    }
}

/// The `ReportList` struct manages the list of imported reports, including
/// their metadata and creation dates. Creation timestamps are persisted in a
/// JSON manifest in the reports directory.
pub struct ReportList {
    reports: Vec<ReportOverview>,
}

impl ReportList {
    pub fn new() -> Self {
        Self {
            reports: Vec::new(),
        }
    }

    pub fn reports(&self) -> &[ReportOverview] {
        &self.reports
    }

    /// Refreshes the report list by loading the report metadata from the
    /// filesystem. This is called when the application starts and whenever the
    /// report list is refreshed.
    pub fn refresh(&mut self) -> Result<()> {
        let report_meta = ReportStore::load()?;

        let mut report_overiews = report_meta
            .into_iter()
            .map(ReportOverview::try_from)
            .collect::<Result<Vec<_>>>()?;

        report_overiews.sort_by(|a, b| b.created_date.cmp(&a.created_date));

        self.reports = report_overiews;

        Ok(())
    }

    /// Registers a created or imported report by storing its creation date.
    /// This ensures that the creation date is preserved even if the report list
    /// is refreshed from the filesystem, which may not retain the original
    /// creation date metadata.
    pub fn register_report(&mut self, report_path: &Path) {
        let now = SystemTime::now();
        let unix_seconds = system_time_to_unix_seconds(now);
        let display_name = report_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.xml")
            .to_string();

        self.reports.push(ReportOverview {
            path: report_path.to_path_buf(),
            display_name,
            unix_seconds,
            created_date: format_date(unix_seconds),
            report_status: ReportStatus::Draft,
        });
    }

    pub fn set_report_status(
        &mut self,
        report_path: &Path,
        status: ReportStatus,
        diagnostics: &mut Vec<AppDiagnostic>,
    ) {
        if let Some(report) = self
            .reports
            .iter_mut()
            .find(|report| report.path == report_path)
        {
            report.report_status = status;
        }

        let manifest = &self.build_manifest();

        if let Err(err) = report_store::save_report_manifest(manifest) {
            diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to save report manifest: {err}"),
            ));
        }
    }

    fn build_manifest(&self) -> HashMap<String, ReportManifestEntry> {
        self.reports
            .iter()
            .map(|report| {
                let key = report.path.to_string_lossy().to_string();
                (
                    key,
                    ReportManifestEntry {
                        created: report.unix_seconds,
                        status: report.report_status,
                    },
                )
            })
            .collect()
    }
}

fn format_date(unix_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(unix_seconds, 0)
        .map(|utc| utc.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn system_time_to_unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
