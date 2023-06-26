//! Generate XML file according to the XBRL standard from a given CSV file.

use std::{
    env::current_dir,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use crate::arg;
use clap::{Arg, ArgMatches};
use taxel_xml::{Reader, ReaderBuilder, Trim, Writer};

pub fn generate_args() -> [Arg<'static>; 3] {
    [arg::csv_file(), arg::template_file(), arg::output_file()]
}

pub fn generate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let csv_file = arg::get_one(matches, arg::CSV_FILE)?;
    let template_file = arg::get_one(matches, arg::TEMPLATE_FILE)?;
    let output_file = arg::get_maybe_one(matches, arg::OUTPUT_FILE);
    let csv_path = Path::new(csv_file);
    let output_path = if let Some(output_file) = output_file {
        PathBuf::from(output_file)
    } else {
        current_dir()?
    };

    // Read the csv file
    let mut csv_reader = ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .trim(Trim::All)
        .from_path(csv_path)?;

    // Read the structure from a template file
    let template_file = File::open(template_file)?;
    let reader = BufReader::new(template_file);

    // Create a reader for parsing the XML file
    let mut xml_reader = Reader::from_reader(reader);
    xml_reader.trim_text(true);

    // Create a new XML file as output
    let output_file = File::create(output_path)?;
    let mut xml_writer = Writer::new(output_file);

    let target_tags = taxel_xml::read_target_tags(&mut csv_reader)?;

    taxel_xml::update_target_tags(target_tags, &mut xml_reader, &mut xml_writer)?;

    // Flush the output XML writer and finalize the file
    xml_writer.into_inner().sync_all()?;

    Ok(())
}
