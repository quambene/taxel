use crate::domain::FactValue;
use anyhow::Context;
use log::debug;
use std::collections::HashMap;
use xbrl_rs::{
    Context as XbrlContext, ContextId, EntityIdentifier, ExpandedName, InstanceDocument, ItemFact,
    NamespacePrefix, NamespaceUri, Period, PeriodType, TaxonomySet, Unit, UnitId,
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

    Ok(instance)
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
    concept: &str,
    taxonomy: Option<&TaxonomySet>,
) -> Result<UpdateOutcome, anyhow::Error> {
    match value {
        FactValue::Text(text) => {
            if let Some(idx) = fact_index {
                if text.is_empty() {
                    instance.set_fact_nil(idx, true);
                } else {
                    instance.set_fact_value(idx, text.clone());
                }
            }
            Ok(UpdateOutcome::NoChange)
        }
        FactValue::Checkbox(checked) => {
            if let Some(idx) = fact_index {
                debug!("Set nil attribute for fact index {}: {}", idx, !checked);

                instance.set_fact_nil(idx, !checked);
            }
            Ok(UpdateOutcome::NoChange)
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
                    .remove_tuple_child(concept, old)
                    .with_context(|| format!("Failed to remove tuple child '{old}'"))?;
            }

            if selected.is_empty() {
                instance
                    .set_tuple_fact_nil(concept, true)
                    .with_context(|| format!("Failed to nil tuple '{concept}'"))?;
            }

            if !selected.is_empty() {
                // Copy namespace + context from the old sibling so the new child
                // inherits the same document context.
                let sibling_info = instance
                    .item_facts()
                    .into_iter()
                    .find(|fact| {
                        if !old.is_empty() {
                            fact.concept_name().local_name == old
                        } else {
                            fact.concept_name().local_name.starts_with(concept)
                        }
                    })
                    .map(|fact| {
                        (
                            fact.concept_name().namespace_uri.clone(),
                            fact.context_ref().to_owned(),
                        )
                    });

                let (namespace_uri, context_ref) = sibling_info.unwrap_or_else(|| {
                    let selected_concept = taxonomy.and_then(|tax| {
                        tax.elements()
                            .into_iter()
                            .find(|concept| concept.name.local_name == selected.as_str())
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

                let new_child = ItemFact::new(
                    None,
                    ExpandedName::new(namespace_uri, selected.clone()),
                    context_ref,
                    None,
                    String::new(),
                    false,
                    None,
                    None,
                );

                match instance
                    .add_tuple_child(concept, &new_child)
                    .with_context(|| format!("Failed to add tuple child '{selected}'"))?
                {
                    0 => {
                        // Child already exists as nil; activate it.
                        instance
                            .set_tuple_child_nil(concept, selected, false)
                            .with_context(|| {
                                format!("Failed to activate tuple child '{selected}'")
                            })?;
                    }
                    _ => {
                        debug!("Added tuple child '{}' for concept '{}'", selected, concept);
                    }
                }
            }

            Ok(UpdateOutcome::Rebuild)
        }
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
