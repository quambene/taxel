mod read_target_tags;
mod read_target_tags_ods;
mod remove_tag_values;
mod update_target_tags;
mod xml;

pub use csv::{ReaderBuilder, Trim};
use log::warn;
pub use quick_xml::{Reader, Writer};
pub use read_target_tags::read_target_tags;
use std::collections::HashMap;
pub use update_target_tags::update_target_tags;
#[cfg(test)]
pub use xml::tests;

#[derive(Debug)]
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

        if entry.is_none() {
            warn!("Duplicate key '{key}'");
        }
    }
}
