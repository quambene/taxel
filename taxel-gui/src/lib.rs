mod app;
mod domain;
mod infrastructure;
pub mod ui;

use anyhow::{Context, Result};
pub use app::TaxelApp;
pub use domain::{FactRow, Report, ReportMeta, ReportSection, ReportStatus};
pub use infrastructure::report_store;
use log::debug;
use std::path::{Path, PathBuf};
use xbrl_rs::{InstanceDocument, TaxonomySet};

/// Loads an XBRL instance document from the specified path, discovers the
/// referenced taxonomies, and populates the fact table with the extracted
/// facts.
pub fn load_xml(path: &Path) -> Result<(TaxonomySet, InstanceDocument), anyhow::Error> {
    debug!("Read xml file: {}", path.display());

    let instance = InstanceDocument::from_file(path)?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("missing path to taxonomies")?
        .join("test_data/taxonomies");
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;

    Ok((taxonomy, instance))
}
