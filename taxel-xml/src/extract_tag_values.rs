use crate::Tag;
use ::std::str;
use log::{debug, error, info};
use quick_xml::{events::Event, Reader};
use std::io::BufRead;

/// Extract tag values from an xml file.
pub fn extract_tag_values<R>(reader: &mut Reader<R>) -> Result<Vec<Tag>, anyhow::Error>
where
    R: std::io::Read + BufRead,
{
    info!("Extract tag values");

    let mut buf = Vec::new();
    let mut extracted_tags = Vec::new();
    let mut start_tag = None;
    let mut tag_value = None;
    let mut end_tag = None;

    // Process each event in the xml file
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) => {
                start_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::End(tag)) => {
                end_tag = Some(tag.clone().into_owned());
            }
            Ok(Event::Empty(_)) => continue,
            Ok(Event::Text(tag)) => {
                tag_value = Some(tag.clone().into_owned());
            }
            Ok(Event::Decl(_)) => continue,
            Ok(Event::Eof) => {
                // Reached the end of the xml file.
                break;
            }
            Err(err) => {
                // Handle error while reading the xml file.
                error!("Can't parse xml file: {err}");
                break;
            }
            _ => (),
        }

        if let (Some(s_tag), Some(value), Some(e_tag)) = (&start_tag, &tag_value, &end_tag) {
            let start_tag_name = s_tag.name();
            let end_tag_name = e_tag.name();

            if start_tag_name == end_tag_name {
                let tag_name = str::from_utf8(start_tag_name.as_ref())?;
                let value = str::from_utf8(value.as_ref())?;
                let extracted_tag = Tag::new(tag_name, Some(value));
                extracted_tags.push(extracted_tag);

                // Reset state
                (start_tag, tag_value, end_tag) = (None, None, None);
            }
        }

        buf.clear();
    }

    debug!("Extracted tag values: {extracted_tags:#?}");

    Ok(extracted_tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_extract_tag_values(xml: &str, expected_tags: Vec<Tag>) {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let actual_tags = extract_tag_values(&mut reader).unwrap();

        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn test_extract_tag_values_single() {
        let xml = r#"
            <root>
                <tag>value</tag>
            </root>
        "#;
        let expected_tags = vec![Tag::new("tag", Some("value"))];

        test_extract_tag_values(xml, expected_tags);
    }

    #[test]
    fn test_extract_tag_values_multiple() {
        let xml = r#"
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
        let expected_tags = vec![
            Tag::new("tag2", Some("value 2")),
            Tag::new("tag5", Some("value 5")),
        ];

        test_extract_tag_values(xml, expected_tags);
    }

    #[test]
    fn test_extract_tag_values_xbrl_gcd() {
        let xml = r#"
            <xbrli:xbrl>
                <de-gcd:genInfo.report.audit.city contextRef="D-AKTJAHR">Berlin</de-gcd:genInfo.report.audit.city>
            </xbrli:xbrl>
        "#;
        let expected_tags = vec![Tag::new("de-gcd:genInfo.report.audit.city", Some("Berlin"))];

        test_extract_tag_values(xml, expected_tags);
    }

    #[test]
    fn test_extract_tag_values_xbrl_gaap() {
        let xml = r#"
            <xbrli:xbrl>
                <de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing contextRef="D-AKTJAHR" unitRef="EUR" decimals="2">550.50</de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing>
            </xbrli:xbrl>
        "#;
        let expected_tags = vec![Tag::new(
            "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
            Some("550.50"),
        )];

        test_extract_tag_values(xml, expected_tags);
    }
}
