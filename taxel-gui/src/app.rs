mod diagnostics;
mod report;
mod report_list;
mod search;
mod settings;

use crate::{
    app::{report_list::ReportList, settings::Settings},
    domain::{Report, ReportStatus},
    ui::{
        diagnostic_panel::draw_error_panel,
        header::{draw_delete_modal, draw_header},
        report_list_view::draw_report_list,
        report_view::{
            search_overlay::{draw_search_results_overlay, highlight_row},
            sidebar::draw_sidebar,
            table::draw_table,
            toolbar::{draw_toolbar, EditAction},
        },
        widgets::draw_unsaved_changes_modal,
    },
};
pub use diagnostics::{AppDiagnostic, DiagnosticCategory, DiagnosticLevel};
use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Key, KeyboardShortcut, Modifiers, Panel, Ui, Visuals},
    App, CreationContext, Frame,
};
use eric_sdk::Eric;
pub use report::{delete_report, load_report, send_report, validate_report};
pub use report_list::ReportOverview;
pub use search::{RowHighlight, Search};
use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::mpsc::{self, Receiver},
};
use xbrl_rs::{InstanceDocument, TaxonomySet};

/// Per-section UI state (collapse state and depth filter).
#[derive(Default)]
pub struct SectionState {
    /// Row indices whose children are collapsed.
    pub collapsed: HashSet<usize>,
    /// Maximum depth to display. None means show all depths.
    pub max_depth: Option<usize>,
}

/// Main application struct for the Taxel GUI, managing the state of the app.
pub struct TaxelApp {
    /// The taxonomy set for the currently loaded XBRL instance document, if
    /// any.
    pub taxonomy: Option<TaxonomySet>,
    /// The instance document currently loaded in the app, if any.
    pub instance_document: Option<InstanceDocument>,
    /// Eric instance to validate XBRL instance documents and provide
    /// diagnostics. Initialized on app start if the data directory can be
    /// determined, otherwise skipped with a warning.
    pub eric: Option<Eric>,
    /// The currently loaded report.
    pub report: Option<Report>,
    /// Imported reports and creation date bookkeeping.
    pub report_list: ReportList,
    /// The index of the currently selected section tab in the sidebar.
    pub selected_tab: usize,
    /// Per-section UI state, indexed analogous to `report.sections`.
    pub section_states: Vec<SectionState>,
    /// Persisted UI settings (language, zoom, theme).
    pub settings: Settings,
    /// Structured diagnostics for non-blocking and blocking issues.
    pub diagnostics: Vec<AppDiagnostic>,
    /// Controls whether the detailed diagnostics panel is visible.
    pub show_error_panel: bool,
    /// Search state.
    pub search: Search,
    /// Receives the result of a background XML load, if one is in progress.
    pub loading: Option<Receiver<anyhow::Result<(TaxonomySet, InstanceDocument, Report)>>>,
    /// Some while the value column of that section is being edited, None
    /// otherwise.
    pub editing_section: Option<usize>,
    /// Snapshot of `row.value` for every row in the edited section at the
    /// moment editing started, indexed by raw row index. Used to restore values
    /// if editing is canceled.
    pub edit_snapshot: Vec<String>,
    /// Controls whether the delete-report confirmation modal is visible.
    pub show_delete_modal: bool,
}

impl TaxelApp {
    /// Creates a new `TaxelApp` instance with the given fact table and error
    /// message. Both parameters are optional to allow starting with an empty
    /// state. Loads persisted settings (language, zoom) from eframe storage if
    /// available.
    pub fn new(
        ctx: &CreationContext<'_>,
        table: Option<Report>,
        error_message: Option<String>,
    ) -> TaxelApp {
        let section_states = table
            .as_ref()
            .map(|table| {
                table
                    .sections
                    .iter()
                    .map(|_| SectionState::default())
                    .collect()
            })
            .unwrap_or_default();
        let mut diagnostics = Vec::new();

        if let Some(message) = error_message {
            diagnostics.push(AppDiagnostic::new_error(DiagnosticCategory::App, message));
        }

        let settings = Settings::load(ctx.storage);
        settings.apply(&ctx.egui_ctx);

        let eric =
            if let Some(log_path) = dirs::data_dir().map(|dir| dir.join("taxel").join("logs")) {
                if let Err(err) = fs::create_dir_all(&log_path) {
                    diagnostics.push(AppDiagnostic::new_error(
                        DiagnosticCategory::App,
                        format!("Failed to create log directory: {err}"),
                    ));
                }

                match Eric::new(&log_path) {
                    Ok(eric) => Some(eric),
                    Err(err) => {
                        diagnostics.push(AppDiagnostic::new_error(
                            DiagnosticCategory::App,
                            format!("Failed to initialize Eric: {err}"),
                        ));
                        None
                    }
                }
            } else {
                diagnostics.push(AppDiagnostic::new_warning(
                    DiagnosticCategory::App,
                    "Could not determine data directory, skipping Eric initialization".to_string(),
                ));
                None
            };

        let show_error_panel = true;

        let mut report_list = ReportList::new();

        if let Err(err) = report_list.refresh() {
            diagnostics.push(AppDiagnostic::new_warning(
                DiagnosticCategory::App,
                format!("Failed to list imported reports: {err}"),
            ));
        }

        Self {
            taxonomy: None,
            instance_document: None,
            report: table,
            selected_tab: 0,
            section_states,
            settings,
            diagnostics,
            show_error_panel,
            loading: None,
            search: Search::default(),
            report_list,
            editing_section: None,
            edit_snapshot: Vec::new(),
            eric,
            show_delete_modal: false,
        }
    }

    pub fn register_report(&mut self, path: &Path) {
        self.report_list.register_report(path)
    }

    /// Registers a newly imported report by adding it to the report list.
    pub fn refresh_reports(&mut self) {
        match self.report_list.refresh() {
            Ok(()) => {}
            Err(err) => {
                self.diagnostics.push(AppDiagnostic::new_warning(
                    DiagnosticCategory::App,
                    format!("Failed to refresh imported reports: {err}"),
                ));
                self.show_error_panel = true;
            }
        }
    }
}

impl App for TaxelApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.save(storage);
    }

    /// The main UI drawing function for the app, called on each frame.
    fn ui(&mut self, ctx: &mut Ui, _: &mut Frame) {
        ctx.ctx().set_visuals(if self.settings.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        });

        load_fact_table(self);

        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            Panel::top("header").min_size(32.0).show_inside(ctx, |ui| {
                draw_header(ui, self);
            });

            let sections = self
                .report
                .as_ref()
                .map(|table| table.sections.as_slice())
                .unwrap_or(&[]);
            draw_sidebar(ctx, sections, &mut self.selected_tab, &self.settings.lang);

            if self.show_error_panel {
                draw_error_panel(ctx, &self.diagnostics, &mut self.show_error_panel);
            }

            let lang = self.settings.lang.clone();

            let central_frame = {
                let mut frame = egui::Frame::central_panel(ctx.style());
                if self.show_error_panel {
                    frame.inner_margin.bottom = 0;
                }
                frame
            };

            CentralPanel::default()
                .frame(central_frame)
                .show_inside(ctx, |ui| {
                    if self.report.is_none() {
                        if let Some(path) =
                            draw_report_list(ui, self.report_list.reports(), self.loading.is_some())
                        {
                            report::load_report(self, path, ui.ctx().clone());
                        }
                        return;
                    }

                    // While the unsaved-changes modal is visible (user clicked a
                    // different section but hasn't confirmed yet), keep rendering
                    // the editing section so the view doesn't jump before the user
                    // decides. `selected_tab` acts as the pending destination.
                    let content_tab = self
                        .editing_section
                        .filter(|&s| s != self.selected_tab)
                        .unwrap_or(self.selected_tab);
                    let editing = self.editing_section == Some(content_tab);

                    // Toolbar block: immutable borrow for read-only access to table
                    // data.
                    let mut action = if let Some(table) = &self.report {
                        if let Some(section) = table.sections.get(content_tab) {
                            let max_depth =
                                section.rows.iter().map(|row| row.depth).max().unwrap_or(0) + 1;
                            let state = &mut self.section_states[content_tab];

                            draw_toolbar(
                                ui,
                                max_depth,
                                &mut state.max_depth,
                                &mut state.collapsed,
                                &section.rows,
                                &mut self.search,
                                table,
                                &lang,
                                editing,
                            )
                        } else {
                            EditAction::None
                        }
                    } else {
                        EditAction::None
                    };

                    // Support keyboard shortcuts Ctrl+S and ESC while editing.
                    let pending_section_switch =
                        self.editing_section.is_some_and(|s| s != self.selected_tab);

                    if editing && !pending_section_switch {
                        let save_shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
                        let save_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&save_shortcut));
                        let cancel_pressed = ui.ctx().input(|input| input.key_pressed(Key::Escape));

                        if save_pressed {
                            // TODO: Save changes to XBRL instance document.
                            // Currently, edits are only stored in the app state and
                            // will be lost on reload.
                            action = EditAction::Save;
                        } else if cancel_pressed {
                            action = EditAction::Cancel;
                        }
                    }

                    // Handle toolbar edit actions.
                    match action {
                        EditAction::Start => {
                            if let Some(table) = &self.report {
                                if let Some(section) = table.sections.get(self.selected_tab) {
                                    self.edit_snapshot =
                                        section.rows.iter().map(|r| r.value.clone()).collect();
                                }
                            }
                            self.editing_section = Some(self.selected_tab);

                            // If the report was previously validated, mark it
                            // as draft again since it has unsaved changes now.
                            if let Some(report) = &self.report {
                                self.report_list.set_report_status(
                                    &report.path,
                                    ReportStatus::Draft,
                                    &mut self.diagnostics,
                                );
                            }
                        }
                        EditAction::Save => {
                            self.editing_section = None;
                            self.edit_snapshot.clear();
                        }
                        EditAction::Cancel => {
                            let editing_tab = self.editing_section.unwrap_or(self.selected_tab);

                            if let Some(table) = &mut self.report {
                                if let Some(section) = table.sections.get_mut(editing_tab) {
                                    for (row, value) in
                                        section.rows.iter_mut().zip(self.edit_snapshot.iter())
                                    {
                                        row.value = value.clone();
                                    }
                                }
                            }
                            self.editing_section = None;
                            self.edit_snapshot.clear();
                        }
                        EditAction::None => {}
                    }

                    // Table block: mutable borrow for in-place editing.
                    if let Some(table) = self.report.as_mut() {
                        let tab = content_tab;
                        let table_rect = ui.available_rect_before_wrap();

                        if let Some(section) = table.sections.get_mut(tab) {
                            let highlighted_row = highlight_row(&mut self.search, tab, ui);
                            let state = &mut self.section_states[tab];
                            let scroll_to = self.search.scroll_to_row.take();

                            draw_table(
                                &mut section.rows,
                                &mut state.collapsed,
                                &lang,
                                scroll_to,
                                highlighted_row,
                                editing,
                                ui,
                            );
                        }

                        if !self.search.results.is_empty() {
                            draw_search_results_overlay(
                                ui.ctx(),
                                table_rect,
                                &mut self.search,
                                table,
                                &mut self.selected_tab,
                                &mut self.section_states,
                            );
                        }
                    }

                    // Section-switch warning when navigating away with unsaved edits.
                    if self
                        .editing_section
                        .is_some_and(|section| section != self.selected_tab)
                    {
                        let mut stay = false;
                        let mut continue_nav = false;

                        draw_unsaved_changes_modal(ui, &mut stay, &mut continue_nav);

                        if stay {
                            self.selected_tab = self.editing_section.unwrap();
                        }
                        if continue_nav {
                            let editing_tab = self.editing_section.unwrap();
                            if let Some(table) = &mut self.report {
                                if let Some(section) = table.sections.get_mut(editing_tab) {
                                    for (row, value) in
                                        section.rows.iter_mut().zip(self.edit_snapshot.iter())
                                    {
                                        row.value = value.clone();
                                    }
                                }
                            }
                            self.editing_section = None;
                            self.edit_snapshot.clear();
                        }
                    }
                });

            if self.show_delete_modal {
                draw_delete_modal(ctx, self);
            }
        });
    }
}

/// Polls the background XML load result and updates the app state accordingly.
fn load_fact_table(app: &mut TaxelApp) {
    if let Some(rx) = &app.loading {
        match rx.try_recv() {
            Ok(Ok((taxonomy, instance, report))) => {
                for missing_role in &report.role_mapping_errors {
                    app.diagnostics.push(AppDiagnostic::new_warning(
                        DiagnosticCategory::Import,
                        format!("Missing report-element mapping for role URI: {missing_role}"),
                    ));
                }

                app.section_states = report
                    .sections
                    .iter()
                    .map(|_| SectionState::default())
                    .collect();
                app.report = Some(report);
                app.taxonomy = Some(taxonomy);
                app.instance_document = Some(instance);
                app.show_error_panel = !app.diagnostics.is_empty();
                app.loading = None;
            }
            Ok(Err(err)) => {
                app.diagnostics.push(AppDiagnostic::new_error(
                    DiagnosticCategory::Import,
                    err.to_string(),
                ));
                app.show_error_panel = true;
                app.loading = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                app.loading = None;
            }
        }
    }
}
