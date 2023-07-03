use log::error;
use quick_xml::{
    events::{BytesDecl, Event},
    Reader, Writer,
};
use std::io::Cursor;

/// Write the xml declaration to the destination file.
pub fn write_declaration<W>(writer: &mut Writer<W>) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    // Write the xml declaration to the destination file
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    Ok(())
}

/// Helper function to remove formatting of xml file.
pub fn remove_formatting(xml: &str) -> Result<String, anyhow::Error> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut buf = Vec::new();

    // Process each event in the xml file
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                writer.write_event(Event::Start(e.clone()))?;
            }
            Ok(Event::End(e)) => {
                writer.write_event(Event::End(e))?;
            }
            Ok(Event::Empty(e)) => {
                writer.write_event(Event::Empty(e))?;
            }
            Ok(Event::Text(e)) => {
                writer.write_event(Event::Text(e))?;
            }
            Ok(Event::Decl(tag)) => {
                writer.write_event(Event::Decl(tag))?;
            }
            Ok(Event::Eof) => {
                // Reached the end of the xml file
                break;
            }
            Err(err) => {
                // Handle error while reading the xml file
                error!("Can't parse xml file: {err}");
                break;
            }
            _ => (),
        }

        buf.clear();
    }

    let formatted_xml: Vec<u8> = writer.into_inner().into_inner();
    let formatted_xml = String::from_utf8(formatted_xml).unwrap();

    Ok(formatted_xml)
}
