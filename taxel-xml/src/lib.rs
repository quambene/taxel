use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::io::BufRead;

pub fn xml_updater<R, W>(
    reader: &mut Reader<R>,
    writer: &mut Writer<W>,
    target_tag_name: &[u8],
    target_tag_value: &str,
) -> Result<(), anyhow::Error>
where
    R: std::io::Read + BufRead,
    W: std::io::Write,
{
    let mut buf = Vec::new();
    let mut is_target_tag = false;

    // Write the XML declaration to the destination file
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    // Process each event in the xml file
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == target_tag_name {
                    // Found the start of target tag
                    is_target_tag = true;
                }

                writer.write_event(Event::Start(e.clone()))?;
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == target_tag_name {
                    // Found an empty tag for the target tag
                    is_target_tag = false;
                }

                writer.write_event(Event::End(e))?;
            }
            Ok(Event::Empty(e)) => {
                // Write the text content to the output XML file
                writer.write_event(Event::Empty(e))?;
            }
            Ok(Event::Text(mut text)) => {
                if is_target_tag {
                    // Modify the value inside the target tag
                    text = BytesText::new(target_tag_value);
                }

                // Write the text content to the output XML file
                writer.write_event(Event::Text(text))?;
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
                        <ProduktVersion>ABC</ProduktVersion>
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
                        <ProduktName>ABC</ProduktName>
                        <ProduktVersion>ABC</ProduktVersion>
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

        // Write the XML declaration to the destination file
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
    fn test_xml_updater() {
        let mut reader = Reader::from_str(INPUT_XML);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        let target_tag_name = b"HerstellerID";
        let target_tag_value = "99999";

        xml_updater(&mut reader, &mut writer, target_tag_name, target_tag_value).unwrap();

        let actual: Vec<u8> = writer.into_inner().into_inner();
        let expected = remove_formatting(OUTPUT_XML).unwrap();

        assert_eq!(String::from_utf8(actual).unwrap(), expected);
    }
}
