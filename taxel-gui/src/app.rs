mod diagnostics;
mod report;
mod report_list;
mod search;
mod settings;

use crate::{
    app::{self, report::NewReportForm, report_list::ReportList, settings::Settings},
    domain::FactValue,
    ui::{self, EditAction},
};
pub use diagnostics::{AppDiagnostic, DiagnosticCategory, DiagnosticLevel};
use dioxus_devtools::subsecond;
use eframe::{
    egui::{CentralPanel, Frame, Key, KeyboardShortcut, Modifiers, Panel, Pos2, Ui, Visuals},
    App, CreationContext,
};
use eric_sdk::Eric;
use report::LoadOutcome;
pub use report::{
    cancel_edit, delete_report, edit_report, export_values, import_report, import_values,
    poll_load_result, save_report, save_report_as, send_report, start_load, validate_report,
    LoadKind, LoadedReport,
};
pub use report_list::ReportOverview;
pub use search::{RowHighlight, Search};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

/// The name of the application.
pub const APP_NAME: &str = "Taxel";
/// The version of the application, derived from Cargo.toml.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The vendor ID to use in generated reports.
pub const VENDOR_ID: &str = env!("VENDOR_ID");

/// Per-section UI state (collapse state and depth filter).
#[derive(Default)]
pub struct SectionState {
    /// Row indices whose children are collapsed.
    pub collapsed: HashSet<usize>,
    /// Maximum depth to display. None means show all depths.
    pub max_depth: Option<usize>,
    /// When true, only rows with `is_required = true` are shown.
    pub show_required_only: bool,
    /// When true, only rows with non-empty values are shown.
    pub show_filled_only: bool,
}

/// Transient state for the diagnostics copy-message popup.
pub struct CopyMessage {
    /// The diagnostic message to copy to the clipboard.
    pub message: String,
    /// Screen-space position where the popup should appear.
    pub position: Pos2,
}

/// Pending uncheck action for a reportElements checkbox.
pub struct PendingReportElementUncheck {
    /// The section index containing the checkbox row.
    pub section_idx: usize,
    /// The raw row index within the section.
    pub row_idx: usize,
}

/// Main application struct for the Taxel GUI, managing the state of the app.
pub struct TaxelApp {
    /// The currently loaded report together with its taxonomy, instance
    /// document, and Elster envelope. `None` when no report is open.
    pub loaded: Option<LoadedReport>,
    /// Eric instance to validate XBRL instance documents and provide
    /// diagnostics. Initialized on app start if the data directory can be
    /// determined, otherwise skipped with a warning.
    pub eric: Option<Eric>,
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
    pub show_diagnostics_panel: bool,
    /// Search state.
    pub search: Search,
    /// Receives the result of a background XML load, if one is in progress.
    pub loading: Option<Receiver<anyhow::Result<LoadOutcome>>>,
    /// Controls whether the taxonomy download confirmation modal is visible.
    pub show_download_modal: bool,
    /// The load kind pending retry after the user confirms a taxonomy download.
    pub pending_load_kind: Option<LoadKind>,
    /// Some while the value column of that section is being edited, None
    /// otherwise.
    pub editing_section: Option<usize>,
    /// Snapshot of `row.value` for every row in the edited section at the
    /// moment editing started, indexed by raw row index. Used to restore values
    /// if editing is canceled.
    pub edit_snapshot: Vec<FactValue>,
    /// Controls whether the delete-report confirmation modal is visible.
    pub show_delete_modal: bool,
    /// Controls whether the send-report modal is visible.
    pub show_send_modal: bool,
    /// Controls whether the import-values modal is visible.
    pub show_import_values_modal: bool,
    /// Controls whether the keyboard shortcuts info modal is visible.
    pub show_shortcuts_modal: bool,
    /// Transient copy-message popup state. None means hidden.
    pub copy_message: Option<CopyMessage>,
    /// Certificate file path selected in the send modal, persisted across opens.
    pub send_certificate_path: Option<PathBuf>,
    /// Password entered in the send modal, persisted across opens (in-memory only).
    pub send_password: String,
    /// Source XML selected in the import-values modal, persisted across opens.
    pub import_values_path: Option<PathBuf>,
    /// Controls whether the new-report creation modal is visible.
    pub show_new_report_modal: bool,
    /// Form state for the new-report dialog, preserved across opens.
    pub new_report_form: NewReportForm,
    /// Controls whether the report-element uncheck warning modal is visible.
    pub show_report_element_uncheck_modal: bool,
    /// Pending report-element checkbox to uncheck on confirmation.
    pub pending_report_element_uncheck: Option<PendingReportElementUncheck>,
}

impl TaxelApp {
    /// Creates a new `TaxelApp` instance. Loads persisted settings (language,
    /// zoom) from eframe storage if available.
    pub fn new(ctx: &CreationContext<'_>) -> TaxelApp {
        let section_states = Vec::new();
        let mut diagnostics = Vec::new();

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

                match Eric::new(Some(&log_path), None) {
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

        let show_diagnostics_panel = true;

        let mut report_list = ReportList::new();

        if let Err(err) = report_list.refresh() {
            diagnostics.push(AppDiagnostic::new_warning(
                DiagnosticCategory::App,
                format!("Failed to list imported reports: {err}"),
            ));
        }

        Self {
            loaded: None,
            selected_tab: 0,
            section_states,
            settings,
            diagnostics,
            show_diagnostics_panel,
            loading: None,
            search: Search::default(),
            report_list,
            editing_section: None,
            edit_snapshot: Vec::new(),
            eric,
            show_delete_modal: false,
            show_send_modal: false,
            show_import_values_modal: false,
            show_shortcuts_modal: false,
            copy_message: None,
            show_download_modal: false,
            pending_load_kind: None,
            send_certificate_path: None,
            send_password: String::new(),
            import_values_path: None,
            show_new_report_modal: false,
            new_report_form: NewReportForm::default(),
            show_report_element_uncheck_modal: false,
            pending_report_element_uncheck: None,
        }
    }

    /// Registers a newly loaded report by adding it to the report list, or
    /// updating it if already present, and persists the updated manifest.
    pub fn upsert_report(
        &mut self,
        path: &Path,
        taxonomy_type: Option<taxel::TaxonomyType>,
        taxonomy_version: Option<String>,
        start_date: Option<String>,
        end_date: Option<String>,
    ) {
        self.report_list.upsert_report(
            path,
            taxonomy_type,
            taxonomy_version,
            start_date,
            end_date,
            &mut self.diagnostics,
        )
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
                self.show_diagnostics_panel = true;
            }
        }
    }

    /// Applies a previously deferred uncheck action for a `reportElements`
    /// checkbox after the user confirms the warning modal.
    fn apply_pending_report_element_uncheck(&mut self) {
        if let Some(pending) = self.pending_report_element_uncheck.take() {
            if let Some(loaded) = self.loaded.as_mut() {
                if let Some(section) = loaded.report.sections.get_mut(pending.section_idx) {
                    if let Some(row) = section.rows.get_mut(pending.row_idx) {
                        if let FactValue::Checkbox(checked) = &mut row.value {
                            *checked = false;
                        }
                    }
                }

                loaded.report.update_disabled_states();
            }
        }
    }
}

impl App for TaxelApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.save(storage);
    }

    /// The main UI drawing function for the app, called on each frame.
    fn ui(&mut self, ctx: &mut Ui, _: &mut eframe::Frame) {
        ctx.ctx().set_visuals(if self.settings.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        });

        app::poll_load_result(self);

        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            Panel::top("header").min_size(32.0).show_inside(ctx, |ui| {
                ui::draw_header(ui, self);
            });

            let sections = self
                .loaded
                .as_ref()
                .map(|loaded| loaded.report.sections.as_slice())
                .unwrap_or(&[]);
            ui::draw_sidebar(ctx, sections, &mut self.selected_tab, &self.settings.lang);

            let diagnostic_action = if self.show_diagnostics_panel {
                ui::draw_error_panel(ctx, &self.diagnostics, &mut self.show_diagnostics_panel)
            } else {
                None
            };

            if let Some(action) = diagnostic_action {
                match action {
                    ui::DiagnosticPanelAction::NavigateToFact(fact) => {
                        if let Some(loaded) = &self.loaded {
                            ui::navigate_to_fact(
                                &fact,
                                &loaded.report,
                                &mut self.selected_tab,
                                &mut self.section_states,
                                &mut self.search,
                            );
                        }
                    }
                    ui::DiagnosticPanelAction::OpenCopyMessage {
                        message,
                        pointer_pos,
                    } => {
                        self.copy_message = Some(CopyMessage {
                            message,
                            position: pointer_pos,
                        });
                    }
                }
            }

            let lang = self.settings.lang.clone();

            let central_frame = {
                let mut frame = Frame::central_panel(ctx.style());
                if self.show_diagnostics_panel {
                    frame.inner_margin.bottom = 0;
                }
                frame
            };

            CentralPanel::default()
                .frame(central_frame)
                .show_inside(ctx, |ui| {
                    if self.loaded.is_none() {
                        if let Some(path) = ui::draw_report_list(
                            ui,
                            self.report_list.reports(),
                            self.loading.is_some(),
                            &self.settings.lang,
                        ) {
                            app::start_load(self, LoadKind::Open(path), ui.ctx().clone(), false);
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
                    let mut action = if let Some(loaded) = &self.loaded {
                        let table = &loaded.report;
                        if let Some(section) = table.sections.get(content_tab) {
                            let max_depth =
                                section.rows.iter().map(|row| row.depth).max().unwrap_or(0) + 1;
                            let state = &mut self.section_states[content_tab];

                            ui::draw_toolbar(
                                ui,
                                max_depth,
                                &mut state.max_depth,
                                &mut state.collapsed,
                                &section.rows,
                                &mut self.search,
                                table,
                                &lang,
                                editing,
                                &mut state.show_required_only,
                                &mut state.show_filled_only,
                            )
                        } else {
                            EditAction::None
                        }
                    } else {
                        EditAction::None
                    };

                    // Support keyboard shortcuts while editing and viewing.
                    let pending_section_switch =
                        self.editing_section.is_some_and(|s| s != self.selected_tab);
                    let mut validate_pressed = false;

                    if editing && !pending_section_switch {
                        let save_shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
                        let validate_shortcut =
                            KeyboardShortcut::new(Modifiers::COMMAND, Key::Space);
                        let search_shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
                        let save_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&save_shortcut));
                        validate_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&validate_shortcut));
                        let search_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&search_shortcut));
                        let cancel_pressed = ui.ctx().input(|input| input.key_pressed(Key::Escape));

                        if search_pressed {
                            self.search.focus_requested = true;
                        }

                        if save_pressed {
                            action = EditAction::Save;
                        } else if cancel_pressed {
                            action = EditAction::Cancel;
                        }
                    } else if !editing && self.loaded.is_some() {
                        let edit_shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
                        let validate_shortcut =
                            KeyboardShortcut::new(Modifiers::COMMAND, Key::Space);
                        let search_shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
                        let edit_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&edit_shortcut));
                        validate_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&validate_shortcut));
                        let search_pressed = ui
                            .ctx()
                            .input_mut(|input| input.consume_shortcut(&search_shortcut));

                        if search_pressed {
                            self.search.focus_requested = true;
                        }

                        if edit_pressed {
                            action = EditAction::Start;
                        }
                    }

                    if validate_pressed {
                        app::validate_report(self);
                    }

                    // Handle toolbar edit actions.
                    match action {
                        EditAction::Start => {
                            app::edit_report(self);
                        }
                        EditAction::Save => {
                            app::save_report(self);
                        }
                        EditAction::Cancel => {
                            app::cancel_edit(self);
                        }
                        EditAction::None => {}
                    }

                    // Table block: mutable borrow for in-place editing.
                    if let Some(loaded) = self.loaded.as_mut() {
                        let table = &mut loaded.report;
                        let tab = content_tab;
                        let table_rect = ui.available_rect_before_wrap();

                        if let Some(section) = table.sections.get_mut(tab) {
                            let highlighted_row = ui::highlight_row(&mut self.search, tab, ui);
                            let state = &mut self.section_states[tab];
                            let scroll_to = self.search.scroll_to_row.take();

                            let pending_uncheck = ui::draw_table(
                                &mut section.rows,
                                &mut state.collapsed,
                                &lang,
                                scroll_to,
                                highlighted_row,
                                editing,
                                ui,
                                state.show_required_only,
                                state.show_filled_only,
                                &self.edit_snapshot,
                            );

                            if let Some(row_idx) = pending_uncheck {
                                self.pending_report_element_uncheck =
                                    Some(PendingReportElementUncheck {
                                        section_idx: tab,
                                        row_idx,
                                    });
                                self.show_report_element_uncheck_modal = true;
                            }
                        }

                        // Keep sidebar enabled/disabled states in sync with
                        // reportElements checkboxes while editing.
                        if editing {
                            table.update_disabled_states();

                            // Only the section currently being edited can have
                            // changed this frame, so recomputing every other
                            // section's calculation tree here would be pure
                            // per-frame waste (and, for reports with many
                            // large sections, a real source of input lag).
                            if let Some(section) = table.sections.get_mut(tab) {
                                section.recompute_calculated_values();
                            }
                        }

                        if !self.search.results.is_empty() {
                            ui::draw_search_results_overlay(
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

                        ui::draw_unsaved_changes_modal(ui, &mut stay, &mut continue_nav);

                        if stay {
                            self.selected_tab = self.editing_section.unwrap();
                        }
                        if continue_nav {
                            let editing_tab = self.editing_section.unwrap();
                            if let Some(loaded) = &mut self.loaded {
                                if let Some(section) = loaded.report.sections.get_mut(editing_tab) {
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

            if !self.settings.terms_accepted {
                ui::draw_terms_modal(ctx, self);
            }

            if self.show_delete_modal {
                ui::draw_delete_modal(ctx, self);
            }

            if self.show_send_modal {
                ui::draw_send_modal(ctx, self);
            }

            if self.show_import_values_modal {
                ui::draw_import_values_modal(ctx, self);
            }

            if self.show_shortcuts_modal {
                ui::draw_shortcuts_modal(ctx, self);
            }

            if self.copy_message.is_some() {
                ui::draw_copy_message_modal(ctx, self);
            }

            if self.show_new_report_modal {
                ui::draw_new_report_modal(ctx, self);
            }

            if self.show_download_modal {
                let mut confirm = false;
                let mut cancel = false;
                ui::draw_download_modal(ctx, &mut confirm, &mut cancel);

                if confirm {
                    self.show_download_modal = false;

                    if let Some(kind) = self.pending_load_kind.take() {
                        app::start_load(self, kind, ctx.ctx().clone(), true);
                    }
                } else if cancel {
                    self.show_download_modal = false;
                    self.pending_load_kind = None;
                }
            }

            if self.show_report_element_uncheck_modal {
                let mut confirm = false;
                let mut cancel = false;
                ui::draw_report_element_uncheck_modal(ctx, &mut confirm, &mut cancel);

                if confirm {
                    self.show_report_element_uncheck_modal = false;
                    self.apply_pending_report_element_uncheck();
                } else if cancel {
                    self.show_report_element_uncheck_modal = false;
                    self.pending_report_element_uncheck = None;
                }
            }
        });
    }
}
