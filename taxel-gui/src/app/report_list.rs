use crate::{
    app::{AppDiagnostic, DiagnosticCategory},
    report_store::{self, ReportManifestEntry, ReportStatus, ReportStore, ReportSummary},
};
use anyhow::Result;
use eframe::egui::{Color32, CursorIcon, Frame, Label, Margin, RichText, Sense, Ui};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Draws the report list view, showing imported reports and allowing the user
/// to select one to open.
pub fn draw_report_list(ui: &mut Ui, reports: &[ReportSummary], loading: bool) -> Option<PathBuf> {
    let list_width = ui.available_width().min(560.0);
    let created_col_width = 120.0;
    let status_col_width = 90.0;
    let column_spacing = ui.spacing().item_spacing.x * 2.0;
    let report_col_width =
        (list_width - created_col_width - status_col_width - column_spacing).max(240.0);

    ui.vertical_centered(|ui| {
        ui.heading("Your Reports");
        ui.add_space(6.0);
    });

    if reports.is_empty() {
        ui.vertical_centered(|ui| {
            ui.label("No imported reports yet. Use Import report to add an XML file.");
        });
        return None;
    }

    ui.vertical_centered(|ui| {
        ui.label(format!("{} report(s)", reports.len()));
    });
    ui.add_space(4.0);

    let mut selected = None;

    ui.vertical_centered(|ui| {
        ui.set_max_width(list_width);
        ui.set_width(list_width);
        ui.horizontal(|ui| {
            ui.add_sized(
                [created_col_width, 18.0],
                Label::new(RichText::new("Created")),
            );
            ui.add_sized(
                [report_col_width, 18.0],
                Label::new(RichText::new("Report")),
            );
            ui.add_sized(
                [status_col_width, 18.0],
                Label::new(RichText::new("Status")),
            );
        });
        ui.separator();

        for (idx, report) in reports.iter().enumerate() {
            let row = Frame::new()
                .fill(Color32::TRANSPARENT)
                .inner_margin(Margin::symmetric(0, 6))
                .show(ui, |ui| {
                    ui.set_width(list_width);
                    ui.horizontal(|ui| {
                        ui.add_sized([created_col_width, 18.0], Label::new(&report.created_date));
                        ui.add_sized(
                            [report_col_width, 18.0],
                            Label::new(&report.display_name).truncate(),
                        );
                        ui.add_sized(
                            [status_col_width, 18.0],
                            Label::new(report.report_status.as_str()),
                        );
                    });
                });

            let response = ui.interact(
                row.response.rect,
                ui.id().with(("report_row", idx)),
                if loading {
                    Sense::hover()
                } else {
                    Sense::click()
                },
            );
            let response = if loading {
                response
            } else {
                response.on_hover_cursor(CursorIcon::PointingHand)
            };

            if response.hovered() && !loading {
                ui.painter().rect_filled(
                    row.response.rect,
                    2.0,
                    ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.25),
                );
            }

            if response.clicked() {
                selected = Some(report.path.clone());
            }
        }
    });

    selected
}

/// The `ReportList` struct manages the list of imported reports, including
/// their metadata and creation dates. Creation timestamps are persisted in a
/// JSON manifest in the reports directory.
pub(super) struct ReportList {
    reports: Vec<ReportSummary>,
    creation_dates: HashMap<String, i64>,
    report_statuses: HashMap<String, ReportStatus>,
}

impl ReportList {
    pub(super) fn new() -> Self {
        Self {
            reports: Vec::new(),
            creation_dates: HashMap::new(),
            report_statuses: HashMap::new(),
        }
    }

    pub(super) fn refresh(&mut self) -> Result<()> {
        let mut reports = ReportStore::load_reports()?;

        let manifest = report_store::load_report_manifest()?;
        self.creation_dates = manifest
            .iter()
            .map(|(path, entry)| (path.clone(), entry.created))
            .collect();
        self.report_statuses = manifest
            .into_iter()
            .map(|(path, entry)| (path, entry.status))
            .collect();

        self.apply_creation_dates(&mut reports.report_list);

        report_store::save_report_manifest(&self.build_manifest())?;

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
        self.creation_dates
            .insert(report_path.clone(), unix_seconds);
        self.report_statuses
            .insert(report_path, ReportStatus::Draft);
    }

    pub(super) fn report_status(&self, report_path: &Path) -> Option<ReportStatus> {
        let report_path = report_path.to_string_lossy().to_string();
        self.report_statuses.get(&report_path).copied()
    }

    pub(super) fn set_report_status(
        &mut self,
        report_path: &Path,
        status: ReportStatus,
        diagnostics: &mut Vec<AppDiagnostic>,
    ) {
        let key = report_path.to_string_lossy().to_string();
        self.report_statuses.insert(key.clone(), status);

        if let Some(report) = self
            .reports
            .iter_mut()
            .find(|report| report.path.to_string_lossy() == key)
        {
            report.report_status = status;
        }

        if let Err(err) = report_store::save_report_manifest(&self.build_manifest()) {
            diagnostics.push(AppDiagnostic::new_error(
                DiagnosticCategory::App,
                format!("Failed to save report manifest: {err}"),
            ));
        }
    }

    pub(super) fn reports(&self) -> &[ReportSummary] {
        &self.reports
    }

    fn apply_creation_dates(&mut self, reports: &mut [ReportSummary]) {
        let mut existing_paths = HashSet::new();

        for report in reports.iter_mut() {
            let key = report.path.to_string_lossy().to_string();
            existing_paths.insert(key.clone());

            let created = *self.creation_dates.entry(key).or_insert(report.created);

            let status = *self
                .report_statuses
                .entry(report.path.to_string_lossy().to_string())
                .or_insert(ReportStatus::Draft);

            report.created = created;
            report.created_date = report_store::format_date(created);
            report.report_status = status;
        }

        self.creation_dates
            .retain(|path, _| existing_paths.contains(path));
        self.report_statuses
            .retain(|path, _| existing_paths.contains(path));

        reports.sort_by(|a, b| b.created.cmp(&a.created));
    }

    fn build_manifest(&self) -> HashMap<String, ReportManifestEntry> {
        self.creation_dates
            .iter()
            .map(|(path, created)| {
                let status = self
                    .report_statuses
                    .get(path)
                    .copied()
                    .unwrap_or(ReportStatus::Draft);

                (
                    path.clone(),
                    ReportManifestEntry {
                        created: *created,
                        status,
                    },
                )
            })
            .collect()
    }
}
