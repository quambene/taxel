//! Generate XML file according to the XBRL standard from a given CSV file.

use crate::arg;
use clap::{Arg, ArgMatches};
use std::{
    env::current_dir,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use taxel_xml::{CsvReaderBuilder, Reader, Tags, Trim, Writer, XbrlElement};

pub fn generate_args() -> [Arg<'static>; 3] {
    [arg::csv_file(), arg::template_file(), arg::output_file()]
}

pub fn generate(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    let csv_file = arg::get_maybe_one(matches, arg::CSV_FILE);
    let template_file = arg::get_one(matches, arg::TEMPLATE_FILE)?;
    let output_file = arg::get_maybe_one(matches, arg::OUTPUT_FILE);
    let csv_path = csv_file.map(Path::new);
    let output_path = match output_file {
        Some(output_file) => PathBuf::from(output_file),
        None => current_dir()?,
    };

    debug!(
        "Run `taxel generate` with configuration:\n{}={:?}\n{}={}\n{}={:?}",
        arg::CSV_FILE,
        csv_file,
        arg::TEMPLATE_FILE,
        template_file,
        arg::OUTPUT_FILE,
        output_file,
    );

    // Read the csv file
    let mut csv_reader = match csv_path {
        Some(csv_path) => {
            let reader = CsvReaderBuilder::new()
                .delimiter(b',')
                .has_headers(true)
                .trim(Trim::All)
                .from_path(csv_path)?;

            Some(reader)
        }
        None => None,
    };

    // Read the structure from a template file
    let template_file = File::open(template_file)?;
    let reader = BufReader::new(template_file);

    // Create a reader for parsing the XML file
    let mut xml_reader = Reader::from_reader(reader);
    xml_reader.trim_text(true);

    // Create a new XML file as output
    let output_file = File::create(output_path)?;
    // Format XML file
    let mut xml_writer = Writer::new_with_indent(output_file, b' ', 4);

    let target_tags = taxel_xml::read_tags(csv_reader.as_mut())?;

    update_values(target_tags, &mut xml_reader, &mut xml_writer)?;

    // Flush the output XML writer and finalize the file
    xml_writer.into_inner().sync_all()?;

    Ok(())
}

/// Update values for xbrl tags.
pub fn update_values<R, W>(
    mut target_tags: Tags,
    xml_reader: &mut Reader<R>,
    xml_writer: &mut Writer<W>,
) -> Result<(), anyhow::Error>
where
    R: std::io::Read + BufRead,
    W: std::io::Write,
{
    target_tags.add_required_tags();
    target_tags.remove_unsupported_tags();
    let mut element = XbrlElement::parse(xml_reader)?;
    element.remove_values();
    element.add_values(&target_tags);
    taxel_xml::write_declaration(xml_writer)?;
    element.serialize(xml_writer)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use taxel_xml::{remove_formatting, Tags};

    // Helper function to test updated tags.
    fn test_update_target_tags(xml: &str, expected_xml: &str, target_tags: Tags) {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        update_values(target_tags, &mut reader, &mut writer).unwrap();

        let actual = writer.into_inner().into_inner();
        let expected = remove_formatting(expected_xml).unwrap();

        assert_eq!(String::from_utf8(actual).unwrap(), expected);
    }

    #[test]
    fn test_update_xml() {
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
                    <HerstellerID>00000</HerstellerID>
                    <Datei>
                        <Verschluesselung>CMSEncryptedData</Verschluesselung>
                        <Kompression>GZIP</Kompression>
                        <TransportSchluessel/>
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
                                <ProduktVersion>0.1.0</ProduktVersion>
                            </Hersteller>
                        </NutzdatenHeader>
                        <Nutzdaten>
                            <ebilanz:EBilanz xmlns:ebilanz="http://rzf.fin-nrw.de/RMS/EBilanz/2016/XMLSchema" version="000001">
                                <ebilanz:stichtag>20201231</ebilanz:stichtag>
                            </ebilanz:EBilanz>
                        </Nutzdaten>
                    </Nutzdatenblock>
                </DatenTeil>
            </Elster>"#;

        let mut target_tags = Tags::new();
        target_tags.insert("Testmerker", Some("700000004"));
        target_tags.insert("NutzdatenTicket", Some("0001"));
        target_tags.insert("Empfaenger", Some("9999"));
        target_tags.insert("ebilanz:stichtag", Some("20201231"));

        test_update_target_tags(ACTUAL_XML, EXPECTED_XML, target_tags);
    }

    #[test]
    fn test_update_xrbl() {
        let actual_xbrl = r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <xbrli:xbrl xmlns:de-gaap-ci="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01"
                xmlns:de-gcd="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01"
                xmlns:hgbref="http://www.xbrl.de/2008/ref" xmlns:iso4217="http://www.xbrl.org/2003/iso4217"
                xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:ref="http://www.xbrl.org/2024/ref"
                xmlns:xbrldi="http://xbrl.org/2006/xbrldi" xmlns:xbrli="http://www.xbrl.org/2003/instance"
                xmlns:xhtml="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink"
                xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
                <link:schemaRef
                    xlink:href="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd"
                    xlink:type="simple"/>
                <link:schemaRef
                    xlink:href="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal-microbilg.xsd"
                    xlink:type="simple"/>
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
            <?xml version="1.0" encoding="UTF-8"?>
            <xbrli:xbrl xmlns:de-gaap-ci="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01" xmlns:de-gcd="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01" xmlns:hgbref="http://www.xbrl.de/2008/ref" xmlns:iso4217="http://www.xbrl.org/2003/iso4217" xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:ref="http://www.xbrl.org/2024/ref" xmlns:xbrldi="http://xbrl.org/2006/xbrldi" xmlns:xbrli="http://www.xbrl.org/2003/instance" xmlns:xhtml="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
                <link:schemaRef xlink:href="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd" xlink:type="simple"/>
                <link:schemaRef xlink:href="http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal-microbilg.xsd" xlink:type="simple"/>
                <xbrli:context id="I-2020">
                    <xbrli:entity>
                        <xbrli:identifier scheme="http://www.rzf-nrw.de/Steuernummer">9999999999999</xbrli:identifier>
                    </xbrli:entity>
                    <xbrli:period>
                        <xbrli:instant>2020-12-31</xbrli:instant>
                    </xbrli:period>
                </xbrli:context>
            </xbrli:xbrl>"#;

        let mut target_tags = Tags::new();
        target_tags.insert("xbrli:identifier", Some("9999999999999"));
        target_tags.insert("xbrli:instant", Some("2020-12-31"));

        test_update_target_tags(actual_xbrl, expected_xbrl, target_tags);
    }
}
