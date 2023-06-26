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

    const ACTUAL_XML: &str = r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <Elster xmlns="http://www.elster.de/elsterxml/schema/v11">
        <TransferHeader version="11">
            <Verfahren>ElsterBilanz</Verfahren>
            <DatenArt>Bilanz</DatenArt>
            <Vorgang>send-Auth</Vorgang>
            <Testmerker>700000004</Testmerker>
            <HerstellerID>00000</HerstellerID>
            <Datei>
                <Verschluesselung>CMSEncryptedData</Verschluesselung>
                <Kompression>GZIP</Kompression>
                <TransportSchluessel></TransportSchluessel>
            </Datei>
            <VersionClient>1</VersionClient>
        </TransferHeader>
        <DatenTeil>
            <Nutzdatenblock>
                <NutzdatenHeader version="11">
                    <NutzdatenTicket>0001</NutzdatenTicket>
                    <Empfaenger id="F">0000</Empfaenger>
                    <Hersteller>
                        <ProduktName>Taxel</ProduktName>
                        <ProduktVersion>0.1.0</ProduktVersion>
                    </Hersteller>
                </NutzdatenHeader>
                <Nutzdaten>
                    <ebilanz:EBilanz xmlns:ebilanz="http://rzf.fin-nrw.de/RMS/EBilanz/2016/XMLSchema"
                        version="000001">
                        <ebilanz:stichtag>00000000</ebilanz:stichtag>
                    </ebilanz:EBilanz>
                </Nutzdaten>
            </Nutzdatenblock>
        </DatenTeil>
    </Elster>"#;

    const EXPECTED_XML: &str = r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <Elster xmlns="http://www.elster.de/elsterxml/schema/v11">
        <TransferHeader version="11">
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
            <VersionClient>1</VersionClient>
        </TransferHeader>
        <DatenTeil>
            <Nutzdatenblock>
                <NutzdatenHeader version="11">
                    <NutzdatenTicket>0001</NutzdatenTicket>
                    <Empfaenger id="F">9999</Empfaenger>
                    <Hersteller>
                        <ProduktName>Taxel</ProduktName>
                        <ProduktVersion>0.2.0</ProduktVersion>
                    </Hersteller>
                </NutzdatenHeader>
                <Nutzdaten>
                    <ebilanz:EBilanz xmlns:ebilanz="http://rzf.fin-nrw.de/RMS/EBilanz/2016/XMLSchema"
                        version="000001">
                        <ebilanz:stichtag>20201231</ebilanz:stichtag>
                    </ebilanz:EBilanz>
                </Nutzdaten>
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

    // Helper function to test updated tags
    fn test_update_target_tags(actual_xml: &str, expected_xml: &str, target_tags: TargetTags) {
        let mut reader = Reader::from_str(actual_xml);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        update_target_tags(target_tags, &mut reader, &mut writer).unwrap();

        let actual: Vec<u8> = writer.into_inner().into_inner();
        let expected = remove_formatting(expected_xml).unwrap();

        assert_eq!(String::from_utf8(actual).unwrap(), expected);
    }

    #[test]
    fn test_update_xml() {
        let mut target_tags = TargetTags::new();
        target_tags.insert("HerstellerID", Some("99999"));
        target_tags.insert("Empfaenger", Some("9999"));
        target_tags.insert("ProduktName", Some("Taxel"));
        target_tags.insert("ProduktVersion", Some("0.2.0"));
        target_tags.insert("ebilanz:stichtag", Some("20201231"));

        test_update_target_tags(ACTUAL_XML, EXPECTED_XML, target_tags);
    }

    #[test]
    fn test_update_xrbl() {
        let actual_xbrl = r#"
            <xbrli:xbrl xmlns:de-gaap-ci="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01"
                xmlns:de-gcd="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01"
                xmlns:hgbref="http://www.xbrl.de/2008/ref" xmlns:iso4217="http://www.xbrl.org/2003/iso4217"
                xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:ref="http://www.xbrl.org/2024/ref"
                xmlns:xbrldi="http://xbrl.org/2006/xbrldi" xmlns:xbrli="http://www.xbrl.org/2003/instance"
                xmlns:xhtml="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink"
                xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
                <link:schemaRef
                    xlink:href="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd"
                    xlink:type="simple" />
                <link:schemaRef
                    xlink:href="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal-microbilg.xsd"
                    xlink:type="simple" />
                <xbrli:context id="I-2020">
                    <xbrli:entity>
                        <xbrli:identifier
                            scheme="http://www.rzf-nrw.de/Steuernummer">0000000000000</xbrli:identifier>
                    </xbrli:entity>
                    <xbrli:period>
                        <xbrli:instant>0000-00-00</xbrli:instant>
                    </xbrli:period>
                </xbrli:context>
            </xbrli:xbrl>"#;

        let expected_xbrl = r#"
            <xbrli:xbrl xmlns:de-gaap-ci="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01"
                xmlns:de-gcd="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01"
                xmlns:hgbref="http://www.xbrl.de/2008/ref" xmlns:iso4217="http://www.xbrl.org/2003/iso4217"
                xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:ref="http://www.xbrl.org/2024/ref"
                xmlns:xbrldi="http://xbrl.org/2006/xbrldi" xmlns:xbrli="http://www.xbrl.org/2003/instance"
                xmlns:xhtml="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink"
                xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
                <link:schemaRef
                    xlink:href="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd"
                    xlink:type="simple" />
                <link:schemaRef
                    xlink:href="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal-microbilg.xsd"
                    xlink:type="simple" />
                <xbrli:context id="I-2020">
                    <xbrli:entity>
                        <xbrli:identifier
                            scheme="http://www.rzf-nrw.de/Steuernummer">9999999999999</xbrli:identifier>
                    </xbrli:entity>
                    <xbrli:period>
                        <xbrli:instant>2020-12-31</xbrli:instant>
                    </xbrli:period>
                </xbrli:context>
            </xbrli:xbrl>"#;

        let mut target_tags = TargetTags::new();
        target_tags.insert("ProduktName", Some("Taxel"));
        target_tags.insert("xbrli:identifier", Some("9999999999999"));
        target_tags.insert("xbrli:instant", Some("2020-12-31"));

        test_update_target_tags(actual_xbrl, expected_xbrl, target_tags);
    }
}
