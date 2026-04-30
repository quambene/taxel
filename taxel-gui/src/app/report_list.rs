use crate::{
    app::{AppDiagnostic, DiagnosticCategory},
    domain::{ReportMeta, ReportStatus},
    infrastructure::report_store::{self, ReportManifestEntry},
};
use anyhow::Result;
use std::{
    cmp,
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};
use taxel::TaxonomyType;

/// The `ReportSummary` struct represents a summary of a created or imported
/// report.
#[derive(Clone)]
pub struct ReportOverview {
    pub path: PathBuf,
    /// The display name of the report, typically derived from the file name.
    pub display_name: String,
    /// The creation date as a unix timestamp.
    pub created_date: i64,
    /// The last change date as a unix timestamp.
    pub changed: i64,
    /// The lifecycle status of the report, used for display and filtering in
    /// the UI.
    pub report_status: ReportStatus,
    /// The eBilanz taxonomy type, if known from the manifest.
    pub taxonomy_type: Option<TaxonomyType>,
    /// The eBilanz taxonomy version, if known from the manifest.
    pub taxonomy_version: Option<String>,
    /// The reporting period start date in `YYYYMMDD` format, if known from the
    /// manifest.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
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
            created_date: meta.created,
            changed: meta.changed,
            report_status: meta.status,
            taxonomy_type: meta.taxonomy_type,
            taxonomy_version: meta.taxonomy_version,
            start_date: meta.start_date,
            end_date: meta.end_date,
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
        let report_meta = report_store::load_report_meta()?;

        let mut report_overiews = report_meta
            .into_iter()
            .map(ReportOverview::try_from)
            .collect::<Result<Vec<_>>>()?;

        report_overiews.sort_by_key(|b| cmp::Reverse(b.created_date));

        self.reports = report_overiews;

        Ok(())
    }

    /// Adds a new report or updates an existing one in the list and persists
    /// the updated manifest.
    pub fn upsert_report(
        &mut self,
        report_path: &Path,
        taxonomy_type: Option<TaxonomyType>,
        taxonomy_version: Option<String>,
        start_date: Option<String>,
        end_date: Option<String>,
        diagnostics: &mut Vec<AppDiagnostic>,
    ) {
        let now = SystemTime::now();
        let now = system_time_to_unix_seconds(now);
        let display_name = report_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.xml")
            .to_string();

        if let Some(existing) = self
            .reports
            .iter_mut()
            .find(|report| report.path == report_path)
        {
            existing.taxonomy_type = taxonomy_type;
            existing.taxonomy_version = taxonomy_version;
            existing.start_date = start_date;
            existing.end_date = end_date;
            existing.changed = now;

            let manifest = self.build_manifest();

            if let Err(err) = report_store::save_report_manifest(&manifest) {
                diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::App,
                    format!("Failed to save report manifest: {err}"),
                ));
            }

            return;
        }

        self.reports.push(ReportOverview {
            path: report_path.to_path_buf(),
            display_name,
            created_date: now,
            changed: now,
            report_status: ReportStatus::Draft,
            taxonomy_type,
            taxonomy_version,
            start_date,
            end_date,
        });

        let manifest = self.build_manifest();

        if let Err(err) = report_store::save_report_manifest(&manifest) {
            diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to save report manifest: {err}"),
            ));
        }
    }

    /// Removes a report from the list and persists the updated manifest.
    pub fn remove_report(&mut self, report_path: &Path, diagnostics: &mut Vec<AppDiagnostic>) {
        self.reports.retain(|report| report.path != report_path);

        let manifest = &self.build_manifest();

        if let Err(err) = report_store::save_report_manifest(manifest) {
            diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to save report manifest: {err}"),
            ));
        }
    }

    /// Persists the current report list to the manifest on disk.
    pub fn save(&self, diagnostics: &mut Vec<AppDiagnostic>) {
        let manifest = self.build_manifest();

        if let Err(err) = report_store::save_report_manifest(&manifest) {
            diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to save report manifest: {err}"),
            ));
        }
    }

    /// Updates the `changed` timestamp of a report in memory.
    pub fn set_timestamp(&mut self, report_path: &Path, now: SystemTime) {
        let now = system_time_to_unix_seconds(now);

        if let Some(report) = self
            .reports
            .iter_mut()
            .find(|report| report.path == report_path)
        {
            report.changed = now;
        }
    }

    pub fn set_report_status(&mut self, report_path: &Path, status: ReportStatus) {
        if let Some(report) = self
            .reports
            .iter_mut()
            .find(|report| report.path == report_path)
        {
            report.report_status = status;
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
                        created: report.created_date,
                        changed: report.changed,
                        status: report.report_status,
                        taxonomy_type: report.taxonomy_type.clone(),
                        taxonomy_version: report.taxonomy_version.clone(),
                        start_date: report.start_date.clone(),
                        end_date: report.end_date.clone(),
                    },
                )
            })
            .collect()
    }
}

fn system_time_to_unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
