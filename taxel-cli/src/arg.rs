use anyhow::anyhow;
use clap::{Arg, ArgMatches};

// args for command
pub const VERBOSE: &str = "verbose";

// args for subcommands
pub const XML_FILE: &str = "xml-file";
pub const TAX_TYPE: &str = "tax-type";
pub const TAX_VERSION: &str = "tax-version";
pub const PRINT: &str = "print";
pub const PDF_NAME: &str = "pdf-name";
pub const CERTIFICATE_FILE: &str = "certificate-file";
pub const PASSWORD: &str = "password";

pub fn get_one<'a>(matches: &'a ArgMatches, id: &str) -> Result<&'a str, anyhow::Error> {
    match matches.get_one::<String>(id) {
        Some(el) => Ok(el),
        None => return Err(anyhow!("Missing value for argument '{}'", id)),
    }
}

#[allow(dead_code)]
pub fn get_maybe_one<'a>(matches: &'a ArgMatches, id: &str) -> Option<&'a str> {
    matches.get_one::<String>(id).map(|el| el.as_str())
}

pub fn xml_file() -> Arg<'static> {
    Arg::new(XML_FILE)
        .long(XML_FILE)
        .required(true)
        .takes_value(true)
        .help("XML file to be validated")
}

pub fn tax_type() -> Arg<'static> {
    Arg::new(TAX_TYPE)
        .long(TAX_TYPE)
        .required(false)
        .takes_value(true)
        .default_value("Bilanz")
        .possible_values(["Bilanz"])
        .help("Tax type of the xml file")
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
        .help("Tax version of the xml file")
}

pub fn print() -> Arg<'static> {
    Arg::new(PRINT)
        .long(PRINT)
        .required(false)
        .takes_value(false)
        .help("Print transmission log as pdf file")
}

pub fn certificate_file() -> Arg<'static> {
    Arg::new(CERTIFICATE_FILE)
        .long(CERTIFICATE_FILE)
        .required(true)
        .takes_value(true)
        .help("Certificate (*.pfx) for encrypting and decrypting tax data")
}

pub fn password() -> Arg<'static> {
    Arg::new(PASSWORD)
        .long(PASSWORD)
        .required(true)
        .takes_value(true)
        .help("Password for using certificate")
}
