use super::{AppIssue, IssueSeverity, SectionState, TaxelApp};
use crate::app::error_panel::WARNING_COLOR;
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
                import_report(app, &path);
            }
        }

        if app.table.is_some() && ui.button("Clear report").clicked() {
            app.table = None;
            app.issues.clear();
            app.show_error_panel = false;
        }

        draw_error_summary(app, ui);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            draw_language_toolbar(ui, &mut app.lang);

            ui.separator();

            draw_dark_mode_toggle(ui);

            ui.separator();

            draw_zoom_toolbar(ui, &mut app.zoom_input);
        });
    });
}

/// Loads an XBRL instance document from the specified path and updates the app
/// state.
fn import_report(app: &mut TaxelApp, path: &Path) {
    app.selected_tab = 0;
    app.table = None;
    app.issues.clear();
    app.show_error_panel = false;
    app.editing_section = None;
    app.edit_snapshot.clear();

    if let Err(err) = load_xml(&mut app.table, path) {
        app.issues.push(AppIssue {
            severity: IssueSeverity::Error,
            message: err.to_string(),
        });
        app.show_error_panel = true;
        return;
    }

    if let Some(table) = &app.table {
        for missing_role in &table.role_mapping_errors {
            app.issues.push(AppIssue {
                severity: IssueSeverity::Warning,
                message: format!("Missing report-element mapping for role URI: {missing_role}"),
            });
        }
    }

    app.show_error_panel = !app.issues.is_empty();

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

/// Draws a summary of errors and warnings in the header. Clicking on the
/// summary toggles the visibility of the detailed diagnostics panel.
fn draw_error_summary(app: &mut TaxelApp, ui: &mut Ui) {
    let error_count = app
        .issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .count();
    let warning_count = app
        .issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Warning)
        .count();

    if error_count > 0 || warning_count > 0 {
        ui.separator();

        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("errors: {error_count}"),
            0.0,
            egui::TextFormat {
                color: Color32::RED,
                ..Default::default()
            },
        );
        job.append(
            &format!("  warnings: {warning_count}"),
            0.0,
            egui::TextFormat {
                color: WARNING_COLOR,
                ..Default::default()
            },
        );

        let bg_idx = ui.painter().add(egui::Shape::Noop);
        let response = ui.add(egui::Button::new(job).frame(false));
        if response.hovered() {
            ui.painter().set(
                bg_idx,
                egui::Shape::rect_filled(
                    response.rect,
                    ui.visuals().widgets.hovered.corner_radius,
                    ui.visuals().widgets.hovered.weak_bg_fill,
                ),
            );
        }

        if response.clicked() {
            app.show_error_panel = !app.show_error_panel;
        }
    }
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

/// Draw the language selector tabs ("en", "de").
fn draw_language_toolbar(ui: &mut Ui, selected_lang: &mut String) {
    for (lang, tooltip) in [("de", "German"), ("en", "English")] {
        if ui
            .selectable_label(*selected_lang == lang, lang)
            .on_hover_text(tooltip)
            .clicked()
            && *selected_lang != lang
        {
            *selected_lang = lang.to_string();
        }
    }
}
