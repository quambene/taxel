use crate::{TargetTags, Taxonomy, DECIMALS_2, NIL_ATTRIBUTE};
use anyhow::anyhow;
use quick_xml::{
    events::{
        attributes::{Attribute, Attributes},
        BytesEnd, BytesStart, BytesText, Event,
    },
    Reader, Writer,
};
use std::io::BufRead;
use std::str;

/// A simple tree structure to store the xml file.
#[derive(Debug, PartialEq)]
pub struct XbrlElement {
    name: String,
    value: Option<String>,
    attributes: Vec<XbrlAttribute>,
    xml_type: XmlType,
    children: Vec<XbrlElement>,
}

#[derive(Debug, PartialEq)]
pub enum XmlType {
    Plain,
    Xbrl,
    Taxonomy(Taxonomy),
}

impl XmlType {
    fn as_str(&self) -> &str {
        match self {
            XmlType::Plain => "plain",
            XmlType::Xbrl => "xbrl",
            XmlType::Taxonomy(Taxonomy::Gcd) => "gcd",
            XmlType::Taxonomy(Taxonomy::GaapCi) => "gaap-ci",
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct XbrlAttribute {
    key: String,
    value: String,
}

impl XbrlAttribute {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

impl XbrlElement {
    /// Create a new XBRL element.
    pub fn new(
        name: impl Into<String>,
        value: Option<String>,
        attributes: Vec<XbrlAttribute>,
        xml_type: XmlType,
        children: Vec<XbrlElement>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.map(|value| value.into()),
            attributes,
            xml_type,
            children,
        }
    }

    /// Parse root element of xml file.
    pub fn parse<R>(reader: &mut Reader<R>) -> Result<XbrlElement, anyhow::Error>
    where
        R: std::io::Read + BufRead,
    {
        // TODO: Handle xml declaration

        let mut buf = Vec::new();
        let mut root_element = None;

        // Process each event in the xml file
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(tag)) => {
                    let current_element = Self::convert_tag(tag)?;
                    root_element = Some(Self::deserialize(reader, current_element)?);
                }
                Ok(Event::End(_)) => (),
                Ok(Event::Empty(_)) => (),
                Ok(Event::Text(_)) => (),
                Ok(Event::Decl(_)) => (),
                Ok(Event::Eof) => {
                    // Reached the end of the xml file.
                    break;
                }
                Err(err) => {
                    return Err(anyhow!("Can't parse xml file: {err}"));
                }
                _ => (),
            }

            buf.clear();
        }

        let root_element = root_element.ok_or(anyhow!("Missing root element"))?;

        Ok(root_element)
    }

    /// Deserialize `XbrlElement` recursively.
    fn deserialize<R>(
        reader: &mut Reader<R>,
        mut element: XbrlElement,
    ) -> Result<XbrlElement, anyhow::Error>
    where
        R: std::io::Read + BufRead,
    {
        let mut buf = Vec::new();

        // Process each event in the xml file
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(tag)) => {
                    let current_element = Self::convert_tag(tag)?;
                    element
                        .children
                        .push(Self::deserialize(reader, current_element)?);
                }
                Ok(Event::End(tag)) => {
                    let tag_name = tag.name();
                    let name = str::from_utf8(tag_name.as_ref())?;

                    if name == element.name {
                        return Ok(element);
                    } else {
                        return Err(anyhow!("Missing end tag: {name}"));
                    }
                }
                Ok(Event::Empty(tag)) => {
                    let current_element = Self::convert_tag(tag)?;
                    element.children.push(current_element);
                }
                Ok(Event::Text(tag)) => {
                    let inner_value = tag.into_inner();
                    let tag_value = str::from_utf8(inner_value.as_ref())?;
                    element.value = Some(tag_value.to_owned());
                }
                Ok(Event::Decl(_)) => {
                    return Err(anyhow!("Unexpected xml declaration"));
                }
                Ok(Event::Eof) => {
                    return Err(anyhow!("Unexpected end of xml document"));
                }
                Err(err) => {
                    return Err(anyhow!("Can't parse xml file: {err}"));
                }
                _ => (),
            }

            buf.clear();
        }
    }

    /// Serialize `XbrlElement` recursively.
    pub fn serialize<W>(&self, writer: &mut Writer<W>) -> Result<(), anyhow::Error>
    where
        W: std::io::Write,
    {
        let mut attributes = vec![];
        for attribute in &self.attributes {
            let attribute = Attribute::from((attribute.key.as_bytes(), attribute.value.as_bytes()));
            attributes.push(attribute);
        }
        let start_tag = BytesStart::new(&self.name).with_attributes(attributes);
        let end_tag = BytesEnd::new(&self.name);

        match &self.value {
            Some(value) => {
                let tag_value = BytesText::new(value);

                writer.write_event(Event::Start(start_tag))?;
                writer.write_event(Event::Text(tag_value))?;
                writer.write_event(Event::End(end_tag))?;
            }
            None => {
                if self.children.is_empty() {
                    writer.write_event(Event::Empty(start_tag))?;
                } else {
                    writer.write_event(Event::Start(start_tag))?;

                    for child in &self.children {
                        child.serialize(writer)?;
                    }

                    writer.write_event(Event::End(end_tag))?;
                }
            }
        }

        Ok(())
    }

    fn convert_tag(tag: BytesStart) -> Result<XbrlElement, anyhow::Error> {
        let tag_name = tag.name();
        let name = str::from_utf8(tag_name.as_ref())?;
        let value = None;
        let attributes = Self::convert_attributes(tag.attributes())?;
        let xml_type = Self::get_xml_type(name);
        let children = vec![];

        Ok(XbrlElement::new(
            name, value, attributes, xml_type, children,
        ))
    }

    fn get_xml_type(name: &str) -> XmlType {
        if name.contains(XmlType::Xbrl.as_str()) {
            XmlType::Xbrl
        } else if name.contains(Taxonomy::Gcd.as_str()) {
            XmlType::Taxonomy(Taxonomy::Gcd)
        } else if name.contains(Taxonomy::GaapCi.as_str()) {
            XmlType::Taxonomy(Taxonomy::GaapCi)
        } else {
            XmlType::Plain
        }
    }

    fn convert_attributes(xml_attributes: Attributes) -> Result<Vec<XbrlAttribute>, anyhow::Error> {
        let mut attributes = vec![];

        for attribute in xml_attributes {
            let attribute = attribute?;
            let attribute = XbrlAttribute::new(
                str::from_utf8(attribute.key.as_ref())?,
                str::from_utf8(attribute.value.as_ref())?,
            );
            attributes.push(attribute);
        }

        Ok(attributes)
    }

    /// Remove all values from `XbrlElement` recursively.
    pub fn remove_values(&mut self) {
        self.value = None;

        if self.xml_type == XmlType::Taxonomy(Taxonomy::GaapCi) {
            // Remove `decimals` attribute
            if let Some(index) = self
                .attributes
                .iter()
                .position(|attribute| attribute.key == DECIMALS_2.key)
            {
                self.attributes.remove(index);
            }

            // Add `xsi:nil` attribute
            self.attributes
                .push(XbrlAttribute::new(NIL_ATTRIBUTE.key, NIL_ATTRIBUTE.value))
        }

        for child in &mut self.children {
            child.remove_values();
        }
    }

    /// Add given values to `XbrlElement` recursively.
    pub fn add_values(&mut self, target_tags: &TargetTags) {
        if let Some(value) = target_tags.get(&self.name) {
            self.value = value.to_owned();

            if self.xml_type == XmlType::Taxonomy(Taxonomy::GaapCi) {
                // Remove `xsi:nil` attribute
                if let Some(index) = self
                    .attributes
                    .iter()
                    .position(|attribute| attribute.key == NIL_ATTRIBUTE.key)
                {
                    self.attributes.remove(index);
                }

                // Add `decimals` attribute
                self.attributes
                    .push(XbrlAttribute::new(DECIMALS_2.key, DECIMALS_2.value))
            }
        }

        for child in &mut self.children {
            child.add_values(target_tags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_add_values() {
        let mut element = XbrlElement::new(
            "root",
            None,
            vec![],
            XmlType::Plain,
            vec![XbrlElement::new(
                "tag",
                Some(String::from("value")),
                vec![],
                XmlType::Plain,
                vec![],
            )],
        );
        let mut target_tags = TargetTags::new();
        target_tags.insert("tag", Some("updated value"));

        element.add_values(&target_tags);

        assert_eq!(
            element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![XbrlElement::new(
                    "tag",
                    Some(String::from("updated value")),
                    vec![],
                    XmlType::Plain,
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_add_values_empy() {
        let mut element = XbrlElement::new(
            "root",
            None,
            vec![],
            XmlType::Plain,
            vec![XbrlElement::new(
                "tag",
                None,
                vec![],
                XmlType::Plain,
                vec![],
            )],
        );
        let mut target_tags = TargetTags::new();
        target_tags.insert("tag", Some("updated value"));

        element.add_values(&target_tags);

        assert_eq!(
            element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![XbrlElement::new(
                    "tag",
                    Some(String::from("updated value")),
                    vec![],
                    XmlType::Plain,
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_add_values_xbrl_gcd() {
        let mut element = XbrlElement::new(
            "xbrli:xbrl",
            None,
            vec![],
            XmlType::Xbrl,
            vec![XbrlElement::new(
                "de-gcd:genInfo.report.audit.city",
                None,
                vec![XbrlAttribute::new("contextRef", "D-AKTJAHR")],
                XmlType::Taxonomy(Taxonomy::Gcd),
                vec![],
            )],
        );
        let mut target_tags = TargetTags::new();
        target_tags.insert("de-gcd:genInfo.report.audit.city", Some("Berlin"));

        element.add_values(&target_tags);

        assert_eq!(
            element,
            XbrlElement::new(
                "xbrli:xbrl",
                None,
                vec![],
                XmlType::Xbrl,
                vec![XbrlElement::new(
                    "de-gcd:genInfo.report.audit.city",
                    Some(String::from("Berlin")),
                    vec![XbrlAttribute::new("contextRef", "D-AKTJAHR")],
                    XmlType::Taxonomy(Taxonomy::Gcd),
                    vec![],
                )]
            )
        );
    }

    #[test]
    fn test_add_values_xbrl_gaap() {
        let mut element = XbrlElement::new(
            "xbrli:xbrl",
            None,
            vec![],
            XmlType::Xbrl,
            vec![XbrlElement::new(
                "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
                None,
                vec![
                    XbrlAttribute::new("contextRef", "D-AKTJAHR"),
                    XbrlAttribute::new("unitRef", "EUR"),
                    XbrlAttribute::new("xsi:nil", "true"),
                ],
                XmlType::Taxonomy(Taxonomy::GaapCi),
                vec![],
            )],
        );
        let mut target_tags = TargetTags::new();
        target_tags.insert(
            "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
            Some("550.50"),
        );

        element.add_values(&target_tags);

        assert_eq!(
            element,
            XbrlElement::new(
                "xbrli:xbrl",
                None,
                vec![],
                XmlType::Xbrl,
                vec![XbrlElement::new(
                    "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
                    Some(String::from("550.50")),
                    vec![
                        XbrlAttribute::new("contextRef", "D-AKTJAHR"),
                        XbrlAttribute::new("unitRef", "EUR"),
                        XbrlAttribute::new("decimals", "2"),
                    ],
                    XmlType::Taxonomy(Taxonomy::GaapCi),
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_remove_values() {
        let mut element = XbrlElement::new(
            "root",
            None,
            vec![],
            XmlType::Plain,
            vec![XbrlElement::new(
                "tag",
                Some(String::from("value")),
                vec![],
                XmlType::Plain,
                vec![],
            )],
        );

        element.remove_values();

        assert_eq!(
            element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![XbrlElement::new(
                    "tag",
                    None,
                    vec![],
                    XmlType::Plain,
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_remove_values_xbrl_gcd() {
        let mut element = XbrlElement::new(
            "xbrli:xbrl",
            None,
            vec![],
            XmlType::Xbrl,
            vec![XbrlElement::new(
                "de-gcd:genInfo.report.audit.city",
                Some(String::from("Berlin")),
                vec![XbrlAttribute::new("contextRef", "D-AKTJAHR")],
                XmlType::Taxonomy(Taxonomy::Gcd),
                vec![],
            )],
        );

        element.remove_values();

        assert_eq!(
            element,
            XbrlElement::new(
                "xbrli:xbrl",
                None,
                vec![],
                XmlType::Xbrl,
                vec![XbrlElement::new(
                    "de-gcd:genInfo.report.audit.city",
                    None,
                    vec![XbrlAttribute::new("contextRef", "D-AKTJAHR")],
                    XmlType::Taxonomy(Taxonomy::Gcd),
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_remove_values_xbrl_gaap() {
        let mut element = XbrlElement::new(
            "xbrli:xbrl",
            None,
            vec![],
            XmlType::Xbrl,
            vec![XbrlElement::new(
                "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
                Some(String::from("550.50")),
                vec![
                    XbrlAttribute::new("contextRef", "D-AKTJAHR"),
                    XbrlAttribute::new("unitRef", "EUR"),
                    XbrlAttribute::new("decimals", "2"),
                ],
                XmlType::Taxonomy(Taxonomy::GaapCi),
                vec![],
            )],
        );

        element.remove_values();

        assert_eq!(
            element,
            XbrlElement::new(
                "xbrli:xbrl",
                None,
                vec![],
                XmlType::Xbrl,
                vec![XbrlElement::new(
                    "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
                    None,
                    vec![
                        XbrlAttribute::new("contextRef", "D-AKTJAHR"),
                        XbrlAttribute::new("unitRef", "EUR"),
                        XbrlAttribute::new("xsi:nil", "true"),
                    ],
                    XmlType::Taxonomy(Taxonomy::GaapCi),
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_serialize_element_empty() {
        let element = XbrlElement::new(
            "root",
            None,
            vec![],
            XmlType::Plain,
            vec![XbrlElement::new(
                "tag",
                None,
                vec![],
                XmlType::Plain,
                vec![],
            )],
        );
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        element.serialize(&mut writer).unwrap();
        let xml = writer.into_inner().into_inner();
        let xml = str::from_utf8(&xml).unwrap();

        assert_eq!(xml, r#"<root><tag/></root>"#);
    }

    #[test]
    fn test_serialize_element_value() {
        let element = XbrlElement::new(
            "root",
            None,
            vec![],
            XmlType::Plain,
            vec![XbrlElement::new(
                "tag",
                Some(String::from("value")),
                vec![],
                XmlType::Plain,
                vec![],
            )],
        );
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        element.serialize(&mut writer).unwrap();
        let xml = writer.into_inner().into_inner();
        let xml = str::from_utf8(&xml).unwrap();

        assert_eq!(xml, r#"<root><tag>value</tag></root>"#);
    }

    #[test]
    fn test_serialize_element_multiple() {
        let element = XbrlElement::new(
            "root",
            None,
            vec![],
            XmlType::Plain,
            vec![
                XbrlElement::new(
                    "tag1",
                    None,
                    vec![XbrlAttribute::new("attribute_key", "attribute value")],
                    XmlType::Plain,
                    vec![XbrlElement::new(
                        "child",
                        Some("child value".to_owned()),
                        vec![],
                        XmlType::Plain,
                        vec![],
                    )],
                ),
                XbrlElement::new(
                    "tag2",
                    Some("tag value".to_owned()),
                    vec![],
                    XmlType::Plain,
                    vec![],
                ),
                XbrlElement::new("tag3", None, vec![], XmlType::Plain, vec![]),
            ],
        );
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        element.serialize(&mut writer).unwrap();
        let xml = writer.into_inner().into_inner();
        let xml = str::from_utf8(&xml).unwrap();

        assert_eq!(
            xml,
            r#"<root><tag1 attribute_key="attribute value"><child>child value</child></tag1><tag2>tag value</tag2><tag3/></root>"#
        );
    }

    #[test]
    fn test_parse_element_empty() {
        let xml = r#"
            <root>
                <tag/>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let parsed_element = XbrlElement::parse(&mut reader).unwrap();

        assert_eq!(
            parsed_element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![XbrlElement::new(
                    "tag",
                    None,
                    vec![],
                    XmlType::Plain,
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_parse_element_start_end() {
        let xml = r#"
            <root>
                <tag></tag>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let parsed_element = XbrlElement::parse(&mut reader).unwrap();

        assert_eq!(
            parsed_element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![XbrlElement::new(
                    "tag",
                    None,
                    vec![],
                    XmlType::Plain,
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_parse_element_value() {
        let xml = r#"
            <root>
                <tag>value</tag>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let parsed_element = XbrlElement::parse(&mut reader).unwrap();

        assert_eq!(
            parsed_element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![XbrlElement::new(
                    "tag",
                    Some(String::from("value")),
                    vec![],
                    XmlType::Plain,
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_parse_element_multiple() {
        let xml = r#"
            <root>
                <tag1 attribute_key="attribute value">
                    <child>child value</child>
                </tag1>
                <tag2>tag value</tag2>
                <tag3></tag3>
                <tag4/>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let parsed_element = XbrlElement::parse(&mut reader).unwrap();

        assert_eq!(
            parsed_element,
            XbrlElement::new(
                "root",
                None,
                vec![],
                XmlType::Plain,
                vec![
                    XbrlElement::new(
                        "tag1",
                        None,
                        vec![XbrlAttribute::new("attribute_key", "attribute value")],
                        XmlType::Plain,
                        vec![XbrlElement::new(
                            "child",
                            Some("child value".to_owned()),
                            vec![],
                            XmlType::Plain,
                            vec![]
                        )]
                    ),
                    XbrlElement::new(
                        "tag2",
                        Some("tag value".to_owned()),
                        vec![],
                        XmlType::Plain,
                        vec![]
                    ),
                    XbrlElement::new("tag3", None, vec![], XmlType::Plain, vec![]),
                    XbrlElement::new("tag4", None, vec![], XmlType::Plain, vec![])
                ]
            ),
            "Pretty print: {:#?}",
            parsed_element
        );
    }

    #[test]
    fn test_parse_element_xbrl_gcd() {
        let xml = r#"
            <xbrli:xbrl>
                <de-gcd:genInfo.report.audit.city contextRef="D-AKTJAHR">Berlin</de-gcd:genInfo.report.audit.city>
            </xbrli:xbrl>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let parsed_element = XbrlElement::parse(&mut reader).unwrap();

        assert_eq!(
            parsed_element,
            XbrlElement::new(
                "xbrli:xbrl",
                None,
                vec![],
                XmlType::Xbrl,
                vec![XbrlElement::new(
                    "de-gcd:genInfo.report.audit.city",
                    Some(String::from("Berlin")),
                    vec![XbrlAttribute::new("contextRef", "D-AKTJAHR")],
                    XmlType::Taxonomy(Taxonomy::Gcd),
                    vec![]
                )]
            )
        );
    }

    #[test]
    fn test_parse_element_xbrl_gaap() {
        let xml = r#"
            <xbrli:xbrl>
                <de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing contextRef="D-AKTJAHR" unitRef="EUR" decimals="2">550.50</de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing>
            </xbrli:xbrl>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let parsed_element = XbrlElement::parse(&mut reader).unwrap();

        assert_eq!(
            parsed_element,
            XbrlElement::new(
                "xbrli:xbrl",
                None,
                vec![],
                XmlType::Xbrl,
                vec![XbrlElement::new(
                    "de-gaap-ci:is.netIncome.regular.operatingTC.otherCost.marketing",
                    Some(String::from("550.50")),
                    vec![
                        XbrlAttribute::new("contextRef", "D-AKTJAHR"),
                        XbrlAttribute::new("unitRef", "EUR"),
                        XbrlAttribute::new("decimals", "2")
                    ],
                    XmlType::Taxonomy(Taxonomy::GaapCi),
                    vec![]
                )]
            )
        );
    }
}
