mod diagnostic_panel;
mod header;
mod modals;
mod report_list_view;
mod report_view;
mod widgets;

pub use diagnostic_panel::draw_error_panel;
pub use header::draw_header;
pub use modals::{
    draw_delete_modal, draw_download_modal, draw_new_report_modal, draw_send_modal,
    draw_shortcuts_modal, draw_unsaved_changes_modal,
};
pub use report_list_view::draw_report_list;
pub use report_view::{
    search_overlay::{draw_search_results_overlay, highlight_row},
    sidebar::draw_sidebar,
    table::draw_table,
    toolbar::{draw_toolbar, EditAction},
};
