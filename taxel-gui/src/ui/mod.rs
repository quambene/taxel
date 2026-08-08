mod diagnostic_panel;
mod header;
mod modals;
mod report_list_view;
mod report_view;
mod widgets;

pub use diagnostic_panel::{draw_error_panel, DiagnosticPanelAction};
pub use header::draw_header;
pub use modals::{
    draw_copy_message_modal, draw_delete_modal, draw_download_modal,
    draw_income_statement_format_modal, draw_import_values_modal, draw_new_report_modal,
    draw_remove_report_modal, draw_report_element_uncheck_modal, draw_send_modal,
    draw_shortcuts_modal, draw_terms_modal, draw_unsaved_changes_modal,
};
pub use report_list_view::{draw_report_list, ReportListAction};
pub use report_view::{
    navigation::navigate_to_fact,
    search_overlay::{draw_search_results_overlay, highlight_row},
    sidebar::draw_sidebar,
    table::draw_table,
    toolbar::{draw_toolbar, EditAction},
};
