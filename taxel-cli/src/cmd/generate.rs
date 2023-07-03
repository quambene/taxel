//! Generate XML file according to the XBRL standard from a given CSV file.

use crate::arg;
use clap::{Arg, ArgMatches};
use std::{
    env::current_dir,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};
use taxel_xml::{CsvReaderBuilder, Reader, Trim, Writer, XbrlElement};

pub fn generate_args() -> [Arg<'static>; 3] {
    [arg::csv_file(), arg::template_file(), arg::output_file()]
}

pub fn generate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let csv_file = arg::get_maybe_one(matches, arg::CSV_FILE);
    let template_file = arg::get_one(matches, arg::TEMPLATE_FILE)?;
    let output_file = arg::get_maybe_one(matches, arg::OUTPUT_FILE);
    let csv_path = csv_file.map(Path::new);
    let output_path = match output_file {
        Some(output_file) => PathBuf::from(output_file),
        None => current_dir()?,
    };

    debug!(
        "Run `taxel generate` with configuration:\n{}={:?}\n{}={}\n{}={:?}",
        arg::CSV_FILE,
        csv_file,
        arg::TEMPLATE_FILE,
        template_file,
        arg::OUTPUT_FILE,
        output_file,
    );

    // Read the csv file
    let mut csv_reader = match csv_path {
        Some(csv_path) => {
            let reader = CsvReaderBuilder::new()
                .delimiter(b',')
                .has_headers(true)
                .trim(Trim::All)
                .from_path(csv_path)?;

            Some(reader)
        }
        None => None,
    };

    // Read the structure from a template file
    let template_file = File::open(template_file)?;
    let reader = BufReader::new(template_file);

    // Create a reader for parsing the XML file
    let mut xml_reader = Reader::from_reader(reader);
    xml_reader.trim_text(true);

    // Create a new XML file as output
    let output_file = File::create(output_path)?;
    let mut xml_writer = Writer::new(output_file);

    let target_tags = taxel_xml::read_target_tags(csv_reader.as_mut())?;

    let mut element = XbrlElement::parse(&mut xml_reader)?;
    element.remove_values();
    element.add_values(&target_tags);
    element.serialize(&mut xml_writer)?;

    // Flush the output XML writer and finalize the file
    xml_writer.into_inner().sync_all()?;

    Ok(())
}
