use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Color32, Panel, Ui},
    App, Frame,
};
use egui_extras::{Column, TableBuilder};
use rfd::FileDialog;
use std::collections::HashSet;
use std::path::Path;
use taxel_gui::{load_xml, FactRow, FactSection, FactTable};

use crate::widgets;

/// Per-section UI state (collapse state and depth filter).
#[derive(Default)]
struct SectionState {
    /// Row indices whose children are collapsed.
    collapsed: HashSet<usize>,
    /// Maximum depth to display. None means show all depths.
    max_depth: Option<usize>,
}

/// Main application struct for the Taxel GUI, managing the state of the app.
pub struct TaxelApp {
    /// The fact table containing the extracted facts from the XBRL instance
    /// document.
    table: Option<FactTable>,
    /// The index of the currently selected section tab in the sidebar.
    selected_tab: usize,
    /// Per-section UI state, indexed analogous to `table.sections`.
    section_states: Vec<SectionState>,
    /// The currently selected language for labels (e.g. "en", "de").
    lang: String,
    /// An optional error message to display in the UI if an error occurs during
    /// XML loading or processing.
    error_message: Option<String>,
}

impl TaxelApp {
    pub fn new(table: Option<FactTable>, error_message: Option<String>) -> TaxelApp {
        let section_states = table
            .as_ref()
            .map(|t| t.sections.iter().map(|_| SectionState::default()).collect())
            .unwrap_or_default();
        Self {
            table,
            selected_tab: 0,
            section_states,
            lang: "en".to_string(),
            error_message,
        }
    }

    fn draw_header(&mut self, ui: &mut Ui) {
        let mut lang_changed = false;

        ui.horizontal_centered(|ui| {
            if ui.button("Import XML").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("XML", &["xml"])
                    .add_filter("All", &["*"])
                    .pick_file()
                {
                    self.load_xml(&path);
                }
            }

            if self.table.is_some() && ui.button("Clear table").clicked() {
                self.table = None;
            }

            if let Some(err) = &self.error_message {
                ui.separator();
                ui.colored_label(Color32::RED, err.to_string());
                if ui.button("Dismiss").clicked() {
                    self.error_message = None;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                lang_changed = draw_language_toolbar(ui, &mut self.lang);
            });
        });
    }

    fn load_xml(&mut self, path: &Path) {
        self.selected_tab = 0;
        self.table = None;

        if let Err(err) = load_xml(&mut self.table, path) {
            self.error_message = Some(format!("{err}"));
        }

        self.section_states = self
            .table
            .as_ref()
            .map(|table| {
                table
                    .sections
                    .iter()
                    .map(|_| SectionState::default())
                    .collect()
            })
            .unwrap_or_default();
    }
}

// Note: dioxus hot reloading support requires the app in main.rs (see
// <https://github.com/DioxusLabs/dioxus/issues/4160>).
impl App for TaxelApp {
    fn ui(&mut self, ctx: &mut Ui, _: &mut Frame) {
        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            Panel::top("header").min_size(32.0).show_inside(ctx, |ui| {
                self.draw_header(ui);
            });

            if let Some(table) = &self.table {
                draw_sidebar(ctx, table.sections.as_slice(), &mut self.selected_tab);
            }

            let lang = self.lang.clone();

            CentralPanel::default().show_inside(ctx, |ui| {
                if let Some(table) = &self.table {
                    if let Some(section) = table.sections.get(self.selected_tab) {
                        let max_depth =
                            section.rows.iter().map(|row| row.depth).max().unwrap_or(0) + 1;
                        let state = &mut self.section_states[self.selected_tab];

                        draw_level_toolbar(
                            ui,
                            max_depth,
                            &mut state.max_depth,
                            &mut state.collapsed,
                            &section.rows,
                        );

                        draw_table(&section.rows, &mut state.collapsed, &lang, ui);
                    }
                }
            })
        });
    }
}

/// Draw the sidebar panel containing the list of sections. Allows the user to
/// select a section to view its facts in the main table.
fn draw_sidebar(ctx: &mut Ui, sections: &[FactSection], selected: &mut usize) {
    Panel::left("sections_panel")
        .resizable(true)
        .default_size(200.0)
        .show_inside(ctx, |ui| {
            // Match the spacing above the first section in the main table for
            // visual alignment.
            ui.add_space(7.0);
            ui.label("Reports");
            ui.add_space(2.0);

            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, section) in sections.iter().enumerate() {
                    let title = section.role.rsplit('/').next().unwrap_or(&section.role);
                    ui.selectable_value(selected, i, title);
                }
            });
        });
}

/// Determines the visible rows in the fact table based on the current collapsed
/// state.
///
/// Compute which visible-list indices should be collapsed to show only rows up
/// to `max_depth` levels. Mirrors the same traversal logic as `visible_rows` so
/// that indices are stable. Returns the raw indices (into `rows`) that should
/// be collapsed to show only `max_depth` levels. Uses raw indices so the set
/// remains stable regardless of expand/collapse state.
fn collapsed_at_depth(rows: &[FactRow], max_depth: usize) -> HashSet<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.has_children && row.depth + 1 >= max_depth)
        .map(|(i, _)| i)
        .collect()
}

/// Returns the visible rows as `(raw_index, &FactRow)` pairs.
/// `collapsed` stores raw indices (positions in `rows`), which are stable
/// across expand/collapse operations.
fn visible_rows<'a>(rows: &'a [FactRow], collapsed: &HashSet<usize>) -> Vec<(usize, &'a FactRow)> {
    let mut visible = Vec::new();
    let mut hidden_above_depth: Option<usize> = None;

    for (raw_idx, row) in rows.iter().enumerate() {
        if let Some(hide_depth) = hidden_above_depth {
            if row.depth > hide_depth {
                continue;
            }
            hidden_above_depth = None;
        }
        visible.push((raw_idx, row));
        if row.has_children && collapsed.contains(&raw_idx) {
            hidden_above_depth = Some(row.depth);
        }
    }

    visible
}

/// Draw the fact table in the main panel, showing only the rows that are not
/// collapsed. Handles the toggle logic for expanding/collapsing rows with
/// children.
fn draw_level_toolbar(
    ui: &mut Ui,
    max_available: usize,
    max_depth: &mut Option<usize>,
    collapsed: &mut HashSet<usize>,
    rows: &[FactRow],
) {
    ui.horizontal(|ui| {
        ui.label("Level:");

        if ui.selectable_label(max_depth.is_none(), "All").clicked() {
            *max_depth = None;
            collapsed.clear();
        }

        for depth in 1..=max_available {
            if ui
                .selectable_label(*max_depth == Some(depth), depth.to_string())
                .clicked()
            {
                *max_depth = Some(depth);
                *collapsed = collapsed_at_depth(rows, depth);
            }
        }
    });

    ui.separator();
}

/// Draw the fact table in the main panel, showing only the rows that are not
/// collapsed. Handles the toggle logic for expanding/collapsing rows with
/// children.
fn draw_table(rows: &[FactRow], collapsed: &mut HashSet<usize>, lang: &str, ui: &mut Ui) {
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    let visible = visible_rows(rows, collapsed);
    let mut toggle: Option<usize> = None;

    TableBuilder::new(ui)
        .resizable(true)
        .striped(true)
        .column(Column::initial(500.0).clip(true))
        .column(Column::initial(500.0).clip(true))
        .column(Column::initial(120.0).clip(true))
        .column(Column::initial(60.0).clip(true))
        .column(Column::remainder().clip(true))
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
                let (raw_idx, fact) = visible[row.index()];
                row.col(|ui| {
                    ui.label(&fact.concept);
                });
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        let triangle_width = 12.0 + ui.spacing().item_spacing.x;
                        let indent = fact.depth as f32 * 24.0;

                        if fact.has_children {
                            ui.add_space(indent);
                            let is_collapsed = collapsed.contains(&raw_idx);

                            if widgets::triangle_button(ui, is_collapsed).clicked() {
                                toggle = Some(raw_idx);
                            }
                        } else {
                            ui.add_space(indent + triangle_width);
                        }

                        ui.label(
                            fact.labels
                                .get(lang)
                                .map(|label| label.as_str())
                                .unwrap_or("-"),
                        );
                    });
                });
                row.col(|ui| {
                    ui.label(&fact.context);
                });
                row.col(|ui| {
                    ui.label(fact.unit.as_deref().unwrap_or("-"));
                });
                row.col(|ui| {
                    ui.label(&fact.value);
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

/// Draw the language selector tabs ("en", "de"). Returns true if the language was changed.
fn draw_language_toolbar(ui: &mut Ui, selected_lang: &mut String) -> bool {
    let mut changed = false;

    for lang in ["de", "en"] {
        if ui.selectable_label(*selected_lang == lang, lang).clicked() && *selected_lang != lang {
            *selected_lang = lang.to_string();
            changed = true;
        }
    }
    changed
}
