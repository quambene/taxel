use crate::{
    app::{RowHighlight, Search, SectionState},
    domain::Report,
    ui::report_view::{
        table::{ensure_row_visible, visible_rows},
        JUMP_HIGHLIGHT_DURATION,
    },
};
use std::time::Instant;

/// Navigates to the first row in any section whose concept matches `fact`.
/// Updates `selected_tab`, uncollapses parent rows if needed, sets
/// `scroll_to_row`, and activates the jump highlight.
pub fn navigate_to_fact(
    fact: &str,
    report: &Report,
    selected_tab: &mut usize,
    section_states: &mut [SectionState],
    search: &mut Search,
) {
    let nav =
        report
            .sections
            .iter()
            .enumerate()
            .find_map(|(section_idx, section)| {
                section.rows.iter().enumerate().find_map(|(row_idx, row)| {
                    (row.concept == fact).then_some((section_idx, row_idx))
                })
            });

    let Some((section_idx, row_idx)) = nav else {
        return;
    };

    *selected_tab = section_idx;

    if let (Some(section), Some(state)) = (
        report.sections.get(section_idx),
        section_states.get_mut(section_idx),
    ) {
        ensure_row_visible(row_idx, &section.rows, &mut state.collapsed);

        let visible = visible_rows(&section.rows, &state.collapsed);

        if let Some(vis_idx) = visible.iter().position(|&raw| raw == row_idx) {
            search.scroll_to_row = Some(vis_idx);
        }
    }

    search.row_highlight = Some(RowHighlight {
        section_idx,
        row_idx,
        until: Instant::now() + JUMP_HIGHLIGHT_DURATION,
    });
}
