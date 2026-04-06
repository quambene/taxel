mod app;
mod widgets;

use crate::app::TaxelApp;
use eframe::egui::{ViewportBuilder, Visuals};
use log::debug;

fn main() -> Result<(), anyhow::Error> {
    // Use hot reload in debug mode
    #[cfg(debug_assertions)]
    dioxus_devtools::connect_subsecond();

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_maximized(true),
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
    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    Ok(())
}
