use crate::{Tag, TargetTags};
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::io::BufRead;

/// Update values in xml file for given target tags.
pub fn update_target_tags<R, W>(
    target_tags: TargetTags,
    reader: &mut Reader<R>,
    writer: &mut Writer<W>,
) -> Result<(), anyhow::Error>
where
    R: std::io::Read + BufRead,
    W: std::io::Write,
{
    let mut buf = Vec::new();
    let mut target_tag = None;

    // Write the xml declaration to the destination file
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    // Process each event in the xml file
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag_name = String::from_utf8(e.name().as_ref().to_vec())?;

                if let Some(tag_value) = target_tags.get(&tag_name) {
                    // Found the start of target tag.
                    target_tag = Some(Tag::new(tag_name, tag_value.to_owned()));
                }

                writer.write_event(Event::Start(e.clone()))?;
            }
            Ok(Event::End(e)) => {
                if target_tag
                    .as_ref()
                    .is_some_and(|tag| tag.name.as_bytes() == e.name().as_ref())
                {
                    // Found the end tag for the target tag.
                    target_tag = None;
                }

                writer.write_event(Event::End(e))?;
            }
            Ok(Event::Empty(e)) => {
                // Write the text content to the output xml file.
                writer.write_event(Event::Empty(e))?;
            }
            Ok(Event::Text(mut text)) => {
                if let Some(tag) = &target_tag {
                    // Modify the value inside the target tag.
                    if let Some(tag_value) = &tag.value {
                        text = BytesText::new(tag_value);
                    } else {
                        text = BytesText::new("")
                    }
                }

                // Write the text content to the output xml file.
                writer.write_event(Event::Text(text))?;
            }
            Ok(Event::Eof) => {
                // Reached the end of the xml file.
                break;
            }
            Err(e) => {
                // Handle error while reading the xml file.
                eprintln!("Error: {}", e);
                break;
            }
            _ => (),
        }

        buf.clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const INPUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <Elster xmlns="http://www.elster.de/elsterxml/schema/v11">
        <TransferHeader>
            <Verfahren>ElsterBilanz</Verfahren>
            <DatenArt>Bilanz</DatenArt>
            <Vorgang>send-Auth</Vorgang>
            <Testmerker>700000004</Testmerker>
            <HerstellerID>11111</HerstellerID>
            <Datei>
                <Verschluesselung>CMSEncryptedData</Verschluesselung>
                <Kompression>GZIP</Kompression>
                <TransportSchluessel></TransportSchluessel>
            </Datei>
            <VersionClient>ABC</VersionClient>
        </TransferHeader>
        <DatenTeil>
            <Nutzdatenblock>
                <NutzdatenHeader>
                    <NutzdatenTicket>0001</NutzdatenTicket>
                    <Empfaenger id="F">1234</Empfaenger>
                    <Hersteller>
                        <ProduktName>ABC</ProduktName>
                        <ProduktVersion>CDE</ProduktVersion>
                    </Hersteller>
                </NutzdatenHeader>
                <Nutzdaten></Nutzdaten>
            </Nutzdatenblock>
        </DatenTeil>
    </Elster>"#;

    const OUTPUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <Elster xmlns="http://www.elster.de/elsterxml/schema/v11">
        <TransferHeader>
            <Verfahren>ElsterBilanz</Verfahren>
            <DatenArt>Bilanz</DatenArt>
            <Vorgang>send-Auth</Vorgang>
            <Testmerker>700000004</Testmerker>
            <HerstellerID>99999</HerstellerID>
            <Datei>
                <Verschluesselung>CMSEncryptedData</Verschluesselung>
                <Kompression>GZIP</Kompression>
                <TransportSchluessel></TransportSchluessel>
            </Datei>
            <VersionClient>ABC</VersionClient>
        </TransferHeader>
        <DatenTeil>
            <Nutzdatenblock>
                <NutzdatenHeader>
                    <NutzdatenTicket>0001</NutzdatenTicket>
                    <Empfaenger id="F">1234</Empfaenger>
                    <Hersteller>
                        <ProduktName>XYZ</ProduktName>
                        <ProduktVersion>UVW</ProduktVersion>
                    </Hersteller>
                </NutzdatenHeader>
                <Nutzdaten></Nutzdaten>
            </Nutzdatenblock>
        </DatenTeil>
    </Elster>"#;

    /// Remove formatting from xml file.
    fn remove_formatting(xml: &str) -> Result<String, anyhow::Error> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        let mut buf = Vec::new();

        // Write the xml declaration to the destination file
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

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
                Ok(Event::Eof) => {
                    // Reached the end of the xml file
                    break;
                }
                Err(e) => {
                    // Handle error while reading the xml file
                    eprintln!("Error: {}", e);
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

    #[test]
    fn test_update_target_tags() {
        let mut reader = Reader::from_str(INPUT_XML);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        let mut target_tags = TargetTags::new();
        target_tags.insert("HerstellerID", Some("99999"));
        target_tags.insert("ProduktName", Some("XYZ"));
        target_tags.insert("ProduktVersion", Some("UVW"));

        update_target_tags(target_tags, &mut reader, &mut writer).unwrap();

        let actual: Vec<u8> = writer.into_inner().into_inner();
        let expected = remove_formatting(OUTPUT_XML).unwrap();

        assert_eq!(String::from_utf8(actual).unwrap(), expected);
    }
}
