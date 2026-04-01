use anyhow::Result;
use log::debug;
use std::path::PathBuf;
use xbrl_rs::{InstanceDocument, TaxonomySet};

#[derive(Debug, Clone)]
pub struct TableRow {
    pub concept: String,
    // Human-readable label
    pub label: Option<String>,
    pub context: String,
    pub unit: Option<String>,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct XbrlTable {
    pub rows: Vec<TableRow>,
}

pub fn load_xml(table: &mut Option<XbrlTable>, path: &PathBuf) -> Result<(), anyhow::Error> {
    debug!("Read xml file: {}", path.display());

    let instance = InstanceDocument::from_file(path)?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from("../../test_data/taxonomies");
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();

    for section in view.sections {
        for node in &section.nodes {
            for &idx in &node.fact_indices {
                debug!("Fact index: {idx}");

                if let Some(fact) = item_facts.get(idx) {
                    table
                        .get_or_insert_with(XbrlTable::default)
                        .rows
                        .push(TableRow {
                            concept: fact.id().unwrap_or_default().to_string(),
                            label: None,
                            context: fact.context_ref().to_string(),
                            unit: fact.unit_ref().map(|unit| unit.to_string()),
                            value: fact.value().to_string(),
                        });
                }
            }
        }
    }

    Ok(())
}
