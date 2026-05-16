use crate::app::ReportOverview;
use chrono::{DateTime, Local, Utc};
use eframe::egui::{Align, CursorIcon, Label, Layout, Sense, TextStyle, TextWrapMode, Ui};
use egui_extras::{Column, TableBuilder};
use std::path::PathBuf;

/// Draws the report list view, showing imported reports and allowing the user
/// to select one to open.
pub fn draw_report_list(
    ui: &mut Ui,
    reports: &[ReportOverview],
    loading: bool,
    language: &str,
) -> Option<PathBuf> {
    let list_width = ui.available_width().min(1200.0);

    ui.vertical_centered(|ui| {
        ui.heading("Your Reports");
        ui.add_space(6.0);
    });

    if reports.is_empty() {
        ui.vertical_centered(|ui| {
            ui.label("No reports imported or created yet.");
        });
        return None;
    }

    ui.vertical_centered(|ui| {
        ui.label(format!("{} report(s)", reports.len()));
    });
    ui.add_space(4.0);

    let mut selected = None;
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

                    if !loading {
                        let response = row.response();
                        if response.hovered() {
                            response.ctx.set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if response.clicked() {
                            selected = Some(report.path.clone());
                        }
                    }
                });
            });
    });

    selected
}

/// Paints a hover highlight behind cell content. All cells in a row share the
/// same Y range, so checking the pointer's Y coordinate gives full-row hover
/// highlighting even though each cell paints its own background.
fn paint_hover_bg(ui: &mut Ui, loading: bool) {
    if loading {
        return;
    }
    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if ui.max_rect().y_range().contains(pos.y) {
            ui.painter().rect_filled(
                ui.max_rect(),
                0.0,
                ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.25),
            );
        }
    }
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
