//! Export fact values from an XBRL file to a semicolon-delimited CSV file.

use crate::arg::{self, LANG, OUTPUT_FILE, XML_FILE};
use anyhow::Context;
use clap::{Arg, ArgMatches};
use log::debug;
use std::path::Path;
use taxel::{load_taxonomies, taxonomy_version_from_schema_refs, CsvWriterBuilder, Report, TaxonomyType};
use xbrl_rs::InstanceDocument;

pub fn export_args() -> [Arg<'static>; 4] {
    [
        arg::xml_file().help("The path to the XML file to export values from."),
        arg::lang(),
        arg::taxonomy_path(),
        arg::output_file()
            .required(true)
            .help("The path to the generated csv file."),
    ]
}

pub fn export(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, XML_FILE)?;
    let lang = arg::get_one(matches, LANG)?;
    let output_file = arg::get_one(matches, OUTPUT_FILE)?;
    let taxonomy_dir = arg::resolve_taxonomy_dir(matches)?;
    let taxonomy_path = arg::get_maybe_one(matches, arg::TAXONOMY_PATH);

    debug!(
        "Run `taxel export` with configuration:\n{XML_FILE}={xml_file}\n{LANG}={lang}\n\
         {OUTPUT_FILE}={output_file}",
    );

    let instance = InstanceDocument::from_file(Path::new(xml_file))
        .with_context(|| format!("Failed to parse XML from {xml_file}"))?;
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let schema_ref_paths = instance.schema_ref_paths();
    let taxonomy_type = TaxonomyType::from_schema_refs(instance.schema_refs()).unwrap_or_default();
    let taxonomy_type_flag = arg::taxonomy_type_flag_value(&taxonomy_type);
    let version = taxonomy_version_from_schema_refs(&schema_refs);

    // Never download here: `export` is a pure, offline file transformation.
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
    let mut report = Report::new(xml_file.into(), taxonomy_type);
    report.populate(view, &item_facts, &taxonomy);

    let mut writer = CsvWriterBuilder::new()
        .delimiter(b';')
        .from_path(output_file)?;
    let row_count = report.write_values_csv(lang, &mut writer)?;

    println!("Exported {row_count} fact values to {output_file}");

    Ok(())
}
