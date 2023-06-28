use quick_xml::{
    events::{BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};
use std::io::BufRead;

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

    // Iterate over each XML event
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref tag)) => {
                // Write start tag for tags of type `<tag1><tag2>`
                if let Some(ref tag) = start_tag {
                    writer.write_event(Event::Start(tag.clone()))?;
                }

                start_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::End(ref tag)) => {
                end_tag = Some(tag.clone().into_owned());
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

        remove_tag_value(writer, &mut start_tag, &mut end_tag, &mut tag_value)?;

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

/// Remove tag value
fn remove_tag_value<W>(
    writer: &mut Writer<W>,
    start_tag: &mut Option<BytesStart>,
    end_tag: &mut Option<BytesEnd>,
    tag_value: &mut Option<BytesText>,
) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    // TODO: validate tag name

    match (&start_tag, &tag_value, &end_tag) {
        // Handle tags of type `<tag>value</tag>`
        (Some(tag), Some(_), Some(_)) => {
            writer.write_event(Event::Empty(tag.clone()))?;
            *start_tag = None;
            *tag_value = None;
            *end_tag = None;
        }
        // Handle tags of type `<tag></tag>`
        (Some(tag), None, Some(_)) => {
            writer.write_event(Event::Empty(tag.clone()))?;
            *start_tag = None;
            *end_tag = None;
        }
        _ => (),
    }

    Ok(())
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
}
