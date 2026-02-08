use anyhow::Context;
use eframe::egui::Visuals;
use log::debug;
use std::{env, fs, path::PathBuf};
use taxel_gui::{read_xbrl, XbrlApp};

fn main() -> Result<(), anyhow::Error> {
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
