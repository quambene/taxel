use crate::{Taxonomy, XmlMode, DECIMAL_ATTRIBUTE, NIL_ATTRIBUTE, XBRL_ATTRIBUTE};
use quick_xml::{
    events::{attributes::Attribute, BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};
use std::{io::BufRead, str};

/// Remove values for all tags in an xml file.
fn remove_tag_values<R, W>(
    reader: &mut Reader<R>,
    writer: &mut Writer<W>,
) -> Result<(), anyhow::Error>
where
    R: std::io::Read + BufRead,
    W: std::io::Write,
{
    let mut buf = Vec::new();
    let mut start_tag: Option<BytesStart> = None;
    let mut end_tag: Option<BytesEnd> = None;
    let mut empty_tag: Option<BytesStart> = None;
    let mut tag_value: Option<BytesText> = None;
    let mut mode = XmlMode::Plain;

    // Iterate over each XML event
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref tag)) => {
                // Write start tag for tags of type `<tag1><tag2>`
                if let Some(ref tag) = start_tag {
                    writer.write_event(Event::Start(tag.clone()))?;
                }

                if tag.name().as_ref() == XBRL_ATTRIBUTE.as_bytes() {
                    mode = XmlMode::Xbrl;
                }

                start_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::End(ref tag)) => {
                end_tag = Some(tag.clone().into_owned());

                if tag.name().as_ref() == XBRL_ATTRIBUTE.as_bytes() {
                    mode = XmlMode::Plain;
                }
            }
            Ok(Event::Text(ref tag)) => {
                tag_value = Some(tag.clone().into_owned());
            }
            Ok(Event::Empty(ref tag)) => {
                empty_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::Decl(tag)) => {
                writer.write_event(Event::Decl(tag))?;
            }
            Ok(Event::Eof) => {
                // Reached the end of the xml file.
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => (),
        }

        remove_tag_value(writer, &mode, &mut start_tag, &mut end_tag, &mut tag_value)?;

        if let Some(ref tag) = empty_tag {
            // Write start tag for tags of type `<tag1><tag2/>`
            if let Some(ref tag) = start_tag {
                writer.write_event(Event::Start(tag.clone()))?;
                start_tag = None;
            }

            writer.write_event(Event::Empty(tag.clone()))?;
            empty_tag = None;
        }

        if let Some(ref tag) = end_tag {
            writer.write_event(Event::End(tag.clone()))?;
            end_tag = None;
        }

        buf.clear();
    }

    Ok(())
}

/// Remove value for xml or xbrl tag
fn remove_tag_value<W>(
    writer: &mut Writer<W>,
    mode: &XmlMode,
    start_tag: &mut Option<BytesStart>,
    end_tag: &mut Option<BytesEnd>,
    tag_value: &mut Option<BytesText>,
) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    match mode {
        XmlMode::Plain => {
            remove_xml_tag_value(writer, start_tag, end_tag, tag_value)?;
        }
        XmlMode::Xbrl => {
            remove_xbrl_tag_value(writer, start_tag, end_tag, tag_value)?;
        }
    }

    Ok(())
}

/// Remove value for xml tag
fn remove_xml_tag_value<W>(
    writer: &mut Writer<W>,
    start_tag: &mut Option<BytesStart>,
    end_tag: &mut Option<BytesEnd>,
    tag_value: &mut Option<BytesText>,
) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    match (&start_tag, &tag_value, &end_tag) {
        // Handle tags of type `<tag>value</tag>`
        (Some(s_tag), Some(_), Some(e_tag)) => {
            if s_tag.name() == e_tag.name() {
                writer.write_event(Event::Empty(s_tag.clone()))?;
                *start_tag = None;
                *tag_value = None;
                *end_tag = None;
            }
        }
        // Handle tags of type `<tag></tag>`
        (Some(s_tag), None, Some(e_tag)) => {
            if s_tag.name() == e_tag.name() {
                writer.write_event(Event::Empty(s_tag.clone()))?;
                *start_tag = None;
                *end_tag = None;
            }
        }
        _ => (),
    }

    Ok(())
}

/// Remove value for xbrl tag
fn remove_xbrl_tag_value<W>(
    writer: &mut Writer<W>,
    start_tag: &mut Option<BytesStart>,
    end_tag: &mut Option<BytesEnd>,
    tag_value: &mut Option<BytesText>,
) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    match (&start_tag, &tag_value, &end_tag) {
        // Handle tags of type `<tag>value</tag>`
        (Some(s_tag), Some(_), Some(e_tag)) => {
            if s_tag.name() == e_tag.name() {
                let qualified_name = s_tag.name();
                let tag_name = str::from_utf8(qualified_name.as_ref())?;
                let tag = if tag_name.contains(Taxonomy::Gcd.to_string().as_str()) {
                    s_tag.to_owned()
                } else {
                    update_attributes(s_tag)?
                };

                writer.write_event(Event::Empty(tag))?;
                *start_tag = None;
                *tag_value = None;
                *end_tag = None;
            }
        }
        // Handle tags of type `<tag></tag>`
        (Some(s_tag), None, Some(e_tag)) => {
            if s_tag.name() == e_tag.name() {
                let qualified_name = s_tag.name();
                let tag_name = str::from_utf8(qualified_name.as_ref())?;
                let tag = if tag_name.contains(Taxonomy::Gcd.to_string().as_str()) {
                    s_tag.to_owned()
                } else {
                    update_attributes(s_tag)?
                };

                writer.write_event(Event::Empty(tag))?;
                *start_tag = None;
                *end_tag = None;
            }
        }
        _ => (),
    }

    Ok(())
}

/// Update the attributes of a given tag.
fn update_attributes<'a>(tag: &BytesStart<'a>) -> Result<BytesStart<'a>, anyhow::Error> {
    let attributes = tag.attributes();

    let mut updated_attributes = vec![];

    for attribute in attributes {
        let attribute = attribute?;

        if attribute.key.as_ref() != DECIMAL_ATTRIBUTE.as_bytes() {
            updated_attributes.push(attribute);
        }
    }

    updated_attributes.push(Attribute::from((
        NIL_ATTRIBUTE.key.as_bytes(),
        NIL_ATTRIBUTE.value.as_bytes(),
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

    fn test_remove_tag_values(actual_xml: &str, expected_xml: &str) {
        let mut reader = Reader::from_str(actual_xml);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        remove_tag_values(&mut reader, &mut writer).unwrap();

        let actual: Vec<u8> = writer.into_inner().into_inner();
        let expected = remove_formatting(expected_xml).unwrap();

        assert_eq!(String::from_utf8(actual).unwrap(), expected);
    }

    #[test]
    fn test_remove_tag_values_start_end() {
        let actual_xml = r#"
            <root>
                <tag>value</tag>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag/>
            </root>
        "#;

        test_remove_tag_values(actual_xml, expected_xml);
    }

    #[test]
    fn test_remove_tag_values_start_end_empty() {
        let actual_xml = r#"
            <root>
                <tag></tag>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag/>
            </root>
        "#;

        test_remove_tag_values(actual_xml, expected_xml);
    }

    #[test]
    fn test_remove_tag_values_empty() {
        let actual_xml = r#"
            <root>
                <tag/>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <tag/>
            </root>
        "#;

        test_remove_tag_values(actual_xml, expected_xml);
    }

    #[test]
    fn test_remove_tag_values_multiple_tags() {
        let actual_xml = r#"
            <root>
                <tag1/>
                <tag2>Value 2</tag2>
                <tag3>
                    <tag31/>
                </tag3>
                <tag4/>
                <tag5>Value 5</tag5>
                <tag6/>
            </root>
        "#;
        let expected_xml = r#"
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

        test_remove_tag_values(actual_xml, expected_xml);
    }

    #[test]
    fn test_remove_xbrl_tag_gcd() {
        let actual_xml = r#"
            <xbrli:xbrl>
                <de-gcd:genInfo.report.audit.city contextRef="D-AKTJAHR">Berlin</de-gcd:genInfo.report.audit.city>
            </xbrli:xbrl>
        "#;
        let expected_xml = r#"
            <xbrli:xbrl>
                <de-gcd:genInfo.report.audit.city contextRef="D-AKTJAHR"/>
            </xbrli:xbrl>
        "#;

        test_remove_tag_values(actual_xml, expected_xml);
    }

    #[test]
    fn test_remove_xbrl_tag_gaap() {
        let actual_xml = r#"
            <xbrli:xbrl>
                <de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing contextRef="D-AKTJAHR" unitRef="EUR" decimals="2">550.50</de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing>
            </xbrli:xbrl>
        "#;
        let expected_xml = r#"
            <xbrli:xbrl>
                <de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing contextRef="D-AKTJAHR" unitRef="EUR" xsi:nil="true"/>
            </xbrli:xbrl>
        "#;

        test_remove_tag_values(actual_xml, expected_xml);
    }
}
