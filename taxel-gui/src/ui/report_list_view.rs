use crate::app::ReportOverview;
use chrono::{DateTime, Local, Utc};
use eframe::egui::{
    Align, Button, CursorIcon, Label, Layout, RichText, Sense, TextStyle, TextWrapMode, Ui,
};
use egui_extras::{Column, TableBuilder};
use std::path::PathBuf;

/// Action requested by the user from the report list view.
pub enum ReportListAction {
    /// Open the report at this path.
    Open(PathBuf),
    /// Remove the report entry at this path from the list (and trash its
    /// file, if it still exists).
    Remove(PathBuf),
}

/// Draws the report list view, showing imported reports and allowing the user
/// to select one to open or remove.
pub fn draw_report_list(
    ui: &mut Ui,
    reports: &[ReportOverview],
    loading: bool,
    language: &str,
) -> Option<ReportListAction> {
    let list_width = ui.available_width().min(1200.0);

    ui.vertical_centered(|ui| {
        ui.heading("Your Reports");
        ui.add_space(6.0);
    });

    if reports.is_empty() {
        ui.vertical_centered(|ui| {
            ui.label("Create or import a report to get started.");
        });
        return None;
    }

    ui.vertical_centered(|ui| {
        ui.label(format!("{} report(s)", reports.len()));
    });
    ui.add_space(4.0);

    let mut action = None;
    // Match the current row height: body text + 6px top/bottom padding.
    let row_height = ui.text_style_height(&TextStyle::Body) + 12.0;

    ui.vertical_centered(|ui| {
        ui.set_max_width(list_width);
        ui.spacing_mut().item_spacing.x = 20.0;
        TableBuilder::new(ui)
            .sense(if loading {
                Sense::hover()
            } else {
                Sense::click()
            })
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::exact(28.0))
            .header(row_height, |mut header| {
                header.col(|ui| {
                    ui.add(
                        Label::new("Period")
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(false),
                    );
                });
                header.col(|ui| {
                    ui.add(
                        Label::new("Taxonomy")
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(false),
                    );
                });
                header.col(|ui| {
                    ui.add(
                        Label::new("Version")
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(false),
                    );
                });
                header.col(|ui| {
                    ui.add(
                        Label::new("Added")
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(false),
                    );
                });
                header.col(|ui| {
                    ui.add(
                        Label::new("Changed")
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(false),
                    );
                });
                header.col(|ui| {
                    ui.add(
                        Label::new("Status")
                            .wrap_mode(TextWrapMode::Extend)
                            .selectable(false),
                    );
                });
                // No label; this column only ever holds the per-row remove
                // button, shown on hover.
                header.col(|_ui| {});
            })
            .body(|body| {
                body.rows(row_height, reports.len(), |mut row| {
                    let idx = row.index();
                    let report = &reports[idx];
                    let period = format_period(report);
                    let created_date = format_date(report.created_date);
                    let changed_date = format_datetime(report.changed);
                    let taxonomy_version = report.taxonomy_version.as_deref().unwrap_or("–");
                    let taxonomy_type = report
                        .taxonomy_type
                        .as_ref()
                        .and_then(|t| t.label(language))
                        .unwrap_or("–");

                    row.col(|ui| {
                        paint_hover_bg(ui, loading);
                        ui.add(
                            Label::new(&period)
                                .wrap_mode(TextWrapMode::Extend)
                                .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        paint_hover_bg(ui, loading);
                        ui.add(
                            Label::new(taxonomy_type)
                                .wrap_mode(TextWrapMode::Extend)
                                .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        paint_hover_bg(ui, loading);
                        ui.add(
                            Label::new(taxonomy_version)
                                .wrap_mode(TextWrapMode::Extend)
                                .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        paint_hover_bg(ui, loading);
                        ui.add(
                            Label::new(created_date)
                                .wrap_mode(TextWrapMode::Extend)
                                .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        paint_hover_bg(ui, loading);
                        ui.add(
                            Label::new(&changed_date)
                                .wrap_mode(TextWrapMode::Extend)
                                .selectable(false),
                        );
                    });
                    row.col(|ui| {
                        paint_hover_bg(ui, loading);
                        ui.add(
                            Label::new(report.report_status.as_str())
                                .wrap_mode(TextWrapMode::Extend)
                                .selectable(false),
                        );
                    });

                    let mut remove_clicked = false;

                    // egui_extras paints its own row-wide hover background
                    // per cell independent of `paint_hover_bg`; turn it off
                    // for the trash-icon column so the highlight stops after
                    // "Status".
                    row.set_hovered(false);

                    row.col(|ui| {
                        if !loading && row_hovered(ui) {
                            let response = ui
                                .add(Button::new(RichText::new("\u{1F5D1}")).frame(false))
                                .on_hover_text("Remove from list");

                            if response.clicked() {
                                remove_clicked = true;
                            }
                        }
                    });

                    if !loading {
                        let response = row.response();
                        if response.hovered() {
                            response.ctx.set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if remove_clicked {
                            action = Some(ReportListAction::Remove(report.path.clone()));
                        } else if response.clicked() {
                            action = Some(ReportListAction::Open(report.path.clone()));
                        }
                    }
                });
            });
    });

    action
}

/// Checks whether the pointer is hovering the row that `ui` (a cell within
/// that row) belongs to. All cells in a row share the same Y range, so
/// checking the pointer's Y coordinate alone gives full-row hover detection
/// even though each cell only knows about its own rect.
fn row_hovered(ui: &Ui) -> bool {
    ui.ctx()
        .pointer_hover_pos()
        .is_some_and(|pos| ui.max_rect().y_range().contains(pos.y))
}

/// Paints a hover highlight behind cell content.
fn paint_hover_bg(ui: &mut Ui, loading: bool) {
    if loading || !row_hovered(ui) {
        return;
    }
    ui.painter().rect_filled(
        ui.max_rect(),
        0.0,
        ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.25),
    );
}

fn format_period(report: &ReportOverview) -> String {
    match (&report.start_date, &report.end_date) {
        (Some(start), Some(end)) => format!("{start} – {end}"),
        (None, Some(end)) => format!("– {end}"),
        (Some(start), None) => format!("{start} –"),
        (None, None) => "–".to_string(),
    }
}

fn format_date(unix_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(unix_seconds, 0)
        .map(|utc| utc.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_datetime(unix_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(unix_seconds, 0)
        .map(|utc| {
            utc.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string())
}
