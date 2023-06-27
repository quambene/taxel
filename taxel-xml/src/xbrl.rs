use quick_xml::{
    events::{BytesEnd, BytesStart, Event},
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
    let mut start_tag: Option<BytesStart<'_>> = None;
    let mut end_tag: Option<BytesEnd<'_>> = None;

    // Iterate over each XML event
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref tag)) => {
                if let Some(ref tag) = start_tag {
                    writer.write_event(Event::Start(tag.clone()))?;
                }

                start_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::End(ref tag)) => {
                if let Some(_) = end_tag {
                    writer.write_event(Event::End(tag.clone()))?;
                }

                end_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::Text(_)) => {
                if let Some(ref tag) = start_tag {
                    writer.write_event(Event::Empty(tag.clone()))?;
                    start_tag = None;
                    end_tag = None;
                }
            }
            // Empty tags don't need to be adjusted.
            Ok(Event::Empty(tag)) => {
                writer.write_event(Event::Empty(tag))?;
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

        if let Some(_) = end_tag {
            if let Some(ref tag) = start_tag {
                writer.write_event(Event::Empty(tag.clone()))?;
                start_tag = None;
                end_tag = None;
            }
        }

        buf.clear();
    }

    Ok(())
}

fn add_tag_value() {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::remove_formatting;
    use std::io::Cursor;

    #[test]
    fn test_remove_tag_values() {
        let actual_xml = r#"
            <root>
                <de-gaap-ci:tag1>Value 1</de-gaap-ci:tag1>
                <de-gaap-ci:tag2></de-gaap-ci:tag2>
                <de-gaap-ci:tag3>Value 3</de-gaap-ci:tag3>
                <de-gaap-ci:tag4/>
            </root>
        "#;
        let expected_xml = r#"
            <root>
                <de-gaap-ci:tag1/>
                <de-gaap-ci:tag2/>
                <de-gaap-ci:tag3/>
                <de-gaap-ci:tag4/>
            </root>
        "#;

        let mut reader = Reader::from_str(actual_xml);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        remove_tag_values(&mut reader, &mut writer).unwrap();

        let actual: Vec<u8> = writer.into_inner().into_inner();
        let expected = remove_formatting(expected_xml).unwrap();

        assert_eq!(String::from_utf8(actual).unwrap(), expected);
    }
}
