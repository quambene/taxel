use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Color32, Ui, Visuals},
    App, Frame,
};
use egui_extras::{Column, TableBuilder};
use log::debug;
use rfd::FileDialog;
use std::path::Path;
use taxel_gui::{load_xml, FactTable, TableRow};

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

pub struct TaxelApp {
    table: Option<FactTable>,
    error_message: Option<String>,
}

impl TaxelApp {
    pub fn new(table: Option<FactTable>, error_message: Option<String>) -> TaxelApp {
        Self {
            table,
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
            CentralPanel::default().show_inside(ctx, |ui| {
                self.import_button(ui);

                ui.heading("eBilanz");

                if let Some(table) = &self.table {
                    draw_table(&table.rows, ui);
                }
            })
        });
    }
}

fn draw_table(rows: &[TableRow], ui: &mut Ui) {
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;

    TableBuilder::new(ui)
        .resizable(true)
        .striped(true)
        .column(Column::initial(200.0).clip(true))
        .column(Column::initial(200.0).clip(true))
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
            body.rows(row_height, rows.len(), |mut row| {
                let idx = row.index();
                row.col(|ui| {
                    ui.label(&rows[idx].concept);
                });
                row.col(|ui| {
                    ui.label(rows[idx].label.as_deref().unwrap_or("-"));
                });
                row.col(|ui| {
                    ui.label(&rows[idx].context);
                });
                row.col(|ui| {
                    ui.label(rows[idx].unit.as_deref().unwrap_or("-"));
                });
                row.col(|ui| {
                    ui.label(&rows[idx].value);
                });
            });
        });
}
