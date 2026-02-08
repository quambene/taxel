use anyhow::Context as AnyhowContext;
use dioxus_devtools::subsecond;
use eframe::{
    egui::{CentralPanel, Context, Grid, ScrollArea, Visuals},
    App, Frame,
};
use log::debug;
use std::{env, fs, path::PathBuf};
use taxel_gui::{read_xbrl, XbrlTable};

fn main() -> Result<(), anyhow::Error> {
    dioxus_devtools::connect_subsecond();

    let workspace_dir = env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;
    let xml_path = PathBuf::from(&workspace_dir)
        .join("../test_data/taxonomy/v6.5/HandelsbilanzLandwirt_GmbH.xml");

    debug!("Read xml file: {}", xml_path.display());

    let xml = fs::read_to_string(&xml_path)?;

    debug!("Parse xml file: {}", xml_path.display());

    let table = read_xbrl(&xml)?;

    let options = eframe::NativeOptions::default();

    debug!("Run app");

    eframe::run_native(
        "Taxel",
        options,
        Box::new(|ctx| {
            ctx.egui_ctx.set_visuals(Visuals::light());
            Ok(Box::new(XbrlApp::new(table)))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}

pub struct XbrlApp {
    table: XbrlTable,
}

impl XbrlApp {
    pub fn new(table: XbrlTable) -> XbrlApp {
        Self { table }
    }
}

// Note: dioxus hot reloading support requires the app at root level (see
// <https://github.com/DioxusLabs/dioxus/issues/4160>).
impl App for XbrlApp {
    fn update(&mut self, ctx: &Context, _: &mut Frame) {
        subsecond::call(|| {
            CentralPanel::default().show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.heading("eBilanz");

                        Grid::new("xbrl_table").striped(true).show(ui, |ui| {
                            ui.label("Key");
                            ui.label("Context");
                            ui.label("Unit");
                            ui.label("Value");
                            ui.end_row();

                            for row in &mut self.table.rows {
                                ui.label(&row.concept);
                                ui.label(&row.context);
                                ui.label(row.unit.as_deref().unwrap_or("-"));
                                ui.text_edit_singleline(&mut row.value);
                                ui.end_row();
                            }
                        });
                    });
            })
        });
    }
}
