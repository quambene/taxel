use crate::app::ReportOverview;
use eframe::egui::{Color32, CursorIcon, Frame, Label, Margin, RichText, Sense, Ui};
use std::path::PathBuf;

/// Draws the report list view, showing imported reports and allowing the user
/// to select one to open.
pub fn draw_report_list(ui: &mut Ui, reports: &[ReportOverview], loading: bool) -> Option<PathBuf> {
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
