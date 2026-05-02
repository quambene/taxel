use crate::domain::FactValue;
use anyhow::Context;
use log::debug;
use xbrl_rs::{ExpandedName, InstanceDocument, ItemFact, NamespaceUri, TaxonomySet};

/// The outcome of [`update_instance_document`].
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// No structural change; the DocumentView does not need to be rebuilt.
    NoChange,
    /// A tuple child was switched; the caller must rebuild the DocumentView.
    Rebuild,
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
                    let namespace_uri = taxonomy
                        .and_then(|tax| {
                            tax.elements()
                                .into_iter()
                                .find(|concept| concept.name.local_name == selected.as_str())
                        })
                        .map(|concept| concept.name.namespace_uri.clone())
                        .unwrap_or_else(|| NamespaceUri::from(""));
                    (namespace_uri, String::new())
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
