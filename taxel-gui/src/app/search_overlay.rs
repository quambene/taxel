use super::{
    table::{ensure_row_visible, visible_rows},
    RowHighlight, Search, SectionState, JUMP_HIGHLIGHT_DURATION,
};
use eframe::egui::{
    pos2, Area, Context, Frame, Id, Key, Label, Margin, Order, Rect, RichText, ScrollArea, Sense,
    Stroke, Ui,
};
use std::time::Instant;
use taxel_gui::FactTable;

/// Draw search results in a foreground overlay above the fact table. Clicking
/// a result jumps to that row in the table.
pub(super) fn draw_search_results_overlay(
    ctx: &Context,
    table_rect: Rect,
    search: &mut Search,
    table: &FactTable,
    selected_tab: &mut usize,
    section_states: &mut [SectionState],
) {
    if search.results.is_empty() {
        return;
    }

    let mut close_overlay = ctx.input(|input| input.key_pressed(Key::Escape));

    let horizontal_margin = 10.0;
    let top_outer_margin = 2.0;
    let bottom_outer_margin = 20.0;
    let overlay_width = (table_rect.width() - horizontal_margin * 2.0).min(700.0);
    let x = table_rect.center().x - overlay_width / 2.0;
    let y = table_rect.top() + top_outer_margin;
    let scroll_height = (table_rect.height() - top_outer_margin - bottom_outer_margin).max(40.0);

    let area_response = Area::new(Id::new("search_results_overlay"))
        .order(Order::Foreground)
        .fixed_pos(pos2(x, y))
        .show(ctx, |ui| {
            Frame::popup(ui.style())
                .stroke(Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .show(ui, |ui| {
                    ui.set_width(overlay_width);

                    ScrollArea::vertical()
                        .id_salt("search_results")
                        .min_scrolled_height(scroll_height)
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            let mut clicked = None;

                            for (i, hit) in search.results.iter().enumerate() {
                                let row = Frame::new()
                                    .fill(ui.visuals().widgets.noninteractive.bg_fill)
                                    .corner_radius(2.0)
                                    .inner_margin(Margin::symmetric(8, 5))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(Label::new(&hit.label).wrap());
                                        ui.add(
                                            Label::new(
                                                RichText::new(format!(
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
                                    Sense::click(),
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

/// Highlight the row that was jumped to via search results, if the highlight
/// duration has not yet expired.
pub(super) fn highlight_row(
    search: &mut Search,
    selected_tab: usize,
    ui: &mut Ui,
) -> Option<usize> {
    let now = Instant::now();

    if search
        .row_highlight
        .as_ref()
        .is_some_and(|highlight| now >= highlight.until)
    {
        search.row_highlight = None;
    }

    search.row_highlight.as_ref().and_then(|highlight| {
        if highlight.section_idx == selected_tab {
            ui.ctx().request_repaint();
            Some(highlight.row_idx)
        } else {
            None
        }
    })
}
