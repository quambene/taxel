use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use taxel::TaxonomyType;

/// The lifecycle status of a report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReportStatus {
    #[default]
    Draft,
    Validated,
    Sent,
}

impl ReportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReportStatus::Draft => "Draft",
            ReportStatus::Validated => "Validated",
            ReportStatus::Sent => "Sent",
        }
    }
}

/// Metadata about a report.
#[derive(Clone, Debug)]
pub struct ReportMeta {
    /// The file path of the report.
    pub path: PathBuf,
    /// The creation date as a unix timestamp.
    pub created: i64,
    /// The last change date as a unix timestamp.
    pub changed: i64,
    /// The lifecycle status of the report.
    pub status: ReportStatus,
    /// The eBilanz taxonomy type.
    pub taxonomy_type: Option<TaxonomyType>,
    /// The eBilanz taxonomy version.
    pub taxonomy_version: Option<String>,
    /// The reporting period start date in `YYYY-MM-DD` format.
    pub start_date: Option<String>,
    /// The reporting period end date in `YYYY-MM-DD` format.
    pub end_date: Option<String>,
}
