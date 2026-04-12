use std::path::PathBuf;

/// The lifecycle status of a report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
    /// The file path to the report file.
    pub path: PathBuf,
    /// The creation date as a unix timestamp.
    pub created: i64,
    /// The lifecycle status of the report.
    pub status: ReportStatus,
}
