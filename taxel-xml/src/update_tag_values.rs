use crate::{Tag, TargetTags, Taxonomy, XmlMode, DECIMALS_2, NIL_ATTRIBUTE, XBRL_ATTRIBUTE};
use quick_xml::{
    events::{attributes::Attribute, BytesStart, BytesText, Event},
    Reader, Writer,
};
use std::{io::BufRead, str};

/// Update values for given tags in an xml file.
pub fn update_tag_values<R, W>(
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
    let mut mode = XmlMode::Plain;

    // Process each event in the xml file
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(s_tag)) => {
                if s_tag.name().as_ref() == XBRL_ATTRIBUTE.as_bytes() {
                    mode = XmlMode::Xbrl;
                }

                let qualified_name = s_tag.name();
                let tag_name = str::from_utf8(qualified_name.as_ref())?;
                let tag = if mode == XmlMode::Xbrl
                    && tag_name.contains(Taxonomy::GaapCi.to_string().as_str())
                {
                    update_attributes(&s_tag)?
                } else if mode == XmlMode::Plain
                    && tag_name.contains(Taxonomy::Gcd.to_string().as_str())
                {
                    s_tag.to_owned()
                } else {
                    s_tag.to_owned()
                };

                if let Some(tag_value) = target_tags.get(tag_name) {
                    // Found the start of target tag.
                    target_tag = Some(Tag::new(tag_name, tag_value.to_owned()));
                }

                writer.write_event(Event::Start(tag.clone()))?;
            }
            Ok(Event::End(e_tag)) => {
                if e_tag.name().as_ref() == XBRL_ATTRIBUTE.as_bytes() {
                    mode = XmlMode::Xbrl;
                }

                if target_tag
                    .as_ref()
                    .is_some_and(|tag| tag.name.as_bytes() == e_tag.name().as_ref())
                {
                    // Found the end tag for the target tag.
                    target_tag = None;
                }

                writer.write_event(Event::End(e_tag))?;
            }
            Ok(Event::Empty(em_tag)) => {
                let qualified_name = em_tag.name();
                let tag_name = str::from_utf8(qualified_name.as_ref())?;
                let tag = if mode == XmlMode::Xbrl
                    && tag_name.contains(Taxonomy::GaapCi.to_string().as_str())
                {
                    update_attributes(&em_tag)?
                } else if mode == XmlMode::Plain
                    && tag_name.contains(Taxonomy::Gcd.to_string().as_str())
                {
                    em_tag.to_owned()
                } else {
                    em_tag.to_owned()
                };

                if let Some(tag_value) = target_tags.get(tag_name) {
                    let text = if let Some(tag_value) = tag_value {
                        BytesText::new(tag_value)
                    } else {
                        BytesText::new("")
                    };

                    writer.write_event(Event::Start(tag.clone()))?;
                    writer.write_event(Event::Text(text))?;
                    writer.write_event(Event::End(tag.to_end()))?;
                } else {
                    // Write the text content to the output xml file.
                    writer.write_event(Event::Empty(em_tag))?;
                }
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
            Ok(Event::Decl(tag)) => {
                writer.write_event(Event::Decl(tag))?;
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

/// Update the attributes of a given tag.
fn update_attributes<'a>(tag: &BytesStart<'a>) -> Result<BytesStart<'a>, anyhow::Error> {
    let attributes = tag.attributes();

    let mut updated_attributes = vec![];

    for attribute in attributes {
        let attribute = attribute?;

        if attribute.key.as_ref() != NIL_ATTRIBUTE.key.as_bytes() {
            updated_attributes.push(attribute);
        }
    }

    updated_attributes.push(Attribute::from((
        DECIMALS_2.key.as_bytes(),
        DECIMALS_2.value.as_bytes(),
    )));

    let mut updated_tag = tag.clone();
    updated_tag.clear_attributes();
    let updated_tag = updated_tag.with_attributes(updated_attributes);

    Ok(updated_tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::remove_formatting;
    use std::io::Cursor;

    #[test]
    fn test_update_tag_values_start_end() {
        let xml = r#"
            <root>
                <tag/>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag>value</tag>
            </root>
        "#;

        let mut target_tags = TargetTags::new();
        target_tags.insert("tag", Some("value"));
        test_update_target_tags(xml, expected_xml, target_tags);
    }

    #[test]
    fn test_update_tag_values_start_end_empty() {
        let xml = r#"
            <root>
                <tag/>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag></tag>
            </root>
        "#;

        let mut target_tags = TargetTags::new();
        target_tags.insert("tag", Some(""));
        test_update_target_tags(xml, expected_xml, target_tags);
    }

    #[test]
    fn test_update_tag_values_empty() {
        let xml = r#"
            <root>
                <tag>value</tag>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag>updated value</tag>
            </root>
        "#;

        let mut target_tags = TargetTags::new();
        target_tags.insert("tag", Some("updated value"));
        test_update_target_tags(xml, expected_xml, target_tags);
    }

    #[test]
    fn test_update_tag_values_multiple_tags() {
        let xml = r#"
            <root>
                <tag1/>
                <tag2/>
                <tag3>
                    <tag31/>
                </tag3>
                <tag4/>
                <tag5/>
                <tag6/>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag1/>
                <tag2>value 2</tag2>
                <tag3>
                    <tag31/>
                </tag3>
                <tag4/>
                <tag5>value 5</tag5>
                <tag6/>
            </root>
        "#;

        let mut target_tags = TargetTags::new();
        target_tags.insert("tag2", Some("value 2"));
        target_tags.insert("tag5", Some("value 5"));
        test_update_target_tags(xml, expected_xml, target_tags);
    }

    #[test]
    fn test_update_xbrl_tag_gcd() {
        let xml = r#"
            <xbrli:xbrl>
                <de-gcd:genInfo.report.audit.city contextRef="D-AKTJAHR"/>
            </xbrli:xbrl>
        "#;
        let expected_xml = r#"
            <xbrli:xbrl>
                <de-gcd:genInfo.report.audit.city contextRef="D-AKTJAHR">Berlin</de-gcd:genInfo.report.audit.city>
            </xbrli:xbrl>
        "#;

        let mut target_tags = TargetTags::new();
        target_tags.insert("de-gcd:genInfo.report.audit.city", Some("Berlin"));
        test_update_target_tags(xml, expected_xml, target_tags);
    }

    #[test]
    fn test_update_xbrl_tag_gaap() {
        let xml = r#"
            <xbrli:xbrl>
                <de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing contextRef="D-AKTJAHR" unitRef="EUR" xsi:nil="true"/>
            </xbrli:xbrl>
        "#;
        let expected_xml = r#"
            <xbrli:xbrl>
                <de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing contextRef="D-AKTJAHR" unitRef="EUR" decimals="2">550.50</de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing>
            </xbrli:xbrl>
        "#;

        let mut target_tags = TargetTags::new();
        target_tags.insert(
            "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
            Some("550.50"),
        );
        test_update_target_tags(xml, expected_xml, target_tags);
    }

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

    // Helper function to test updated tags
    fn test_update_target_tags(xml: &str, expected_xml: &str, target_tags: TargetTags) {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        update_tag_values(target_tags, &mut reader, &mut writer).unwrap();

        let actual = writer.into_inner().into_inner();
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
