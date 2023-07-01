mod extract_tag_values;
mod read_target_tags;
mod read_target_tags_ods;
mod remove_tag_values;
mod update_tag_values;
mod xml;

pub use csv::{Reader as CsvReader, ReaderBuilder as CsvReaderBuilder, Trim};
pub use extract_tag_values::extract_tag_values;
use log::warn;
pub use quick_xml::{Reader, Writer};
pub use read_target_tags::read_target_tags;
pub use remove_tag_values::remove_tag_values;
use std::{collections::HashMap, fmt};
pub use update_tag_values::update_tag_values;
#[cfg(test)]
use xml::tests;

const XBRL_ATTRIBUTE: &str = "xbrli:xbrl";
const DECIMAL_ATTRIBUTE: &str = "decimals";

struct Attribute<'a> {
    key: &'a str,
    value: &'a str,
}

const NIL_ATTRIBUTE: Attribute = Attribute {
    key: "xsi:nil",
    value: "true",
};

const DECIMALS_0: Attribute = Attribute {
    key: "decimals",
    value: "0",
};

const DECIMALS_2: Attribute = Attribute {
    key: "decimals",
    value: "2",
};

/// A struct representing the supported taxonomies.
enum Taxonomy {
    /// The Global Common Document (GCD) financial reporting taxonomy.
    Gcd,
    /// The Generally Accepted Accounting Principles (GAAP) - current/invested
    /// (CI) - financial reporting taxonomy.
    GaapCi,
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

#[derive(Debug, PartialEq)]
enum XmlMode {
    /// A plain xml file.
    Plain,
    /// An xml in the xbrl standard.
    Xbrl,
}

#[derive(Debug, PartialEq)]
pub struct Tag {
    name: String,
    value: Option<String>,
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
