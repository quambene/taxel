use eframe::egui::ViewportBuilder;
use env_logger::{Builder, Env};
use log::debug;
use taxel_gui::TaxelApp;

fn main() -> Result<(), anyhow::Error> {
    // Init logging
    let env = Env::default().default_filter_or("info");
    Builder::from_env(env).init();

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
