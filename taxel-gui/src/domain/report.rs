use crate::domain::ReportStatus;
use std::{collections::HashMap, path::PathBuf};
use taxel::{
    ElsterReport, TaxonomyType, CLOSING_DATE, COMPANY_CITY, COMPANY_COUNTRY, COMPANY_HOUSE_NO,
    COMPANY_NAME, COMPANY_STREET, COMPANY_TAX_NUMBER, COMPANY_TAX_NUMBER_PARENT, COMPANY_ZIP_CODE,
    FISCAL_YEAR_BEGIN, FISCAL_YEAR_END, GCD_LABEL, GCD_ROLE_URI, ROLE_URI_TO_REPORT_ELEMENT,
};
use xbrl_rs::{
    Concept, ConceptView, DocumentView, InstanceDocument, ItemFact, Label, Particle, Period,
    TaxonomySet, TreeNode, TupleParticleView, ROLE_LABEL, ROLE_TERSE,
};

/// The value of a fact, determining both its display widget and write-back behaviour.
#[derive(Debug, Clone, PartialEq)]
pub enum FactValue {
    /// Plain text — rendered as a text input.
    Text(String),
    /// Boolean presence — rendered as a checkbox. `true` means the fact is non-nil.
    Checkbox(bool),
    /// Single-select choice — rendered as a dropdown. `selected` is the local concept
    /// name of the active option; `options` are `(key, lang→label)` pairs sourced from
    /// the presentation children of the parent tuple.
    Dropdown {
        selected: String,
        options: Vec<(String, HashMap<String, String>)>,
    },
}

impl Default for FactValue {
    fn default() -> Self {
        FactValue::Text(String::new())
    }
}

/// A row in the fact table, representing a single fact or a concept without
/// facts.
#[derive(Debug, Clone)]
pub struct FactRow {
    /// The concept name, e.g. "bs.ass.fixAss".
    pub concept: String,
    /// Immediate parent concept in the presentation tree, if any.
    pub parent_concept: Option<String>,
    /// Labels resolved at load time, keyed by language code (e.g. "en", "de").
    pub labels: HashMap<String, String>,
    /// The depth of the node in the tree, used for indentation.
    pub depth: usize,
    /// The context reference for the fact, if applicable.
    pub context: String,
    /// The unit reference for the fact, if applicable.
    pub unit: Option<String>,
    /// The value of the fact. `Text("")` for concepts without facts.
    pub value: FactValue,
    /// Whether this concept has child concepts in the presentation tree.
    pub has_children: bool,
    /// Depth-first index into `InstanceDocument`, used to write back edits via
    /// `InstanceDocument::set_fact_value`. `None` for concept-only rows that
    /// have no associated fact.
    pub fact_index: Option<usize>,
    /// Whether this concept is a "Mussfeld" (required field) per the taxonomy
    /// reference linkbase.
    pub is_required: bool,
    /// Whether this concept is abstract in the taxonomy schema.
    pub is_abstract: bool,
    /// Whether this row represents a tuple-origin concept in the tree.
    pub is_tuple: bool,
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
    pub fn populate(
        &mut self,
        view: DocumentView,
        item_facts: &[&ItemFact],
        taxonomy: &TaxonomySet,
    ) {
        self.sections.clear();
        self.role_mapping_errors.clear();

        let concept_map: HashMap<&str, &Concept> = taxonomy
            .elements()
            .into_iter()
            .map(|concept| (concept.name.local_name.as_str(), concept))
            .collect();

        // Maps each substitution-group head's local name to all non-abstract
        // concepts that directly substitute for it. Built once here so
        // collect_choice_options can do O(1) lookups instead of scanning all
        // taxonomy elements per abstract head.
        let mut substitution_map: HashMap<&str, Vec<&Concept>> = HashMap::new();

        for concept in taxonomy.elements() {
            if !concept.is_abstract {
                let head = concept.substitution_group.original.local_name.as_str();
                substitution_map.entry(head).or_default().push(concept);
            }
        }

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
                collect_node(
                    node,
                    item_facts,
                    &concept_map,
                    &substitution_map,
                    taxonomy,
                    &mut fact_section.rows,
                    false,
                    None,
                );
            }

            self.sections.push(fact_section);
        }

        // Enabled sections are shown first, followed by disabled sections. The
        // relative order within each group is determined by the original order
        // in the presentation linkbase.
        self.sections.sort_by_key(|section| section.disabled);
    }

    /// Recomputes `disabled` on every non-GCD section from the current in-memory
    /// GCD checkbox rows, without touching `InstanceDocument`. Called during
    /// editing so that toggling a `reportElements` checkbox immediately enables or
    /// disables the corresponding sidebar section.
    pub fn update_disabled_states(&mut self) {
        let concept_to_role: HashMap<&str, &'static str> = ROLE_URI_TO_REPORT_ELEMENT
            .iter()
            .map(|(&role, &concept)| (concept, role))
            .collect();

        let enabled_roles: HashMap<&str, bool> = self
            .sections
            .iter()
            .find(|section| section.role == GCD_ROLE_URI)
            .map(|gcd| {
                gcd.rows
                    .iter()
                    .filter_map(|row| {
                        concept_to_role
                            .get(row.concept.as_str())
                            .map(|&role| (role, matches!(row.value, FactValue::Checkbox(true))))
                    })
                    .collect()
            })
            .unwrap_or_default();

        for section in &mut self.sections {
            if section.role == GCD_ROLE_URI {
                continue;
            }
            if let Some(&enabled) = enabled_roles.get(section.role.as_str()) {
                section.disabled = !enabled;
            }
        }

        // Enabled sections are shown first, followed by disabled sections.
        self.sections.sort_by_key(|section| section.disabled);
    }

    /// Returns the text value of the first fact matching `concept` (and
    /// optionally `parent_concept`) in the section identified by `role`.
    /// Returns `None` if not found or empty.
    pub fn find_in_section(
        &self,
        role: &str,
        concept: &str,
        parent_concept: Option<&str>,
    ) -> Option<&str> {
        self.sections
            .iter()
            .find(|section| section.role == role)
            .and_then(|section| {
                section
                    .rows
                    .iter()
                    .find(|row| {
                        row.concept == concept
                            && parent_concept
                                .is_none_or(|parent| row.parent_concept.as_deref() == Some(parent))
                    })
                    .and_then(|row| match &row.value {
                        FactValue::Text(text) if !text.is_empty() => Some(text.as_str()),
                        _ => None,
                    })
            })
    }

    /// Propagates GCD fact values into the ElsterReport envelope and all XBRL
    /// contexts so that the saved file stays consistent with what the user entered.
    pub fn sync_gcd_to_elster(&self, instance: &mut InstanceDocument, elster: &mut ElsterReport) {
        let fiscal_year_begin = self.find_in_section(GCD_ROLE_URI, FISCAL_YEAR_BEGIN, None);
        let fiscal_year_end = self.find_in_section(GCD_ROLE_URI, FISCAL_YEAR_END, None);
        let closing_date = self.find_in_section(GCD_ROLE_URI, CLOSING_DATE, None);
        let tax_number = self.find_in_section(
            GCD_ROLE_URI,
            COMPANY_TAX_NUMBER,
            Some(COMPANY_TAX_NUMBER_PARENT),
        );
        let company_name = self.find_in_section(GCD_ROLE_URI, COMPANY_NAME, None);
        let street = self.find_in_section(GCD_ROLE_URI, COMPANY_STREET, None);
        let house_no = self.find_in_section(GCD_ROLE_URI, COMPANY_HOUSE_NO, None);
        let zip_code = self.find_in_section(GCD_ROLE_URI, COMPANY_ZIP_CODE, None);
        let city = self.find_in_section(GCD_ROLE_URI, COMPANY_CITY, None);
        let country = self.find_in_section(GCD_ROLE_URI, COMPANY_COUNTRY, None);

        // Update Submitter (transfer header; payload header if present).
        let street_full = match (street, house_no) {
            (Some(street), Some(house_no)) => Some(format!("{street} {house_no}")),
            (Some(street), None) => Some(street.to_string()),
            (None, Some(house_no)) => Some(house_no.to_string()),
            (None, None) => None,
        };

        if let Some(payload_block) = elster.data_section.payload_blocks.first_mut() {
            if let Some(submitter) = payload_block.payload_header.submitter.as_mut() {
                if let Some(company_name) = company_name {
                    submitter.name = company_name.to_string();
                }

                submitter.street = street_full.clone();
                submitter.postal_code = zip_code.map(str::to_string);
                submitter.city = city.map(str::to_string);
                submitter.country = country.map(str::to_string);
            }
        }

        // Update Recipient (first 4 digits of ST13 = BUFA code) and balance date.
        if let Some(payload_block) = elster.data_section.payload_blocks.first_mut() {
            if let Some(bufa) = tax_number.and_then(|s| s.get(..4)) {
                payload_block.payload_header.recipient.id = "F".to_string();
                payload_block.payload_header.recipient.value = bufa.to_string();
            }

            if let Some(closing_date) = closing_date {
                if let Ok(date_u32) = closing_date.replace('-', "").parse::<u32>() {
                    payload_block.ebilanz.balance_date = date_u32;
                }
            }
        }

        // Update all XBRL contexts: entity identifier and period dates.
        for ctx in instance.contexts_mut().values_mut() {
            if let Some(tax_number) = tax_number {
                ctx.entity.value = tax_number.to_string();
            }

            match &mut ctx.period {
                Period::Instant { date } => {
                    if let Some(end) = fiscal_year_end {
                        *date = end.to_string();
                    }
                }
                Period::Duration { start, end } => {
                    if let Some(begin) = fiscal_year_begin {
                        *start = begin.to_string();
                    }

                    if let Some(end_date) = fiscal_year_end {
                        *end = end_date.to_string();
                    }
                }
                Period::Forever => {}
            }
        }
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

/// Resolves labels from a raw label slice using the same terse-then-standard
/// fallback as `resolve_label`.
fn resolve_labels_from_labels(labels: &[Label]) -> HashMap<String, String> {
    ["en", "de"]
        .iter()
        .filter_map(|&lang| {
            labels
                .iter()
                .find(|label| label.lang == lang && label.role == ROLE_TERSE)
                .or_else(|| {
                    labels
                        .iter()
                        .find(|label| label.lang == lang && label.role == ROLE_LABEL)
                })
                .map(|label| (lang.to_string(), label.text.clone()))
        })
        .collect()
}

/// Recursively walks a `TupleParticleView` and appends every leaf `Element` as
/// a `(local_name, labels)` option pair.
fn collect_choice_options(
    particle: &TupleParticleView,
    subst_map: &HashMap<&str, Vec<&Concept>>,
    taxonomy: &TaxonomySet,
    out: &mut Vec<(String, HashMap<String, String>)>,
) {
    match particle {
        TupleParticleView::Element { element, .. } => {
            if element
                .concept
                .map(|concept| concept.is_abstract)
                .unwrap_or(false)
            {
                // Abstract head: expand to all direct concrete substitutions via
                // the pre-built map (O(1) vs. scanning all taxonomy elements).
                if let Some(concepts) = subst_map.get(element.local_name) {
                    for concept in concepts {
                        let labels = resolve_labels_from_labels(
                            ConceptView::build(concept, taxonomy).labels,
                        );
                        out.push((concept.name.local_name.clone(), labels));
                    }
                }
            } else {
                let labels = element
                    .concept
                    .map(|concept| {
                        resolve_labels_from_labels(ConceptView::build(concept, taxonomy).labels)
                    })
                    .unwrap_or_default();

                out.push((element.local_name.to_string(), labels));
            }
        }
        TupleParticleView::Choice { children, .. }
        | TupleParticleView::Sequence { children, .. } => {
            for child in children {
                collect_choice_options(child, subst_map, taxonomy, out);
            }
        }
        TupleParticleView::GroupDef { particle, .. } => {
            collect_choice_options(particle, subst_map, taxonomy, out);
        }
        TupleParticleView::GroupRef { .. } => {}
    }
}

fn is_required(concept: Option<&Concept>, taxonomy: &TaxonomySet) -> bool {
    concept
        .and_then(|concept| concept.id.as_deref())
        .and_then(|id| taxonomy.references_for(id))
        .is_some_and(|references| {
            references.iter().any(|reference| {
                reference.parts.iter().any(|part| {
                    part.name == "hgbref:fiscalRequirement"
                        && (part.value.starts_with("Mussfeld")
                            || part.value.starts_with("Mussfeld, Kontennachweis erwünscht")
                            || part.value.starts_with("Summenmussfeld")
                            || part
                                .value
                                .starts_with("Rechnerisch notwendig, soweit vorhanden"))
                })
            })
        })
}

/// Recursively collects facts from the tree nodes and populates the fact table
/// rows.
///
/// `is_in_multi_choice` is `true` when this node is a direct child of a tuple
/// whose content model is a multi-select Choice particle; those children are
/// rendered as checkboxes.
#[allow(clippy::too_many_arguments)]
fn collect_node(
    node: &TreeNode,
    facts: &[&ItemFact],
    concept_map: &HashMap<&str, &Concept>,
    substitution_map: &HashMap<&str, Vec<&Concept>>,
    taxonomy: &TaxonomySet,
    rows: &mut Vec<FactRow>,
    is_in_multi_choice: bool,
    parent_concept: Option<&str>,
) {
    let labels = resolve_labels(node);
    let has_children = !node.children.is_empty();
    let concept = concept_map.get(node.concept_name).copied();
    let is_tuple_concept = concept
        .and_then(|concept| concept.content_model.as_ref())
        .is_some();

    // Detect Choice content model on the current concept (only relevant for tuples).
    let choice_max = if !is_in_multi_choice {
        concept
            .and_then(|concept| concept.content_model.as_ref())
            .and_then(|particle| match particle {
                Particle::Choice { occurs, .. } => Some(occurs.max),
                _ => None,
            })
    } else {
        None
    };

    if let Some(max) = choice_max {
        if max == Some(1) {
            // Single-select choice → Dropdown row; options come from the
            // taxonomy schema so all declared choices are shown as dropdown
            // options even when only one child is present in the instance
            // document.
            let options = {
                let mut opts = vec![(
                    String::new(),
                    HashMap::from([
                        ("en".to_owned(), "—".to_owned()),
                        ("de".to_owned(), "—".to_owned()),
                    ]),
                )];

                if let Some(concept) = concept {
                    let concept_view = ConceptView::build(concept, taxonomy);

                    if let Some(particle) = &concept_view.tuple_content {
                        collect_choice_options(particle, substitution_map, taxonomy, &mut opts);
                    }
                }
                opts
            };
            let selected = node
                .children
                .iter()
                .find(|child| {
                    child
                        .fact_indices
                        .iter()
                        .any(|&idx| facts.get(idx).is_some_and(|fact| !fact.is_nil()))
                })
                .map(|child| child.concept_name.to_string())
                .unwrap_or_default();

            rows.push(FactRow {
                concept: node.concept_name.to_string(),
                parent_concept: parent_concept.map(str::to_string),
                labels,
                depth: node.depth,
                context: String::new(),
                unit: None,
                value: FactValue::Dropdown { selected, options },
                has_children: false,
                fact_index: None,
                is_required: is_required(concept, taxonomy),
                is_abstract: concept.is_some_and(|concept| concept.is_abstract),
                is_tuple: is_tuple_concept,
            });

            // Children are represented inside the dropdown; don't recurse.
            return;
        } else {
            // Multi-select choice → concept-only parent row, then recurse as checkboxes.
            rows.push(FactRow {
                concept: node.concept_name.to_string(),
                parent_concept: parent_concept.map(str::to_string),
                labels,
                depth: node.depth,
                context: String::new(),
                unit: None,
                value: FactValue::default(),
                has_children,
                fact_index: None,
                is_required: is_required(concept, taxonomy),
                is_abstract: concept.is_some_and(|concept| concept.is_abstract),
                is_tuple: is_tuple_concept,
            });

            for child in &node.children {
                collect_node(
                    child,
                    facts,
                    concept_map,
                    substitution_map,
                    taxonomy,
                    rows,
                    is_in_multi_choice,
                    Some(node.concept_name),
                );
            }

            return;
        }
    }

    // Normal item fact, or checkbox item inside a multi-select choice.
    if node.fact_indices.is_empty() {
        rows.push(FactRow {
            concept: node.concept_name.to_string(),
            parent_concept: parent_concept.map(str::to_string),
            labels,
            depth: node.depth,
            context: String::new(),
            unit: None,
            value: if is_in_multi_choice {
                FactValue::Checkbox(false)
            } else {
                FactValue::default()
            },
            has_children,
            fact_index: None,
            is_required: is_required(concept, taxonomy),
            is_abstract: concept.is_some_and(|concept| concept.is_abstract),
            is_tuple: is_tuple_concept,
        });
    } else {
        for &idx in &node.fact_indices {
            if let Some(fact) = facts.get(idx) {
                rows.push(FactRow {
                    concept: node.concept_name.to_string(),
                    parent_concept: parent_concept.map(str::to_string),
                    labels: labels.clone(),
                    depth: node.depth,
                    context: fact.context_ref().to_string(),
                    unit: fact.unit_ref().map(|unit| unit.to_string()),
                    value: if is_in_multi_choice {
                        FactValue::Checkbox(!fact.is_nil())
                    } else {
                        FactValue::Text(fact.value().to_string())
                    },
                    has_children,
                    fact_index: Some(idx),
                    is_required: is_required(concept, taxonomy),
                    is_abstract: concept.is_some_and(|concept| concept.is_abstract),
                    is_tuple: is_tuple_concept,
                });
            }
        }
    }

    for child in &node.children {
        collect_node(
            child,
            facts,
            concept_map,
            substitution_map,
            taxonomy,
            rows,
            is_in_multi_choice,
            Some(node.concept_name),
        );
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
