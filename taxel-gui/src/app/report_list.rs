use anyhow::Result;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::SystemTime,
};
use taxel_gui::report_store::{self, ReportStore, ReportSummary};

/// The `ReportList` struct manages the list of imported reports, including
/// their metadata and creation dates. Creation timestamps are persisted in a
/// JSON manifest in the reports directory.
pub(super) struct ReportList {
    reports: Vec<ReportSummary>,
    creation_dates: HashMap<String, i64>,
}

impl ReportList {
    pub(super) fn new() -> Self {
        Self {
            reports: Vec::new(),
            creation_dates: HashMap::new(),
        }
    }

    pub(super) fn refresh(&mut self) -> Result<()> {
        let mut reports = ReportStore::load_reports()?;

        self.creation_dates = report_store::load_creation_manifest()?;

        self.apply_creation_dates(&mut reports.report_list);

        report_store::save_creation_manifest(&self.creation_dates)?;

        self.reports = reports.report_list;

        Ok(())
    }

    /// Registers a newly imported report by storing its creation date in the
    /// `creation_dates` HashMap. This ensures that the creation date is
    /// preserved even if the report list is refreshed from the filesystem,
    /// which may not retain the original creation date metadata.
    pub(super) fn register_imported_report(&mut self, report_path: &Path) {
        let unix_seconds = report_store::system_time_to_unix_seconds(SystemTime::now());
        let report_path = report_path.to_string_lossy().to_string();
        self.creation_dates.insert(report_path, unix_seconds);
    }

    pub(super) fn reports(&self) -> &[ReportSummary] {
        &self.reports
    }

    fn apply_creation_dates(&mut self, reports: &mut [ReportSummary]) {
        let mut existing_paths = HashSet::new();

        for report in reports.iter_mut() {
            let key = report.path.to_string_lossy().to_string();
            existing_paths.insert(key.clone());

            let created_unix = *self
                .creation_dates
                .entry(key)
                .or_insert(report.created_unix);

            report.created_unix = created_unix;
            report.created_date = report_store::format_date(created_unix);
        }

        self.creation_dates
            .retain(|path, _| existing_paths.contains(path));

        reports.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));
    }
}
