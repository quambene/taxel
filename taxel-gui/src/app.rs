use crate::widgets::{self, draw_dark_button, draw_unsaved_changes_modal};
use dioxus_devtools::subsecond;
use eframe::{
    egui::{self, CentralPanel, Color32, Panel, Ui},
    App, Frame,
};
use egui_extras::{Column, TableBuilder};
use rfd::FileDialog;
use std::{
    collections::HashSet,
    path::Path,
    time::{Duration, Instant},
};
use taxel_gui::{load_xml, FactRow, FactSection, FactTable, SearchHit};

const JUMP_HIGHLIGHT_DURATION: Duration = Duration::from_millis(1400);

/// Action returned by [`draw_toolbar`] when the user clicks an edit-mode
/// button.
enum EditAction {
    None,
    Start,
    Save,
    Cancel,
}

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
    /// An optional error message to display in the UI if an error occurs during
    /// XML loading or processing.
    error_message: Option<String>,
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

impl TaxelApp {
    /// Creates a new `TaxelApp` instance with the given fact table and error
    /// message. Both parameters are optional to allow starting with an empty
    /// state.
    pub fn new(table: Option<FactTable>, error_message: Option<String>) -> TaxelApp {
        let section_states = table
            .as_ref()
            .map(|t| t.sections.iter().map(|_| SectionState::default()).collect())
            .unwrap_or_default();
        Self {
            table,
            selected_tab: 0,
            section_states,
            lang: "en".to_string(),
            error_message,
            zoom_input: "100".to_string(),
            search: Search::default(),
            editing_section: None,
            edit_snapshot: Vec::new(),
        }
    }

    /// Draws the header panel of the application, including the "Import report"
    /// button, the "Clear report" button, any error messages, and the language
    /// selector tabs.
    fn draw_header(&mut self, ui: &mut Ui) {
        let mut lang_changed = false;

        ui.horizontal_centered(|ui| {
            if ui.button("Import report").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("XML", &["xml"])
                    .add_filter("All", &["*"])
                    .pick_file()
                {
                    self.load_xml(&path);
                }
            }

            if self.table.is_some() && ui.button("Clear report").clicked() {
                self.table = None;
            }

            if let Some(err) = &self.error_message {
                ui.separator();
                ui.colored_label(Color32::RED, err.to_string());
                if ui.button("Dismiss").clicked() {
                    self.error_message = None;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                lang_changed = draw_language_toolbar(ui, &mut self.lang);

                ui.separator();

                draw_dark_mode_toggle(ui);

                ui.separator();

                draw_zoom_toolbar(ui, &mut self.zoom_input);
            });
        });
    }

    /// Loads an XBRL instance document from the specified path and updates the
    /// app. If an error occurs during loading, the error message is stored in
    /// the app state to be displayed in the UI.
    fn load_xml(&mut self, path: &Path) {
        self.selected_tab = 0;
        self.table = None;
        self.editing_section = None;
        self.edit_snapshot.clear();

        if let Err(err) = load_xml(&mut self.table, path) {
            self.error_message = Some(format!("{err}"));
        }

        self.section_states = self
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
}

impl App for TaxelApp {
    /// The main UI drawing function for the app, called on each frame.
    fn ui(&mut self, ctx: &mut Ui, _: &mut Frame) {
        // TODO: remove hot reloading support for release builds
        subsecond::call(|| {
            Panel::top("header").min_size(32.0).show_inside(ctx, |ui| {
                self.draw_header(ui);
            });

            if let Some(table) = &self.table {
                draw_sidebar(ctx, table.sections.as_slice(), &mut self.selected_tab);
            }

            let lang = self.lang.clone();

            CentralPanel::default().show_inside(ctx, |ui| {
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
                    let save_shortcut =
                        egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
                    let save_pressed = ui
                        .ctx()
                        .input_mut(|input| input.consume_shortcut(&save_shortcut));
                    let cancel_pressed =
                        ui.ctx().input(|input| input.key_pressed(egui::Key::Escape));

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

/// Draw the sidebar panel containing the list of sections. Allows the user to
/// select a section to view its facts in the main table.
fn draw_sidebar(ctx: &mut Ui, sections: &[FactSection], selected: &mut usize) {
    Panel::left("sections_panel")
        .resizable(true)
        .default_size(200.0)
        .show_inside(ctx, |ui| {
            // Match the spacing above the first section in the main table for
            // visual alignment.
            ui.add_space(7.0);
            ui.label("Report sections");
            ui.add_space(2.0);

            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, section) in sections.iter().enumerate() {
                    let title = section.role.rsplit('/').next().unwrap_or(&section.role);
                    ui.selectable_value(selected, i, title);
                }
            });
        });
}

/// Determines the visible rows in the fact table based on the current collapsed
/// state.
///
/// Compute which visible-list indices should be collapsed to show only rows up
/// to `max_depth` levels. Mirrors the same traversal logic as `visible_rows` so
/// that indices are stable. Returns the raw indices (into `rows`) that should
/// be collapsed to show only `max_depth` levels. Uses raw indices so the set
/// remains stable regardless of expand/collapse state.
fn collapsed_at_depth(rows: &[FactRow], max_depth: usize) -> HashSet<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.has_children && row.depth + 1 >= max_depth)
        .map(|(i, _)| i)
        .collect()
}

/// Returns the raw indices of visible rows (positions in `rows`).
/// `collapsed` stores raw indices, which are stable across expand/collapse
/// operations.
fn visible_rows(rows: &[FactRow], collapsed: &HashSet<usize>) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut hidden_above_depth: Option<usize> = None;

    for (raw_idx, row) in rows.iter().enumerate() {
        if let Some(hide_depth) = hidden_above_depth {
            if row.depth > hide_depth {
                continue;
            }
            hidden_above_depth = None;
        }
        visible.push(raw_idx);
        if row.has_children && collapsed.contains(&raw_idx) {
            hidden_above_depth = Some(row.depth);
        }
    }

    visible
}

/// Draw the toolbar above the fact table, including the level filter, search
/// bar, and edit-mode buttons.
#[allow(clippy::too_many_arguments)]
fn draw_toolbar(
    ui: &mut Ui,
    max_available: usize,
    max_depth: &mut Option<usize>,
    collapsed: &mut HashSet<usize>,
    rows: &[FactRow],
    search: &mut Search,
    table: &FactTable,
    lang: &str,
    editing: bool,
) -> EditAction {
    let total_width = ui.available_width();
    let mut action = EditAction::None;

    ui.horizontal(|ui| {
        draw_level_toolbar(ui, max_available, max_depth, collapsed, rows);
        draw_search_bar(ui, search, table, lang, total_width);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if editing {
                if draw_dark_button(ui, "Save").clicked() {
                    action = EditAction::Save;
                }

                if ui.button("Cancel").clicked() {
                    action = EditAction::Cancel;
                }
            } else {
                let button = draw_dark_button(ui, "Edit report");

                if button.clicked() {
                    action = EditAction::Start;
                }
            }
        });
    });

    ui.separator();
    action
}

/// Draw the level filter toolbar, allowing the user to select which levels of
/// the fact tree to display. Handles the logic for collapsing/expanding rows to
/// show only rows up to the selected depth level. Also updates the `collapsed`
/// set to reflect the new collapsed state based on the selected level.
fn draw_level_toolbar(
    ui: &mut Ui,
    max_available: usize,
    max_depth: &mut Option<usize>,
    collapsed: &mut HashSet<usize>,
    rows: &[FactRow],
) {
    ui.label("Level:");

    if ui.selectable_label(max_depth.is_none(), "All").clicked() {
        *max_depth = None;
        collapsed.clear();
    }

    for depth in 1..=max_available {
        if ui
            .selectable_label(*max_depth == Some(depth), depth.to_string())
            .clicked()
        {
            *max_depth = Some(depth);
            *collapsed = collapsed_at_depth(rows, depth);
        }
    }
}

/// Draw the level filter toolbar with search bar, allowing the user to select
/// which levels of the fact tree to display and to search for
/// concepts/labels/values.
fn draw_search_bar(
    ui: &mut Ui,
    search: &mut Search,
    table: &FactTable,
    lang: &str,
    total_width: f32,
) {
    let search_width = 400.0; // icon + text field
    let cursor_x = ui.cursor().left() - ui.min_rect().left();
    let padding = ((total_width - search_width) / 2.0 - cursor_x).max(0.0);
    ui.add_space(padding);

    ui.label("\u{1F50D}");
    let response = ui.add(
        egui::TextEdit::singleline(&mut search.query)
            .desired_width(380.0)
            .hint_text("Search ID, name, value ..."),
    );

    let query = search.query.trim();

    if response.changed() {
        if query.is_empty() {
            search.results.clear();
        } else {
            search.results = table.search(query, lang);
        }
    } else if (response.gained_focus() || response.clicked()) && !query.is_empty() {
        // Re-open results for the existing query when the user returns focus
        // to the search field.
        search.results = table.search(query, lang);
    }
}

/// Ensures that a given row is visible by expanding (uncollapsing) all its
/// ancestors in the tree.
fn ensure_row_visible(row_idx: usize, rows: &[FactRow], collapsed: &mut HashSet<usize>) {
    let target_depth = rows[row_idx].depth;
    let mut depth = target_depth;
    for i in (0..row_idx).rev() {
        if rows[i].depth < depth && rows[i].has_children {
            collapsed.remove(&i);
            depth = rows[i].depth;
            if depth == 0 {
                break;
            }
        }
    }
}

/// Draw search results in a foreground overlay above the fact table. Clicking
/// a result jumps to that row in the table.
fn draw_search_results_overlay(
    ctx: &egui::Context,
    table_rect: egui::Rect,
    search: &mut Search,
    table: &FactTable,
    selected_tab: &mut usize,
    section_states: &mut [SectionState],
) {
    if search.results.is_empty() {
        return;
    }

    let mut close_overlay = ctx.input(|input| input.key_pressed(egui::Key::Escape));

    let horizontal_margin = 10.0;
    let top_outer_margin = 2.0;
    let bottom_outer_margin = 20.0;
    let overlay_width = (table_rect.width() - horizontal_margin * 2.0).min(700.0);
    let x = table_rect.center().x - overlay_width / 2.0;
    let y = table_rect.top() + top_outer_margin;
    let scroll_height = (table_rect.height() - top_outer_margin - bottom_outer_margin).max(40.0);

    let area_response = egui::Area::new(egui::Id::new("search_results_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .stroke(egui::Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .show(ui, |ui| {
                    ui.set_width(overlay_width);

                    egui::ScrollArea::vertical()
                        .id_salt("search_results")
                        .min_scrolled_height(scroll_height)
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            let mut clicked = None;

                            for (i, hit) in search.results.iter().enumerate() {
                                let row = egui::Frame::new()
                                    .fill(ui.visuals().widgets.noninteractive.bg_fill)
                                    .corner_radius(2.0)
                                    .inner_margin(egui::Margin::symmetric(8, 5))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(egui::Label::new(&hit.label).wrap());
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "{} [{}]",
                                                    hit.concept, hit.section_name
                                                ))
                                                .color(ui.visuals().weak_text_color()),
                                            )
                                            .wrap(),
                                        );
                                    });

                                let response = ui.interact(
                                    row.response.rect,
                                    ui.id().with(("search_result", i)),
                                    egui::Sense::click(),
                                );

                                if response.is_pointer_button_down_on() {
                                    ui.painter().rect_filled(
                                        row.response.rect,
                                        2.0,
                                        ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                                    );
                                } else if response.hovered() {
                                    ui.painter().rect_filled(
                                        row.response.rect,
                                        2.0,
                                        ui.visuals().widgets.hovered.bg_fill.gamma_multiply(0.25),
                                    );
                                }

                                if response.clicked() {
                                    clicked = Some(i);
                                }

                                ui.separator();
                            }

                            if let Some(i) = clicked {
                                let hit = &search.results[i];
                                let section_idx = hit.section_idx;
                                let row_idx = hit.row_idx;

                                *selected_tab = section_idx;

                                if let Some(section) = table.sections.get(section_idx) {
                                    let state = &mut section_states[section_idx];
                                    ensure_row_visible(
                                        row_idx,
                                        &section.rows,
                                        &mut state.collapsed,
                                    );

                                    let visible = visible_rows(&section.rows, &state.collapsed);
                                    if let Some(vis_idx) =
                                        visible.iter().position(|&raw| raw == row_idx)
                                    {
                                        search.scroll_to_row = Some(vis_idx);
                                    }
                                }

                                search.row_highlight = Some(RowHighlight {
                                    section_idx,
                                    row_idx,
                                    until: Instant::now() + JUMP_HIGHLIGHT_DURATION,
                                });

                                search.results.clear();
                            }
                        });
                });
        });

    let clicked_outside = ctx.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|pos| !area_response.response.rect.contains(pos))
    });

    if clicked_outside {
        close_overlay = true;
    }

    if close_overlay {
        search.results.clear();
    }
}

/// Draw the fact table in the main panel, showing only the rows that are not
/// collapsed. Handles the toggle logic for expanding/collapsing rows with
/// children.
fn draw_table(
    rows: &mut [FactRow],
    collapsed: &mut HashSet<usize>,
    lang: &str,
    scroll_to_row: Option<usize>,
    highlight_row: Option<usize>,
    editing: bool,
    ui: &mut Ui,
) {
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    let visible = visible_rows(rows, collapsed);
    let mut toggle: Option<usize> = None;

    let mut builder = TableBuilder::new(ui)
        .resizable(true)
        .striped(true)
        .column(Column::initial(250.0).clip(true))
        .column(Column::initial(500.0).clip(true))
        .column(Column::initial(120.0).clip(true))
        .column(Column::initial(60.0).clip(true))
        .column(Column::remainder().clip(true));

    if let Some(row) = scroll_to_row {
        builder = builder.scroll_to_row(row, Some(egui::Align::Center));
    }

    builder
        .header(row_height, |mut header| {
            header.col(|ui| {
                ui.label("ID");
            });
            header.col(|ui| {
                ui.label("Name");
            });
            header.col(|ui| {
                ui.label("Context");
            });
            header.col(|ui| {
                ui.label("Unit");
            });
            header.col(|ui| {
                ui.label("Value");
            });
        })
        .body(|body| {
            body.rows(row_height, visible.len(), |mut row| {
                let raw_idx = visible[row.index()];
                let is_highlighted = highlight_row == Some(raw_idx);
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.label(&rows[raw_idx].concept);
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.horizontal(|ui| {
                        let triangle_width = 12.0 + ui.spacing().item_spacing.x;
                        let indent = rows[raw_idx].depth as f32 * 24.0;

                        if rows[raw_idx].has_children {
                            ui.add_space(indent);
                            let is_collapsed = collapsed.contains(&raw_idx);

                            if widgets::triangle_button(ui, is_collapsed).clicked() {
                                toggle = Some(raw_idx);
                            }
                        } else {
                            ui.add_space(indent + triangle_width);
                        }

                        ui.label(
                            rows[raw_idx]
                                .labels
                                .get(lang)
                                .map(|label| label.as_str())
                                .unwrap_or("-"),
                        );
                    });
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.label(&rows[raw_idx].context);
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    ui.label(rows[raw_idx].unit.as_deref().unwrap_or("-"));
                });
                row.col(|ui| {
                    if is_highlighted {
                        ui.painter().rect_filled(
                            ui.max_rect(),
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
                        );
                    }
                    if editing {
                        ui.text_edit_singleline(&mut rows[raw_idx].value);
                    } else {
                        ui.label(&rows[raw_idx].value);
                    }
                });
            });
        });

    if let Some(raw_idx) = toggle {
        if collapsed.contains(&raw_idx) {
            // Expanding: reveal one level by collapsing direct children that have children.
            collapsed.remove(&raw_idx);
            let parent_depth = rows[raw_idx].depth;
            for (i, row) in rows[raw_idx + 1..].iter().enumerate() {
                if row.depth <= parent_depth {
                    break;
                }
                if row.depth == parent_depth + 1 && row.has_children {
                    collapsed.insert(raw_idx + 1 + i);
                }
            }
        } else {
            collapsed.insert(raw_idx);
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

/// Highlight the row that was jumped to via search results, if the highlight
/// duration has not yet expired.
fn highlight_row(search: &mut Search, selected_tab: usize, ui: &mut Ui) -> Option<usize> {
    let now = Instant::now();

    if search
        .row_highlight
        .as_ref()
        .is_some_and(|highlight| now >= highlight.until)
    {
        search.row_highlight = None;
    }

    let highlight_row = search.row_highlight.as_ref().and_then(|highlight| {
        if highlight.section_idx == selected_tab {
            ui.ctx().request_repaint();
            Some(highlight.row_idx)
        } else {
            None
        }
    });

    highlight_row
}
