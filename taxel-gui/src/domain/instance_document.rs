use crate::domain::{FactValue, ReportSection};
use anyhow::Context;
use log::debug;
use rust_decimal::Decimal;
use taxel::{create_item_fact, ensure_nil_tuple_child, REQUIRED_NIL_TUPLE_CHILDREN};
use xbrl_rs::{Decimals, FactAttribute, FactAttributeName, InstanceDocument, TaxonomySet};

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
    parent_concept_name: Option<&str>,
    concept_name: &str,
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
        FactValue::Decimal { raw, value } => {
            if let Some(idx) = fact_index {
                if raw.is_empty() {
                    write_decimal_fact(instance, idx, None);
                } else if let Some(decimal) = value {
                    write_decimal_fact(instance, idx, Some(*decimal));
                }
            }
            Ok(UpdateOutcome::NoChange)
        }
        FactValue::Integer(value) => {
            if let Some(idx) = fact_index {
                if value.is_empty() {
                    instance.set_fact_nil(idx, true);
                    instance.clear_fact_attribute(idx, FactAttributeName::Decimals);
                } else {
                    instance.set_fact_value(idx, value.clone());
                    instance.set_fact_attribute(idx, FactAttribute::Decimals(Decimals::Infinite));
                }
            }
            Ok(UpdateOutcome::NoChange)
        }
        FactValue::BooleanDropdown(value) => {
            if let Some(idx) = fact_index {
                if value.is_empty() {
                    instance.set_fact_nil(idx, true);
                } else {
                    instance.set_fact_value(idx, value.clone());
                    instance.set_fact_nil(idx, false);
                }
            }
            Ok(UpdateOutcome::NoChange)
        }
        FactValue::Date { raw, value } => {
            if let Some(idx) = fact_index {
                if raw.is_empty() {
                    instance.set_fact_nil(idx, true);
                } else if let Some(date) = value {
                    instance.set_fact_value(idx, date.format("%Y-%m-%d").to_string());
                    instance.set_fact_nil(idx, false);
                }
            }
            Ok(UpdateOutcome::NoChange)
        }
    }
}

/// Writes a decimal fact value (or clears it to nil when `value` is `None`),
/// setting the `decimals` attribute consistently. Shared by
/// [`update_instance_document`]'s `FactValue::Decimal` arm and
/// [`write_calculated_values_to_instance`] so the two write paths can't
/// drift apart.
fn write_decimal_fact(instance: &mut InstanceDocument, idx: usize, value: Option<Decimal>) {
    match value {
        Some(decimal) => {
            instance.set_fact_value(idx, decimal.to_string());
            instance.set_fact_attribute(idx, FactAttribute::Decimals(Decimals::Finite(2)));
        }
        None => {
            instance.set_fact_nil(idx, true);
            instance.clear_fact_attribute(idx, FactAttributeName::Decimals);
        }
    }
}

/// Writes every calculated-total row's current (recomputed) value into
/// `instance`, bypassing [`update_instance_document`]'s snapshot/dropdown/
/// tuple machinery since calculated rows are always plain `Decimal` facts.
/// No-op for rows with `fact_index: None` (the concept isn't represented as
/// a fact in this instance document).
///
/// This exists because [`Report::recompute_calculated_values`] only fixes
/// the in-memory `FactRow` values shown in the table — it doesn't itself
/// persist into `InstanceDocument`. Persistence for a live edit happens
/// through the normal snapshot-diff path in `save_report`; this function is
/// for the one path (`rebuild_instance`) that persists to `InstanceDocument`
/// directly, bypassing that diff.
pub fn write_calculated_values_to_instance(
    section: &ReportSection,
    instance: &mut InstanceDocument,
) {
    for row in &section.rows {
        if !row.is_calculated {
            continue;
        }

        if let (Some(idx), FactValue::Decimal { value, .. }) = (row.fact_index, &row.value) {
            write_decimal_fact(instance, idx, *value);
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
