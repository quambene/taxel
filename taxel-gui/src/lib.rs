mod app;
mod domain;
mod infrastructure;
mod ui;

pub use app::TaxelApp;
// Narrowly re-exported (not a stable public API) solely so
// `tests/rebuild_instance.rs` can exercise `rebuild_instance` against real
// taxonomy fixtures without the whole `app`/`domain` module trees being
// part of the crate's public surface.
#[doc(hidden)]
pub use app::{
    report::{rebuild_instance, LoadedReport, NewReportForm},
    report_list::ReportList,
    search::Search,
    settings::Settings,
    SectionState,
};
#[doc(hidden)]
pub use domain::{update_instance_document, FactValue, Report, ReportStatus};
