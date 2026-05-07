use crate::{
    app::{self, DiagnosticLevel},
    ui::diagnostic_panel::{ERROR_COLOR, SUCCESS_COLOR, WARNING_COLOR},
    TaxelApp,
};
use eframe::egui::{
    text::LayoutJob, vec2, Align, Button, Layout, Response, Shape, TextEdit, TextFormat, Ui,
};
use rfd::FileDialog;

/// Draws the header panel of the application, including the "Import report"
/// button, the "Close report" button, any error messages, and the language
/// selector tabs.
pub fn draw_header(ui: &mut Ui, app: &mut TaxelApp) {
    ui.horizontal_centered(|ui| {
        draw_new_report_button(ui, app);
        draw_import_button(ui, app);

        if app.loading.is_some() {
            ui.spinner();
            ui.label("Loading…");
        }

        if app.report.is_some() && ui.button("Close report").clicked() {
            app.report = None;
            app.taxonomy = None;
            app.instance_document = None;
            app.selected_tab = 0;
            app.search.results.clear();
            app.search.scroll_to_row = None;
            app.search.row_highlight = None;
            app.loading = None;
            app.diagnostics.clear();
            app.show_error_panel = true;
            app.editing_section = None;
            app.edit_snapshot.clear();
        }

        if app.report.is_some() && ui.button("Delete report").clicked() {
            app.show_delete_modal = true;
        }

        if app.report.is_some() && ui.button("Validate report").clicked() {
            app::validate_report(app);
        }

        if app.report.is_some() && ui.button("Send report").clicked() {
            app.show_send_modal = true;
        }

        if app.report.is_some() && ui.button("Import values").clicked() {
            app.show_import_values_modal = true;
        }

        draw_error_summary(app, ui);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            draw_language_toolbar(ui, &mut app.settings.lang);

            ui.separator();

            draw_dark_mode_toggle(ui, &mut app.settings.dark_mode);

            ui.separator();

            draw_zoom_toolbar(ui, &mut app.settings.zoom_input);

            ui.separator();

            if draw_info_button(ui).clicked() {
                app.show_shortcuts_modal = true;
            }
        });
    });
}

/// Draws the "?" info button that opens the keyboard shortcuts modal.
fn draw_info_button(ui: &mut Ui) -> Response {
    ui.add(Button::new("?").min_size(vec2(24.0, 24.0)))
        .on_hover_text("Keyboard shortcuts")
}

/// Draws the "New report" button. Clicking opens the report creation modal.
fn draw_new_report_button(ui: &mut Ui, app: &mut TaxelApp) {
    if app.report.is_none() && app.loading.is_none() && ui.button("New report").clicked() {
        app.show_new_report_modal = true;
    }
}

/// Draws the "Import report" button. Clicking the button opens a file dialog to
/// select an XML file.
fn draw_import_button(ui: &mut Ui, app: &mut TaxelApp) {
    if app.report.is_none() && app.loading.is_none() && ui.button("Import report").clicked() {
        if let Some(path) = FileDialog::new()
            .add_filter("XML", &["xml"])
            .add_filter("All", &["*"])
            .pick_file()
        {
            app::import_report(app, path, ui.ctx());
        }
    }
}

/// Draws a summary of errors and warnings in the header. Clicking on the
/// summary toggles the visibility of the detailed diagnostics panel.
fn draw_error_summary(app: &mut TaxelApp, ui: &mut Ui) {
    let error_count = app
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
        .count();
    let warning_count = app
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Warning)
        .count();
    let success_count = app
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Success)
        .count();

    ui.separator();

    let mut job = LayoutJob::default();
    job.append(
        &format!("errors: {error_count}"),
        0.0,
        TextFormat {
            color: ERROR_COLOR,
            ..Default::default()
        },
    );
    job.append(
        &format!("  warnings: {warning_count}"),
        0.0,
        TextFormat {
            color: WARNING_COLOR,
            ..Default::default()
        },
    );
    job.append(
        &format!("  success: {success_count}"),
        0.0,
        TextFormat {
            color: SUCCESS_COLOR,
            ..Default::default()
        },
    );

    let bg_idx = ui.painter().add(Shape::Noop);
    let response = ui.add(Button::new(job).frame(false));
    if response.hovered() {
        ui.painter().set(
            bg_idx,
            Shape::rect_filled(
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

/// Draw the zoom controls: `[+] [100%] [-]`.
fn draw_zoom_toolbar(ui: &mut Ui, zoom_input: &mut String) {
    let zoom = ui.ctx().zoom_factor();

    if ui
        .add(Button::new("−").min_size(vec2(24.0, 24.0)))
        .clicked()
    {
        let new_zoom = (zoom - 0.1).max(0.5);
        ui.ctx().set_zoom_factor(new_zoom);
        *zoom_input = format!("{}", (new_zoom * 100.0).round() as u32);
    }

    ui.label("%");

    let response = ui.add(
        TextEdit::singleline(zoom_input)
            .desired_width(35.0)
            .horizontal_align(Align::Center),
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
        .add(Button::new("+").min_size(vec2(24.0, 24.0)))
        .clicked()
    {
        let new_zoom = (zoom + 0.1).min(4.0);
        ui.ctx().set_zoom_factor(new_zoom);
        *zoom_input = format!("{}", (new_zoom * 100.0).round() as u32);
    }
}

/// Draw the dark mode toggle button (☀ / ☾).
fn draw_dark_mode_toggle(ui: &mut Ui, dark_mode: &mut bool) {
    // Show sun icon in dark mode (to switch to light) and moon icon in light
    // mode (to switch to dark).
    let icon = if *dark_mode { "\u{2600}" } else { "\u{1F319}" };

    let tooltip = if *dark_mode {
        "Switch to light mode"
    } else {
        "Switch to dark mode"
    };

    if ui
        .add(Button::new(icon).min_size(vec2(24.0, 24.0)))
        .on_hover_text(tooltip)
        .clicked()
    {
        *dark_mode = !*dark_mode;
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
