use crate::{domain::FactRow, ui::widgets};
use eframe::egui::{Align, TextStyle, Ui};
use egui_extras::{Column, TableBuilder};
use std::collections::HashSet;

/// Draw the fact table in the main panel, showing only the rows that are not
/// collapsed. Handles the toggle logic for expanding/collapsing rows with
/// children.
pub fn draw_table(
    rows: &mut [FactRow],
    collapsed: &mut HashSet<usize>,
    lang: &str,
    scroll_to_row: Option<usize>,
    highlight_row: Option<usize>,
    editing: bool,
    ui: &mut Ui,
) {
    let row_height = ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y;
    let visible = visible_rows(rows, collapsed);
    let mut toggle: Option<usize> = None;

    let mut builder = TableBuilder::new(ui)
        .resizable(true)
        .striped(true)
        .column(Column::initial(250.0).clip(true))
        .column(Column::initial(500.0).clip(true))
        .column(Column::initial(120.0).clip(true))
        .column(Column::initial(60.0).clip(true))
        .column(Column::remainder().clip(true));

    if let Some(row) = scroll_to_row {
        builder = builder.scroll_to_row(row, Some(Align::Center));
    }

    builder
        .header(row_height, |mut header| {
            header.col(|ui| {
                ui.label("ID");
            });
            header.col(|ui| {
                ui.label("Name");
            });
            header.col(|ui| {
                ui.label("Context");
            });
            header.col(|ui| {
                ui.label("Unit");
            });
            header.col(|ui| {
                ui.label("Value");
            });
        })
        .body(|body| {
            body.rows(row_height, visible.len(), |mut row| {
                let raw_idx = visible[row.index()];
                let is_highlighted = highlight_row == Some(raw_idx);
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.label(&rows[raw_idx].concept);
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.horizontal(|ui| {
                        let triangle_width = 12.0 + ui.spacing().item_spacing.x;
                        let indent = rows[raw_idx].depth as f32 * 24.0;

                        if rows[raw_idx].has_children {
                            ui.add_space(indent);
                            let is_collapsed = collapsed.contains(&raw_idx);

                            if widgets::triangle_button(ui, is_collapsed).clicked() {
                                toggle = Some(raw_idx);
                            }
                        } else {
                            ui.add_space(indent + triangle_width);
                        }

                        ui.label(
                            rows[raw_idx]
                                .labels
                                .get(lang)
                                .map(|label| label.as_str())
                                .unwrap_or("-"),
                        );
                    });
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.label(&rows[raw_idx].context);
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.label(rows[raw_idx].unit.as_deref().unwrap_or("-"));
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    if editing {
                        ui.text_edit_singleline(&mut rows[raw_idx].value);
                    } else {
                        ui.label(&rows[raw_idx].value);
                    }
                });
            });
        });

    if let Some(raw_idx) = toggle {
        if collapsed.contains(&raw_idx) {
            // Expanding: reveal one level by collapsing direct children that have children.
            collapsed.remove(&raw_idx);
            let parent_depth = rows[raw_idx].depth;
            for (i, row) in rows[raw_idx + 1..].iter().enumerate() {
                if row.depth <= parent_depth {
                    break;
                }
                if row.depth == parent_depth + 1 && row.has_children {
                    collapsed.insert(raw_idx + 1 + i);
                }
            }
        } else {
            collapsed.insert(raw_idx);
        }
    }
}

/// Compute which visible-list indices should be collapsed to show only rows up
/// to `max_depth` levels.
pub fn collapsed_at_depth(rows: &[FactRow], max_depth: usize) -> HashSet<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.has_children && row.depth + 1 >= max_depth)
        .map(|(i, _)| i)
        .collect()
}

/// Returns the raw indices of visible rows (positions in `rows`).
/// `collapsed` stores raw indices, which are stable across expand/collapse
/// operations.
pub fn visible_rows(rows: &[FactRow], collapsed: &HashSet<usize>) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut hidden_above_depth: Option<usize> = None;

    for (raw_idx, row) in rows.iter().enumerate() {
        if let Some(hide_depth) = hidden_above_depth {
            if row.depth > hide_depth {
                continue;
            }
            hidden_above_depth = None;
        }
        visible.push(raw_idx);
        if row.has_children && collapsed.contains(&raw_idx) {
            hidden_above_depth = Some(row.depth);
        }
    }

    visible
}

/// Ensures that a given row is visible by expanding (uncollapsing) all its
/// ancestors in the tree.
pub fn ensure_row_visible(row_idx: usize, rows: &[FactRow], collapsed: &mut HashSet<usize>) {
    let target_depth = rows[row_idx].depth;
    let mut depth = target_depth;
    for i in (0..row_idx).rev() {
        if rows[i].depth < depth && rows[i].has_children {
            collapsed.remove(&i);
            depth = rows[i].depth;
            if depth == 0 {
                break;
            }
        }
    }
}
