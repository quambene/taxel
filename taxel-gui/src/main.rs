mod app;
mod widgets;

use crate::app::TaxelApp;
use eframe::egui::ViewportBuilder;
use log::debug;
fn main() -> Result<(), anyhow::Error> {
    // Use hot reload in debug mode
    #[cfg(debug_assertions)]
    dioxus_devtools::connect_subsecond();

    let persistence_path = dirs::config_dir().map(|path| path.join("taxel").join("settings.ron"));

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_maximized(true),
        persistence_path,
        ..Default::default()
    };

    debug!("Run app");

    eframe::run_native(
        "Taxel",
        options,
        Box::new(|ctx| Ok(Box::new(TaxelApp::new(ctx, None, None)))),
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    Ok(())
}
