use eframe::{
    egui::{Context, Visuals},
    Storage,
};

/// User-configurable UI settings persisted via eframe storage.
pub struct Settings {
    /// The language code for the UI, e.g. "en" for English or "de" for German.
    pub lang: String,
    /// The zoom level for the UI, stored as a percentage string, e.g. "100" for
    /// 100%.
    pub zoom_input: String,
    /// Whether to use dark mode for the UI.
    pub dark_mode: bool,
    /// Whether the user has accepted the Terms of Use and Privacy Notice.
    pub terms_accepted: bool,
}

impl Settings {
    /// Load settings from storage, applying defaults for any missing values.
    pub fn load(storage: Option<&dyn Storage>) -> Self {
        let lang = storage
            .and_then(|storage| eframe::get_value::<String>(storage, "lang"))
            .unwrap_or_else(|| "en".to_string());

        let zoom_input = storage
            .and_then(|storage| eframe::get_value::<String>(storage, "zoom_input"))
            .unwrap_or_else(|| "100".to_string());

        let dark_mode = storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "dark_mode"))
            .unwrap_or(false);

        let terms_accepted = storage
            .and_then(|storage| eframe::get_value::<bool>(storage, "terms_accepted"))
            .unwrap_or(false);

        Self {
            lang,
            zoom_input,
            dark_mode,
            terms_accepted,
        }
    }

    /// Apply the current settings to the UI context.
    pub fn apply(&self, ctx: &Context) {
        if let Ok(percent) = self.zoom_input.trim().parse::<u32>() {
            ctx.set_zoom_factor(percent as f32 / 100.0);
        }

        ctx.set_visuals(if self.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        });
    }

    /// Save the current settings to storage.
    pub fn save(&self, storage: &mut dyn Storage) {
        eframe::set_value(storage, "lang", &self.lang);
        eframe::set_value(storage, "zoom_input", &self.zoom_input);
        eframe::set_value(storage, "dark_mode", &self.dark_mode);
        eframe::set_value(storage, "terms_accepted", &self.terms_accepted);
    }
}
