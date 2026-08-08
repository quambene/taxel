//! Integration test for `rebuild_instance`, which needs the real taxonomy
//! fixtures in `test_data/taxonomies` — the underlying filtering logic is
//! driven entirely by `hgbref:*` reference annotations resolved through
//! `xbrl_rs::TaxonomySet`, which has no in-memory constructor (only
//! `TaxonomySet::discover`, which parses real XSD/reference/presentation
//! files), so this can't be reduced to a fixture-free unit test.
//!
//! Everything imported from `taxel_gui` here is narrowly re-exported
//! (`#[doc(hidden)]`) from `lib.rs` solely to support this test — it is not
//! a stable public API.

use std::path::PathBuf;
use taxel::{elster::Submitter, ElsterReport, TaxonomyType, GCD_ROLE_URI};
use taxel_gui::{
    create_instance_document, rebuild_instance, update_instance_document, FactValue,
    LoadedReport, NewReportForm, Report, ReportList, Search, SectionState, Settings, TaxelApp,
};
use xbrl_rs::{InstanceDocument, RoleUri, TaxonomySet};

/// Regression test for the bug fixed by the second `remove_forbidden_facts`
/// pass in `rebuild_instance`: legal-form-based removal depends on the
/// entity's legal status, which isn't known yet at the point
/// `create_instance_document` runs its own (first) call — `fresh_instance`
/// is still blank there, so `entity_legal_forms` is always empty and that
/// call's legal-form check is unconditionally skipped (it fails *safe*,
/// unlike the STU check, which fails by actively removing — a fact deleted
/// that early can never come back, so a second pass can't help STU, only
/// legal-form). Without the second pass, a fact restricted to a legal form
/// the entity doesn't have would incorrectly survive forever once any
/// report-element/consolidation-range change ever triggers a rebuild.
#[test]
fn rebuild_removes_facts_not_applicable_to_entity_legal_form() {
    let taxonomy_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test_data")
        .join("taxonomies");
    let date = "2024-04-01";
    let taxonomy_type = TaxonomyType::CoreFiscal;
    let schema_refs = taxonomy_type.schema_refs(date);
    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_dir).unwrap();

    let roles = vec![
        RoleUri::from(GCD_ROLE_URI),
        RoleUri::from("http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet"),
    ];

    let namespace_uri = taxonomy_type.namespace_uri(date);
    let mut instance = create_instance_document(
        "2024-01-01",
        "2024-12-31",
        taxonomy_type.namespace_prefix(),
        &namespace_uri,
        date,
        &taxonomy,
        &roles,
    )
    .unwrap();

    // Activate the `B` (balance sheet) report element so `active_roles`
    // includes the balance sheet role during rebuild.
    set_fact_active(
        &mut instance,
        "genInfo.report.id.reportElement.reportElements.B",
    );

    // Declare the entity's legal form as "AG" (Aktiengesellschaft), which
    // the taxonomy tags `hgbref:legalFormKSt=true` only (see
    // de-gcd-2024-04-01-reference-fiscal.xml). `legalStatus` is a
    // single-select choice tuple, so — unlike the multi-select
    // `reportElements.*` checkboxes above — the fresh instance has no
    // pre-existing nil placeholder to flip; the child fact has to be
    // created via the same `update_instance_document` path a real dropdown
    // edit takes.
    let ag_value = FactValue::Dropdown {
        selected: "genInfo.company.id.legalStatus.legalStatus.AG".to_owned(),
        options: vec![],
    };
    let empty_snapshot = FactValue::Dropdown {
        selected: String::new(),
        options: vec![],
    };
    update_instance_document(
        &mut instance,
        &ag_value,
        Some(&empty_snapshot),
        None,
        None,
        "genInfo.company.id.legalStatus",
        Some(&taxonomy),
    )
    .unwrap();

    // `bs.ass.fixAss.fin.sharesInAffil.generalPartners` is tagged
    // `hgbref:legalFormPG=true` only (no `legalFormKSt`) and is not a
    // Mussfeld — i.e. it's restricted to partnerships and inapplicable for
    // an AG, and not exempt via the "mandatory" override.
    let target_concept = "bs.ass.fixAss.fin.sharesInAffil.generalPartners";
    let target_idx = fact_index(&instance, target_concept);
    instance.set_fact_nil(target_idx, false);
    instance.set_fact_value(target_idx, "1000".to_owned());

    let mut report = Report::new(
        PathBuf::from("/tmp/rebuild_test.xml"),
        taxonomy_type.clone(),
    );
    {
        let view = instance.view(&taxonomy);
        let item_facts = instance.item_facts();
        report.populate(view, &item_facts, &taxonomy);
    }

    let elster = ElsterReport::new(
        "test-vendor".to_string(),
        Submitter::default(),
        "",
        "",
        20241231,
        None::<String>,
    );

    let mut app = TaxelApp {
        loaded: Some(LoadedReport {
            taxonomy,
            instance,
            elster,
            report,
        }),
        eric: None,
        report_list: ReportList::new(),
        selected_tab: 0,
        section_states: (0..5).map(|_| SectionState::default()).collect(),
        settings: Settings {
            lang: "en".to_string(),
            zoom_input: "100".to_string(),
            dark_mode: false,
            terms_accepted: true,
        },
        diagnostics: Vec::new(),
        show_diagnostics_panel: false,
        search: Search::default(),
        loading: None,
        show_download_modal: false,
        pending_load_kind: None,
        editing_section: None,
        edit_snapshot: Vec::new(),
        show_delete_modal: false,
        show_send_modal: false,
        show_import_values_modal: false,
        show_shortcuts_modal: false,
        copy_message: None,
        send_certificate_path: None,
        send_password: String::new(),
        import_values_path: None,
        show_new_report_modal: false,
        new_report_form: NewReportForm::default(),
        show_report_element_uncheck_modal: false,
        pending_report_element_uncheck: None,
        pending_remove_report: None,
    };

    rebuild_instance(&mut app, false).unwrap();

    let rebuilt_instance = &app.loaded.as_ref().unwrap().instance;
    let still_present = rebuilt_instance
        .item_facts()
        .iter()
        .any(|fact| fact.concept_name().local_name == target_concept);

    assert!(
        !still_present,
        "'{target_concept}' is restricted to partnerships (legalFormPG) and should have \
         been removed for an AG (legalFormKSt) entity during rebuild"
    );
}

fn fact_index(instance: &InstanceDocument, concept: &str) -> usize {
    instance
        .item_facts()
        .iter()
        .position(|fact| fact.concept_name().local_name == concept)
        .unwrap_or_else(|| panic!("fact '{concept}' not found in instance"))
}

fn set_fact_active(instance: &mut InstanceDocument, concept: &str) {
    let idx = fact_index(instance, concept);
    instance.set_fact_nil(idx, false);
}
