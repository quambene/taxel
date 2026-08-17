//! Build a new, taxonomy-valid eBilanz XBRL report from scratch.

use crate::arg::{self, END_DATE, OUTPUT_FILE, START_DATE, TAXONOMY_TYPE, TAXONOMY_VERSION};
use anyhow::Context;
use clap::{Arg, ArgMatches};
use log::debug;
use std::{env, fs, path::PathBuf};
use taxel::{
    create_instance_document, elster::Submitter, load_taxonomies, schema_ref_paths, ElsterReport,
    Report, TaxonomyType, BASELINE_ROLE_URIS, TAXONOMY_VERSION_TO_DATE,
};
use xbrl_rs::RoleUri;

pub fn new_args() -> [Arg<'static>; 6] {
    [
        arg::start_date(),
        arg::end_date(),
        arg::taxonomy_version(),
        arg::taxonomy_type(),
        arg::taxonomy_path(),
        // Unlike `generate`/`extract`, there is no sensible "current
        // directory" fallback for the output path, so it's required here.
        arg::output_file()
            .required(true)
            .help("The path to the generated xml file."),
    ]
}

/// Build a new, taxonomy-valid eBilanz XBRL report from scratch.
///
/// Missing taxonomies are not downloaded automatically. Use `taxel download` to
/// fetch the required taxonomies first.
pub fn new(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let start_date = arg::get_one(matches, START_DATE)?;
    let end_date = arg::get_one(matches, END_DATE)?;
    let taxonomy_version = arg::get_one(matches, TAXONOMY_VERSION)?;
    let taxonomy_type: TaxonomyType = arg::get_one(matches, TAXONOMY_TYPE)?.parse()?;
    let output_file = arg::get_one(matches, OUTPUT_FILE)?;
    let output_path = PathBuf::from(output_file);

    debug!(
        "Run `taxel new` with configuration:\n{START_DATE}={start_date}\n{END_DATE}={end_date}\n\
         {TAXONOMY_VERSION}={taxonomy_version}\n{TAXONOMY_TYPE}={taxonomy_type}\n{OUTPUT_FILE}={output_file}",
    );

    let vendor_id = env::var("VENDOR_ID").unwrap_or_else(|_| env!("VENDOR_ID").to_string());
    let test_marker = env::var("TEST_MARKER").ok();

    let taxonomy_date = TAXONOMY_VERSION_TO_DATE
        .get(taxonomy_version)
        .with_context(|| format!("No taxonomy date known for version {taxonomy_version}"))?;

    let schema_refs = taxonomy_type.schema_refs(taxonomy_date);
    let namespace_prefix = taxonomy_type.namespace_prefix();
    let namespace_uri = taxonomy_type.namespace_uri(taxonomy_date);
    let paths = schema_ref_paths(&schema_refs);
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let taxonomy_dir = arg::resolve_taxonomy_dir(matches)?;
    let taxonomy_path = arg::get_maybe_one(matches, arg::TAXONOMY_PATH);

    let taxonomy = load_taxonomies(schema_refs, &path_refs, false, &taxonomy_dir)?.with_context(
        || {
            let mut suggestion = format!(
                "taxel download --taxonomy-version {taxonomy_version} --taxonomy-type {taxonomy_type}"
            );
            if let Some(path) = taxonomy_path {
                suggestion.push_str(&format!(" --taxonomy-path {path}"));
            }

            format!(
                "Taxonomy v{taxonomy_version} ({taxonomy_type}) is not downloaded yet in {}. \
                 Run `{suggestion}` first.",
                taxonomy_dir.display()
            )
        },
    )?;

    let baseline_roles: Vec<RoleUri> = BASELINE_ROLE_URIS
        .iter()
        .map(|&uri| RoleUri::from(uri))
        .collect();

    let mut instance = create_instance_document(
        start_date,
        end_date,
        namespace_prefix,
        &namespace_uri,
        taxonomy_date,
        &taxonomy,
        &baseline_roles,
    )?;

    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();
    let mut report = Report::new(output_path.clone(), taxonomy_type);
    report.populate(view, &item_facts, &taxonomy);
    report.initialize_period_dates(&mut instance, start_date, end_date);

    let mut xbrl_bytes: Vec<u8> = Vec::new();
    instance.to_writer(&mut xbrl_bytes)?;

    let balance_date: u32 = end_date
        .replace('-', "")
        .parse()
        .with_context(|| format!("Failed to parse end date '{end_date}' as YYYYMMDD"))?;

    let mut elster = ElsterReport::new(
        vendor_id,
        Submitter::default(),
        "",
        "",
        balance_date,
        test_marker,
    );
    elster.set_payload_xbrl(xbrl_bytes);
    let xml = elster.to_xml()?;

    fs::write(&output_path, &xml)
        .with_context(|| format!("Failed to write new report to {}", output_path.display()))?;

    println!("Created new report at {}", output_path.display());

    Ok(())
}
