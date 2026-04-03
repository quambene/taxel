use anyhow::{Context, Result};
use log::debug;
use std::path::{Path, PathBuf};
use xbrl_rs::{
    DocumentView, InstanceDocument, ItemFact, TaxonomySet, TreeNode, ROLE_LABEL, ROLE_TERSE,
};

/// A row in the fact table, representing a single fact or a concept without
/// facts.
#[derive(Debug, Clone)]
pub struct TableRow {
    /// The concept name, e.g. "us-gaap:Assets".
    pub concept: String,
    /// The resolved label for the concept, if available.
    pub label: Option<String>,
    /// The depth of the node in the tree, used for indentation.
    pub depth: usize,
    /// The context reference for the fact, if applicable.
    pub context: String,
    /// The unit reference for the fact, if applicable.
    pub unit: Option<String>,
    /// The value of the fact, or an empty string for concepts without facts.
    pub value: String,
}

/// A simple wrapper around the fact table data.
#[derive(Debug, Default)]
pub struct FactTable {
    /// The rows of the fact table.
    pub rows: Vec<TableRow>,
}

/// Resolves the label for a given tree node, preferring terse labels over
/// regular labels, and filtering by language.
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

/// Recursively collects facts from the tree nodes and populates the fact table
/// rows.
fn collect_node(node: &TreeNode, facts: &[&ItemFact], rows: &mut Vec<TableRow>) {
    let label = resolve_label(node, "en");

    if node.fact_indices.is_empty() {
        rows.push(TableRow {
            concept: node.concept_name.to_string(),
            label: label.map(|label| label.to_string()),
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

/// Populates the fact table by traversing the document view and collecting
/// facts from the tree nodes.
fn populate_table(view: DocumentView, item_facts: &[&ItemFact], table: &mut FactTable) {
    for section in &view.sections {
        for node in &section.nodes {
            collect_node(node, item_facts, &mut table.rows);
        }
    }
}

/// Loads an XBRL instance document from the specified path, discovers the
/// referenced taxonomies, and populates the fact table with the extracted
/// facts.
pub fn load_xml(table: &mut Option<FactTable>, path: &Path) -> Result<(), anyhow::Error> {
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
    let table = table.get_or_insert_with(FactTable::default);

    populate_table(view, &item_facts, table);

    Ok(())
}
