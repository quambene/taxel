use crate::{CsvRow, Tag};
use csv::Writer;
use log::info;

pub fn write_tags<W>(writer: &mut Writer<W>, extracted_tags: Vec<Tag>) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    info!("Write tags to csv file");

    for tag in extracted_tags {
        let row = CsvRow::new(tag.name, tag.value);
        writer.serialize(row)?;
    }

    writer.flush()?;

    Ok(())
}
