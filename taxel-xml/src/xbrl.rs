use crate::Taxonomy;
use anyhow::anyhow;
use quick_xml::{
    events::{attributes::Attributes, BytesStart, Event},
    Reader,
};
use std::io::BufRead;
use std::str;

/// A simple tree structure to store the xml file.
#[derive(Debug, PartialEq)]
struct XbrlElement {
    name: String,
    value: Option<String>,
    attributes: Vec<Attribute>,
    xml_type: XmlType,
    children: Vec<XbrlElement>,
}

#[derive(Debug, PartialEq)]
enum XmlType {
    Plain,
    Xbrl,
    Taxonomy(Taxonomy),
}

impl XmlType {
    fn as_str(&self) -> &str {
        match self {
            XmlType::Plain => "Plain",
            XmlType::Xbrl => "Xbrl",
            XmlType::Taxonomy(Taxonomy::Gcd) => "gcd",
            XmlType::Taxonomy(Taxonomy::GaapCi) => "gaap-ci",
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct Attribute {
    key: String,
    value: String,
}

impl Attribute {
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
        attributes: Vec<Attribute>,
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
        }

        let root_element = root_element.ok_or(anyhow!("Missing root element"))?;

        Ok(root_element)
    }

    /// Deserialize `XbrlElement`s recursively.
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
        }
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

    fn convert_attributes(xml_attributes: Attributes) -> Result<Vec<Attribute>, anyhow::Error> {
        let mut attributes = vec![];

        for attribute in xml_attributes {
            let attribute = attribute?;
            let attribute = Attribute::new(
                str::from_utf8(attribute.key.as_ref())?,
                str::from_utf8(attribute.value.as_ref())?,
            );
            attributes.push(attribute);
        }

        Ok(attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                        vec![Attribute::new("attribute_key", "attribute value")],
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
}
