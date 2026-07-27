use crate::{
    domain::{FactRow, FactValue},
    ui::widgets,
};
use eframe::egui::{self, Align, ComboBox, TextStyle, Ui};
use egui_extras::{Column, TableBuilder};
use std::collections::HashSet;
use taxel::REPORT_ELEMENT_PREFIX;

/// Draw the fact table in the main panel, showing only the rows that are not
/// collapsed. Handles the toggle logic for expanding/collapsing rows with
/// children. Returns the raw index of a row that the user attempted to uncheck
/// (if any).
#[allow(clippy::too_many_arguments)]
pub fn draw_table(
    rows: &mut [FactRow],
    collapsed: &mut HashSet<usize>,
    lang: &str,
    scroll_to_row: Option<usize>,
    highlight_row: Option<usize>,
    editing: bool,
    ui: &mut Ui,
    show_required_only: bool,
    show_filled_only: bool,
) -> Option<usize> {
    let row_height = ui.text_style_height(&TextStyle::Body) + ui.spacing().item_spacing.y;
    let visible = visible_rows(rows, collapsed, show_required_only, show_filled_only);
    let mut toggle: Option<usize> = None;
    let mut pending_report_element_uncheck: Option<usize> = None;

    let mut builder = TableBuilder::new(ui)
        .resizable(true)
        .striped(true)
        .column(Column::initial(250.0).clip(true))
        .column(Column::initial(500.0).clip(true))
        .column(Column::initial(250.0).clip(true))
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
                ui.label("Value");
            });
            header.col(|ui| {
                ui.label("Unit");
            });
            header.col(|ui| {
                ui.label("Context");
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
                    ui.label(&rows[raw_idx].concept)
                        .on_hover_text(describe_flags(&rows[raw_idx]));
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
                    let row_editing =
                        editing && !rows[raw_idx].is_abstract && !rows[raw_idx].is_calculated;

                    match &mut rows[raw_idx].value {
                        FactValue::Text(text) => {
                            // Tuples are structural and shouldn't be edited as
                            // text value.
                            if row_editing && !rows[raw_idx].is_tuple {
                                ui.text_edit_singleline(text);
                            } else {
                                ui.label(text.as_str());
                            }
                        }
                        FactValue::Checkbox(checked) => {
                            if row_editing {
                                let was_checked = *checked;
                                let response = ui.checkbox(checked, "");

                                if response.changed()
                                    && was_checked
                                    && !*checked
                                    && rows[raw_idx].concept.starts_with(REPORT_ELEMENT_PREFIX)
                                {
                                    // Revert immediately; the app will ask for explicit
                                    // confirmation before actually unchecking.
                                    *checked = true;
                                    pending_report_element_uncheck = Some(raw_idx);
                                }
                            } else {
                                ui.add_enabled(false, egui::Checkbox::new(checked, ""));
                            }
                        }
                        FactValue::Dropdown { selected, options } => {
                            let display_label = options
                                .iter()
                                .find(|(k, _)| k == selected)
                                .and_then(|(_, labels)| labels.get(lang))
                                .map(String::as_str)
                                .unwrap_or(if selected.is_empty() {
                                    "—"
                                } else {
                                    selected.as_str()
                                });

                            if row_editing {
                                ComboBox::from_id_salt(raw_idx)
                                    .selected_text(display_label)
                                    .show_ui(ui, |ui| {
                                        for (key, labels) in options.iter() {
                                            let label = labels
                                                .get(lang)
                                                .map(String::as_str)
                                                .unwrap_or(key.as_str());

                                            ui.selectable_value(selected, key.clone(), label);
                                        }
                                    });
                            } else {
                                ui.label(display_label);
                            }
                        }
                        FactValue::BooleanDropdown(selected) => {
                            let display = match selected.as_str() {
                                "true" => "true",
                                "false" => "false",
                                _ => "—",
                            };

                            if row_editing {
                                ComboBox::from_id_salt(raw_idx)
                                    .selected_text(display)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(selected, String::new(), "—");
                                        ui.selectable_value(selected, "true".to_owned(), "true");
                                        ui.selectable_value(selected, "false".to_owned(), "false");
                                    });
                            } else {
                                ui.label(display);
                            }
                        }
                        FactValue::Decimal { raw, value } => {
                            if row_editing && !rows[raw_idx].is_tuple {
                                let valid = raw.is_empty()
                                    || value.as_ref().is_some_and(|decimal| decimal.scale() == 2);
                                let color = if valid {
                                    ui.visuals().text_color()
                                } else {
                                    egui::Color32::RED
                                };
                                let resp =
                                    ui.add(egui::TextEdit::singleline(raw).text_color(color));

                                if resp.changed() {
                                    // Filter to valid decimal characters.
                                    let mut out = String::new();
                                    let mut chars = raw.chars().peekable();

                                    if chars.peek() == Some(&'-') {
                                        out.push('-');
                                        chars.next();
                                    }

                                    let mut seen_dot = false;

                                    for char in chars {
                                        if char == '.' && !seen_dot {
                                            seen_dot = true;
                                            out.push(char);
                                        } else if char.is_ascii_digit() {
                                            out.push(char);
                                        }
                                    }
                                    *raw = out;
                                    *value = raw.parse().ok();
                                }
                            } else {
                                ui.label(raw.as_str());
                            }
                        }
                        FactValue::Integer(text) => {
                            if row_editing && !rows[raw_idx].is_tuple {
                                let resp = ui.text_edit_singleline(text);

                                if resp.changed() {
                                    let mut out = String::new();
                                    let mut chars = text.chars().peekable();
                                    if chars.peek() == Some(&'-') {
                                        out.push('-');
                                        chars.next();
                                    }
                                    for char in chars {
                                        if char.is_ascii_digit() {
                                            out.push(char);
                                        }
                                    }
                                    *text = out;
                                }
                            } else {
                                ui.label(text.as_str());
                            }
                        }
                        FactValue::Date { raw, value } => {
                            if row_editing && !rows[raw_idx].is_tuple {
                                let valid = raw.is_empty() || value.is_some();
                                let color = if valid {
                                    ui.visuals().text_color()
                                } else {
                                    egui::Color32::RED
                                };
                                let resp =
                                    ui.add(egui::TextEdit::singleline(raw).text_color(color));

                                if resp.changed() {
                                    *value =
                                        chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok();
                                }
                            } else {
                                ui.label(raw.as_str());
                            }
                        }
                    }
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
                    ui.label(&rows[raw_idx].context);
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

    pending_report_element_uncheck
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
/// operations. When `show_required_only` is true, only rows with
/// `is_required = true` are included. When `show_filled_only` is true, only
/// rows with non-empty values are included.
pub fn visible_rows(
    rows: &[FactRow],
    collapsed: &HashSet<usize>,
    show_required_only: bool,
    show_filled_only: bool,
) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut hidden_above_depth: Option<usize> = None;

    for (raw_idx, row) in rows.iter().enumerate() {
        if let Some(hide_depth) = hidden_above_depth {
            if row.depth > hide_depth {
                continue;
            }
            hidden_above_depth = None;
        }

        if show_required_only && !row.is_required {
            if row.has_children && collapsed.contains(&raw_idx) {
                hidden_above_depth = Some(row.depth);
            }
            continue;
        }

        if show_filled_only && !is_filled(row) {
            if row.has_children && collapsed.contains(&raw_idx) {
                hidden_above_depth = Some(row.depth);
            }
            continue;
        }

        visible.push(raw_idx);
        if row.has_children && collapsed.contains(&raw_idx) {
            hidden_above_depth = Some(row.depth);
        }
    }

    visible
}

/// Builds the hover-tooltip text for the ID column, listing the
/// taxonomy-derived flags that aren't otherwise visible in the table.
fn describe_flags(row: &FactRow) -> String {
    format!(
        "Abstract: {}\nRequired: {}\nTuple: {}\nCalculated: {}",
        yes_no(row.is_abstract),
        yes_no(row.is_required),
        yes_no(row.is_tuple),
        yes_no(row.is_calculated),
    )
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn is_filled(row: &FactRow) -> bool {
    match &row.value {
        FactValue::Text(text) => !text.trim().is_empty(),
        FactValue::Checkbox(checked) => *checked,
        FactValue::Dropdown { selected, .. } => !selected.is_empty(),
        FactValue::BooleanDropdown(s) | FactValue::Integer(s) => !s.is_empty(),
        FactValue::Decimal { raw, .. } | FactValue::Date { raw, .. } => !raw.is_empty(),
    }
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
