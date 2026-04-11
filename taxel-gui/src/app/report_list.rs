use anyhow::Result;
use eframe::Storage;
use std::{collections::HashMap, path::Path, time::SystemTime};
use taxel_gui::report_store::{self, ReportStore, ReportSummary};

const CREATION_DATES_STORAGE_KEY: &str = "report_creation_dates";

/// The `ReportList` struct manages the list of imported reports, including
/// their metadata and creation dates. It provides methods to load and save this
/// information from persistent storage, refresh the list from the filesystem,
/// and register newly imported reports. The creation dates are stored in a
/// separate HashMap keyed by the report path to ensure they persist across
/// refreshes and are not lost when the list of reports is reloaded from the
/// filesystem.
pub(super) struct ReportList {
    reports: Vec<ReportSummary>,
    creation_dates: HashMap<String, i64>,
}

impl ReportList {
    pub(super) fn load_from_storage(storage: Option<&dyn Storage>) -> Self {
        let creation_dates = storage
            .and_then(|storage| {
                eframe::get_value::<HashMap<String, i64>>(storage, CREATION_DATES_STORAGE_KEY)
            })
            .unwrap_or_default();

        Self {
            reports: Vec::new(),
            creation_dates,
        }
    }

    pub(super) fn save_to_storage(&self, storage: &mut dyn Storage) {
        eframe::set_value(storage, CREATION_DATES_STORAGE_KEY, &self.creation_dates);
    }

    pub(super) fn refresh(&mut self) -> Result<()> {
        let mut reports = ReportStore::load_reports()?;
        self.apply_creation_dates(&mut reports.report_list);
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
        for report in reports.iter_mut() {
            let key = report.path.to_string_lossy().to_string();
            let created_unix = *self
                .creation_dates
                .entry(key)
                .or_insert(report.created_unix);

            report.created_unix = created_unix;
            report.created_date = report_store::format_date(created_unix);
        }

        reports.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));
    }
}
