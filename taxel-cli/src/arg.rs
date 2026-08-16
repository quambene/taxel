use anyhow::anyhow;
use clap::{Arg, ArgMatches};
use std::path::PathBuf;
use taxel::{TaxonomyType, TAXONOMY_VERSION_TO_DATE};

// args for command
pub const VERBOSE: &str = "verbose";

// args for subcommands
pub const XML_FILE: &str = "xml-file";
pub const CSV_FILE: &str = "csv-file";
pub const TEMPLATE_FILE: &str = "template-file";
pub const OUTPUT_FILE: &str = "output-file";
pub const LOG_DIR: &str = "log-dir";
pub const TAX_TYPE: &str = "tax-type";
pub const TAX_VERSION: &str = "tax-version";
pub const PRINT: &str = "print";
pub const START_DATE: &str = "start-date";
pub const END_DATE: &str = "end-date";
pub const TAXONOMY_VERSION: &str = "taxonomy-version";
pub const TAXONOMY_TYPE: &str = "taxonomy-type";
pub const TARGET_FILE: &str = "target-file";
pub const SOURCE_FILE: &str = "source-file";
pub const IMPORT_REPORT_ELEMENTS: &str = "import-report-elements";
pub const TAXONOMY_PATH: &str = "taxonomy-path";

pub fn get_one<'a>(matches: &'a ArgMatches, id: &str) -> Result<&'a str, anyhow::Error> {
    match matches.get_one::<String>(id) {
        Some(el) => Ok(el),
        None => Err(anyhow!("Missing value for argument '{}'", id)),
    }
}

pub fn get_maybe_one<'a>(matches: &'a ArgMatches, id: &str) -> Option<&'a str> {
    matches.get_one::<String>(id).map(|el| el.as_str())
}

pub fn xml_file() -> Arg<'static> {
    Arg::new(XML_FILE)
        .long(XML_FILE)
        .required(true)
        .takes_value(true)
        .help("The path to the XML file to be validated.")
}

pub fn csv_file() -> Arg<'static> {
    Arg::new(CSV_FILE)
        .long(CSV_FILE)
        .required(false)
        .takes_value(true)
        .help("The path to the csv file used to generate the xml file.")
}

pub fn template_file() -> Arg<'static> {
    Arg::new(TEMPLATE_FILE)
        .long(TEMPLATE_FILE)
        .required(true)
        .takes_value(true)
        .help("The path to the template file used to generate the xml file.")
}

pub fn output_file() -> Arg<'static> {
    Arg::new(OUTPUT_FILE)
        .long(OUTPUT_FILE)
        .required(false)
        .takes_value(true)
        .help("The path to the generated the xml file. If no path is specified the current directory will be used as output path.")
}

pub fn log_dir() -> Arg<'static> {
    Arg::new(LOG_DIR)
        .long(LOG_DIR)
        .required(false)
        .takes_value(true)
        .help("The directory for log output.")
}

pub fn tax_type() -> Arg<'static> {
    Arg::new(TAX_TYPE)
        .long(TAX_TYPE)
        .required(false)
        .takes_value(true)
        .default_value("Bilanz")
        .possible_values(["Bilanz"])
        .help("The tax type of the xml file.")
}

pub fn tax_version() -> Arg<'static> {
    Arg::new(TAX_VERSION)
        .long(TAX_VERSION)
        .required(false)
        .takes_value(true)
        .default_value("6.5")
        .possible_values([
            "5.0", "5.1", "5.2", "5.3", "5.4", "6.0", "6.1", "6.2", "6.3", "6.4", "6.5",
        ])
        .help("The tax version of the xml file.")
}

pub fn print() -> Arg<'static> {
    Arg::new(PRINT)
        .long(PRINT)
        .value_name("pdf-name")
        .required(false)
        .takes_value(true)
        .help("Print the transmission confirmation as pdf file.")
}

pub fn start_date() -> Arg<'static> {
    Arg::new(START_DATE)
        .long(START_DATE)
        .required(true)
        .takes_value(true)
        .help("The start date of the reporting period, in YYYY-MM-DD format.")
}

pub fn end_date() -> Arg<'static> {
    Arg::new(END_DATE)
        .long(END_DATE)
        .required(true)
        .takes_value(true)
        .help("The end date (balance sheet date) of the reporting period, in YYYY-MM-DD format.")
}

pub fn target_file() -> Arg<'static> {
    Arg::new(TARGET_FILE)
        .long(TARGET_FILE)
        .required(true)
        .takes_value(true)
        .help("The path to the XML file to import values into.")
}

pub fn source_file() -> Arg<'static> {
    Arg::new(SOURCE_FILE)
        .long(SOURCE_FILE)
        .required(true)
        .takes_value(true)
        .help("The path to the XML file to import values from.")
}

pub fn import_report_elements() -> Arg<'static> {
    Arg::new(IMPORT_REPORT_ELEMENTS)
        .long(IMPORT_REPORT_ELEMENTS)
        .required(false)
        .takes_value(false)
        .help("Also import report-element (section) selections from the source file.")
}

pub fn taxonomy_version() -> Arg<'static> {
    let mut versions: Vec<&str> = TAXONOMY_VERSION_TO_DATE.keys().copied().collect();
    versions.sort_unstable();

    Arg::new(TAXONOMY_VERSION)
        .long(TAXONOMY_VERSION)
        .required(true)
        .takes_value(true)
        .possible_values(versions)
        .help("The eBilanz taxonomy version.")
}

pub fn taxonomy_type() -> Arg<'static> {
    Arg::new(TAXONOMY_TYPE)
        .long(TAXONOMY_TYPE)
        .required(true)
        .takes_value(true)
        .possible_values([
            "core-fiscal",
            "core-fiscal-microbilg",
            "supplementary-fiscal",
            "supplementary-fiscal-microbilg",
            "credit-institution",
            "payment-institution",
            "insurance",
        ])
        .help("The eBilanz taxonomy module.")
}

/// Parses a `--taxonomy-type` flag value into a `TaxonomyType`. The inverse
/// of [`taxonomy_type_flag_value`].
pub fn parse_taxonomy_type(value: &str) -> Result<TaxonomyType, anyhow::Error> {
    Ok(match value {
        "core-fiscal" => TaxonomyType::CoreFiscal,
        "core-fiscal-microbilg" => TaxonomyType::CoreFiscalMicroBilG,
        "supplementary-fiscal" => TaxonomyType::SupplementaryFiscal,
        "supplementary-fiscal-microbilg" => TaxonomyType::SupplementaryFiscalMicroBilG,
        "credit-institution" => TaxonomyType::CreditInstitution,
        "payment-institution" => TaxonomyType::PaymentInstitution,
        "insurance" => TaxonomyType::Insurance,
        other => return Err(anyhow!("Unknown taxonomy type '{other}'")),
    })
}

/// Formats a `TaxonomyType` back into its `--taxonomy-type` flag value. The
/// inverse of [`parse_taxonomy_type`]; used to build copy-pasteable
/// `taxel download` suggestions in error messages.
pub fn taxonomy_type_flag_value(taxonomy_type: &TaxonomyType) -> &'static str {
    match taxonomy_type {
        TaxonomyType::CoreFiscal => "core-fiscal",
        TaxonomyType::CoreFiscalMicroBilG => "core-fiscal-microbilg",
        TaxonomyType::SupplementaryFiscal => "supplementary-fiscal",
        TaxonomyType::SupplementaryFiscalMicroBilG => "supplementary-fiscal-microbilg",
        TaxonomyType::CreditInstitution => "credit-institution",
        TaxonomyType::PaymentInstitution => "payment-institution",
        TaxonomyType::Insurance => "insurance",
    }
}

pub fn taxonomy_path() -> Arg<'static> {
    Arg::new(TAXONOMY_PATH)
        .long(TAXONOMY_PATH)
        .required(false)
        .takes_value(true)
        .help(
            "The directory containing cached taxonomy files. Defaults to the OS data directory \
             used by `taxel download`.",
        )
}

/// Resolves the effective taxonomy directory: the `--taxonomy-path` override
/// if given, otherwise `taxel::taxonomy_dir()`.
pub fn resolve_taxonomy_dir(matches: &ArgMatches) -> Result<PathBuf, anyhow::Error> {
    match get_maybe_one(matches, TAXONOMY_PATH) {
        Some(path) => Ok(PathBuf::from(path)),
        None => taxel::taxonomy_dir(),
    }
}
