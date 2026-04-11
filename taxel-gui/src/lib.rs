use anyhow::{Context, Result};
use log::debug;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use taxel::{GCD_LABEL, GCD_ROLE_URI, ROLE_URI_TO_REPORT_ELEMENT};
use xbrl_rs::{
    DocumentView, InstanceDocument, ItemFact, TaxonomySet, TreeNode, ROLE_LABEL, ROLE_TERSE,
};

/// A single search result pointing to a specific row in a specific section.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Index into `FactTable::sections`.
    pub section_idx: usize,
    /// Raw index into `FactSection::rows`.
    pub row_idx: usize,
    /// The concept name of the matched row.
    pub concept: String,
    /// The resolved label of the matched row.
    pub label: String,
    /// The section role (short name) for display.
    pub section_name: String,
}

/// A row in the fact table, representing a single fact or a concept without
/// facts.
#[derive(Debug, Clone)]
pub struct FactRow {
    /// The concept name, e.g. "bs.ass.fixAss".
    pub concept: String,
    /// Labels resolved at load time, keyed by language code (e.g. "en", "de").
    pub labels: HashMap<String, String>,
    /// The depth of the node in the tree, used for indentation.
    pub depth: usize,
    /// The context reference for the fact, if applicable.
    pub context: String,
    /// The unit reference for the fact, if applicable.
    pub unit: Option<String>,
    /// The value of the fact, or an empty string for concepts without facts.
    pub value: String,
    /// Whether this concept has child concepts in the presentation tree.
    pub has_children: bool,
}

/// One presentation section with its rows.
#[derive(Debug, Default, Clone)]
pub struct FactSection {
    /// The full extended link role URI, e.g. `http://example.com/role/BalanceSheet`.
    pub role: String,
    /// Sidebar display labels resolved from taxonomy concepts, keyed by
    /// language code (e.g. "en", "de").
    pub labels: HashMap<String, String>,
    /// The rows for this section.
    pub rows: Vec<FactRow>,
}

/// A collection of fact sections, one per presentation section in the XBRL document.
#[derive(Debug, Default)]
pub struct FactTable {
    /// The sections in the order they appear in the presentation linkbase.
    pub sections: Vec<FactSection>,
    /// Role URIs for sections that could not be mapped to a known report
    /// element concept.
    pub role_mapping_errors: Vec<String>,
}

impl FactTable {
    /// Search all sections for rows matching `query` (case-insensitive substring
    /// match on concept, label, or value).
    pub fn search(&self, query: &str, lang: &str) -> Vec<SearchHit> {
        let query_lower = query.to_lowercase();
        let mut hits = Vec::new();

        for (section_idx, section) in self.sections.iter().enumerate() {
            let section_name = section
                .labels
                .get(lang)
                .map(|lang| lang.as_str())
                .unwrap_or_else(|| section.role.rsplit('/').next().unwrap_or(&section.role));

            for (row_idx, row) in section.rows.iter().enumerate() {
                let label = row
                    .labels
                    .get(lang)
                    .map(|label| label.as_str())
                    .unwrap_or("");

                if row.concept.to_lowercase().contains(&query_lower)
                    || label.to_lowercase().contains(&query_lower)
                    || row.value.to_lowercase().contains(&query_lower)
                {
                    hits.push(SearchHit {
                        section_idx,
                        row_idx,
                        concept: row.concept.clone(),
                        label: label.to_string(),
                        section_name: section_name.to_owned(),
                    });
                }
            }
        }

        hits
    }
}

/// Loads an XBRL instance document from the specified path, discovers the
/// referenced taxonomies, and populates the fact table with the extracted
/// facts.
pub fn load_xml(path: &Path) -> Result<FactTable, anyhow::Error> {
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
    let mut table = FactTable::default();

    populate_fact_table(view, &item_facts, &mut table);

    Ok(table)
}

/// Populates the fact table by traversing the document view and collecting
/// facts from the tree nodes.
fn populate_fact_table(view: DocumentView, item_facts: &[&ItemFact], table: &mut FactTable) {
    table.sections.clear();
    table.role_mapping_errors.clear();

    // Labels for report elements are sourced from the dedicated GCD section.
    let gcd_nodes = view
        .sections
        .iter()
        .find(|section| section.role == GCD_ROLE_URI)
        .map(|section| section.nodes.as_slice())
        .unwrap_or(&[]);
    let labels_map = build_labels_map(gcd_nodes);

    for section in &view.sections {
        let role_uri = section.role;

        let labels = if role_uri == GCD_ROLE_URI {
            // The GCD section itself is a special case: we use the same label
            // for all languages since it doesn't represent a report element.
            Some(HashMap::from([
                ("en".to_owned(), GCD_LABEL.to_string()),
                ("de".to_owned(), GCD_LABEL.to_string()),
            ]))
        } else if let Some(concept_name) = ROLE_URI_TO_REPORT_ELEMENT.get(role_uri) {
            labels_map.get(concept_name).cloned()
        } else {
            table.role_mapping_errors.push(role_uri.to_owned());
            None
        };

        let mut fact_section = FactSection {
            role: role_uri.to_owned(),
            labels: labels.unwrap_or_default(),
            rows: Vec::new(),
        };

        for node in &section.nodes {
            collect_node(node, item_facts, &mut fact_section.rows);
        }

        table.sections.push(fact_section);
    }
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

/// Resolves labels for all supported languages for a given tree node.
fn resolve_labels(node: &TreeNode) -> HashMap<String, String> {
    ["en", "de"]
        .iter()
        .filter_map(|&lang| {
            resolve_label(node, lang).map(|label| (lang.to_string(), label.to_string()))
        })
        .collect()
}

/// Recursively collects facts from the tree nodes and populates the fact table
/// rows.
fn collect_node(node: &TreeNode, facts: &[&ItemFact], rows: &mut Vec<FactRow>) {
    let labels = resolve_labels(node);
    let has_children = !node.children.is_empty();

    if node.fact_indices.is_empty() {
        rows.push(FactRow {
            concept: node.concept_name.to_string(),
            labels,
            depth: node.depth,
            context: String::new(),
            unit: None,
            value: String::new(),
            has_children,
        });
    } else {
        for &idx in &node.fact_indices {
            debug!("Fact index: {idx}");

            if let Some(fact) = facts.get(idx) {
                if fact.is_nil() {
                    continue;
                }
                rows.push(FactRow {
                    concept: node.concept_name.to_string(),
                    labels: labels.clone(),
                    depth: node.depth,
                    context: fact.context_ref().to_string(),
                    unit: fact.unit_ref().map(|u| u.to_string()),
                    value: fact.value().to_string(),
                    has_children,
                });
            }
        }
    }

    for child in &node.children {
        collect_node(child, facts, rows);
    }
}

/// Recursively collects labels for a given concept name from the GCD nodes.
fn collect_labels<'a>(node: &'a TreeNode<'a>, map: &mut HashMap<&'a str, HashMap<String, String>>) {
    map.entry(node.concept_name)
        .or_insert_with(|| resolve_labels(node));

    for child in &node.children {
        collect_labels(child, map);
    }
}

/// Builds a map of concept names to their labels for all nodes in the GCD
/// section.
fn build_labels_map<'a>(nodes: &'a [TreeNode<'a>]) -> HashMap<&'a str, HashMap<String, String>> {
    let mut map = HashMap::new();

    for node in nodes {
        collect_labels(node, &mut map);
    }

    map
}
