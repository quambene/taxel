mod extract_tag_values;
mod read_tags;
mod read_tags_ods;
mod write_tags;
mod xbrl;
mod xml;

pub use csv::{
    Reader as CsvReader, ReaderBuilder as CsvReaderBuilder, Trim, WriterBuilder as CsvWriterBuilder,
};
pub use extract_tag_values::extract_tag_values;
use log::warn;
pub use quick_xml::{Reader, Writer};
pub use read_tags::read_target_tags;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
pub use write_tags::write_tags;
pub use xbrl::XbrlElement;
#[cfg(test)]
use xml::tests;

struct Attribute<'a> {
    key: &'a str,
    value: &'a str,
}

const NIL_ATTRIBUTE: Attribute = Attribute {
    key: "xsi:nil",
    value: "true",
};

const DECIMALS_2: Attribute = Attribute {
    key: "decimals",
    value: "2",
};

#[derive(Debug, PartialEq, Clone)]
/// A struct representing the supported taxonomies.
pub enum Taxonomy {
    /// The Global Common Document (GCD) financial reporting taxonomy.
    Gcd,
    /// The Generally Accepted Accounting Principles (GAAP) - current/invested
    /// (CI) - financial reporting taxonomy.
    GaapCi,
}

impl Taxonomy {
    fn as_str(&self) -> &str {
        match self {
            Self::Gcd => "gcd",
            Self::GaapCi => "gaap-ci",
        }
    }
}

impl fmt::Display for Taxonomy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let taxonomy = match self {
            Self::Gcd => "gcd",
            Self::GaapCi => "gaap-ci",
        };

        write!(f, "{}", taxonomy)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CsvRow {
    #[serde(rename = "ebilanz_key")]
    key: String,
    #[serde(rename = "ebilanz_value")]
    value: Option<String>,
}

impl CsvRow {
    pub fn new(key: String, value: Option<String>) -> Self {
        Self { key, value }
    }
}

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

impl Default for TargetTags {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq)]
pub struct TargetTags(HashMap<String, Option<String>>);

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
}
