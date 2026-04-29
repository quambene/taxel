use crate::domain::ReportStatus;
use std::{collections::HashMap, path::PathBuf};
use taxel::{GCD_LABEL, GCD_ROLE_URI, ROLE_URI_TO_REPORT_ELEMENT};
use taxel::TaxonomyType;
use xbrl_rs::{DocumentView, ItemFact, TreeNode, ROLE_LABEL, ROLE_TERSE};

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
    /// Depth-first index into `InstanceDocument`, used to write back edits via
    /// `InstanceDocument::set_fact_value`. `None` for concept-only rows that
    /// have no associated fact.
    pub fact_index: Option<usize>,
}

/// One presentation section with its rows.
#[derive(Debug, Default, Clone)]
pub struct ReportSection {
    /// The full extended link role URI, e.g.
    /// `http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet`.
    pub role: String,
    /// Sidebar display labels resolved from taxonomy concepts, keyed by
    /// language code (e.g. "en", "de").
    pub labels: HashMap<String, String>,
    /// The rows for this section.
    pub rows: Vec<FactRow>,
    /// Whether this section is disabled.
    ///
    /// A report section is disabled if the corresponding report element from
    /// the GCD section is announced but has no value (i.e. `xsi:nil="true"`).
    /// Disabled sections are still displayed in the sidebar but are visually
    /// de-emphasized and can't be edited.
    pub disabled: bool,
}

/// The report containing the extracted facts from the XBRL instance document,
/// enriched with the concept labels and presentation structure.
#[derive(Debug)]
pub struct Report {
    /// The file path of the report, used for persistence.
    pub path: PathBuf,
    /// The taxonomy type, derived from the schema ref URLs in the instance document.
    pub taxonomy_type: TaxonomyType,
    /// The report status for lifecycle management.
    pub status: ReportStatus,
    /// The sections in the order they appear in the presentation linkbase.
    pub sections: Vec<ReportSection>,
    /// Role URIs for sections that could not be mapped to a known report
    /// element concept.
    pub role_mapping_errors: Vec<String>,
}

impl Report {
    pub fn new(path: PathBuf, taxonomy_type: TaxonomyType) -> Self {
        Self {
            path,
            taxonomy_type,
            status: ReportStatus::Draft,
            sections: Vec::new(),
            role_mapping_errors: Vec::new(),
        }
    }

    /// Populates the fact table by traversing the document view and collecting
    /// facts from the tree nodes.
    pub fn populate(&mut self, view: DocumentView, item_facts: &[&ItemFact]) {
        self.sections.clear();
        self.role_mapping_errors.clear();

        // Labels for report elements are sourced from the dedicated GCD section.
        let gcd_nodes = view
            .sections
            .iter()
            .find(|section| section.role == GCD_ROLE_URI)
            .map(|section| section.nodes.as_slice())
            .unwrap_or(&[]);
        let labels_map = build_labels_map(gcd_nodes);
        let announced_roles = collect_announced_roles(gcd_nodes, item_facts);

        for section in &view.sections {
            let role_uri = section.role;

            let disabled = if role_uri == GCD_ROLE_URI {
                false
            } else {
                match announced_roles.get(role_uri) {
                    None => continue,
                    Some(&enabled) => !enabled,
                }
            };

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
                self.role_mapping_errors.push(role_uri.to_owned());
                None
            };

            let mut fact_section = ReportSection {
                role: role_uri.to_owned(),
                labels: labels.unwrap_or_default(),
                rows: Vec::new(),
                disabled,
            };

            for node in &section.nodes {
                collect_node(node, item_facts, &mut fact_section.rows);
            }

            self.sections.push(fact_section);
        }

        // Enabled sections are shown first, followed by disabled sections. The
        // relative order within each group is determined by the original order
        // in the presentation linkbase.
        self.sections.sort_by_key(|section| section.disabled);
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

/// Returns a map from role URI to enabled state for all announced report
/// elements in the GCD section. `true` means the fact has a non-nil value
/// (section is active); `false` means it was declared with `xsi:nil="true"`.
fn collect_announced_roles(
    nodes: &[TreeNode<'_>],
    facts: &[&ItemFact],
) -> HashMap<&'static str, bool> {
    let concept_to_role: HashMap<&'static str, &'static str> = ROLE_URI_TO_REPORT_ELEMENT
        .iter()
        .map(|(&role, &concept)| (concept, role))
        .collect();

    let mut announced = HashMap::new();

    for node in nodes {
        collect_announced_roles_node(node, facts, &concept_to_role, &mut announced);
    }

    announced
}

/// Recursively traverses the GCD tree nodes to find concepts that are announced
/// as report elements and collects their corresponding role URIs.
fn collect_announced_roles_node(
    node: &TreeNode<'_>,
    facts: &[&ItemFact],
    concept_to_role: &HashMap<&'static str, &'static str>,
    announced: &mut HashMap<&'static str, bool>,
) {
    if let Some(&role) = concept_to_role.get(node.concept_name) {
        if !node.fact_indices.is_empty() {
            let enabled = node
                .fact_indices
                .iter()
                .any(|&idx| facts.get(idx).is_some_and(|fact| !fact.is_nil()));
            announced.insert(role, enabled);
        }
    }

    for child in &node.children {
        collect_announced_roles_node(child, facts, concept_to_role, announced);
    }
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
            fact_index: None,
        });
    } else {
        for &idx in &node.fact_indices {
            if let Some(fact) = facts.get(idx) {
                rows.push(FactRow {
                    concept: node.concept_name.to_string(),
                    labels: labels.clone(),
                    depth: node.depth,
                    context: fact.context_ref().to_string(),
                    unit: fact.unit_ref().map(|u| u.to_string()),
                    value: fact.value().to_string(),
                    has_children,
                    fact_index: Some(idx),
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
