//! Integration tests for `Report::apply_csv_values` and
//! `Report::apply_imported_values`, which need the real taxonomy fixtures in
//! `test_data/taxonomies` — both take a `taxonomy: &TaxonomySet`, and
//! `xbrl_rs::TaxonomySet` has no in-memory constructor (only
//! `TaxonomySet::discover`, which parses real XSD/reference/presentation
//! files), so these can't be reduced to fixture-free unit tests. Mirrors the
//! setup used by `taxel-gui/tests/rebuild_instance.rs`.
//!
//! These tests each encode a real correctness bug found by hand during CLI
//! smoke-testing (export a report to CSV, edit it, reimport, diff a
//! re-export against the edit) before the fix shipped:
//! - `csv_round_trip_with_no_edits_reports_ambiguous_repeating_tuple`:
//!   matching solely by `(concept, context, unit)` silently mis-attributed
//!   values across repeating tuples that share a context (e.g. two
//!   `shareholder` entries) — fixed by matching on `(concept, parent, unit)`
//!   with disagreeing duplicates collapsed to a warning instead of a guess.
//! - `csv_export_excludes_calculated_totals`: a fact's `is_calculated`
//!   status is computed per presentation section, so the same fact can be a
//!   plain leaf in one section and a derived rollup in another — exporting
//!   whichever section happened to be enabled baked a derived number into
//!   the CSV as if it were the fact's real value.
//! - `csv_import_skips_dropdown_fields_with_warning`: the dropdown-skip
//!   warning was dead code because `Dropdown` rows always have
//!   `fact_index: None`, and the `fact_index` guard ran before the
//!   `Dropdown` check.

use std::path::PathBuf;
use taxel::{
    create_instance_document, CsvImportOutcome, CsvReaderBuilder, CsvWriterBuilder, Report,
    TaxonomyType, GCD_ROLE_URI,
};
use xbrl_rs::{InstanceDocument, RoleUri, TaxonomySet};

const FIXTURE: &str = "SteuerbilanzAutoverkaeufer_PersG.xml";

fn taxonomy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test_data")
        .join("taxonomies")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test_data")
        .join("instances")
        .join("v6.5")
        .join(name)
}

/// Parses a fixture, discovers its taxonomy, and populates a `Report` —
/// mirroring the exact sequence `taxel-cli`'s `export`/`import` commands run.
fn load_report(name: &str) -> (InstanceDocument, TaxonomySet, Report) {
    let path = fixture_path(name);
    let instance = InstanceDocument::from_file(&path)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let taxonomy_type = TaxonomyType::from_schema_refs(instance.schema_refs()).unwrap_or_default();
    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_dir())
        .unwrap_or_else(|err| panic!("failed to discover taxonomy for {}: {err}", path.display()));

    let mut report = Report::new(path, taxonomy_type);
    {
        let view = instance.view(&taxonomy);
        let item_facts = instance.item_facts();
        report.populate(view, &item_facts, &taxonomy);
    }

    (instance, taxonomy, report)
}

/// Serializes a report's fact values to CSV text, using an LF-only line
/// terminator so the result is byte-comparable with `edit_csv_value`'s
/// reconstruction regardless of platform CRLF defaults.
fn export_csv(report: &Report) -> String {
    let mut buf = Vec::new();
    {
        let mut writer = CsvWriterBuilder::new()
            .delimiter(b';')
            .terminator(csv::Terminator::Any(b'\n'))
            .from_writer(&mut buf);
        report.write_values_csv("en", &mut writer).unwrap();
    }
    String::from_utf8(buf).unwrap()
}

/// Replaces the `Value` column (index 5) of the row whose `ID` column
/// (index 1) equals `id`, leaving every other row and the line structure
/// untouched. Panics if no matching row is found.
fn edit_csv_value(csv: &str, id: &str, new_value: &str) -> String {
    let mut found = false;
    let lines: Vec<String> = csv
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                return line.to_string();
            }
            let mut fields: Vec<&str> = line.split(';').collect();
            if fields.get(1) == Some(&id) {
                found = true;
                fields[5] = new_value;
            }
            fields.join(";")
        })
        .collect();
    assert!(found, "csv row for '{id}' not found");
    lines.join("\n")
}

fn apply_csv(
    report: &mut Report,
    csv: &str,
    instance: &mut InstanceDocument,
    taxonomy: &TaxonomySet,
    import_report_elements: bool,
) -> CsvImportOutcome {
    let mut reader = CsvReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_reader(csv.as_bytes());
    report
        .apply_csv_values(&mut reader, instance, taxonomy, import_report_elements)
        .unwrap()
}

/// Regression test for the bug where `is_calculated` (a per-section
/// calculation-linkbase total) was not excluded from CSV export: the same
/// fact is a plain leaf in one section but a derived rollup in another, so
/// exporting whichever section happened to be enabled baked a recomputed
/// number into the CSV as if it were the fact's own stored value.
#[test]
fn csv_export_excludes_calculated_totals() {
    let (_instance, _taxonomy, report) = load_report(FIXTURE);
    let csv = export_csv(&report);

    let ids: Vec<&str> = csv
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| line.split(';').nth(1).unwrap())
        .collect();

    assert!(
        !ids.contains(&"bs.eqLiab"),
        "'bs.eqLiab' is a calculation-linkbase total in this fixture's balance sheet \
         section and must not appear in the CSV export, got ids: {ids:?}"
    );
    assert!(
        ids.contains(&"bs.eqLiab.pretaxRes.misc"),
        "'bs.eqLiab.pretaxRes.misc' is a genuine leaf fact and should still be exported, \
         got ids: {ids:?}"
    );
}

/// Round trip: export a report, edit exactly one leaf fact's value, reimport
/// into an independently-loaded copy, and confirm only that fact changed —
/// then re-export the mutated document and confirm it's byte-identical to
/// the edited CSV, the same check used by hand to confirm the fix for the
/// `(concept, context, unit)`-collision bug.
#[test]
fn csv_round_trip_updates_only_the_targeted_fact() {
    let (_source_instance, _source_taxonomy, source_report) = load_report(FIXTURE);
    let csv = export_csv(&source_report);
    let edited_csv = edit_csv_value(&csv, "bs.eqLiab.pretaxRes.misc", "13500.00");

    let (mut instance, taxonomy, mut report) = load_report(FIXTURE);
    let outcome = apply_csv(&mut report, &edited_csv, &mut instance, &taxonomy, false);

    assert_eq!(
        outcome.updated, 1,
        "expected exactly one updated fact, warnings: {:?}",
        outcome.warnings
    );

    let mut reexported_report = Report::new(fixture_path(FIXTURE), report.taxonomy_type.clone());
    {
        let view = instance.view(&taxonomy);
        let item_facts = instance.item_facts();
        reexported_report.populate(view, &item_facts, &taxonomy);
    }
    let reexported_csv = export_csv(&reexported_report);

    assert_eq!(
        reexported_csv, edited_csv,
        "re-exporting the mutated document should reproduce the edited csv exactly"
    );
}

/// Regression test for the original silent-corruption bug: two `shareholder`
/// tuple instances share `(concept, context, unit)` for
/// `genInfo.company.id.shareholder.name` but have no other aspect to tell
/// them apart, so a completely unedited round trip must report the ambiguity
/// as a warning and update nothing — a regression here would show up as
/// `updated > 0` on a CSV that was never touched.
#[test]
fn csv_round_trip_with_no_edits_reports_ambiguous_repeating_tuple() {
    let (_source_instance, _source_taxonomy, source_report) = load_report(FIXTURE);
    let csv = export_csv(&source_report);

    let (mut instance, taxonomy, mut report) = load_report(FIXTURE);
    let outcome = apply_csv(&mut report, &csv, &mut instance, &taxonomy, false);

    assert_eq!(
        outcome.updated, 0,
        "an unedited round trip must not update any fact, warnings: {:?}",
        outcome.warnings
    );
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("ambiguous")
                && warning.contains("genInfo.company.id.shareholder.name")),
        "expected an ambiguous-value warning naming the repeating shareholder tuple, got: {:?}",
        outcome.warnings
    );
}

/// Regression test for the dead `Dropdown`-skip check: dropdown rows always
/// have `fact_index: None`, so the `fact_index` guard used to run first and
/// silently swallow the edit before the dropdown-specific warning could
/// fire. An edited dropdown field must be reported by name and never counted
/// as updated.
#[test]
fn csv_import_skips_dropdown_fields_with_warning() {
    let (_source_instance, _source_taxonomy, source_report) = load_report(FIXTURE);
    let csv = export_csv(&source_report);
    let target_id = "genInfo.report.id.reportType";
    let edited_csv = edit_csv_value(&csv, target_id, "some other value entirely");

    let (mut instance, taxonomy, mut report) = load_report(FIXTURE);
    let outcome = apply_csv(&mut report, &edited_csv, &mut instance, &taxonomy, false);

    assert_eq!(
        outcome.updated, 0,
        "dropdown edits must never be applied, warnings: {:?}",
        outcome.warnings
    );
    assert!(
        outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("dropdown") && warning.contains(target_id)),
        "expected a dropdown-unsupported warning naming '{target_id}', got: {:?}",
        outcome.warnings
    );
}

/// `Report::apply_imported_values` merges fact values from a source document
/// into a freshly-created target document (the "new report, then bring
/// values back in" workflow) — mirrors the "Muster Autoverkäufer" check done
/// by hand via `grep` during smoke testing.
#[test]
fn merge_copies_matching_values_between_documents() {
    let date = "2021-04-14";
    let taxonomy_type = TaxonomyType::CoreFiscal;
    let schema_refs = taxonomy_type.schema_refs(date);
    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_dir())
        .expect("failed to discover taxonomy for target instance");

    let roles = vec![
        RoleUri::from(GCD_ROLE_URI),
        RoleUri::from("http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet"),
    ];

    let namespace_uri = taxonomy_type.namespace_uri(date);
    let mut target_instance = create_instance_document(
        "2021-01-01",
        "2021-12-31",
        taxonomy_type.namespace_prefix(),
        &namespace_uri,
        date,
        &taxonomy,
        &roles,
    )
    .unwrap();

    // `Report::path` is pure metadata — never read or written by `Report`'s
    // own methods — so a placeholder name with no real filesystem location
    // is deliberate here, not an oversight.
    let mut target_report = Report::new(PathBuf::from("placeholder.xml"), taxonomy_type.clone());
    {
        let view = target_instance.view(&taxonomy);
        let item_facts = target_instance.item_facts();
        target_report.populate(view, &item_facts, &taxonomy);
    }

    let (source_instance, _source_taxonomy, source_report) = load_report(FIXTURE);
    let source_item_facts = source_instance.item_facts();

    let (matched_count, imported_count) = target_report.apply_imported_values(
        &source_report,
        &source_item_facts,
        &mut target_instance,
        &taxonomy,
        false,
    );

    assert!(matched_count > 0, "expected at least one matched fact");
    assert!(imported_count > 0, "expected at least one imported fact");

    let company_name_fact = target_instance
        .item_facts()
        .into_iter()
        .find(|fact| fact.concept_name().local_name == "genInfo.company.id.name")
        .expect("'genInfo.company.id.name' fact not found in target instance");

    assert_eq!(company_name_fact.value(), "Muster Autoverkäufer");
}
