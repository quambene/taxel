mod error_panel;
mod header;
mod search_overlay;
mod sidebar;
mod table;
mod toolbar;

use self::{
    error_panel::draw_error_panel,
    header::draw_header,
    search_overlay::{draw_search_results_overlay, highlight_row},
    sidebar::draw_sidebar,
    table::draw_table,
    toolbar::{draw_toolbar, EditAction},
};
use crate::widgets::draw_unsaved_changes_modal;
use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Key, KeyboardShortcut, Modifiers, Panel, Ui},
    App, CreationContext, Frame,
};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use taxel_gui::{FactTable, SearchHit};

const JUMP_HIGHLIGHT_DURATION: Duration = Duration::from_millis(1400);

/// Transient highlight for a row that was jumped to via search results, cleared
/// after a short duration.
struct RowHighlight {
    section_idx: usize,
    row_idx: usize,
    until: Instant,
}

/// Grouped search state.
#[derive(Default)]
struct Search {
    /// The current search query text.
    query: String,
    /// Cached search results, updated when the query or language changes.
    results: Vec<SearchHit>,
    /// Visible row index to scroll to after a search result click. Consumed
    /// after one frame.
    scroll_to_row: Option<usize>,
    /// Transient highlight for the row selected via search results.
    row_highlight: Option<RowHighlight>,
}

/// Per-section UI state (collapse state and depth filter).
#[derive(Default)]
struct SectionState {
    /// Row indices whose children are collapsed.
    collapsed: HashSet<usize>,
    /// Maximum depth to display. None means show all depths.
    max_depth: Option<usize>,
}

/// Main application struct for the Taxel GUI, managing the state of the app.
pub struct TaxelApp {
    /// The fact table containing the extracted facts from the XBRL instance
    /// document.
    table: Option<FactTable>,
    /// The index of the currently selected section tab in the sidebar.
    selected_tab: usize,
    /// Per-section UI state, indexed analogous to `table.sections`.
    section_states: Vec<SectionState>,
    /// The currently selected language for labels (e.g. "en", "de").
    lang: String,
    /// Structured diagnostics for non-blocking and blocking issues.
    issues: Vec<AppIssue>,
    /// Controls whether the detailed diagnostics panel is visible.
    show_error_panel: bool,
    /// The text buffer for the zoom percentage input field.
    zoom_input: String,
    /// Search state.
    search: Search,
    /// Some while the value column of that section is being edited, None
    /// otherwise.
    editing_section: Option<usize>,
    /// Snapshot of `row.value` for every row in the edited section at the
    /// moment editing started, indexed by raw row index. Used to restore values
    /// if editing is canceled.
    edit_snapshot: Vec<String>,
}

/// Indicates the issue severity for diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IssueSeverity {
    Error,
    Warning,
}

/// Collects all information about an error or warning to display in the
/// diagnostics panel and error summary in the header.
#[derive(Clone, Debug)]
pub(super) struct AppIssue {
    pub(super) severity: IssueSeverity,
    pub(super) message: String,
}

impl TaxelApp {
    /// Creates a new `TaxelApp` instance with the given fact table and error
    /// message. Both parameters are optional to allow starting with an empty
    /// state. Loads persisted settings (language, zoom) from eframe storage if
    /// available.
    pub fn new(
        ctx: &CreationContext<'_>,
        table: Option<FactTable>,
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
        let mut issues = Vec::new();

        if let Some(message) = error_message {
            issues.push(AppIssue {
                severity: IssueSeverity::Error,
                message,
            });
        }

        let show_error_panel = !issues.is_empty();

        let lang = ctx
            .storage
            .and_then(|s| eframe::get_value::<String>(s, "lang"))
            .unwrap_or_else(|| "en".to_string());

        let zoom_input = ctx
            .storage
            .and_then(|storage| eframe::get_value::<String>(storage, "zoom_input"))
            .unwrap_or_else(|| "100".to_string());

        if let Ok(percent) = zoom_input.trim().parse::<u32>() {
            ctx.egui_ctx.set_zoom_factor(percent as f32 / 100.0);
        }

        Self {
            table,
            selected_tab: 0,
            section_states,
            lang,
            issues,
            show_error_panel,
            zoom_input,
            search: Search::default(),
            editing_section: None,
            edit_snapshot: Vec::new(),
        }
    }
}

impl App for TaxelApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "lang", &self.lang);
        eframe::set_value(storage, "zoom_input", &self.zoom_input);
    }

    /// The main UI drawing function for the app, called on each frame.
    fn ui(&mut self, ctx: &mut Ui, _: &mut Frame) {
        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            Panel::top("header").min_size(32.0).show_inside(ctx, |ui| {
                draw_header(self, ui);
            });

            if let Some(table) = &self.table {
                draw_sidebar(
                    ctx,
                    table.sections.as_slice(),
                    &mut self.selected_tab,
                    &self.lang,
                );
            }

            if !self.issues.is_empty() && self.show_error_panel {
                draw_error_panel(ctx, &self.issues, &mut self.show_error_panel);
            }

            let lang = self.lang.clone();

            let central_frame = {
                let mut frame = egui::Frame::central_panel(ctx.style());
                if !self.issues.is_empty() && self.show_error_panel {
                    frame.inner_margin.bottom = 0;
                }
                frame
            };

            CentralPanel::default()
                .frame(central_frame)
                .show_inside(ctx, |ui| {
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
                    let mut action = if let Some(table) = &self.table {
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
                            if let Some(table) = &self.table {
                                if let Some(section) = table.sections.get(self.selected_tab) {
                                    self.edit_snapshot =
                                        section.rows.iter().map(|r| r.value.clone()).collect();
                                }
                            }
                            self.editing_section = Some(self.selected_tab);
                        }
                        EditAction::Save => {
                            self.editing_section = None;
                            self.edit_snapshot.clear();
                        }
                        EditAction::Cancel => {
                            let editing_tab = self.editing_section.unwrap_or(self.selected_tab);

                            if let Some(table) = &mut self.table {
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
                    if let Some(table) = self.table.as_mut() {
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
                            if let Some(table) = &mut self.table {
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
                })
        });
    }
}
