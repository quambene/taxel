//! Reapply fact values from a CSV file (produced by `taxel export`) into an
//! XBRL file.

use crate::arg::{self, CSV_FILE, IMPORT_REPORT_ELEMENTS, OUTPUT_FILE, XML_FILE};
use anyhow::Context;
use clap::{Arg, ArgMatches};
use log::debug;
use std::{
    fs,
    path::{Path, PathBuf},
};
use taxel::{
    load_taxonomies, taxonomy_version_from_schema_refs, CsvReaderBuilder, ElsterReport, Report,
    TaxonomyType,
};
use xbrl_rs::InstanceDocument;

pub fn import_args() -> [Arg<'static>; 5] {
    [
        arg::xml_file().help("The path to the XML file to import CSV values into."),
        arg::csv_file(),
        arg::import_report_elements(),
        arg::taxonomy_path(),
        arg::output_file()
            .required(true)
            .help("The path to the generated xml file."),
    ]
}

pub fn import(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, XML_FILE)?;
    let csv_file = arg::get_one(matches, CSV_FILE)?;
    let import_report_elements = matches.is_present(IMPORT_REPORT_ELEMENTS);
    let output_file = arg::get_one(matches, OUTPUT_FILE)?;
    let output_path = PathBuf::from(output_file);
    let taxonomy_dir = arg::resolve_taxonomy_dir(matches)?;
    let taxonomy_path = arg::get_maybe_one(matches, arg::TAXONOMY_PATH);

    debug!(
        "Run `taxel import` with configuration:\n{XML_FILE}={xml_file}\n{CSV_FILE}={csv_file}\n\
         {IMPORT_REPORT_ELEMENTS}={import_report_elements}\n{OUTPUT_FILE}={output_file}",
    );

    let mut instance = InstanceDocument::from_file(Path::new(xml_file))
        .with_context(|| format!("Failed to parse XML from {xml_file}"))?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let schema_ref_paths = instance.schema_ref_paths();
    let taxonomy_type = TaxonomyType::from_schema_refs(instance.schema_refs()).unwrap_or_default();
    let taxonomy_type_flag = arg::taxonomy_type_flag_value(&taxonomy_type);
    let version = taxonomy_version_from_schema_refs(&schema_refs);

    // Never download here: `import` is a pure, offline file transformation.
    // Missing taxonomies are fetched explicitly via `taxel download`.
    let taxonomy = load_taxonomies(schema_refs, &schema_ref_paths, false, &taxonomy_dir)?
        .with_context(|| match version {
            Some(version) => {
                let mut suggestion = format!(
                    "taxel download --taxonomy-version {version} --taxonomy-type {taxonomy_type_flag}"
                );
                if let Some(path) = taxonomy_path {
                    suggestion.push_str(&format!(" --taxonomy-path {path}"));
                }

                format!(
                    "Taxonomy v{version} ({taxonomy_type_flag}) is not downloaded yet in {}. Run \
                     `{suggestion}` first.",
                    taxonomy_dir.display()
                )
            }
            None => "Could not determine the taxonomy version for this file.".to_string(),
        })?;

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let mut report = Report::new(PathBuf::from(xml_file), taxonomy_type);
    report.populate(view, &item_facts, &taxonomy);

    let xml = fs::read_to_string(xml_file)
        .with_context(|| format!("Failed to read XML from {xml_file}"))?;
    let mut elster = ElsterReport::parse(&xml)
        .with_context(|| format!("Failed to parse ElsterReport from {xml_file}"))?;

    let mut csv_reader = CsvReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(csv_file)
        .with_context(|| format!("Failed to open csv file {csv_file}"))?;

    let outcome = report.apply_csv_values(
        &mut csv_reader,
        &mut instance,
        &taxonomy,
        import_report_elements,
    )?;

    for warning in &outcome.warnings {
        eprintln!("Warning: {warning}");
    }

    let mut xbrl_bytes: Vec<u8> = Vec::new();
    instance.to_writer(&mut xbrl_bytes)?;
    elster.set_payload_xbrl(xbrl_bytes);
    let xml = elster.to_xml()?;

    fs::write(&output_path, &xml).with_context(|| {
        format!(
            "Failed to write imported report to {}",
            output_path.display()
        )
    })?;

    println!(
        "Applied CSV values to {}: matched {} rows, updated {} facts",
        output_path.display(),
        outcome.matched,
        outcome.updated
    );

    Ok(())
}
