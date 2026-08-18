//! Merge fact values from one XBRL file into another.

use crate::arg::{self, IMPORT_REPORT_ELEMENTS, OUTPUT_FILE, SOURCE_FILE, TARGET_FILE};
use anyhow::Context;
use clap::{Arg, ArgMatches};
use log::debug;
use std::{fs, path::PathBuf};
use taxel::{
    load_taxonomies, restore_required_nil_tuple_children, taxonomy_version_from_schema_refs,
    ElsterReport, Report, TaxonomyType,
};
use xbrl_rs::InstanceDocument;

pub fn merge_args() -> [Arg<'static>; 5] {
    [
        arg::target_file(),
        arg::source_file(),
        arg::import_report_elements(),
        arg::taxonomy_path(),
        // Unlike `generate`/`extract`, there is no sensible "current
        // directory" fallback for the output path, so it's required here.
        arg::output_file()
            .required(true)
            .help("The path to the generated xml file."),
    ]
}

/// Merge fact values from one XBRL file into another.
///
/// Missing taxonomies are not downloaded automatically. Use `taxel download` to
/// fetch the required taxonomies first.
pub fn merge(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let target_path = PathBuf::from(arg::get_one(matches, TARGET_FILE)?);
    let source_path = PathBuf::from(arg::get_one(matches, SOURCE_FILE)?);
    let import_report_elements = matches.is_present(IMPORT_REPORT_ELEMENTS);
    let output_file = arg::get_one(matches, OUTPUT_FILE)?;
    let output_path = PathBuf::from(output_file);
    let taxonomy_dir = arg::resolve_taxonomy_dir(matches)?;
    let taxonomy_path = arg::get_maybe_one(matches, arg::TAXONOMY_PATH);

    debug!(
        "Run `taxel merge` with configuration:\n{TARGET_FILE}={}\n{SOURCE_FILE}={}\n\
         {IMPORT_REPORT_ELEMENTS}={import_report_elements}\n{OUTPUT_FILE}={output_file}",
        target_path.display(),
        source_path.display(),
    );

    // Target: the report being built up. Its full Elster envelope is parsed
    // too, since that's what gets re-serialized with the merged instance.
    let mut target_instance = InstanceDocument::from_file(&target_path)
        .with_context(|| format!("Failed to parse target XML from {}", target_path.display()))?;
    let target_schema_refs: Vec<String> = target_instance.schema_refs().to_vec();
    let target_schema_ref_paths = target_instance.schema_ref_paths();
    let target_taxonomy_type =
        TaxonomyType::from_schema_refs(target_instance.schema_refs()).unwrap_or_default();
    let target_version = taxonomy_version_from_schema_refs(&target_schema_refs);

    let target_taxonomy =
        load_taxonomies(target_schema_refs, &target_schema_ref_paths, false, &taxonomy_dir)?
            .with_context(|| match target_version {
                Some(version) => {
                    let mut suggestion = format!(
                        "taxel download --taxonomy-version {version} --taxonomy-type {target_taxonomy_type}"
                    );
                    if let Some(path) = taxonomy_path {
                        suggestion.push_str(&format!(" --taxonomy-path {path}"));
                    }

                    format!(
                        "Taxonomy v{version} ({target_taxonomy_type}) for the target file is \
                         not downloaded yet in {}. Run `{suggestion}` first.",
                        taxonomy_dir.display()
                    )
                }
                None => "Could not determine the taxonomy version for the target file.".to_string(),
            })?;

    let target_view = target_instance.view(&target_taxonomy);
    let target_item_facts = target_instance.item_facts();
    let mut target_report = Report::new(target_path.clone(), target_taxonomy_type);
    target_report.populate(target_view, &target_item_facts, &target_taxonomy);

    let target_xml = fs::read_to_string(&target_path)
        .with_context(|| format!("Failed to read target XML from {}", target_path.display()))?;
    let mut elster = ElsterReport::parse(&target_xml).with_context(|| {
        format!(
            "Failed to parse ElsterReport from {}",
            target_path.display()
        )
    })?;

    // Source: values to pull in.
    let source_instance = InstanceDocument::from_file(&source_path)
        .with_context(|| format!("Failed to parse source XML from {}", source_path.display()))?;
    let source_schema_refs: Vec<String> = source_instance.schema_refs().to_vec();
    let source_schema_ref_paths = source_instance.schema_ref_paths();
    let source_taxonomy_type =
        TaxonomyType::from_schema_refs(source_instance.schema_refs()).unwrap_or_default();
    let source_version = taxonomy_version_from_schema_refs(&source_schema_refs);

    let source_taxonomy =
        load_taxonomies(source_schema_refs, &source_schema_ref_paths, false, &taxonomy_dir)?
            .with_context(|| match source_version {
                Some(version) => {
                    let mut suggestion = format!(
                        "taxel download --taxonomy-version {version} --taxonomy-type {source_taxonomy_type}"
                    );
                    if let Some(path) = taxonomy_path {
                        suggestion.push_str(&format!(" --taxonomy-path {path}"));
                    }

                    format!(
                        "Taxonomy v{version} ({source_taxonomy_type}) for the source file is \
                         not downloaded yet in {}. Run `{suggestion}` first.",
                        taxonomy_dir.display()
                    )
                }
                None => "Could not determine the taxonomy version for the source file.".to_string(),
            })?;

    let source_view = source_instance.view(&source_taxonomy);
    let source_item_facts = source_instance.item_facts();
    let mut source_report = Report::new(source_path.clone(), source_taxonomy_type);
    source_report.populate(source_view, &source_item_facts, &source_taxonomy);

    let (matched_count, imported_count) = target_report.apply_imported_values(
        &source_report,
        &source_item_facts,
        &mut target_instance,
        &target_taxonomy,
        import_report_elements,
    );

    restore_required_nil_tuple_children(&mut target_instance, &target_taxonomy);

    let mut xbrl_bytes: Vec<u8> = Vec::new();
    target_instance.to_writer(&mut xbrl_bytes)?;
    elster.set_payload_xbrl(xbrl_bytes);
    let xml = elster.to_xml()?;

    fs::write(&output_path, &xml)
        .with_context(|| format!("Failed to write merged report to {}", output_path.display()))?;

    println!(
        "Merged values into {} (matched facts: {matched_count}, updated facts: {imported_count})",
        output_path.display()
    );

    Ok(())
}
