use crate::domain::ReportStatus;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use taxel::{
    ElsterReport, TaxonomyType, CLOSING_DATE, COMPANY_CITY, COMPANY_COUNTRY, COMPANY_HOUSE_NO,
    COMPANY_NAME, COMPANY_STREET, COMPANY_TAX_NUMBER, COMPANY_TAX_NUMBER_PARENT, COMPANY_ZIP_CODE,
    FISCAL_YEAR_BEGIN, FISCAL_YEAR_END, GCD_LABEL, GCD_ROLE_URI, INCOME_STATEMENT_FORMAT_GKV,
    INCOME_STATEMENT_FORMAT_UKV, REPORT_ELEMENT_PREFIX, ROLE_URI_TO_REPORT_ELEMENT,
};
use xbrl_rs::{
    Concept, ConceptView, Decimals, DocumentView, FactAttribute, FactAttributeName,
    InstanceDocument, ItemFact, Label, Particle, Period, TaxonomySet, TreeNode, TupleParticleView,
    XbrlType, ROLE_LABEL, ROLE_TERSE,
};

/// The XBRL 2.1 arcrole marking a calculation-linkbase arc as a
/// summation-item relationship (parent = weighted sum of children).
const SUMMATION_ITEM_ARCROLE: &str = "http://www.xbrl.org/2003/arcrole/summation-item";

/// An imported fact value extracted from a source instance document for merging
/// into the current report. Carries nil-ness separately from the text value
/// because an empty string and `xsi:nil="true"` are distinct states in XBRL.
#[derive(Clone, PartialEq)]
enum SourceImportValue {
    Text { value: String, is_nil: bool },
    Checkbox(bool),
    Dropdown(String),
}

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
    /// Scalar boolean fact — rendered as a true/false/nil dropdown. Payload is
    /// "true", "false", or "" (nil).
    BooleanDropdown(String),
    /// Decimal/monetary/float fact — rendered as a numeric text input filtered
    /// to digits, sign, and decimal point. `raw` is the edit buffer; `value`
    /// is the parsed result (`None` when raw is empty = nil, or when raw is an
    /// incomplete/invalid number).
    Decimal { raw: String, value: Option<Decimal> },
    /// Integer fact — rendered as a numeric text input filtered to digits and
    /// optional leading sign. "" = nil.
    Integer(String),
    /// Date fact — rendered as a text input validated against YYYY-MM-DD.
    /// `raw` is the edit buffer; `value` is the parsed result (`None` when
    /// raw is empty = nil, or when the format is invalid).
    Date {
        raw: String,
        value: Option<NaiveDate>,
    },
}

impl Default for FactValue {
    fn default() -> Self {
        FactValue::Text(String::new())
    }
}

impl FactValue {
    /// Returns `false` when the user has typed something that cannot be parsed
    /// into the expected type. Nil (empty raw) is always valid.
    pub fn is_type_valid(&self) -> bool {
        match self {
            FactValue::Decimal { raw, value } => {
                raw.is_empty() || value.as_ref().is_some_and(|decimal| decimal.scale() == 2)
            }
            FactValue::Date { raw, value } => raw.is_empty() || value.is_some(),
            _ => true,
        }
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
    /// Whether this concept is a computed total per the calculation linkbase
    /// for this section's role — the weighted, recursive sum of its
    /// calculation children. Read-only; its value is derived, never
    /// hand-edited.
    pub is_calculated: bool,
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
    /// Calculation-linkbase children for this role, built once in `populate`
    /// and reused by `recompute_calculated_values` on every edit so the
    /// linkbase isn't reparsed per keystroke. Empty when the role has no
    /// calculation network.
    pub calc_children: HashMap<String, Vec<(String, Decimal)>>,
}

impl ReportSection {
    /// Recomputes every calculated-total row's displayed value from its
    /// calculation-linkbase children, bottom-up, restricted to same-context
    /// siblings (a total only sums facts sharing its own `contextRef`, e.g.
    /// current-period vs. prior-period columns are never mixed).
    pub fn recompute_calculated_values(&mut self) {
        if self.calc_children.is_empty() {
            return;
        }

        // Only rows whose own concept is a genuine calculation-linkbase
        // total (a key in `calc_children`) get overwritten here. Collected
        // as `(row index, computed value)` — resolved via `row_index` while
        // it's still in scope — rather than applied by re-scanning `rows`
        // for every entry in `compute_value`'s memo, since that memo also
        // contains every *leaf* concept touched during recursion (needed
        // for memoization) and must never be written back: doing so would
        // silently overwrite a leaf's live, hand-typed value with its own
        // canonical `Decimal::to_string()` on every frame, e.g. turning a
        // still-being-typed "100." back into "100".
        let updates: Vec<(usize, Option<Decimal>)> = {
            let mut row_index: HashMap<(&str, &str), usize> = HashMap::new();
            for (i, row) in self.rows.iter().enumerate() {
                if !row.context.is_empty() {
                    row_index.insert((row.concept.as_str(), row.context.as_str()), i);
                }
            }
            let contexts: HashSet<&str> = row_index.keys().map(|&(_, ctx)| ctx).collect();

            let mut memo = HashMap::new();
            let mut updates = Vec::new();

            for &context in &contexts {
                for concept in self.calc_children.keys() {
                    if let Some(&idx) = row_index.get(&(concept.as_str(), context)) {
                        let mut visiting = HashSet::new();
                        let value = compute_value(
                            concept,
                            context,
                            &self.calc_children,
                            &row_index,
                            &self.rows,
                            &mut memo,
                            &mut visiting,
                        );
                        updates.push((idx, value));
                    }
                }
            }

            updates
        };

        for (idx, value) in updates {
            apply_computed_decimal(&mut self.rows[idx], value);
        }
    }
}

/// `None` in `calc_children` for `concept` means it's a calculation leaf; its
/// value is read straight from its own row. `Some(children)` means it's a
/// total: the weighted sum of `children` found in the same `context`. A
/// child with no resolvable value (nil, or not rendered in this section)
/// contributes nothing; if literally no child resolves to a value, the
/// total itself resolves to `None` (left nil). `visiting` guards against infinite recursion on a
/// circular/malformed calculation linkbase.
#[allow(clippy::too_many_arguments)]
fn compute_value(
    concept: &str,
    context: &str,
    calc_children: &HashMap<String, Vec<(String, Decimal)>>,
    row_index: &HashMap<(&str, &str), usize>,
    rows: &[FactRow],
    memo: &mut HashMap<(String, String), Option<Decimal>>,
    visiting: &mut HashSet<(String, String)>,
) -> Option<Decimal> {
    let key = (concept.to_owned(), context.to_owned());
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }
    if !visiting.insert(key.clone()) {
        return None;
    }

    let value = match calc_children.get(concept) {
        None => row_index
            .get(&(concept, context))
            .and_then(|&idx| decimal_of(&rows[idx])),
        Some(children) => {
            let mut sum = Decimal::ZERO;
            let mut any_found = false;
            for (child, weight) in children {
                if let Some(v) = compute_value(
                    child,
                    context,
                    calc_children,
                    row_index,
                    rows,
                    memo,
                    visiting,
                ) {
                    sum += *weight * v;
                    any_found = true;
                }
            }
            // `round_dp` only reduces excess precision; it leaves the scale
            // untouched (e.g. still 0) when the accumulated sum already has <=
            // 2 decimal places, which is common since eBilanz facts are often
            // whole-euro values with no decimal point in the XML. `rescale`
            // unconditionally sets the scale to exactly 2 in both directions,
            // matching what `FactValue::is_type_valid()` requires. If no child
            // resolved to a value at all (the breakdown is entirely empty), the
            // total is also empty (`None`).
            any_found.then(|| {
                let mut total = sum;
                total.rescale(2);
                total
            })
        }
    };

    visiting.remove(&key);
    memo.insert(key.clone(), value);
    value
}

fn decimal_of(row: &FactRow) -> Option<Decimal> {
    match &row.value {
        FactValue::Decimal { value, .. } => *value,
        _ => None,
    }
}

fn apply_computed_decimal(row: &mut FactRow, computed: Option<Decimal>) {
    row.value = match computed {
        Some(v) => FactValue::Decimal {
            raw: v.to_string(),
            value: Some(v),
        },
        None => FactValue::Decimal {
            raw: String::new(),
            value: None,
        },
    };
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
            .concepts()
            .map(|concept| (concept.name.local_name.as_str(), concept))
            .collect();

        // Maps each substitution-group head's local name to all non-abstract
        // concepts that directly substitute for it. Built once here so
        // collect_choice_options can do O(1) lookups instead of scanning all
        // taxonomy elements per abstract head.
        let mut substitution_map: HashMap<&str, Vec<&Concept>> = HashMap::new();

        for concept in taxonomy.concepts() {
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

        // The income statement (`GuV`) presentation role contains both the
        // GKV (Gesamtkostenverfahren) and UKV (Umsatzkostenverfahren)
        // breakdowns side by side. Once the user has picked one via
        // `incomeStatementFormat`, hide the other breakdown's *rows* — the
        // underlying `InstanceDocument` is never touched by this, only what
        // the table displays. Leave both visible when neither format is
        // selected, or when `GuV` isn't the active report element (e.g.
        // `GuVMicroBilG`, which reuses the same GKV-tagged concepts for its
        // own format-invariant presentation and has no UKV counterpart at
        // all — hiding by format there would wrongly prune it).
        let guv_active = item_facts.iter().any(|fact| {
            fact.concept_name().local_name == "genInfo.report.id.reportElement.reportElements.GuV"
                && !fact.is_nil()
        });
        let hidden_operating_result = guv_active
            .then(|| {
                item_facts.iter().find_map(|fact| {
                    let local = fact.concept_name().local_name.as_str();
                    (!fact.is_nil()
                        && matches!(
                            local,
                            INCOME_STATEMENT_FORMAT_GKV | INCOME_STATEMENT_FORMAT_UKV
                        ))
                    .then(|| {
                        if local == INCOME_STATEMENT_FORMAT_GKV {
                            "UKV"
                        } else {
                            "GKV"
                        }
                    })
                })
            })
            .flatten();

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

            let calc_children = build_calc_children(taxonomy, role_uri);

            let mut fact_section = ReportSection {
                role: role_uri.to_owned(),
                labels: labels.unwrap_or_default(),
                rows: Vec::new(),
                disabled,
                calc_children: calc_children.clone(),
            };

            for node in &section.nodes {
                collect_node(
                    node,
                    item_facts,
                    &concept_map,
                    &substitution_map,
                    taxonomy,
                    &calc_children,
                    &mut fact_section.rows,
                    false,
                    None,
                    hidden_operating_result,
                );
            }

            self.sections.push(fact_section);
        }

        // Enabled sections are shown first, followed by disabled sections. The
        // relative order within each group is determined by the original order
        // in the presentation linkbase.
        self.sections.sort_by_key(|section| section.disabled);

        self.recompute_calculated_values();
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

    /// Recomputes every calculated-total row's displayed value from its
    /// calculation-linkbase children, in every section. Pure in-memory —
    /// does not touch `InstanceDocument`; persisting a correction happens
    /// through the normal snapshot-diff logic in `save_report`, which treats
    /// a changed calculated row exactly like any other changed Decimal row.
    pub fn recompute_calculated_values(&mut self) {
        for section in &mut self.sections {
            section.recompute_calculated_values();
        }
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
                        FactValue::Date { raw, .. } if !raw.is_empty() => Some(raw.as_str()),
                        FactValue::BooleanDropdown(s) | FactValue::Integer(s) if !s.is_empty() => {
                            Some(s.as_str())
                        }
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

        if let Some(company_name) = company_name {
            elster.transfer_header.submitter.name = company_name.to_string();
        }

        elster.transfer_header.submitter.street = street_full.clone();
        elster.transfer_header.submitter.postal_code = zip_code.map(str::to_string);
        elster.transfer_header.submitter.city = city.map(str::to_string);
        elster.transfer_header.submitter.country = country.map(str::to_string);

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

    /// Pre-populates the three GCD period-date concepts from the form dates on
    /// new report creation. Updates both the `Report` rows (for the UI) and the
    /// underlying `InstanceDocument` facts.
    pub fn initialize_period_dates(
        &mut self,
        instance: &mut InstanceDocument,
        start_date: &str,
        end_date: &str,
    ) {
        let mappings = [
            (FISCAL_YEAR_BEGIN, start_date),
            (FISCAL_YEAR_END, end_date),
            (CLOSING_DATE, end_date),
        ];

        let Some(section) = self
            .sections
            .iter_mut()
            .find(|section| section.role == GCD_ROLE_URI)
        else {
            return;
        };

        for row in &mut section.rows {
            for &(concept, date) in &mappings {
                if row.concept == concept {
                    match &mut row.value {
                        FactValue::Text(text) => {
                            *text = date.to_string();
                        }
                        FactValue::Date { raw, value } => {
                            *raw = date.to_string();
                            *value = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok();
                        }
                        _ => {}
                    }

                    if let Some(idx) = row.fact_index {
                        instance.set_fact_value(idx, date.to_string());
                    }
                }
            }
        }
    }

    /// Merges fact values from `source_report` into this report and the
    /// accompanying `instance`. Returns `(matched_count, imported_count)`.
    ///
    /// Use `import_report_elements` = `false` to skip importing report-element
    /// selections from the source report. This prevents deletion of existing
    /// sections from the target report; the source's un-selected sections are
    /// consequently left with no fact index and their values are not
    /// imported. Selecting them is the user's job via the GCD checkboxes,
    /// which trigger a rebuild.
    pub fn apply_imported_values(
        &mut self,
        source_report: &Report,
        source_item_facts: &[&ItemFact],
        instance: &mut InstanceDocument,
        taxonomy: &TaxonomySet,
        import_report_elements: bool,
    ) -> (usize, usize) {
        // Keyed by `(concept, parent, unit)`; collapsed to `None` when multiple
        // source rows share the key but disagree on value (conflicting contexts).
        let mut source_map: HashMap<
            (String, Option<String>, Option<String>),
            Option<SourceImportValue>,
        > = HashMap::new();

        for source_section in &source_report.sections {
            for row in &source_section.rows {
                if !import_report_elements && row.concept.starts_with(REPORT_ELEMENT_PREFIX) {
                    continue;
                }

                let source_value = match &row.value {
                    FactValue::Text(text) => {
                        let is_nil = row
                            .fact_index
                            .and_then(|idx| source_item_facts.get(idx).map(|fact| fact.is_nil()))
                            .unwrap_or(text.is_empty());

                        SourceImportValue::Text {
                            value: text.clone(),
                            is_nil,
                        }
                    }
                    FactValue::Checkbox(checked) => SourceImportValue::Checkbox(*checked),
                    FactValue::Dropdown { selected, .. } => {
                        SourceImportValue::Dropdown(selected.clone())
                    }
                    FactValue::BooleanDropdown(value) | FactValue::Integer(value) => {
                        let is_nil = row
                            .fact_index
                            .and_then(|idx| source_item_facts.get(idx).map(|fact| fact.is_nil()))
                            .unwrap_or(value.is_empty());
                        SourceImportValue::Text {
                            value: value.clone(),
                            is_nil,
                        }
                    }
                    FactValue::Decimal { raw, .. } => {
                        let is_nil = row
                            .fact_index
                            .and_then(|idx| source_item_facts.get(idx).map(|fact| fact.is_nil()))
                            .unwrap_or(raw.is_empty());
                        SourceImportValue::Text {
                            value: raw.clone(),
                            is_nil,
                        }
                    }
                    FactValue::Date { raw, .. } => {
                        let is_nil = row
                            .fact_index
                            .and_then(|idx| source_item_facts.get(idx).map(|fact| fact.is_nil()))
                            .unwrap_or(raw.is_empty());
                        SourceImportValue::Text {
                            value: raw.clone(),
                            is_nil,
                        }
                    }
                };

                let key = (
                    row.concept.clone(),
                    row.parent_concept.clone(),
                    row.unit.clone(),
                );
                source_map
                    .entry(key)
                    .and_modify(|current| match current {
                        Some(existing) if *existing == source_value => {}
                        _ => *current = None,
                    })
                    .or_insert_with(|| Some(source_value));
            }
        }

        let mut matched_count = 0usize;
        let mut imported_count = 0usize;

        for section in &mut self.sections {
            for row in &mut section.rows {
                let key = (
                    row.concept.clone(),
                    row.parent_concept.clone(),
                    row.unit.clone(),
                );

                let source_value = source_map.get(&key).and_then(|v| v.clone());

                let Some(source_value) = source_value else {
                    continue;
                };

                match (&mut row.value, source_value) {
                    (FactValue::Text(text), SourceImportValue::Text { value, is_nil }) => {
                        matched_count += 1;

                        if let Some(idx) = row.fact_index {
                            let is_numeric = taxonomy
                                .concepts()
                                .find(|concept| concept.name.local_name == row.concept)
                                .map(|concept| concept.data_type.is_numeric())
                                .unwrap_or(false);

                            if is_nil {
                                instance.set_fact_nil(idx, true);

                                if is_numeric {
                                    instance.clear_fact_attribute(idx, FactAttributeName::Decimals);
                                }
                            } else {
                                instance.set_fact_value(idx, value.clone());

                                if is_numeric {
                                    instance.set_fact_attribute(
                                        idx,
                                        FactAttribute::Decimals(Decimals::Finite(2)),
                                    );
                                }
                            }
                        }

                        let new_text = if is_nil { String::new() } else { value };

                        if *text != new_text {
                            imported_count += 1;
                        }

                        *text = new_text;
                    }
                    (FactValue::Checkbox(checked), SourceImportValue::Checkbox(new_checked)) => {
                        matched_count += 1;

                        if let Some(idx) = row.fact_index {
                            instance.set_fact_nil(idx, !new_checked);
                        }

                        if *checked != new_checked {
                            imported_count += 1;
                        }

                        *checked = new_checked;
                    }
                    (
                        FactValue::Dropdown { selected, .. },
                        SourceImportValue::Dropdown(new_selected),
                    ) => {
                        matched_count += 1;

                        if *selected != new_selected {
                            imported_count += 1;
                        }

                        // Dropdown rows represent tuple choices. Only the UI
                        // selection is staged here; tuple-child creation is
                        // deferred to Save via update_instance_document.
                        *selected = new_selected;
                    }
                    (
                        FactValue::Decimal { raw, value: parsed },
                        SourceImportValue::Text { value, is_nil },
                    ) => {
                        matched_count += 1;

                        if let Some(idx) = row.fact_index {
                            if is_nil {
                                instance.set_fact_nil(idx, true);
                                instance.clear_fact_attribute(idx, FactAttributeName::Decimals);
                            } else {
                                instance.set_fact_value(idx, value.clone());
                                instance.set_fact_attribute(
                                    idx,
                                    FactAttribute::Decimals(Decimals::Finite(2)),
                                );
                            }
                        }

                        let new_raw = if is_nil { String::new() } else { value };

                        if *raw != new_raw {
                            imported_count += 1;
                        }

                        *parsed = new_raw.parse::<Decimal>().ok();
                        *raw = new_raw;
                    }
                    (FactValue::Integer(text), SourceImportValue::Text { value, is_nil }) => {
                        matched_count += 1;

                        if let Some(idx) = row.fact_index {
                            if is_nil {
                                instance.set_fact_nil(idx, true);
                                instance.clear_fact_attribute(idx, FactAttributeName::Decimals);
                            } else {
                                instance.set_fact_value(idx, value.clone());
                                instance.set_fact_attribute(
                                    idx,
                                    FactAttribute::Decimals(Decimals::Infinite),
                                );
                            }
                        }

                        let new_text = if is_nil { String::new() } else { value };

                        if *text != new_text {
                            imported_count += 1;
                        }

                        *text = new_text;
                    }
                    (
                        FactValue::BooleanDropdown(text),
                        SourceImportValue::Text { value, is_nil },
                    ) => {
                        matched_count += 1;

                        if let Some(idx) = row.fact_index {
                            if is_nil {
                                instance.set_fact_nil(idx, true);
                            } else {
                                instance.set_fact_value(idx, value.clone());
                            }
                        }

                        let new_text = if is_nil { String::new() } else { value };

                        if *text != new_text {
                            imported_count += 1;
                        }

                        *text = new_text;
                    }
                    (
                        FactValue::Date { raw, value: parsed },
                        SourceImportValue::Text { value, is_nil },
                    ) => {
                        matched_count += 1;

                        if let Some(idx) = row.fact_index {
                            if is_nil {
                                instance.set_fact_nil(idx, true);
                            } else {
                                instance.set_fact_value(idx, value.clone());
                            }
                        }

                        let new_raw = if is_nil { String::new() } else { value };

                        if *raw != new_raw {
                            imported_count += 1;
                        }

                        *parsed = NaiveDate::parse_from_str(&new_raw, "%Y-%m-%d").ok();
                        *raw = new_raw;
                    }
                    _ => {}
                }
            }
        }

        (matched_count, imported_count)
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

/// Returns the concept's `hgbref:typeOperatingResult` reference annotation
/// (`"GKV"`, `"UKV"`, or `"neutral"`), if present, indicating which income
/// statement format (Gesamtkostenverfahren vs. Umsatzkostenverfahren) the
/// concept belongs to. This is a display-only classification — it never
/// affects which facts exist in the `InstanceDocument`, only which rows
/// `Report::populate` builds for the table.
fn operating_result_type(
    concept: Option<&Concept>,
    taxonomy: &TaxonomySet,
) -> Option<&'static str> {
    let id = concept?.id.as_deref()?;
    let references = taxonomy.references_for(id)?;

    references.iter().find_map(|reference| {
        reference.parts.iter().find_map(|part| {
            (part.name == "hgbref:typeOperatingResult").then_some(match part.value.as_str() {
                "GKV" => "GKV",
                "UKV" => "UKV",
                _ => "neutral",
            })
        })
    })
}

/// Builds the calculation-linkbase child map for one extended-link role:
/// a total concept's local name maps to its `(child local name, signed
/// weight)` pairs. Arcs from multiple merged calculation linkbase files are
/// deduplicated by `(from, to)` (last-write-wins on a differing weight,
/// which would indicate a taxonomy-authoring inconsistency). Returns an
/// empty map for roles with no calculation network.
fn build_calc_children(
    taxonomy: &TaxonomySet,
    role_uri: &str,
) -> HashMap<String, Vec<(String, Decimal)>> {
    let mut deduped: HashMap<(&str, &str), Decimal> = HashMap::new();

    if let Some(arcs) = taxonomy.calculation_arcs(role_uri) {
        for arc in arcs {
            if arc.arcrole.as_str() != SUMMATION_ITEM_ARCROLE {
                continue;
            }

            let key = (arc.from.local_name.as_str(), arc.to.local_name.as_str());
            match deduped.get(&key) {
                Some(&existing_weight) if existing_weight != arc.weight => {
                    log::warn!(
                        "calculation arc {} -> {} in role {role_uri} has conflicting weights \
                         ({existing_weight} vs {}); using the latter",
                        key.0,
                        key.1,
                        arc.weight
                    );
                }
                _ => {}
            }
            deduped.insert(key, arc.weight);
        }
    }

    let mut children: HashMap<String, Vec<(String, Decimal)>> = HashMap::new();
    for ((from, to), weight) in deduped {
        children
            .entry(from.to_string())
            .or_default()
            .push((to.to_string(), weight));
    }
    children
}

fn fact_value_for_type(data_type: &XbrlType, raw_value: &str, is_nil: bool) -> FactValue {
    let raw = if is_nil {
        String::new()
    } else {
        raw_value.to_string()
    };
    match data_type {
        XbrlType::Boolean => FactValue::BooleanDropdown(raw),
        XbrlType::Decimal
        | XbrlType::Monetary
        | XbrlType::Float
        | XbrlType::Double
        | XbrlType::Shares
        | XbrlType::Fraction
        | XbrlType::Percent
        | XbrlType::PerShare
        | XbrlType::Pure => {
            let value = raw.parse::<Decimal>().ok();
            FactValue::Decimal { raw, value }
        }
        XbrlType::Integer => FactValue::Integer(raw),
        XbrlType::Date => {
            let value = NaiveDate::parse_from_str(&raw, "%Y-%m-%d").ok();
            FactValue::Date { raw, value }
        }
        _ => FactValue::Text(raw),
    }
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
    calc_children: &HashMap<String, Vec<(String, Decimal)>>,
    rows: &mut Vec<FactRow>,
    is_in_multi_choice: bool,
    parent_concept: Option<&str>,
    hidden_operating_result: Option<&'static str>,
) {
    let concept = concept_map.get(node.concept_name).copied();

    // Prune the whole subtree when this concept belongs to the currently
    // hidden GKV/UKV branch — no row for it, and no recursion into its
    // children, since each concept carries its own `hgbref:typeOperatingResult`
    // tag rather than inheriting one from its parent.
    if let Some(hidden) = hidden_operating_result {
        if operating_result_type(concept, taxonomy) == Some(hidden) {
            return;
        }
    }

    let labels = resolve_labels(node);
    let has_children = !node.children.is_empty();
    let is_tuple_concept = concept
        .and_then(|concept| concept.content_model.as_ref())
        .is_some();
    let data_type = concept
        .map(|c| c.data_type.clone())
        .unwrap_or(XbrlType::String);

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
                is_calculated: calc_children.contains_key(node.concept_name),
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
                is_calculated: calc_children.contains_key(node.concept_name),
            });

            for child in &node.children {
                collect_node(
                    child,
                    facts,
                    concept_map,
                    substitution_map,
                    taxonomy,
                    calc_children,
                    rows,
                    // Children of a multi-select choice are rendered as
                    // checkboxes, so we set this flag to true.
                    true,
                    Some(node.concept_name),
                    hidden_operating_result,
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
                fact_value_for_type(&data_type, "", true)
            },
            has_children,
            fact_index: None,
            is_required: is_required(concept, taxonomy),
            is_abstract: concept.is_some_and(|concept| concept.is_abstract),
            is_tuple: is_tuple_concept,
            is_calculated: calc_children.contains_key(node.concept_name),
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
                        fact_value_for_type(&data_type, fact.value(), fact.is_nil())
                    },
                    has_children,
                    fact_index: Some(idx),
                    is_required: is_required(concept, taxonomy),
                    is_abstract: concept.is_some_and(|concept| concept.is_abstract),
                    is_tuple: is_tuple_concept,
                    is_calculated: calc_children.contains_key(node.concept_name),
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
            calc_children,
            rows,
            is_in_multi_choice,
            Some(node.concept_name),
            hidden_operating_result,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_child_leaves_total_empty() {
        // A total whose only child is empty (never filled) resolves to
        // None itself — an empty breakdown means an empty total.
        let mut section = section_with(
            vec![
                decimal_row("sub_total", "I", None),
                decimal_row("breakdown_child", "I", None),
            ],
            HashMap::from([(
                "sub_total".to_owned(),
                vec![("breakdown_child".to_owned(), Decimal::ONE)],
            )]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "sub_total"), None);
    }

    #[test]
    fn missing_child_row_is_treated_as_absent_not_zero() {
        // A calc child with no row at all — e.g. pruned from the table
        // because `collect_node` hid it as part of a GKV/UKV branch not
        // currently selected — must not error and must not be treated as a
        // zero contributor. The total still resolves purely from whichever
        // sibling's row is present, exactly like `is.netIncome.eat` (which
        // sums the GKV and UKV branch roots as siblings) must keep working
        // when only one branch has rows.
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("present_child", "I", Some("10.00")),
                // "missing_child" has no row at all.
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("present_child".to_owned(), Decimal::ONE),
                    ("missing_child".to_owned(), Decimal::ONE),
                ],
            )]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "total"), Some(Decimal::new(1000, 2)));
    }

    #[test]
    fn does_not_overwrite_a_leaf_rows_own_raw_text_mid_edit() {
        // Regression test for a bug where `recompute_calculated_values`
        // rewrote every row `compute_value` had visited during recursion —
        // not just the total itself — since `compute_value` memoizes every
        // concept it touches, leaves included. That silently clobbered a
        // leaf's live, in-progress typed text with `Decimal::to_string()`
        // of its own value every frame: typing "100." (a valid partial
        // decimal, scale 0) got immediately rewritten back to "100",
        // permanently eating the just-typed trailing dot.
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("leaf", "I", Some("10.00")),
            ],
            HashMap::from([("total".to_owned(), vec![("leaf".to_owned(), Decimal::ONE)])]),
        );

        // Simulate mid-typing: the leaf's raw text has a trailing dot that
        // hasn't parsed to a 2-decimal value yet ("100." -> scale 0), which
        // does NOT round-trip losslessly through `Decimal::to_string()`.
        section.rows[1].value = FactValue::Decimal {
            raw: "100.".to_owned(),
            value: "100.".parse().ok(),
        };

        section.recompute_calculated_values();

        assert_eq!(raw_of(&section, "leaf"), "100.");
        // The total itself is still correctly (re)computed from the leaf.
        assert_eq!(value_of(&section, "total"), Some(Decimal::new(10000, 2)));
    }

    fn decimal_row(concept: &str, context: &str, value: Option<&str>) -> FactRow {
        let value = value.map(|v| v.parse::<Decimal>().unwrap());
        FactRow {
            concept: concept.to_owned(),
            parent_concept: None,
            labels: HashMap::new(),
            depth: 0,
            context: context.to_owned(),
            unit: None,
            value: FactValue::Decimal {
                raw: value.map(|v| v.to_string()).unwrap_or_default(),
                value,
            },
            has_children: false,
            fact_index: Some(0),
            is_required: false,
            is_abstract: false,
            is_tuple: false,
            is_calculated: false,
        }
    }

    fn section_with(
        rows: Vec<FactRow>,
        calc_children: HashMap<String, Vec<(String, Decimal)>>,
    ) -> ReportSection {
        ReportSection {
            role: "test-role".to_owned(),
            labels: HashMap::new(),
            rows,
            disabled: false,
            calc_children,
        }
    }

    fn value_of(section: &ReportSection, concept: &str) -> Option<Decimal> {
        section
            .rows
            .iter()
            .find(|row| row.concept == concept)
            .and_then(decimal_of)
    }

    fn raw_of<'a>(section: &'a ReportSection, concept: &str) -> &'a str {
        match &section
            .rows
            .iter()
            .find(|row| row.concept == concept)
            .unwrap()
            .value
        {
            FactValue::Decimal { raw, .. } => raw,
            _ => panic!("expected a Decimal fact"),
        }
    }

    #[test]
    fn sums_children_with_weight_one() {
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("childA", "I", Some("10.00")),
                decimal_row("childB", "I", Some("5.00")),
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("childA".to_owned(), Decimal::ONE),
                    ("childB".to_owned(), Decimal::ONE),
                ],
            )]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "total"), Some(Decimal::new(1500, 2)));
    }

    #[test]
    fn applies_negative_weight_for_contra_items() {
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("gross", "I", Some("100.00")),
                decimal_row("depreciation", "I", Some("30.00")),
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("gross".to_owned(), Decimal::ONE),
                    ("depreciation".to_owned(), Decimal::NEGATIVE_ONE),
                ],
            )]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "total"), Some(Decimal::new(7000, 2)));
    }

    #[test]
    fn recurses_through_nested_totals() {
        // total = mid + leafC; mid = leafA + leafB
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("mid", "I", None),
                decimal_row("leafA", "I", Some("1.00")),
                decimal_row("leafB", "I", Some("2.00")),
                decimal_row("leafC", "I", Some("3.00")),
            ],
            HashMap::from([
                (
                    "total".to_owned(),
                    vec![
                        ("mid".to_owned(), Decimal::ONE),
                        ("leafC".to_owned(), Decimal::ONE),
                    ],
                ),
                (
                    "mid".to_owned(),
                    vec![
                        ("leafA".to_owned(), Decimal::ONE),
                        ("leafB".to_owned(), Decimal::ONE),
                    ],
                ),
            ]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "mid"), Some(Decimal::new(300, 2)));
        assert_eq!(value_of(&section, "total"), Some(Decimal::new(600, 2)));
    }

    #[test]
    fn keeps_contexts_isolated() {
        let mut section = section_with(
            vec![
                decimal_row("total", "current", None),
                decimal_row("child", "current", Some("10.00")),
                decimal_row("total", "prior", None),
                decimal_row("child", "prior", Some("99.00")),
            ],
            HashMap::from([("total".to_owned(), vec![("child".to_owned(), Decimal::ONE)])]),
        );

        section.recompute_calculated_values();

        let current_total = section
            .rows
            .iter()
            .find(|row| row.concept == "total" && row.context == "current")
            .and_then(decimal_of);
        let prior_total = section
            .rows
            .iter()
            .find(|row| row.concept == "total" && row.context == "prior")
            .and_then(decimal_of);

        assert_eq!(current_total, Some(Decimal::new(1000, 2)));
        assert_eq!(prior_total, Some(Decimal::new(9900, 2)));
    }

    #[test]
    fn nil_child_is_skipped_but_all_nil_yields_nil_total() {
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("childA", "I", Some("10.00")),
                decimal_row("childB", "I", None),
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("childA".to_owned(), Decimal::ONE),
                    ("childB".to_owned(), Decimal::ONE),
                ],
            )]),
        );
        section.recompute_calculated_values();
        assert_eq!(value_of(&section, "total"), Some(Decimal::new(1000, 2)));

        let mut all_nil_section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("childA", "I", None),
                decimal_row("childB", "I", None),
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("childA".to_owned(), Decimal::ONE),
                    ("childB".to_owned(), Decimal::ONE),
                ],
            )]),
        );
        all_nil_section.recompute_calculated_values();
        assert_eq!(value_of(&all_nil_section, "total"), None);
    }

    #[test]
    fn zero_sum_of_whole_number_children_still_has_two_decimal_places() {
        // Children whose raw XBRL value has no decimal point at all (e.g.
        // "0", common for whole-euro eBilanz facts) parse to a Decimal with
        // scale 0. `round_dp` only trims excess precision and leaves a
        // scale-0 value untouched, so a naive `sum.round_dp(2)` would store
        // "0" instead of "0.00", failing `FactValue::is_type_valid()`'s
        // `scale() == 2` requirement.
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("childA", "I", Some("0")),
                decimal_row("childB", "I", Some("0")),
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("childA".to_owned(), Decimal::ONE),
                    ("childB".to_owned(), Decimal::ONE),
                ],
            )]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "total").unwrap().scale(), 2);
        assert_eq!(raw_of(&section, "total"), "0.00");
    }

    #[test]
    fn nonzero_whole_number_sum_still_has_two_decimal_places() {
        let mut section = section_with(
            vec![
                decimal_row("total", "I", None),
                decimal_row("childA", "I", Some("5")),
                decimal_row("childB", "I", Some("5")),
            ],
            HashMap::from([(
                "total".to_owned(),
                vec![
                    ("childA".to_owned(), Decimal::ONE),
                    ("childB".to_owned(), Decimal::ONE),
                ],
            )]),
        );

        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "total").unwrap().scale(), 2);
        assert_eq!(raw_of(&section, "total"), "10.00");
    }

    #[test]
    fn circular_calc_arcs_do_not_infinite_loop() {
        let mut section = section_with(
            vec![decimal_row("a", "I", None), decimal_row("b", "I", None)],
            HashMap::from([
                ("a".to_owned(), vec![("b".to_owned(), Decimal::ONE)]),
                ("b".to_owned(), vec![("a".to_owned(), Decimal::ONE)]),
            ]),
        );

        // Must terminate rather than recurse forever.
        section.recompute_calculated_values();

        assert_eq!(value_of(&section, "a"), None);
        assert_eq!(value_of(&section, "b"), None);
    }

    #[test]
    fn build_calc_children_dedupes_and_filters_arcrole() {
        use xbrl_rs::{CalculationArc, ExpandedName, NamespaceUri};

        let make_name = |local: &str| ExpandedName {
            namespace_uri: NamespaceUri::from("http://example.com".to_owned()),
            local_name: local.to_owned(),
        };

        let arcs = [
            CalculationArc {
                from: make_name("total"),
                to: make_name("child"),
                order: None,
                weight: Decimal::ONE,
                arcrole: SUMMATION_ITEM_ARCROLE.to_owned().into(),
            },
            CalculationArc {
                from: make_name("total"),
                to: make_name("unrelated"),
                order: None,
                weight: Decimal::ONE,
                arcrole: "http://www.xbrl.org/2003/arcrole/parent-child"
                    .to_owned()
                    .into(),
            },
        ];

        // Sanity-check the filtering logic in isolation, since building a real
        // TaxonomySet is out of scope for a fast unit test.
        let filtered: Vec<_> = arcs
            .iter()
            .filter(|arc| arc.arcrole.as_str() == SUMMATION_ITEM_ARCROLE)
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].to.local_name, "child");
    }
}
