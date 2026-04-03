use anyhow::{Context, Result};
use log::debug;
use std::path::PathBuf;
use xbrl_rs::{InstanceDocument, ItemFact, TaxonomySet, TreeNode, ROLE_LABEL, ROLE_TERSE};

#[derive(Debug, Clone)]
pub struct TableRow {
    pub concept: String,
    // Human-readable label
    pub label: Option<String>,
    pub depth: usize,
    pub context: String,
    pub unit: Option<String>,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct XbrlTable {
    pub rows: Vec<TableRow>,
}

fn resolve_label<'a>(node: &'a TreeNode<'a>, lang: &str) -> Option<&'a str> {
    if let Some(label) = node
        .labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_TERSE)
    {
        return Some(label.text.as_str());
    }

    if let Some(label) = node
        .labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_LABEL)
    {
        return Some(label.text.as_str());
    }

    None
}

fn collect_node(node: &TreeNode, facts: &[&ItemFact], rows: &mut Vec<TableRow>) {
    let label = resolve_label(node, "en");

    if node.fact_indices.is_empty() {
        rows.push(TableRow {
            concept: node.concept_name.to_string(),
            label: label.map(|l| l.to_string()),
            depth: node.depth,
            context: String::new(),
            unit: None,
            value: String::new(),
        });
    } else {
        for &idx in &node.fact_indices {
            debug!("Fact index: {idx}");

            if let Some(fact) = facts.get(idx) {
                if fact.is_nil() {
                    continue;
                }
                rows.push(TableRow {
                    concept: node.concept_name.to_string(),
                    label: label.map(|l| l.to_string()),
                    depth: node.depth,
                    context: fact.context_ref().to_string(),
                    unit: fact.unit_ref().map(|u| u.to_string()),
                    value: fact.value().to_string(),
                });
            }
        }
    }

    for child in &node.children {
        collect_node(child, facts, rows);
    }
}

pub fn load_xml(table: &mut Option<XbrlTable>, path: &PathBuf) -> Result<(), anyhow::Error> {
    debug!("Read xml file: {}", path.display());

    let instance = InstanceDocument::from_file(path)?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("missing path to taxonomies")?
        .join("test_data/taxonomies");
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();

    let rows = table.get_or_insert_with(XbrlTable::default);

    for section in &view.sections {
        for node in &section.nodes {
            collect_node(node, &item_facts, &mut rows.rows);
        }
    }

    Ok(())
}
