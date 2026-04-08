use super::Search;
use crate::{app::table::collapsed_at_depth, widgets::draw_dark_button};
use eframe::egui::{self, Ui};
use std::collections::HashSet;
use taxel_gui::{FactRow, FactTable};

/// Action returned by [`draw_toolbar`] when the user clicks an edit-mode
/// button.
pub(super) enum EditAction {
    None,
    Start,
    Save,
    Cancel,
}

/// Draw the toolbar above the fact table, including the level filter, search
/// bar, and edit-mode buttons.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_toolbar(
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
/// the fact tree to display.
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
