use crate::domain::FactValue;
use anyhow::Context;
use log::debug;
use std::collections::HashMap;
use xbrl_rs::{
    Context as XbrlContext, ContextId, Decimals, EntityIdentifier, ExpandedName, Fact,
    FactAttribute, FactAttributeName, InstanceDocument, ItemFact, NamespacePrefix, NamespaceUri,
    Period, PeriodType, TaxonomySet, Unit, UnitId,
};

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

    // TODO: build instance selectively for chosen sections only.
    // Currently builds the full instance from all sections in the taxonomy.
    let instance = InstanceDocument::from_taxonomy(
        taxonomy,
        namespaces,
        instant_context,
        duration_context,
        &units,
    );

    let instance = remove_forbidden_facts(instance, taxonomy);

    Ok(instance)
}

/// Removes facts from the instance document that are not allowed for submission
/// to the Finanzverwaltung. This is necessary because the instance is built
/// from the full taxonomy, which contains some concepts that are only relevant
/// for other use cases (e.g. internal reporting) but not for tax filing.
fn remove_forbidden_facts(instance: InstanceDocument, taxonomy: &TaxonomySet) -> InstanceDocument {
    let has_forbidden = instance
        .facts()
        .iter()
        .any(|fact| is_not_permitted(fact, taxonomy));

    // Fast path: if there are no forbidden facts, return the original instance
    if !has_forbidden {
        return instance;
    }

    let filtered_facts: Vec<Fact> = instance
        .facts()
        .iter()
        .filter(|fact| !is_not_permitted(fact, taxonomy))
        .cloned()
        .map(|mut fact| {
            remove_forbidden_children(&mut fact, taxonomy);
            fact
        })
        .collect();

    let role_refs = instance.role_refs().to_vec();
    let arcrole_refs = instance.arcrole_refs().to_vec();

    // TODO: use retain_facts from xbrl-rs when available instead of
    // reconstructing the whole instance document.
    let mut filtered = InstanceDocument::new(
        instance.schema_refs().to_vec(),
        instance.contexts().clone(),
        instance.units().clone(),
        filtered_facts,
        instance.namespaces().clone(),
        instance.footnote_links().to_vec(),
    );

    for role_ref in role_refs {
        filtered.add_role_ref(role_ref);
    }
    for arcrole_ref in arcrole_refs {
        filtered.add_arcrole_ref(arcrole_ref);
    }

    filtered
}

fn remove_forbidden_children(fact: &mut Fact, taxonomy: &TaxonomySet) {
    if let Fact::Tuple(tuple) = fact {
        tuple
            .children_mut()
            .retain(|child| !is_not_permitted(child, taxonomy));

        for child in tuple.children_mut().iter_mut() {
            remove_forbidden_children(child, taxonomy);
        }
    }
}

fn is_not_permitted(fact: &Fact, taxonomy: &TaxonomySet) -> bool {
    let Some(concept) = taxonomy.find_concept(fact.concept_name()) else {
        return false;
    };
    let Some(id) = &concept.id else {
        return false;
    };
    let Some(references) = taxonomy.references_for(id) else {
        return false;
    };

    references.iter().any(|reference| {
        reference.parts.iter().any(|part| {
            part.name == "hgbref:notPermittedFor"
                && matches!(
                    part.value.as_str(),
                    "Einreichung an Finanzverwaltung" | "steuerlich"
                )
        })
    })
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

            if !old.is_empty() {
                instance
                    .remove_tuple_child(concept_name, old)
                    .with_context(|| format!("Failed to remove tuple child '{old}'"))?;
            }

            if selected.is_empty() {
                instance
                    .set_tuple_fact_nil(concept_name, true)
                    .with_context(|| format!("Failed to nil tuple '{concept_name}'"))?;
            }

            if !selected.is_empty() {
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
    let new_child = create_item_fact(instance, taxonomy, selected, match_sibling);

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
        false,
        None,
        None,
    )
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
