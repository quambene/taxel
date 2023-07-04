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
pub struct TargetTags(HashMap<String, Option<String>>);

impl Default for TargetTags {
    fn default() -> Self {
        Self::new()
    }
}

impl TargetTags {
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

    /// Add required target tags for processing eBilanz.
    pub fn add_required_tags(&mut self) {
        self.insert("Verfahren", Some("ElsterBilanz"));
        self.insert("DatenArt", Some("Bilanz"));
        self.insert("Vorgang", Some("send-Auth"));
        self.insert("HerstellerID", Some("21694"));
        self.insert("Kompression", Some("GZIP"));
        self.insert("Verschluesselung", Some("CMSEncryptedData"));
        self.insert("VersionClient", Some("1"));
        self.insert("ProduktName", Some("Taxel"));
        self.insert("ProduktVersion", Some("0.1.0"));
    }
}
