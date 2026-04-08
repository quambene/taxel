use super::{SectionState, TaxelApp};
use eframe::egui::{self, Color32, Ui};
use rfd::FileDialog;
use std::path::Path;
use taxel_gui::load_xml;

/// Draws the header panel of the application, including the "Import report"
/// button, the "Clear report" button, any error messages, and the language
/// selector tabs.
pub(super) fn draw_header(app: &mut TaxelApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        if ui.button("Import report").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("XML", &["xml"])
                .add_filter("All", &["*"])
                .pick_file()
            {
                load_xml_into_app(app, &path);
            }
        }

        if app.table.is_some() && ui.button("Clear report").clicked() {
            app.table = None;
        }

        if let Some(err) = &app.error_message {
            ui.separator();
            ui.colored_label(Color32::RED, err.to_string());
            if ui.button("Dismiss").clicked() {
                app.error_message = None;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let _ = draw_language_toolbar(ui, &mut app.lang);

            ui.separator();

            draw_dark_mode_toggle(ui);

            ui.separator();

            draw_zoom_toolbar(ui, &mut app.zoom_input);
        });
    });
}

/// Loads an XBRL instance document from the specified path and updates the app
/// state.
fn load_xml_into_app(app: &mut TaxelApp, path: &Path) {
    app.selected_tab = 0;
    app.table = None;
    app.editing_section = None;
    app.edit_snapshot.clear();

    if let Err(err) = load_xml(&mut app.table, path) {
        app.error_message = Some(format!("{err}"));
    }

    app.section_states = app
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

/// Draw the zoom controls: `[+] [100%] [-]`.
fn draw_zoom_toolbar(ui: &mut Ui, zoom_input: &mut String) {
    let zoom = ui.ctx().zoom_factor();

    if ui
        .add(egui::Button::new("−").min_size(egui::vec2(24.0, 24.0)))
        .clicked()
    {
        let new_zoom = (zoom - 0.1).max(0.5);
        ui.ctx().set_zoom_factor(new_zoom);
        *zoom_input = format!("{}", (new_zoom * 100.0).round() as u32);
    }

    ui.label("%");

    let response = ui.add(
        egui::TextEdit::singleline(zoom_input)
            .desired_width(35.0)
            .horizontal_align(egui::Align::Center),
    );

    if response.lost_focus() {
        if let Ok(percent) = zoom_input.trim().parse::<u32>() {
            let clamped = percent.clamp(50, 400);
            ui.ctx().set_zoom_factor(clamped as f32 / 100.0);
            *zoom_input = format!("{}", clamped);
        } else {
            *zoom_input = format!("{}", (zoom * 100.0).round() as u32);
        }
    } else if !response.has_focus() {
        *zoom_input = format!("{}", (zoom * 100.0).round() as u32);
    }

    if ui
        .add(egui::Button::new("+").min_size(egui::vec2(24.0, 24.0)))
        .clicked()
    {
        let new_zoom = (zoom + 0.1).min(4.0);
        ui.ctx().set_zoom_factor(new_zoom);
        *zoom_input = format!("{}", (new_zoom * 100.0).round() as u32);
    }
}

/// Draw the dark mode toggle button (☀ / ☾).
fn draw_dark_mode_toggle(ui: &mut Ui) {
    let dark_mode = ui.ctx().global_style().visuals.dark_mode;

    // Show sun icon in dark mode (to switch to light) and moon icon in light
    // mode (to switch to dark).
    let icon = if dark_mode { "\u{2600}" } else { "\u{1F319}" };

    let tooltip = if dark_mode {
        "Switch to light mode"
    } else {
        "Switch to dark mode"
    };

    if ui
        .add(egui::Button::new(icon).min_size(egui::vec2(24.0, 24.0)))
        .on_hover_text(tooltip)
        .clicked()
    {
        let visuals = if dark_mode {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        ui.ctx().set_visuals(visuals);
    }
}

/// Draw the language selector tabs ("en", "de"). Returns true if the language was changed.
fn draw_language_toolbar(ui: &mut Ui, selected_lang: &mut String) -> bool {
    let mut changed = false;

    for (lang, tooltip) in [("de", "German"), ("en", "English")] {
        if ui
            .selectable_label(*selected_lang == lang, lang)
            .on_hover_text(tooltip)
            .clicked()
            && *selected_lang != lang
        {
            *selected_lang = lang.to_string();
            changed = true;
        }
    }
    changed
}
