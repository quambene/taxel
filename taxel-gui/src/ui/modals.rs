use crate::{
    app::{self, LoadKind},
    TaxelApp,
};
use eframe::egui::{
    pos2, Align2, Button, ComboBox, Grid, Id, Modal, TextEdit, Ui, Vec2, Widget, Window,
};
use rfd::FileDialog;
use taxel::{TAXONOMY_TYPES, TAXONOMY_VERSION_TO_DATE};

/// Draws the delete confirmation modal when the user clicks "Delete report".
pub fn draw_delete_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let filename = app
        .loaded
        .as_ref()
        .and_then(|l| l.report.path.file_name())
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
        app::send_report(app);
    }
}

/// Draws the import-values modal when the user clicks "Import values".
pub fn draw_import_values_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let mut cancel = false;
    let mut confirm = false;

    let modal = Modal::new(Id::new("import_values_modal")).show(ui.ctx(), |ui| {
        ui.heading("Import values");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Source XML");
            let path_label = app
                .import_values_path
                .as_deref()
                .and_then(|p| p.to_str())
                .unwrap_or("No file selected");
            ui.label(path_label);

            if ui.button("Browse…").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("XML", &["xml"])
                    .add_filter("All", &["*"])
                    .pick_file()
                {
                    app.import_values_path = Some(path);
                }
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                cancel = true;
            }

            if ui
                .add_enabled(app.import_values_path.is_some(), Button::new("Import"))
                .clicked()
            {
                confirm = true;
            }
        });
    });

    if modal.should_close() || cancel {
        app.show_import_values_modal = false;
    }

    if confirm {
        app.show_import_values_modal = false;
        app::import_values(app);
    }
}

/// Draws a modal dialog asking the user to confirm downloading missing
/// taxonomies, with "Download" and "Cancel" buttons.
pub fn draw_download_modal(ui: &mut Ui, confirm: &mut bool, cancel: &mut bool) {
    Window::new("Download taxonomies")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.label("The required taxonomies are not yet downloaded.");
            ui.label("Do you want to download them now?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Download").clicked() {
                    *confirm = true;
                }
                if ui.button("Cancel").clicked() {
                    *cancel = true;
                }
            });
        });
}

/// Draws the keyboard shortcuts info modal.
pub fn draw_shortcuts_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let modal = Modal::new(Id::new("shortcuts_modal")).show(ui.ctx(), |ui| {
        ui.heading("Keyboard shortcuts");
        ui.add_space(8.0);

        Grid::new("shortcuts_grid")
            .num_columns(2)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                ui.strong("Ctrl+F");
                ui.label("Focus the search bar");
                ui.end_row();

                ui.strong("Ctrl+E");
                ui.label("Start editing the report section");
                ui.end_row();

                ui.strong("Ctrl+S");
                ui.label("Save changes");
                ui.end_row();

                ui.strong("Esc");
                ui.label("Cancel editing");
                ui.end_row();
            });

        ui.add_space(8.0);

        if ui.button("Close").clicked() {
            app.show_shortcuts_modal = false;
        }
    });

    if modal.should_close() {
        app.show_shortcuts_modal = false;
    }
}

/// Draws the report creation modal.
pub fn draw_new_report_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let mut cancel = false;
    let mut confirm = false;

    let mut available_versions: Vec<&str> = TAXONOMY_VERSION_TO_DATE.keys().copied().collect();
    available_versions.sort_unstable_by(|a, b| b.cmp(a));

    let taxonomy_label = app
        .new_report_form
        .taxonomy_type
        .label(&app.settings.lang)
        .unwrap_or("Unknown");

    let modal = Modal::new(Id::new("new_report_modal")).show(ui.ctx(), |ui| {
        ui.heading("Create new report");
        ui.add_space(8.0);

        Grid::new("new_report_grid")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Start date");
                TextEdit::singleline(&mut app.new_report_form.start_date)
                    .hint_text("YYYY-MM-DD")
                    .desired_width(120.0)
                    .ui(ui);
                ui.end_row();

                ui.label("End date");
                TextEdit::singleline(&mut app.new_report_form.end_date)
                    .hint_text("YYYY-MM-DD")
                    .desired_width(120.0)
                    .ui(ui);
                ui.end_row();

                ui.label("Taxonomy version");
                ComboBox::from_id_salt("new_report_taxonomy_version")
                    .selected_text(format!(
                        "{} ({})",
                        app.new_report_form.taxonomy_version,
                        TAXONOMY_VERSION_TO_DATE
                            .get(app.new_report_form.taxonomy_version.as_str())
                            .unwrap_or(&"unknown")
                    ))
                    .show_ui(ui, |ui| {
                        for &version in &available_versions {
                            ui.selectable_value(
                                &mut app.new_report_form.taxonomy_version,
                                version.to_string(),
                                format!(
                                    "{version} ({})",
                                    TAXONOMY_VERSION_TO_DATE.get(version).unwrap_or(&"unknown")
                                ),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Taxonomy type");
                ComboBox::from_id_salt("new_report_taxonomy_type")
                    .selected_text(taxonomy_label)
                    .show_ui(ui, |ui| {
                        for taxonomy_type in &TAXONOMY_TYPES {
                            ui.selectable_value(
                                &mut app.new_report_form.taxonomy_type,
                                taxonomy_type.clone(),
                                taxonomy_type.label(&app.settings.lang).unwrap_or("Unknown"),
                            );
                        }
                    });
                ui.end_row();
            });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                cancel = true;
            }

            let can_create = !app.new_report_form.selected_elements.is_empty();

            if ui.add_enabled(can_create, Button::new("Create")).clicked() {
                confirm = true;
            }
        });
    });

    if modal.should_close() || cancel {
        app.show_new_report_modal = false;
    }

    if confirm {
        app.show_new_report_modal = false;
        let form = app.new_report_form.clone();
        let ctx = ui.ctx().clone();

        app::start_load(app, LoadKind::Create(form), ctx, false);
    }
}

/// Draws a modal dialog warning about unsaved changes, with "Stay" and
/// "Continue" buttons.
pub fn draw_unsaved_changes_modal(ui: &mut Ui, stay: &mut bool, continue_nav: &mut bool) {
    Window::new("Unsaved changes")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.label("Switching sections will discard unsaved changes.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Stay").clicked() {
                    *stay = true;
                }
                if ui.button("Continue").clicked() {
                    *continue_nav = true;
                }
            });
        });
}

/// Draws a modal dialog warning that unchecking a report element deletes
/// entered values from the corresponding report section.
pub fn draw_report_element_uncheck_modal(ui: &mut Ui, confirm: &mut bool, cancel: &mut bool) {
    let modal = Modal::new(Id::new("report_element_uncheck_modal")).show(ui.ctx(), |ui| {
        ui.heading("Uncheck report section?");
        ui.add_space(8.0);
        ui.label("Warning: entered values of this report section will be deleted after saving.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                *cancel = true;
            }
            if ui.button("Uncheck").clicked() {
                *confirm = true;
            }
        });
    });

    if modal.should_close() {
        *cancel = true;
    }
}

/// Draws a modal to copy a diagnostic message to the clipboard.
pub fn draw_copy_message_modal(ui: &mut Ui, app: &mut TaxelApp) {
    let mut copied = false;

    let Some(copy_message) = app.copy_message.as_ref() else {
        return;
    };

    let click_pos = copy_message.position;
    let mut window_pos = pos2(click_pos.x, click_pos.y - 44.0);
    let viewport = ui.ctx().content_rect();
    window_pos.x = window_pos
        .x
        .clamp(viewport.left() + 8.0, viewport.right() - 160.0);
    window_pos.y = window_pos
        .y
        .clamp(viewport.top() + 8.0, viewport.bottom() - 48.0);

    Window::new("copy_message_modal")
        .id(Id::new("copy_message_modal"))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .fixed_pos(window_pos)
        .show(ui.ctx(), |ui| {
            if ui.button("Copy message").clicked() {
                ui.ctx().copy_text(copy_message.message.clone());
                copied = true;
            }
        });

    let clicked_anywhere = ui.ctx().input(|input| input.pointer.primary_clicked());

    if copied || clicked_anywhere {
        app.copy_message = None;
    }
}
