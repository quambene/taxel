mod csv;
mod ebilanz;
pub mod elster;
mod eric;
mod instance_document;
mod ods;
mod report;
mod taxonomy_loader;
mod xbrl;
mod xml;

pub use crate::{
    csv::{
        CsvImportOutcome, Reader as CsvReader, ReaderBuilder as CsvReaderBuilder, Trim,
        Writer as CsvWriter, WriterBuilder as CsvWriterBuilder,
    },
    ebilanz::{
        taxonomy_version_from_schema_refs, TaxonomyType, BASELINE_ROLE_URIS, CLOSING_DATE,
        COMPANY_CITY, COMPANY_COUNTRY, COMPANY_HOUSE_NO, COMPANY_NAME, COMPANY_STREET,
        COMPANY_TAX_NUMBER, COMPANY_TAX_NUMBER_PARENT, COMPANY_ZIP_CODE, FISCAL_YEAR_BEGIN,
        FISCAL_YEAR_END, GCD_LABEL, GCD_ROLE_URI, REPORT_ELEMENT_PREFIX,
        REPORT_ELEMENT_TO_ROLE_URI, REQUIRED_GCD_FACTS, REQUIRED_NIL_TUPLE_CHILDREN,
        ROLE_URI_TO_REPORT_ELEMENT, TAXONOMY_DATE_TO_VERSION, TAXONOMY_TYPES,
        TAXONOMY_VERSION_TO_DATE,
    },
    elster::{ElsterReport, TEST_MARKER},
    eric::extract_fact_name,
    instance_document::{
        active_roles, create_instance_document, create_item_fact, ensure_nil_tuple_child,
        extract_period, remove_forbidden_facts, remove_trade_accounting_facts,
        restore_required_nil_tuple_children,
    },
    report::{FactRow, FactValue, Report, ReportSection},
    taxonomy_loader::{download_taxonomy, load_taxonomies, schema_ref_paths, taxonomy_dir},
};
use log::warn;
pub use quick_xml::{Reader, Writer};
use std::collections::HashMap;
pub use xbrl::XbrlElement;
pub use xml::{extract_tag_values, remove_formatting, write_declaration};

#[derive(Debug, PartialEq)]
pub struct Tag {
    pub name: String,
    pub value: Option<String>,
}

impl Tag {
    pub fn new(name: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        Self {
            name: name.into(),
            value: value.map(|inner| inner.into()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Tags(HashMap<String, Option<String>>);

impl Default for Tags {
    fn default() -> Self {
        Self::new()
    }
}

impl Tags {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get(&self, target_key: &str) -> Option<&Option<String>> {
        self.0.get(target_key)
    }

    pub fn insert(
        &mut self,
        target_key: impl Into<String>,
        target_value: Option<impl Into<String>>,
    ) {
        let key = target_key.into();
        let value = target_value.map(|inner| inner.into());
        let entry = self.0.insert(key.clone(), value);

        if entry.is_some() {
            warn!("Duplicate key '{key}'");
        }
    }

    pub fn remove(&mut self, target_key: impl Into<String>) {
        let key = target_key.into();
        let entry = self.0.remove(&key);

        if let Some(entry) = entry {
            warn!("Key not supported: '{key}', removing value: '{entry:#?}'",);
        }
    }
}
