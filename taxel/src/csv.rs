pub use csv::{Reader, ReaderBuilder, Trim, Writer, WriterBuilder};
use serde::Deserialize;

/// The outcome of [`crate::Report::apply_csv_values`].
#[derive(Debug)]
pub struct CsvImportOutcome {
    /// CSV rows that matched an existing fact (whether or not the value changed).
    pub matched: usize,
    /// CSV rows whose value differed from the current fact and were written.
    pub updated: usize,
    /// Human-readable notices for CSV rows that couldn't be matched or
    /// couldn't be written (e.g. `Dropdown` fields).
    pub warnings: Vec<String>,
}

/// A single row as written by [`crate::Report::write_values_csv`], read back
/// by [`crate::Report::apply_csv_values`].
#[derive(Debug, Deserialize)]
pub(crate) struct CsvExportRow {
    #[serde(rename = "Section")]
    #[allow(dead_code)]
    pub(crate) section: String,
    #[serde(rename = "ID")]
    pub(crate) id: String,
    #[serde(rename = "Parent")]
    pub(crate) parent: String,
    #[serde(rename = "Depth")]
    #[allow(dead_code)]
    pub(crate) depth: usize,
    #[serde(rename = "Name")]
    #[allow(dead_code)]
    pub(crate) name: String,
    #[serde(rename = "Value")]
    pub(crate) value: String,
    #[serde(rename = "Unit")]
    pub(crate) unit: String,
    #[serde(rename = "Context")]
    #[allow(dead_code)]
    pub(crate) context: String,
}
