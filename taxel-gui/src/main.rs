use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Color32, Panel, Ui, Visuals},
    App, Frame,
};
use egui_extras::{Column, TableBuilder};
use log::debug;
use rfd::FileDialog;
use std::collections::HashSet;
use std::path::Path;
use taxel_gui::{load_xml, FactRow, FactSection, FactTable};

fn main() -> Result<(), anyhow::Error> {
    // TODO: remove hot reloading support for release builds
    dioxus_devtools::connect_subsecond();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };

    debug!("Run app");

    eframe::run_native(
        "Taxel",
        options,
        Box::new(|ctx| {
            ctx.egui_ctx.set_visuals(Visuals::light());
            Ok(Box::new(TaxelApp::new(None, None)))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}

/// Main application struct for the Taxel GUI, managing the state of the app.
pub struct TaxelApp {
    /// The fact table containing the extracted facts from the XBRL instance
    /// document.
    table: Option<FactTable>,
    /// The index of the currently selected section tab in the sidebar.
    selected_tab: usize,
    /// Row indices (within the current section) whose children are collapsed.
    collapsed: HashSet<usize>,
    /// An optional error message to display in the UI if an error occurs during
    /// XML loading or processing.
    error_message: Option<String>,
}

impl TaxelApp {
    pub fn new(table: Option<FactTable>, error_message: Option<String>) -> TaxelApp {
        Self {
            table,
            selected_tab: 0,
            collapsed: HashSet::new(),
            error_message,
        }
    }

    fn import_button(&mut self, ui: &mut Ui) {
        if ui.button("Import XML").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("XML", &["xml"])
                .add_filter("All", &["*"])
                .pick_file()
            {
                self.load_xml(&path);
            }
        }

        ui.separator();

        // Display error if present
        if let Some(err) = &self.error_message {
            ui.colored_label(Color32::RED, err.to_string());

            if ui.button("Dismiss").clicked() {
                self.error_message = None;
            }
        }
    }

    fn load_xml(&mut self, path: &Path) {
        self.selected_tab = 0;
        self.collapsed.clear();
        if let Err(err) = load_xml(&mut self.table, path) {
            self.error_message = Some(format!("{err}"));
        }
    }
}

// Note: dioxus hot reloading support requires the app in main.rs (see
// <https://github.com/DioxusLabs/dioxus/issues/4160>).
impl App for TaxelApp {
    fn ui(&mut self, ctx: &mut Ui, _: &mut Frame) {
        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            if let Some(table) = &self.table {
                draw_sidebar(ctx, table.sections.as_slice(), &mut self.selected_tab);
            }

            CentralPanel::default().show_inside(ctx, |ui| {
                self.import_button(ui);

                ui.heading("eBilanz");

                if let Some(table) = &self.table {
                    if let Some(section) = table.sections.get(self.selected_tab) {
                        draw_table(&section.rows, &mut self.collapsed, ui);
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
            ui.heading("Sections");
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
fn visible_rows<'a>(rows: &'a [FactRow], collapsed: &HashSet<usize>) -> Vec<&'a FactRow> {
    let mut visible = Vec::new();
    let mut hidden_above_depth: Option<usize> = None;

    for row in rows {
        if let Some(hide_depth) = hidden_above_depth {
            if row.depth > hide_depth {
                continue;
            }
            hidden_above_depth = None;
        }
        visible.push(row);
        if row.has_children && collapsed.contains(&visible.len().saturating_sub(1)) {
            hidden_above_depth = Some(row.depth);
        }
    }

    visible
}

/// Draw the fact table in the main panel, showing only the rows that are not
/// collapsed. Handles the toggle logic for expanding/collapsing rows with
/// children.
fn draw_table(rows: &[FactRow], collapsed: &mut HashSet<usize>, ui: &mut Ui) {
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
                ui.label("Concept");
            });
            header.col(|ui| {
                ui.label("Label");
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
                let idx = row.index();
                let fact = visible[idx];
                row.col(|ui| {
                    ui.label(&fact.concept);
                });
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        let triangle_width = 12.0 + ui.spacing().item_spacing.x;
                        let indent = fact.depth as f32 * 24.0;

                        if fact.has_children {
                            ui.add_space(indent);
                            let is_collapsed = collapsed.contains(&idx);

                            if triangle_button(ui, is_collapsed).clicked() {
                                toggle = Some(idx);
                            }
                        } else {
                            ui.add_space(indent + triangle_width);
                        }

                        ui.label(fact.label.as_deref().unwrap_or("-"));
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

    if let Some(idx) = toggle {
        if !collapsed.remove(&idx) {
            collapsed.insert(idx);
        }
    }
}

/// A small clickable triangle button: points right when collapsed, down when expanded.
/// Painted directly rather than using a Unicode glyph to avoid font coverage issues.
fn triangle_button(ui: &mut Ui, collapsed: bool) -> egui::Response {
    let size = egui::vec2(12.0, 12.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let color = ui.visuals().text_color();
        let center = rect.center();
        let r = 4.0_f32;
        let points = if collapsed {
            vec![
                center + egui::vec2(-r * 0.6, -r),
                center + egui::vec2(-r * 0.6, r),
                center + egui::vec2(r, 0.0),
            ]
        } else {
            vec![
                center + egui::vec2(-r, -r * 0.6),
                center + egui::vec2(r, -r * 0.6),
                center + egui::vec2(0.0, r),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(
            points,
            color,
            egui::Stroke::NONE,
        ));
    }

    response
}
