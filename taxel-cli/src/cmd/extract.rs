//! Generate csv from a given xml file in the XBRL standard.

use crate::arg::{self, OUTPUT_FILE, XML_FILE};
use clap::{Arg, ArgMatches};
use std::{env::current_dir, fs::File, io::BufReader, path::PathBuf};
use taxel_xml::{CsvWriterBuilder, Reader};

pub fn xml_file() -> Arg<'static> {
    Arg::new(XML_FILE)
        .long(XML_FILE)
        .required(true)
        .takes_value(true)
        .help("The path to the XML file to be extracted.")
}

pub fn output_file() -> Arg<'static> {
    Arg::new(OUTPUT_FILE)
        .long(OUTPUT_FILE)
        .required(false)
        .takes_value(true)
        .help("The path to the generated the csv file. If no path is specified the current directory will be used as output path.")
}

pub fn extract_args() -> [Arg<'static>; 2] {
    [xml_file(), output_file()]
}

pub fn extract(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let xml_file = arg::get_one(matches, arg::XML_FILE)?;
    let output_file = arg::get_maybe_one(matches, arg::OUTPUT_FILE);
    let output_path = match output_file {
        Some(output_file) => PathBuf::from(output_file),
        None => current_dir()?,
    };

    debug!(
        "Run `taxel extract` with configuration:\n{}={}\n{}={:?}",
        arg::XML_FILE,
        xml_file,
        arg::OUTPUT_FILE,
        output_file,
    );

    // Read tag values from an xml file
    let xml_file = File::open(xml_file)?;
    let reader = BufReader::new(xml_file);

    // Create a reader for parsing the XML file
    let mut xml_reader = Reader::from_reader(reader);
    xml_reader.trim_text(true);

    let extracted_tags = taxel_xml::extract_tag_values(&mut xml_reader)?;

    let mut csv_writer = CsvWriterBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .from_path(output_path)?;

    taxel_xml::write_tags(&mut csv_writer, extracted_tags)?;

    Ok(())
}
