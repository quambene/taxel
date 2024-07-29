mod csv;
mod ods;
mod xbrl;
mod xml;

pub use crate::csv::{
    read_tags, write_tags, Reader as CsvReader, ReaderBuilder as CsvReaderBuilder, Trim,
    Writer as CsvWriter, WriterBuilder as CsvWriterBuilder,
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

        if entry.is_some() {
            warn!(
                "Key not supported: '{key}', removing value: '{:#?}'",
                entry.unwrap()
            );
        }
    }

    /// Add required target tags for processing eBilanz.
    pub fn add_required_tags(&mut self) {
        self.insert("Verfahren", Some("ElsterBilanz"));
        self.insert("DatenArt", Some("Bilanz"));
        self.insert("Vorgang", Some("send-Auth"));
        self.insert("HerstellerID", Some("00000"));
        self.insert("Kompression", Some("GZIP"));
        self.insert("Verschluesselung", Some("CMSEncryptedData"));
        self.insert("VersionClient", Some("1"));
        self.insert("ProduktName", Some("Taxel"));
        self.insert("ProduktVersion", Some("0.1.0"));
        self.insert("Testmerker", Some("700000004"));
    }

    /// Remove unsupported tags
    /// TODO: populate content from multiple selection and dropdown fields from CSV to XBRL file
    pub fn remove_unsupported_tags(&mut self) {
        // dropdown fields
        self.remove("de-gcd:genInfo.report.id.reportType.reportType.JA");
        self.remove("de-gcd:genInfo.report.id.reportStatus.reportStatus.E");
        self.remove("de-gcd:genInfo.report.id.revisionStatus.revisionStatus.E");
        self.remove("de-gcd:genInfo.report.id.reportElement.reportElements.B");
        self.remove("de-gcd:genInfo.report.id.reportElement.reportElements.GuVMicroBilG");
        self.remove("de-gcd:genInfo.report.id.reportElement.reportElements.BVV");
        self.remove("de-gcd:genInfo.report.id.statementType.statementType.E");
        self.remove("de-gcd:genInfo.report.id.incomeStatementendswithBalProfit ");
        self.remove("de-gcd:genInfo.report.id.accountingStandard.accountingStandard.AO");
        self.remove("de-gcd:genInfo.report.id.incomeStatementFormat.incomeStatementFormat.GKV");
        self.remove("de-gcd:genInfo.report.id.consolidationRange.consolidationRange.EA");
        self.remove("de-gcd:genInfo.company.id.incomeClassification.trade");
        self.remove("de-gcd:genInfo.company.id.shareholder.legalStatus.legalStatus.KOER");
    }
}
