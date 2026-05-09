use crate::domain::FactValue;
use anyhow::Context;
use chrono::NaiveDate;
use log::debug;
use std::collections::HashMap;
use taxel::{BASELINE_ROLE_URIS, REPORT_ELEMENT_TO_ROLE_URI};
use xbrl_rs::{
    Context as XbrlContext, ContextId, Decimals, EntityIdentifier, ExpandedName, Fact,
    FactAttribute, FactAttributeName, InstanceDocument, ItemFact, NamespacePrefix, NamespaceUri,
    Period, PeriodType, RoleUri, TaxonomySet, Unit, UnitId,
};

/// Tuples that must always contain exactly one nil child when no option is
/// selected. ERiC rejects the instance if these tuples are absent or empty.
/// Used by both [`create_instance_document`] and [`update_instance_document`].
const REQUIRED_NIL_TUPLE_CHILDREN: &[(&str, &str)] = &[
    (
        "genInfo.report.id.statementType.tax",
        "genInfo.report.id.statementType.tax.statementTypeTax.GHB",
    ),
    (
        "genInfo.company.id.shareholder.group",
        "genInfo.company.id.shareholder.group.genPartnerPersLiableOHG",
    ),
    (
        "genInfo.company.id.entityWithTaxablePurposeBusiness",
        "genInfo.company.id.entityWithTaxablePurposeBusiness.normal",
    ),
];

/// The outcome of [`update_instance_document`].
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// No structural change; the DocumentView does not need to be rebuilt.
    NoChange,
    /// A tuple child was switched; the caller must rebuild the DocumentView.
    Rebuild,
}

/// Creates a new instance document with the given parameters.
pub fn create_instance_document(
    start_date: &str,
    end_date: &str,
    namespace_prefix: &str,
    namespace_uri: &str,
    taxonomy_date: &str,
    taxonomy: &TaxonomySet,
    roles: &[RoleUri],
) -> Result<InstanceDocument, anyhow::Error> {
    let mut namespaces: HashMap<NamespacePrefix, NamespaceUri> = [
        (
            "de-gcd",
            format!("http://www.xbrl.de/taxonomies/de-gcd-{taxonomy_date}"),
        ),
        ("link", "http://www.xbrl.org/2003/linkbase".to_string()),
        ("hgbref", "http://www.xbrl.de/2008/ref".to_string()),
        ("xhtml", "http://www.w3.org/1999/xhtml".to_string()),
        (
            "xsi",
            "http://www.w3.org/2001/XMLSchema-instance".to_string(),
        ),
        ("xbrli", "http://www.xbrl.org/2003/instance".to_string()),
        ("xbrldi", "http://xbrl.org/2006/xbrldi".to_string()),
        ("iso4217", "http://www.xbrl.org/2003/iso4217".to_string()),
        ("xlink", "http://www.w3.org/1999/xlink".to_string()),
        ("ref", "http://www.xbrl.org/2024/ref".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (NamespacePrefix::from(k), NamespaceUri::from(v)))
    .collect();

    namespaces.insert(
        NamespacePrefix::from(namespace_prefix),
        NamespaceUri::from(namespace_uri),
    );

    let entity = EntityIdentifier {
        scheme: "http://www.rzf-nrw.de/Steuernummer".to_string(),
        value: String::new(),
    };

    let instant_context = XbrlContext::new(
        ContextId::from("I"),
        entity.clone(),
        Period::Instant {
            date: end_date.to_string(),
        },
    );

    let duration_context = XbrlContext::new(
        ContextId::from("D"),
        entity,
        Period::Duration {
            start: start_date.to_string(),
            end: end_date.to_string(),
        },
    );

    let units = [
        Unit::new(
            UnitId::from("EUR"),
            vec![ExpandedName::new(
                NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
                "EUR".to_string(),
            )],
            vec![],
        ),
        Unit::new(
            UnitId::from("pure"),
            vec![ExpandedName::new(
                NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                "pure".to_string(),
            )],
            vec![],
        ),
        Unit::new(
            UnitId::from("shares"),
            vec![ExpandedName::new(
                NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                "shares".to_string(),
            )],
            vec![],
        ),
    ];

    let mut instance = InstanceDocument::from_sections(
        taxonomy,
        roles,
        namespaces,
        instant_context,
        duration_context,
        &units,
    );

    let end_date = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
        .with_context(|| format!("Failed to parse end date '{end_date}'"))?;
    remove_forbidden_facts(&mut instance, taxonomy, &end_date);

    for (tuple, child) in REQUIRED_NIL_TUPLE_CHILDREN {
        ensure_nil_tuple_child(&mut instance, Some(taxonomy), tuple, child);
    }

    Ok(instance)
}

/// Restores the nil placeholder children for all tuples in
/// [`REQUIRED_NIL_TUPLE_CHILDREN`]. Call this after any operation that may have
/// removed those children (e.g. `apply_imported_values`).
pub fn restore_required_nil_tuple_children(
    instance: &mut InstanceDocument,
    taxonomy: &TaxonomySet,
) {
    for (tuple, child) in REQUIRED_NIL_TUPLE_CHILDREN {
        ensure_nil_tuple_child(instance, Some(taxonomy), tuple, child);
    }
}

/// Returns the baseline roles plus one role URI for each non-nil
/// `reportElements.*` fact in the instance. Used to determine which sections
/// to include when rebuilding the instance after a report element selection
/// change.
///
/// The baseline always includes GCD, EV, SGE, and SGEP because ERiC requires
/// their Mussfeld facts to be present as nil even when the user has not
/// selected those sections (see [`BASELINE_ROLE_URIS`]).
pub fn active_roles(instance: &InstanceDocument) -> Vec<RoleUri> {
    let mut roles: Vec<RoleUri> = BASELINE_ROLE_URIS
        .iter()
        .map(|&uri| RoleUri::from(uri))
        .collect();

    for fact in instance.item_facts() {
        let local = &fact.concept_name().local_name;

        if fact.is_nil() {
            continue;
        }

        if let Some(&role_uri) = REPORT_ELEMENT_TO_ROLE_URI.get(local.as_str()) {
            let role = RoleUri::from(role_uri);

            if !roles.contains(&role) {
                roles.push(role);
            }
        }
    }

    roles
}

/// Removes facts from the instance document that are not allowed for submission
/// to the Finanzverwaltung. This is necessary because the instance is built
/// from the full taxonomy, which contains some concepts that are only relevant
/// for other use cases (e.g. internal reporting) but not for tax filing.
pub fn remove_forbidden_facts(
    instance: &mut InstanceDocument,
    taxonomy: &TaxonomySet,
    end_date: &NaiveDate,
) {
    filter_facts_by(instance, |fact| {
        is_not_permitted(fact, taxonomy, end_date).unwrap_or_default()
    });

    // ERiC rule 170155121: `collItemChangeProfitHbst` belongs only in the STU
    // (Steuerliche Überleitung) section. The taxonomy also places it in the
    // GuV/GuVMicroBilG presentation roles, which causes the error when STU is
    // not selected. Keep the facts when STU is active; remove them otherwise.
    let stu_active = instance.item_facts().iter().any(|fact| {
        fact.concept_name().local_name == "genInfo.report.id.reportElement.reportElements.STU"
            && !fact.is_nil()
    });

    if !stu_active {
        filter_facts_by(instance, |fact| {
            matches!(
                fact.concept_name().local_name.as_str(),
                "ismi.netIncome.collItemChangeProfitHbst" | "is.netIncome.collItemChangeProfitHbst"
            )
        });
    }
}

/// Checks if the fact is marked as not permitted.
fn is_not_permitted(fact: &Fact, taxonomy: &TaxonomySet, end_date: &NaiveDate) -> Option<bool> {
    let concept = taxonomy.find_concept(fact.concept_name())?;
    let id = concept.id.as_deref()?;
    let references = taxonomy.references_for(id)?;

    Some(references.iter().any(|reference| {
        reference.parts.iter().any(|part| {
            if let Ok(value_date) = NaiveDate::parse_from_str(&part.value, "%Y-%m-%d") {
                (part.name == "hgbref:ValidThrough" && value_date < *end_date)
                    || (part.name == "hgbref:ValidSince" && value_date > *end_date)
            } else {
                part.name == "hgbref:notPermittedFor"
                    && matches!(
                        part.value.as_str(),
                        "Einreichung an Finanzverwaltung" | "steuerlich"
                    )
            }
        })
    }))
}

/// Removes facts not permitted for handelsrechtlicher Einzelabschluss (EA).
/// Only reconstructs the document when there are facts to remove.
pub fn remove_trade_accounting_facts(instance: &mut InstanceDocument, taxonomy: &TaxonomySet) {
    filter_facts_by(instance, |fact| {
        is_trade_accounting_not_permitted(fact, taxonomy).unwrap_or_default()
    });
}
/// Checks if the fact is not permitted for trade accounting.
fn is_trade_accounting_not_permitted(fact: &Fact, taxonomy: &TaxonomySet) -> Option<bool> {
    let concept = taxonomy.find_concept(fact.concept_name())?;
    let id = concept.id.as_deref()?;
    let references = taxonomy.references_for(id)?;

    Some(references.iter().any(|reference| {
        reference.parts.iter().any(|part| {
            part.name == "hgbref:tradeAccountingNotPermittedFor"
                && part.value == "handelsrechtlicher Einzelabschluss"
        })
    }))
}

/// Filters facts from the instance document based on the given predicate,
/// removing any facts (and their children) for which the predicate returns
/// true.
fn filter_facts_by(instance: &mut InstanceDocument, should_remove: impl Fn(&Fact) -> bool) {
    if !instance.facts().iter().any(&should_remove) {
        return;
    }

    let source = instance.clone();

    let filtered_facts: Vec<Fact> = source
        .facts()
        .iter()
        .filter(|fact| !should_remove(fact))
        .cloned()
        .map(|mut fact| {
            remove_children_by(&mut fact, &should_remove);
            fact
        })
        .collect();

    let role_refs = source.role_refs().to_vec();
    let arcrole_refs = source.arcrole_refs().to_vec();

    let mut filtered = InstanceDocument::new(
        source.schema_refs().to_vec(),
        source.contexts().clone(),
        source.units().clone(),
        filtered_facts,
        source.namespaces().clone(),
        source.footnote_links().to_vec(),
    );

    for role_ref in role_refs {
        filtered.add_role_ref(role_ref);
    }
    for arcrole_ref in arcrole_refs {
        filtered.add_arcrole_ref(arcrole_ref);
    }

    *instance = filtered;
}

/// Recursively removes child facts that match the given predicate.
fn remove_children_by(fact: &mut Fact, should_remove: &impl Fn(&Fact) -> bool) {
    if let Fact::Tuple(tuple) = fact {
        tuple.children_mut().retain(|child| !should_remove(child));

        for child in tuple.children_mut().iter_mut() {
            remove_children_by(child, should_remove);
        }
    }
}

/// Writes one edited fact value back into the instance document.
///
/// `snapshot` is the value at the time editing started. For `Dropdown` it
/// supplies the old child name that must be removed. Returns `Rebuild` when a
/// tuple child was added or activated so the caller knows to re-run
/// [`Report::populate`].
pub fn update_instance_document(
    instance: &mut InstanceDocument,
    value: &FactValue,
    snapshot: Option<&FactValue>,
    fact_index: Option<usize>,
    parent_concept_name: Option<&str>,
    concept_name: &str,
    taxonomy: Option<&TaxonomySet>,
) -> Result<UpdateOutcome, anyhow::Error> {
    match value {
        FactValue::Text(text) => {
            if let Some(idx) = fact_index {
                // TODO: add lookup table for concepts.
                let is_numeric = taxonomy
                    .and_then(|tax| {
                        tax.concepts()
                            .find(|concept| concept.name.local_name == concept_name)
                    })
                    .map(|concept| concept.data_type.is_numeric())
                    .unwrap_or(false);

                if text.is_empty() {
                    instance.set_fact_nil(idx, true);

                    if is_numeric {
                        instance.clear_fact_attribute(idx, FactAttributeName::Decimals);
                    }
                } else {
                    instance.set_fact_value(idx, text.clone());

                    if is_numeric {
                        instance
                            .set_fact_attribute(idx, FactAttribute::Decimals(Decimals::Finite(2)));
                    }
                }
            }
            Ok(UpdateOutcome::NoChange)
        }
        FactValue::Checkbox(checked) => {
            if let Some(idx) = fact_index {
                debug!("Set nil attribute for fact index {}: {}", idx, !checked);

                instance.set_fact_nil(idx, !checked);
                return Ok(UpdateOutcome::NoChange);
            }

            if !checked {
                return Ok(UpdateOutcome::NoChange);
            }

            // Checkbox rows with no fact index are typically tuple-choice
            // children that do not yet exist in the instance. Create or
            // activate the selected tuple child so Save can persist it.
            let Some(parent_concept_name) = parent_concept_name else {
                return Ok(UpdateOutcome::NoChange);
            };

            instance
                .set_tuple_fact_nil(parent_concept_name, false)
                .with_context(|| format!("Failed to activate tuple '{parent_concept_name}'"))?;

            let match_sibling = |name: &str| name == concept_name || name.starts_with(concept_name);

            add_tuple_child(
                instance,
                taxonomy,
                parent_concept_name,
                concept_name,
                match_sibling,
            )?;

            Ok(UpdateOutcome::Rebuild)
        }
        FactValue::Dropdown { selected, .. } => {
            let old = match snapshot {
                Some(FactValue::Dropdown { selected, .. }) => selected.as_str(),
                _ => "",
            };

            if old == selected.as_str() {
                return Ok(UpdateOutcome::NoChange);
            }

            let nil_default = REQUIRED_NIL_TUPLE_CHILDREN
                .iter()
                .find(|(tuple, _)| *tuple == concept_name)
                .map(|(_, child)| *child);

            if !old.is_empty() {
                instance
                    .remove_tuple_child(concept_name, old)
                    .with_context(|| format!("Failed to remove tuple child '{old}'"))?;
            }

            if selected.is_empty() {
                if let Some(nil_child) = nil_default {
                    // Restore the nil placeholder instead of nilifying the whole
                    // tuple; ERiC requires these tuples to always have a nil child.
                    ensure_nil_tuple_child(instance, taxonomy, concept_name, nil_child);
                } else {
                    instance
                        .set_tuple_fact_nil(concept_name, true)
                        .with_context(|| format!("Failed to nil tuple '{concept_name}'"))?;
                }
            }

            if !selected.is_empty() {
                // When coming from the nil state (old == "") a nil placeholder
                // child may still be present. Remove it first so we don't end up
                // with two children, unless the user is selecting that exact child
                // (in which case add_tuple_child's Ok(0) path activates it).
                if old.is_empty() {
                    if let Some(nil_child) = nil_default {
                        if selected.as_str() != nil_child {
                            let _ = instance.remove_tuple_child(concept_name, nil_child);
                        }
                    }
                }

                instance
                    .set_tuple_fact_nil(concept_name, false)
                    .with_context(|| format!("Failed to activate tuple '{concept_name}'"))?;

                let match_sibling = |name: &str| {
                    if !old.is_empty() {
                        name == old
                    } else {
                        name.starts_with(concept_name)
                    }
                };

                add_tuple_child(instance, taxonomy, concept_name, selected, match_sibling)?;
            }

            Ok(UpdateOutcome::Rebuild)
        }
    }
}

/// Adds a new tuple child to the instance document, trying to reuse the
/// contextRef of an existing sibling fact if possible.
fn add_tuple_child(
    instance: &mut InstanceDocument,
    taxonomy: Option<&TaxonomySet>,
    concept_name: &str,
    selected: &str,
    match_sibling: impl Fn(&str) -> bool,
) -> Result<(), anyhow::Error> {
    let new_child = create_item_fact(instance, taxonomy, selected, match_sibling, false);

    match instance
        .add_tuple_child(concept_name, &new_child)
        .with_context(|| format!("Failed to add tuple child '{selected}'"))?
    {
        0 => {
            // Child already exists as nil; activate it.
            instance
                .set_tuple_child_nil(concept_name, selected, false)
                .with_context(|| format!("Failed to activate tuple child '{selected}'"))?;
        }
        _ => {
            debug!(
                "Added tuple child '{}' for concept '{}'",
                selected, concept_name
            );
        }
    }

    Ok(())
}

/// Creates a new `ItemFact` for the given concept name, trying to reuse the
/// contextRef of an existing sibling fact if possible.
fn create_item_fact(
    instance: &InstanceDocument,
    taxonomy: Option<&TaxonomySet>,
    fact_name: &str,
    match_sibling: impl Fn(&str) -> bool,
    is_nil: bool,
) -> ItemFact {
    let sibling_info = instance
        .item_facts()
        .into_iter()
        .find(|fact| match_sibling(&fact.concept_name().local_name))
        .map(|fact| {
            (
                fact.concept_name().namespace_uri.clone(),
                fact.context_ref().to_owned(),
            )
        });

    let (namespace_uri, context_ref) = sibling_info.unwrap_or_else(|| {
        let selected_concept = taxonomy.and_then(|tax| {
            tax.concepts()
                .into_iter()
                .find(|concept| concept.name.local_name == fact_name)
        });

        let namespace_uri = selected_concept
            .map(|concept| concept.name.namespace_uri.clone())
            .unwrap_or_else(|| NamespaceUri::from(""));

        let context_ref = selected_concept
            .and_then(|concept| {
                concept.period_type.as_ref().and_then(|period_type| {
                    instance.contexts().iter().find_map(|(id, ctx)| {
                        let matches = matches!(
                            (period_type, &ctx.period),
                            (PeriodType::Duration, Period::Duration { .. })
                                | (PeriodType::Instant, Period::Instant { .. })
                        );

                        if matches {
                            Some(id.to_string())
                        } else {
                            None
                        }
                    })
                })
            })
            .or_else(|| instance.contexts().keys().next().map(ToString::to_string))
            .unwrap_or_default();

        (namespace_uri, context_ref)
    });

    ItemFact::new(
        None,
        ExpandedName::new(namespace_uri, fact_name.to_owned()),
        context_ref,
        None,
        String::new(),
        is_nil,
        None,
        None,
    )
}

/// Ensures `tuple_concept` contains `child_concept` as a nil child. If the
/// child does not yet exist it is added; in either case it is forced to nil.
/// `add_tuple_child` in xbrl-rs always sets `is_nil = false` on the new fact,
/// so a separate `set_tuple_child_nil` call is always required.
fn ensure_nil_tuple_child(
    instance: &mut InstanceDocument,
    taxonomy: Option<&TaxonomySet>,
    tuple_concept: &str,
    child_concept: &str,
) {
    let new_child = create_item_fact(
        instance,
        taxonomy,
        child_concept,
        |name| name.starts_with(tuple_concept),
        false,
    );

    match instance.add_tuple_child(tuple_concept, &new_child) {
        Ok(_) => {}
        Err(_) => return, // Tuple absent for this taxonomy type.
    }

    // Force the child to nil regardless of whether it was just added or already
    // existed (xbrl-rs forces is_nil=false on every add).
    if let Err(err) = instance.set_tuple_child_nil(tuple_concept, child_concept, true) {
        debug!(
            "Failed to nil child '{}' of '{}': {}",
            child_concept, tuple_concept, err
        );
    }
}

/// Extracts the reporting period from the instance document, if available.
pub fn extract_period(instance: &InstanceDocument) -> Option<(String, String)> {
    instance
        .contexts()
        .values()
        .find_map(|ctx| match &ctx.period {
            Period::Duration { start, end } => Some((start.clone(), end.clone())),
            _ => None,
        })
}
