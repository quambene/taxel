use crate::{
    app::{self, AppDiagnostic, DiagnosticCategory, DiagnosticLevel},
    infrastructure::report_store,
    ui::diagnostic_panel::{SUCCESS_COLOR, WARNING_COLOR},
    TaxelApp,
};
use eframe::egui::{
    text::LayoutJob, vec2, Align, Button, Color32, Id, Layout, Modal, Shape, TextEdit, TextFormat,
    Ui, Widget,
};
use rfd::FileDialog;

/// Draws the header panel of the application, including the "Import report"
/// button, the "Close report" button, any error messages, and the language
/// selector tabs.
pub fn draw_header(ui: &mut Ui, app: &mut TaxelApp) {
    ui.horizontal_centered(|ui| {
        if app.report.is_none() && app.loading.is_none() && ui.button("Import report").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("XML", &["xml"])
                .add_filter("All", &["*"])
                .pick_file()
            {
                match report_store::copy_report(&path) {
                    Ok(copied_path) => {
                        app.register_report(&copied_path);
                        app.refresh_reports();
                        app::load_report(app, copied_path, ui.ctx().clone());
                    }
                    Err(err) => {
                        app.diagnostics.push(AppDiagnostic::new_error(
                            DiagnosticCategory::App,
                            format!("Failed to import report: {err}"),
                        ));
                        app.show_error_panel = true;
                    }
                }
            }
        }

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

        draw_error_summary(app, ui);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            draw_language_toolbar(ui, &mut app.settings.lang);

            ui.separator();

            draw_dark_mode_toggle(ui, &mut app.settings.dark_mode);

            ui.separator();

            draw_zoom_toolbar(ui, &mut app.settings.zoom_input);
        });
    });
}

/// Draws the delete confirmation modal when the user clicks "Delete report".
pub fn draw_delete_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let filename = app
        .report
        .as_ref()
        .and_then(|report| report.path.file_name())
        .and_then(|file_name| file_name.to_str())
        .unwrap_or("this report");

    let mut cancel = false;
    let mut confirm = false;

    let modal = Modal::new(Id::new("delete_report_modal")).show(ui.ctx(), |ui| {
        ui.heading("Move report to Trash?");
        ui.add_space(4.0);
        ui.label(filename);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
            if ui.button("Move to Trash").clicked() {
                confirm = true;
            }
        });
    });

    if modal.should_close() || cancel {
        app.show_delete_modal = false;
    }

    if confirm {
        app.show_delete_modal = false;
        app::delete_report(app);
    }
}

/// Draws the send report modal when the user clicks "Send report".
pub fn draw_send_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let mut cancel = false;
    let mut confirm = false;

    let modal = Modal::new(Id::new("send_report_modal")).show(ui.ctx(), |ui| {
        ui.heading("Send report");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Certificate");
            let path_label = app
                .send_certificate_path
                .as_deref()
                .and_then(|p| p.to_str())
                .unwrap_or("No file selected");
            ui.label(path_label);
            if ui.button("Browse…").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("PFX certificate", &["pfx", "p12"])
                    .add_filter("All", &["*"])
                    .pick_file()
                {
                    app.send_certificate_path = Some(path);
                }
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Password");
            TextEdit::singleline(&mut app.send_password)
                .password(true)
                .desired_width(180.0)
                .ui(ui);
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
            let can_send = app.send_certificate_path.is_some() && !app.send_password.is_empty();
            if ui.add_enabled(can_send, Button::new("Send")).clicked() {
                confirm = true;
            }
        });
    });

    if modal.should_close() || cancel {
        app.show_send_modal = false;
    }

    if confirm {
        app.show_send_modal = false;
        if let Some(path) = app.send_certificate_path.clone() {
            app::send_report(app, path, app.send_password.clone());
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
            color: Color32::RED,
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
