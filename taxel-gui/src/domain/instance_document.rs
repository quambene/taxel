use crate::domain::{FactValue, ReportSection};
use anyhow::Context;
use chrono::NaiveDate;
use log::debug;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use taxel::{
    BASELINE_ROLE_URIS, INCOME_STATEMENT_FORMAT_GKV, INCOME_STATEMENT_FORMAT_UKV,
    REPORT_ELEMENT_TO_ROLE_URI, REQUIRED_NIL_TUPLE_CHILDREN,
};
use xbrl_rs::{
    Concept, Context as XbrlContext, ContextId, Decimals, ElementParticle, EntityIdentifier,
    ExpandedName, Fact, FactAttribute, FactAttributeName, GroupParticle, InstanceDocument,
    ItemFact, NamespacePrefix, NamespaceUri, Particle, Period, PeriodType, RoleUri, TaxonomySet,
    Unit, UnitId,
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
        entity.clone(),
        Period::Duration {
            start: start_date.to_string(),
            end: end_date.to_string(),
        },
    );

    let gaap_ci_ns = format!("http://www.xbrl.de/taxonomies/de-gaap-ci-{taxonomy_date}");
    let dimensional_duration_contexts =
        fixed_assets_dimensional_contexts(&entity, start_date, end_date, &gaap_ci_ns);

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

    let cube_nt_ass_gross = ExpandedName::new(
        NamespaceUri::from(gaap_ci_ns.as_str()),
        "cube_.nt.ass.gross".to_string(),
    );

    let mut instance = InstanceDocument::from_sections(
        taxonomy,
        roles,
        namespaces,
        instant_context,
        duration_context,
        vec![],
        dimensional_duration_contexts,
        &units,
        &[cube_nt_ass_gross],
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
    let entity_legal_forms = entity_legal_forms(instance, taxonomy);

    // Use cascade removal: when a required tuple child (minOccurs ≥ 1) is forbidden,
    // remove the entire parent tuple to keep the XSD content model valid. Without
    // cascading, expired required children leave sibling facts in wrong positions
    // and ERiC rejects the instance with error 170105000.
    filter_facts_with_cascade(instance, taxonomy, |fact| {
        should_remove_fact(fact, taxonomy, end_date, &entity_legal_forms)
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

    // The `incomeStatement` (GuV) presentation role contains both the GKV
    // (Gesamtkostenverfahren) and UKV (Umsatzkostenverfahren) breakdowns side
    // by side. Once the user has picked one via `incomeStatementFormat`, hide
    // the other breakdown's facts (identified via the taxonomy's
    // `hgbref:typeOperatingResult` reference annotation) so only the selected
    // format's line items remain. Leave both in place when neither format is
    // explicitly selected yet.
    //
    // This must only run when `GuV` (the regular income statement) is
    // active. `incomeStatementMicroBilG` (`GuVMicroBilG`) reuses the very
    // same GKV-tagged concepts (e.g. `is.netIncome.regular.operatingTC.*`)
    // for its own, format-invariant presentation — §267a HGB permits only
    // Gesamtkostenverfahren there, so it has no UKV counterpart at all.
    // Filtering by format while GuV is inactive would wrongly strip those
    // shared concepts out of the MicroBilG section.
    let guv_active = instance.item_facts().iter().any(|fact| {
        fact.concept_name().local_name == "genInfo.report.id.reportElement.reportElements.GuV"
            && !fact.is_nil()
    });

    if guv_active {
        let active_income_statement_format = instance
            .item_facts()
            .iter()
            .find(|fact| {
                !fact.is_nil()
                    && matches!(
                        fact.concept_name().local_name.as_str(),
                        INCOME_STATEMENT_FORMAT_GKV | INCOME_STATEMENT_FORMAT_UKV
                    )
            })
            .map(|fact| fact.concept_name().local_name.clone());

        if let Some(active) = active_income_statement_format {
            let hidden_format = if active == INCOME_STATEMENT_FORMAT_GKV {
                "UKV"
            } else {
                "GKV"
            };

            // Mussfeld facts of the hidden format must stay (as nil) even
            // though they're not applicable — ERiC's plausibility check
            // (error 170155092, "kein Fakt zum Mussfeld ... berichtet")
            // requires every Mussfeld concept to be reported for context
            // 'D' regardless of which format tag it carries. Only
            // non-mandatory (Kannfeld) facts of the hidden format are
            // actually removed. Mirrors the same exception already made for
            // legal-form-based removal above.
            filter_facts_by(instance, |fact| {
                operating_result_type(fact, taxonomy) == Some(hidden_format)
                    && !is_fiscal_mandatory_fact(fact, taxonomy)
            });
        }
    }
}

/// Returns the concept's `hgbref:typeOperatingResult` reference annotation
/// (`"GKV"`, `"UKV"`, or `"neutral"`), if present, indicating which income
/// statement format (Gesamtkostenverfahren vs. Umsatzkostenverfahren) the
/// concept belongs to.
fn operating_result_type(fact: &Fact, taxonomy: &TaxonomySet) -> Option<&'static str> {
    let concept = taxonomy.find_concept(fact.concept_name())?;
    let id = concept.id.as_deref()?;
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

/// Returns `true` when the fact should be removed from the instance document
/// based on its annotations and the filing entity's legal form. This includes:
/// - Facts marked as not permitted for the filing date
/// - Facts marked as not permitted for trade accounting
/// - Facts that are not applicable for the entity's legal form, unless they are
///   mandatory for that legal form
fn should_remove_fact(
    fact: &Fact,
    taxonomy: &TaxonomySet,
    end_date: &NaiveDate,
    entity_legal_forms: &HashSet<String>,
) -> bool {
    let local = fact.concept_name().local_name.as_str();

    // ERiC requires these tuple placeholders to remain present as nil facts,
    // even when other annotation-based filters would classify them as not
    // applicable (e.g. legal-form restrictions).
    if REQUIRED_NIL_TUPLE_CHILDREN
        .iter()
        .any(|(tuple, child)| local == *tuple || local == *child)
    {
        return false;
    }

    if is_not_permitted(fact, taxonomy, end_date).unwrap_or_default() {
        return true;
    }

    if is_trade_accounting_not_permitted(fact, taxonomy).unwrap_or_default() {
        return true;
    }

    if entity_legal_forms.is_empty() {
        return false;
    }

    is_not_applicable_for_legal_form(fact, taxonomy, entity_legal_forms).unwrap_or_default()
        && !is_fiscal_mandatory_fact(fact, taxonomy)
}

/// Checks if the fact is marked as not permitted.
///
/// This includes both time-based restrictions (via `hgbref:ValidThrough` and
/// `hgbref:ValidSince`) and explicit `hgbref:notPermittedFor` annotations.
/// Returns `None` if the concept has no such annotations.
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

/// Checks if the fact is marked as mandatory for fiscal reporting.
fn is_fiscal_mandatory_fact(fact: &Fact, taxonomy: &TaxonomySet) -> bool {
    let Some(concept) = taxonomy.find_concept(fact.concept_name()) else {
        return false;
    };
    let Some(id) = concept.id.as_deref() else {
        return false;
    };
    let Some(references) = taxonomy.references_for(id) else {
        return false;
    };

    references.iter().any(|reference| {
        reference.parts.iter().any(|part| {
            part.name == "hgbref:fiscalRequirement" && is_mandatory_fiscal_requirement(&part.value)
        })
    })
}

/// Checks if the fact value is marked as mandatory for fiscal reporting.
fn is_mandatory_fiscal_requirement(value: &str) -> bool {
    let normalized = value.trim().to_lowercase();

    normalized.starts_with("mussfeld")
        || normalized.starts_with("summenmussfeld")
        || normalized.starts_with("rechnerisch notwendig")
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

/// Returns the set of legal-form category names (e.g. `"hgbref:legalFormKSt"`)
/// that apply to the filing entity, by reading the active (non-nil) legal-status
/// item from the instance and looking up its `mandatoryDisclosureRef` annotations.
fn entity_legal_forms(instance: &InstanceDocument, taxonomy: &TaxonomySet) -> HashSet<String> {
    const LEGAL_STATUS_PREFIX: &str = "genInfo.company.id.legalStatus.legalStatus.";
    const MANDATORY_DISCLOSURE_ROLE: &str = "http://www.xbrl.org/2003/role/mandatoryDisclosureRef";
    const LEGAL_FORM_PARTS: &[&str] = &[
        "hgbref:legalFormEU",
        "hgbref:legalFormKSt",
        "hgbref:legalFormPG",
    ];

    let mut forms = HashSet::new();

    for item in instance.item_facts() {
        let local = &item.concept_name().local_name;
        if !local.starts_with(LEGAL_STATUS_PREFIX) || item.is_nil() {
            continue;
        }
        let Some(concept) = taxonomy.find_concept(item.concept_name()) else {
            continue;
        };
        let Some(id) = concept.id.as_deref() else {
            continue;
        };
        let Some(references) = taxonomy.references_for(id) else {
            continue;
        };

        for reference in references {
            if reference.role != MANDATORY_DISCLOSURE_ROLE {
                continue;
            }

            for part in &reference.parts {
                if LEGAL_FORM_PARTS.contains(&part.name.as_str()) && part.value == "true" {
                    forms.insert(part.name.clone());
                }
            }
        }
    }

    forms
}

/// Returns `Some(true)` if the fact's concept is restricted to specific legal
/// forms (via `mandatoryDisclosureRef`) that do not include the entity's legal
/// form. Returns `None` when the concept has no legal-form restriction.
fn is_not_applicable_for_legal_form(
    fact: &Fact,
    taxonomy: &TaxonomySet,
    entity_legal_forms: &HashSet<String>,
) -> Option<bool> {
    const MANDATORY_DISCLOSURE_ROLE: &str = "http://www.xbrl.org/2003/role/mandatoryDisclosureRef";
    const LEGAL_FORM_PARTS: &[&str] = &[
        "hgbref:legalFormEU",
        "hgbref:legalFormKSt",
        "hgbref:legalFormPG",
    ];

    let concept = taxonomy.find_concept(fact.concept_name())?;
    let id = concept.id.as_deref()?;
    let references = taxonomy.references_for(id)?;

    let mut concept_legal_forms = HashSet::new();
    for reference in references {
        if reference.role != MANDATORY_DISCLOSURE_ROLE {
            continue;
        }

        for part in &reference.parts {
            if LEGAL_FORM_PARTS.contains(&part.name.as_str()) && part.value == "true" {
                concept_legal_forms.insert(part.name.clone());
            }
        }
    }

    if concept_legal_forms.is_empty() {
        return None;
    }

    // Remove if none of the concept's legal forms match the entity's forms.
    Some(concept_legal_forms.is_disjoint(entity_legal_forms))
}

/// Filters facts from the instance document based on the given predicate,
/// removing any facts (and their children) for which the predicate returns
/// true.
fn filter_facts_by(instance: &mut InstanceDocument, should_remove: impl Fn(&Fact) -> bool) {
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

    // Skip reconstruction when nothing changed.
    if filtered_facts.len() == source.facts().len()
        && filtered_facts
            .iter()
            .zip(source.facts().iter())
            .all(|(a, b)| a == b)
    {
        return;
    }

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

/// Like [`filter_facts_by`] but cascades removal upward when a child that is
/// required (minOccurs ≥ 1) in its parent's XSD content model matches the
/// predicate. Removing a required child would leave sibling facts in wrong
/// positions and cause ERiC error 170105000; instead the entire parent tuple
/// is dropped so the document stays schema-valid.
fn filter_facts_with_cascade(
    instance: &mut InstanceDocument,
    taxonomy: &TaxonomySet,
    should_remove: impl Fn(&Fact) -> bool,
) {
    let source = instance.clone();

    let filtered_facts: Vec<Fact> = source
        .facts()
        .iter()
        .filter(|fact| !should_remove(fact))
        .cloned()
        .filter_map(|mut fact| {
            if remove_children_cascade(&mut fact, &should_remove, taxonomy) {
                None
            } else {
                Some(fact)
            }
        })
        .collect();

    if filtered_facts.len() == source.facts().len()
        && filtered_facts
            .iter()
            .zip(source.facts().iter())
            .all(|(a, b)| a == b)
    {
        return;
    }

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

/// Removes forbidden children from a tuple with cascade semantics. Returns
/// `true` when the **caller** should discard this fact entirely (because a
/// required child was removed).
fn remove_children_cascade(
    fact: &mut Fact,
    should_remove: &impl Fn(&Fact) -> bool,
    taxonomy: &TaxonomySet,
) -> bool {
    let Fact::Tuple(tuple) = fact else {
        return false;
    };

    let parent_concept = taxonomy.find_concept(tuple.concept_name());

    // If any child that would be removed is required in the parent's schema,
    // signal that the whole tuple should be dropped instead.
    let has_required_removal = tuple.children().iter().any(|child| {
        should_remove(child) && is_required_in_content_model(parent_concept, child.concept_name())
    });
    if has_required_removal {
        return true;
    }

    // Remove optional forbidden children.
    tuple.children_mut().retain(|child| !should_remove(child));

    // Recurse; if a nested tuple signals cascade removal, drop it too.
    let n = tuple.children().len();
    let mut cascade = vec![false; n];
    for (i, child) in tuple.children_mut().iter_mut().enumerate() {
        cascade[i] = remove_children_cascade(child, should_remove, taxonomy);
    }
    for i in (0..n).rev() {
        if cascade[i] {
            tuple.children_mut().remove(i);
        }
    }

    false
}

/// Returns `true` when `child_name` appears in `parent_concept`'s XSD content
/// model with `minOccurs ≥ 1`.
fn is_required_in_content_model(
    parent_concept: Option<&Concept>,
    child_name: &ExpandedName,
) -> bool {
    let Some(concept) = parent_concept else {
        return false;
    };
    let Some(model) = &concept.content_model else {
        return false;
    };

    particle_requires_local_name(model, &child_name.local_name)
}

/// Recursively checks if the given particle or any of its descendants is an
/// element with the specified local name and `minOccurs ≥ 1`.
fn particle_requires_local_name(particle: &Particle, local_name: &str) -> bool {
    match particle {
        Particle::Element { element, occurs } => {
            let element_local = match element {
                ElementParticle::Ref(qname) => qname.local_name.as_str(),
                ElementParticle::Decl(decl) => decl.name.as_str(),
            };
            element_local == local_name && occurs.min >= 1
        }
        Particle::Sequence { children, .. } => children
            .iter()
            .any(|particle| particle_requires_local_name(particle, local_name)),
        Particle::Choice { .. } => false,
        Particle::Group { group, .. } => match group {
            GroupParticle::Def(group_def) => {
                particle_requires_local_name(&group_def.particle, local_name)
            }
            GroupParticle::Ref(_) => false,
        },
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
                    let period_matches = |ctx: &XbrlContext| {
                        matches!(
                            (period_type, &ctx.period),
                            (PeriodType::Duration, Period::Duration { .. })
                                | (PeriodType::Instant, Period::Instant { .. })
                        )
                    };
                    // Prefer plain (non-dimensional) contexts so that new tuple
                    // children don't accidentally inherit a dimensional contextRef
                    // from the HashMap's non-deterministic iteration order.
                    instance
                        .contexts()
                        .iter()
                        .find(|(_, ctx)| period_matches(ctx) && ctx.dimensions.is_empty())
                        .or_else(|| {
                            instance
                                .contexts()
                                .iter()
                                .find(|(_, ctx)| period_matches(ctx))
                        })
                        .map(|(id, _)| id.to_string())
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

/// Builds the 12 dimensional duration contexts required for the eBilanz fixed
/// assets movement table (`cube_.nt.ass.gross`). ERiC rule 170405045 requires
/// at least one fact from this hypercube to be present whenever
/// `fiscalYearBegin` is set. The contexts cover all four asset-class members
/// (`bs.ass.fixAss`, `.fin`, `.tan`, `.intan`) combined with all three
/// tax/commercial balance members (`dim_taxBal`, `dim_comBal`,
/// `dim_diffComToTaxBal`).
fn fixed_assets_dimensional_contexts(
    entity: &EntityIdentifier,
    start_date: &str,
    end_date: &str,
    gaap_ci_ns: &str,
) -> Vec<XbrlContext> {
    let period = Period::Duration {
        start: start_date.to_string(),
        end: end_date.to_string(),
    };
    let ns = NamespaceUri::from(gaap_ci_ns);
    let dim_asset = ExpandedName::new(ns.clone(), "dim_changes.nt.ass.gross".to_string());
    let dim_tax = ExpandedName::new(ns.clone(), "dim_taxTrans".to_string());

    let asset_members = [
        "bs.ass.fixAss",
        "bs.ass.fixAss.fin",
        "bs.ass.fixAss.tan",
        "bs.ass.fixAss.intan",
    ];
    let tax_members = [
        ("dim_taxBal", ""),
        ("dim_comBal", "dim_comBal-"),
        ("dim_diffComToTaxBal", "dim_diffComToTaxBal-"),
    ];

    let mut contexts = Vec::with_capacity(12);
    for asset in asset_members {
        for (tax, prefix) in tax_members {
            let id = format!("D-{prefix}{asset}");
            let mut ctx =
                XbrlContext::new(ContextId::from(id.as_str()), entity.clone(), period.clone());
            ctx.add_dimension(
                dim_asset.clone(),
                ExpandedName::new(ns.clone(), asset.to_string()),
            );
            ctx.add_dimension(
                dim_tax.clone(),
                ExpandedName::new(ns.clone(), tax.to_string()),
            );
            contexts.push(ctx);
        }
    }
    contexts
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
    // Keep the tuple itself non-nil: xbrl-rs `add_tuple_child` is a no-op when
    // the child already exists and won't clear tuple nil in that branch.
    if instance.set_tuple_fact_nil(tuple_concept, false).is_err() {
        return; // Tuple absent for this taxonomy type.
    }

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
