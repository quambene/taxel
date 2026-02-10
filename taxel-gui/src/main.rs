use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Color32, Context, Grid, ScrollArea, Ui, Visuals},
    App, Frame,
};
use log::debug;
use rfd::FileDialog;
use std::{fs, path::PathBuf};
use taxel_gui::{read_xbrl, TableRow, XbrlTable};

fn main() -> Result<(), anyhow::Error> {
    // TODO: remove hot reloading support for release builds
    dioxus_devtools::connect_subsecond();

    let options = eframe::NativeOptions::default();

    debug!("Run app");

    eframe::run_native(
        "Taxel",
        options,
        Box::new(|ctx| {
            ctx.egui_ctx.set_visuals(Visuals::light());
            Ok(Box::new(XbrlApp::new(None, None)))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}

pub struct XbrlApp {
    table: Option<XbrlTable>,
    error_message: Option<String>,
}

impl XbrlApp {
    pub fn new(table: Option<XbrlTable>, error_message: Option<String>) -> XbrlApp {
        Self {
            table,
            error_message,
        }
    }

    fn import_button(&mut self, ui: &mut Ui) {
        if ui.button("📁 Import XML").clicked() {
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
            ui.colored_label(Color32::RED, format!("{err}"));
            if ui.button("Dismiss").clicked() {
                self.error_message = None;
            }
        }
    }

    fn load_xml(&mut self, path: &PathBuf) {
        debug!("Read xml file: {}", path.display());

        match fs::read_to_string(path) {
            Ok(xml) => {
                debug!("Parse xml file: {}", path.display());

                match read_xbrl(&xml) {
                    Ok(table) => {
                        self.table = Some(table);
                        self.error_message = None;
                    }
                    Err(err) => {
                        self.error_message = Some(format!("Failed to parse XML: {err}",));
                    }
                }
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to read file: {err}"));
            }
        }
    }
}

// Note: dioxus hot reloading support requires the app at root level (see
// <https://github.com/DioxusLabs/dioxus/issues/4160>).
impl App for XbrlApp {
    fn update(&mut self, ctx: &Context, _: &mut Frame) {
        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            CentralPanel::default().show(ctx, |ui| {
                self.import_button(ui);

                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.heading("eBilanz");

                        if let Some(table) = &mut self.table {
                            draw_xbrl_table(&mut table.rows, ui);
                        }
                    });
            })
        });
    }
}

fn draw_xbrl_table(rows: &mut [TableRow], ui: &mut Ui) {
    Grid::new("xbrl_table").show(ui, |ui| {
        ui.label("Key");
        ui.label("Context");
        ui.label("Unit");
        ui.label("Value");
        ui.end_row();

        for row in rows {
            ui.label(&row.concept);
            ui.label(&row.context);
            ui.label(row.unit.as_deref().unwrap_or("-"));

            egui::Frame::new()
                .inner_margin(egui::Margin::ZERO)
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(600.0, ui.spacing().interact_size.y),
                        egui::Layout::left_to_right(egui::Align::Min),
                        |ui| {
                            ui.add(egui::TextEdit::singleline(&mut row.value));
                        },
                    );
                });
            ui.end_row();
        }
    });
}
