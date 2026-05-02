use crate::ebilanz::EBilanz;
use anyhow::anyhow;
use quick_xml::{
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};
use std::{io::Cursor, str};

/// Marker value for the `Testmerker` field in the Elster transfer header,
/// indicating a non-production submission.
pub const TEST_MARKER: &str = "700000004";

/// Typed representation of an Elster eBilanz XML report, including the transfer
/// header and one or more payload blocks containing eBilanz data. The
/// `xbrli:xbrl` subtree within each payload block is captured verbatim as raw
/// bytes and not interpreted by this struct; it is the caller's responsibility
/// to parse it separately if needed.
#[derive(Debug)]
pub struct ElsterReport {
    /// The transfer header contains metadata about the report and the
    /// submitter.
    pub transfer_header: TransferHeader,
    /// The data section contains one or more payload blocks, each with its own
    /// header and eBilanz content.
    pub data_section: DataSection,
}

/// The transfer header of an Elster eBilanz report, containing metadata about
/// the report and the submitter. This corresponds to the `<TransferHeader>`
/// element in the Elster eBilanz XML schema.
#[derive(Debug)]
pub struct TransferHeader {
    /// Header schema version, e.g. `"11"`.
    pub version: String,
    /// The procedure for which the report is being submitted, e.g.
    /// `"ElsterBilanz"`.
    pub procedure: String,
    /// The type of data being submitted, e.g. `"Bilanz"`.
    pub data_type: String,
    /// The operation being performed, e.g. `"send-Auth"`.
    pub operation: String,
    /// An optional test marker for non-production submissions, e.g.
    /// `"700000004"`.
    pub test_marker: Option<String>,
    /// The 5-digit manufacturer ID assigned by the tax authority.
    pub manufacturer_id: String,
    /// Contact information for the submitter, encoded as a semicolon-separated
    /// string in the XML.
    pub submitter: Submitter,
    /// File-related metadata, including encryption and compression methods and
    /// an optional transport key.
    pub file: File,
    /// An optional client version string for the software generating the
    /// report.
    pub client_version: Option<String>,
}

/// File-related metadata for the Elster eBilanz transfer header.
#[derive(Debug)]
pub struct File {
    /// The encryption method used for the file, e.g. `"CMSEncryptedData"`.
    pub encryption: String,
    /// The compression method used for the file, e.g. `"GZIP"`.
    pub compression: String,
    /// `Some("")` when `<TransportSchluessel/>` is present but empty.
    pub transport_key: Option<String>,
}

/// The data section of an Elster eBilanz report.
#[derive(Debug)]
pub struct DataSection {
    /// The list of payload blocks in the data section.
    pub payload_blocks: Vec<PayloadBlock>,
}

/// The payload block of an Elster eBilanz report.
#[derive(Debug)]
pub struct PayloadBlock {
    /// The header of the payload block, containing metadata about the payload.
    pub payload_header: PayloadHeader,
    /// The eBilanz content of the payload block, including the raw `xbrli:xbrl`.
    pub ebilanz: EBilanz,
}

/// The payload header corresponding to the `<NutzdatenHeader>` element in the
/// Elster eBilanz XML schema.
#[derive(Debug)]
pub struct PayloadHeader {
    /// Header schema version, e.g. `"11"`.
    pub version: String,
    /// A unique ticket identifier for the payload, e.g. `"0001"`.
    pub payload_ticket: String,
    /// The recipient of the payload.
    pub recipient: Recipient,
    /// Optional manufacturer information for the payload.q
    pub manufacturer: Option<Manufacturer>,
    /// Optional submitter information for the payload, encoded as a
    /// semicolon-separated string in the XML.
    pub submitter: Option<Submitter>,
}

/// A recipient of an Elster eBilanz payload, corresponding to the
/// `<Empfaenger>` element in the XML schema.
#[derive(Debug)]
pub struct Recipient {
    /// `"F"` = Finanzamt, `"L"` = Bundesland.
    pub id: String,
    pub value: String,
}

/// The manufacturer of the software generating the Elster eBilanz report,
/// corresponding to the `<Hersteller>` element in the XML schema.
#[derive(Debug)]
pub struct Manufacturer {
    pub product_name: String,
    pub product_version: String,
}

/// Contact information for the eBilanz submitter, encoded as a
/// semicolon-separated string.
///
/// The XSD only enforces a max length of 256 characters; the field order below
/// follows the documentation comment: `Lieferant
/// (Firma);Ansprechpartner;Telefon;E-Mail Adresse;Ort;PLZ;Straße;Land;`
#[derive(Debug, Clone, Default)]
pub struct Submitter {
    /// Company or person name (Lieferant/Firma).
    pub name: String,
    pub contact_person: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub street: Option<String>,
    pub country: Option<String>,
}

impl Submitter {
    /// Parse a semicolon-separated Elster `DatenLieferant` string.
    ///
    /// Fields are mapped positionally to the documented order. Empty fields
    /// become `None`. Extra trailing fields beyond the eight documented
    /// positions are ignored.
    pub fn from_elster_string(s: &str) -> Self {
        let mut parts = s.split(';');
        let field = |p: Option<&str>| -> Option<String> {
            p.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
        };
        Self {
            name: field(parts.next()).unwrap_or_default(),
            contact_person: field(parts.next()),
            phone: field(parts.next()),
            email: field(parts.next()),
            city: field(parts.next()),
            postal_code: field(parts.next()),
            street: field(parts.next()),
            country: field(parts.next()),
        }
    }

    /// Serialize to the documented Elster semicolon-separated wire format.
    pub fn to_elster_string(&self) -> String {
        format!(
            "{};{};{};{};{};{};{};{};",
            self.name,
            self.contact_person.as_deref().unwrap_or(""),
            self.phone.as_deref().unwrap_or(""),
            self.email.as_deref().unwrap_or(""),
            self.city.as_deref().unwrap_or(""),
            self.postal_code.as_deref().unwrap_or(""),
            self.street.as_deref().unwrap_or(""),
            self.country.as_deref().unwrap_or(""),
        )
    }
}

impl ElsterReport {
    /// Create an `ElsterReport` pre-populated with standard eBilanz defaults.
    ///
    /// Fixed values (schema version, procedure, encryption, product name, …)
    /// are taken from the Elster eBilanz template. Pass the fields that vary
    /// per submission:
    ///
    /// - `manufacturer_id`: 5-digit manufacturer ID assigned by the tax authority
    /// - `submitter`: contact information for the person or company submitting the report
    /// - `recipient_id`: `"F"` (Finanzamt) or `"L"` (Bundesland)
    /// - `recipient_value`: BUFA number (4 digits) or Bundesland code
    /// - `balance_date`: reporting cut-off date in `YYYYMMDD` format
    /// - `ebilanz_version`: eBilanz schema version, e.g. `"000002"`
    /// - `test_marker`: optional test marker for non-production submissions
    pub fn new(
        manufacturer_id: impl Into<String>,
        submitter: Submitter,
        recipient_id: impl Into<String>,
        recipient_value: impl Into<String>,
        balance_date: u32,
        test_marker: Option<impl Into<String>>,
    ) -> Self {
        Self {
            transfer_header: TransferHeader {
                version: "11".to_string(),
                procedure: "ElsterBilanz".to_string(),
                data_type: "Bilanz".to_string(),
                operation: "send-Auth".to_string(),
                test_marker: test_marker.map(|t| t.into()),
                manufacturer_id: manufacturer_id.into(),
                submitter: submitter.clone(),
                file: File {
                    encryption: "CMSEncryptedData".to_string(),
                    compression: "GZIP".to_string(),
                    transport_key: Some(String::new()),
                },
                client_version: Some("1.0".to_string()),
            },
            data_section: DataSection {
                payload_blocks: vec![PayloadBlock {
                    payload_header: PayloadHeader {
                        version: "11".to_string(),
                        payload_ticket: "0001".to_string(),
                        recipient: Recipient {
                            id: recipient_id.into(),
                            value: recipient_value.into(),
                        },
                        manufacturer: Some(Manufacturer {
                            product_name: "Taxel".to_string(),
                            product_version: env!("CARGO_PKG_VERSION").to_string(),
                        }),
                        submitter: Some(submitter),
                    },
                    ebilanz: EBilanz::new("000002", balance_date),
                }],
            },
        }
    }

    /// Replaces the raw `<xbrli:xbrl>` bytes in the first payload block.
    pub fn set_payload_xbrl(&mut self, xbrl_bytes: Vec<u8>) {
        if let Some(block) = self.data_section.payload_blocks.get_mut(0) {
            block.ebilanz.set_xbrl_raw(xbrl_bytes);
        }
    }

    /// Parse an Elster eBilanz XML string into a typed `ElsterReport`.
    ///
    /// The `xbrli:xbrl` subtree is captured verbatim and stored in
    /// [`EBilanz::xbrl_raw`]; it is not interpreted here.
    pub fn parse(xml: &str) -> Result<Self, anyhow::Error> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut buf = Vec::new();
        // Stack of local XML element names tracking the current path.
        let mut path: Vec<String> = Vec::new();

        // TransferHeader fields
        let mut th_version = String::new();
        let mut procedure = String::new();
        let mut data_type = String::new();
        let mut operation = String::new();
        let mut test_marker: Option<String> = None;
        let mut manufacturer_id = String::new();
        let mut submitter_th = String::new();
        let mut encryption = String::new();
        let mut compression = String::new();
        let mut transport_key: Option<String> = None;
        let mut client_version: Option<String> = None;

        // PayloadHeader fields (reset per PayloadBlock)
        let mut payload_version = String::new();
        let mut payload_ticket = String::new();
        let mut recipient_id = String::new();
        let mut recipient_value = String::new();
        let mut product_name: Option<String> = None;
        let mut product_version: Option<String> = None;
        let mut submitter_ndh: Option<String> = None;

        // EBilanz fields (reset per PayloadBlock)
        let mut ebilanz_version = String::new();
        let mut balance_date: u32 = 0;
        let mut xbrl_raw: Vec<u8> = Vec::new();

        let mut payload_blocks: Vec<PayloadBlock> = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) => {
                    let local = str::from_utf8(event.local_name().as_ref())?.to_string();

                    // xbrl: capture the full subtree verbatim before pushing to path
                    if local == "xbrl" {
                        let owned_start = event.clone().into_owned();
                        // e is no longer referenced — borrow on buf ends here (NLL)
                        buf.clear();
                        xbrl_raw = capture_xbrl(&mut reader, owned_start, &mut buf)?;
                        buf.clear();
                        continue;
                    }

                    // Read attributes for elements that carry them
                    match local.as_str() {
                        "TransferHeader" => {
                            th_version = get_attr(event, b"version").unwrap_or_default();
                        }
                        "NutzdatenHeader" => {
                            payload_version = get_attr(event, b"version").unwrap_or_default();
                        }
                        // Only the NutzdatenHeader/Empfaenger has text content + id attr.
                        // TransferHeader/Empfaenger (rare, id="L") is ignored.
                        "Empfaenger"
                            if path.last().map(|s| s.as_str()) == Some("NutzdatenHeader") =>
                        {
                            recipient_id = get_attr(event, b"id").unwrap_or_default();
                        }
                        "EBilanz" => {
                            ebilanz_version = get_attr(event, b"version").unwrap_or_default();
                        }
                        _ => {}
                    }

                    path.push(local);
                }

                Ok(Event::End(ref event)) => {
                    let local = str::from_utf8(event.local_name().as_ref())?.to_string();

                    if local == "Nutzdatenblock" {
                        let manufacturer = match (product_name.take(), product_version.take()) {
                            (Some(name), Some(version)) => Some(Manufacturer {
                                product_name: name,
                                product_version: version,
                            }),
                            _ => None,
                        };

                        payload_blocks.push(PayloadBlock {
                            payload_header: PayloadHeader {
                                version: payload_version.clone(),
                                payload_ticket: payload_ticket.clone(),
                                recipient: Recipient {
                                    id: recipient_id.clone(),
                                    value: recipient_value.clone(),
                                },
                                manufacturer,
                                submitter: submitter_ndh
                                    .take()
                                    .as_deref()
                                    .map(Submitter::from_elster_string),
                            },
                            ebilanz: EBilanz {
                                version: ebilanz_version.clone(),
                                balance_date,
                                xbrl_raw: xbrl_raw.clone(),
                            },
                        });

                        // Reset per-block fields
                        payload_version.clear();
                        payload_ticket.clear();
                        recipient_id.clear();
                        recipient_value.clear();
                        ebilanz_version.clear();
                        balance_date = 0;
                        xbrl_raw.clear();
                    }

                    path.pop();
                }

                Ok(Event::Empty(ref e)) => {
                    let local = str::from_utf8(e.local_name().as_ref())?.to_string();
                    // <TransportSchluessel/> — present but empty
                    if local == "TransportSchluessel" {
                        transport_key = Some(String::new());
                    }
                }

                Ok(Event::Text(ref t)) => {
                    let value = str::from_utf8(t.as_ref())?.to_string();
                    let current = path.last().map(|s| s.as_str());
                    let parent = path.iter().rev().nth(1).map(|s| s.as_str());

                    match (parent, current) {
                        (_, Some("Verfahren")) => procedure = value,
                        (_, Some("DatenArt")) => data_type = value,
                        (_, Some("Vorgang")) => operation = value,
                        (_, Some("Testmerker")) => test_marker = Some(value),
                        (_, Some("HerstellerID")) => manufacturer_id = value,
                        (Some("TransferHeader"), Some("DatenLieferant")) => submitter_th = value,
                        (_, Some("Verschluesselung")) => encryption = value,
                        (_, Some("Kompression")) => compression = value,
                        (_, Some("VersionClient")) => client_version = Some(value),
                        (_, Some("NutzdatenTicket")) => payload_ticket = value,
                        (Some("NutzdatenHeader"), Some("Empfaenger")) => recipient_value = value,
                        (_, Some("ProduktName")) => product_name = Some(value),
                        (_, Some("ProduktVersion")) => product_version = Some(value),
                        (Some("NutzdatenHeader"), Some("DatenLieferant")) => {
                            submitter_ndh = Some(value)
                        }
                        (_, Some("stichtag")) => balance_date = value.parse()?,
                        _ => {}
                    }
                }

                Ok(Event::Eof) => break,
                Err(err) => return Err(err.into()),
                _ => {}
            }

            buf.clear();
        }

        Ok(ElsterReport {
            transfer_header: TransferHeader {
                version: th_version,
                procedure,
                data_type,
                operation,
                test_marker,
                manufacturer_id,
                submitter: Submitter::from_elster_string(&submitter_th),
                file: File {
                    encryption,
                    compression,
                    transport_key,
                },
                client_version,
            },
            data_section: DataSection { payload_blocks },
        })
    }

    /// Serialize the `ElsterReport` back to an XML string.
    ///
    /// The `xbrli:xbrl` subtree stored in each [`EBilanz::xbrl_raw`] is
    /// written verbatim without modification.
    pub fn to_xml(&self) -> Result<String, anyhow::Error> {
        let mut output = Vec::new();
        let mut writer = Writer::new(Cursor::new(&mut output));

        // <?xml version="1.0" encoding="UTF-8"?>
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // <Elster xmlns="http://www.elster.de/elsterxml/schema/v11">
        let mut elster_start = BytesStart::new("Elster");
        elster_start.push_attribute(("xmlns", "http://www.elster.de/elsterxml/schema/v11"));
        writer.write_event(Event::Start(elster_start))?;

        self.write_transfer_header(&mut writer)?;
        self.write_data_section(&mut writer)?;

        writer.write_event(Event::End(BytesEnd::new("Elster")))?;

        Ok(String::from_utf8(output)?)
    }

    fn write_transfer_header<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<(), anyhow::Error> {
        let th = &self.transfer_header;

        let mut th_start = BytesStart::new("TransferHeader");
        th_start.push_attribute(("version", th.version.as_str()));
        writer.write_event(Event::Start(th_start))?;

        write_text_element(writer, "Verfahren", &th.procedure)?;
        write_text_element(writer, "DatenArt", &th.data_type)?;
        write_text_element(writer, "Vorgang", &th.operation)?;

        if let Some(ref v) = th.test_marker {
            write_text_element(writer, "Testmerker", v)?;
        }

        write_text_element(writer, "HerstellerID", &th.manufacturer_id)?;
        write_text_element(writer, "DatenLieferant", &th.submitter.to_elster_string())?;

        // <Datei>
        writer.write_event(Event::Start(BytesStart::new("Datei")))?;
        write_text_element(writer, "Verschluesselung", &th.file.encryption)?;
        write_text_element(writer, "Kompression", &th.file.compression)?;
        match th.file.transport_key.as_deref() {
            Some("") | None => {
                writer.write_event(Event::Empty(BytesStart::new("TransportSchluessel")))?;
            }
            Some(val) => {
                write_text_element(writer, "TransportSchluessel", val)?;
            }
        }
        writer.write_event(Event::End(BytesEnd::new("Datei")))?;

        if let Some(ref v) = th.client_version {
            write_text_element(writer, "VersionClient", v)?;
        }

        writer.write_event(Event::End(BytesEnd::new("TransferHeader")))?;
        Ok(())
    }

    fn write_data_section<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<(), anyhow::Error> {
        writer.write_event(Event::Start(BytesStart::new("DatenTeil")))?;

        for block in &self.data_section.payload_blocks {
            writer.write_event(Event::Start(BytesStart::new("Nutzdatenblock")))?;
            write_payload_header(writer, &block.payload_header)?;
            write_payload(writer, &block.ebilanz)?;
            writer.write_event(Event::End(BytesEnd::new("Nutzdatenblock")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("DatenTeil")))?;
        Ok(())
    }
}

fn write_payload_header<W: std::io::Write>(
    writer: &mut Writer<W>,
    header: &PayloadHeader,
) -> Result<(), anyhow::Error> {
    let mut ndh_start = BytesStart::new("NutzdatenHeader");
    ndh_start.push_attribute(("version", header.version.as_str()));
    writer.write_event(Event::Start(ndh_start))?;

    write_text_element(writer, "NutzdatenTicket", &header.payload_ticket)?;

    let mut emp_start = BytesStart::new("Empfaenger");
    emp_start.push_attribute(("id", header.recipient.id.as_str()));
    writer.write_event(Event::Start(emp_start))?;
    writer.write_event(Event::Text(BytesText::new(&header.recipient.value)))?;
    writer.write_event(Event::End(BytesEnd::new("Empfaenger")))?;

    if let Some(ref m) = header.manufacturer {
        writer.write_event(Event::Start(BytesStart::new("Hersteller")))?;
        write_text_element(writer, "ProduktName", &m.product_name)?;
        write_text_element(writer, "ProduktVersion", &m.product_version)?;
        writer.write_event(Event::End(BytesEnd::new("Hersteller")))?;
    }

    if let Some(ref s) = header.submitter {
        write_text_element(writer, "DatenLieferant", &s.to_elster_string())?;
    }

    writer.write_event(Event::End(BytesEnd::new("NutzdatenHeader")))?;
    Ok(())
}

fn write_payload<W: std::io::Write>(
    writer: &mut Writer<W>,
    ebilanz: &EBilanz,
) -> Result<(), anyhow::Error> {
    writer.write_event(Event::Start(BytesStart::new("Nutzdaten")))?;

    let mut eb_start = BytesStart::new("ebilanz:EBilanz");
    eb_start.push_attribute((
        "xmlns:ebilanz",
        "http://rzf.fin-nrw.de/RMS/EBilanz/2016/XMLSchema",
    ));
    eb_start.push_attribute(("version", ebilanz.version.as_str()));
    writer.write_event(Event::Start(eb_start))?;

    write_text_element(
        writer,
        "ebilanz:stichtag",
        &ebilanz.balance_date.to_string(),
    )?;

    // Write xbrli:xbrl subtree verbatim (handled by xbrl-rs)
    if !ebilanz.xbrl_raw.is_empty() {
        writer.get_mut().write_all(&ebilanz.xbrl_raw)?;
    }

    writer.write_event(Event::End(BytesEnd::new("ebilanz:EBilanz")))?;
    writer.write_event(Event::End(BytesEnd::new("Nutzdaten")))?;
    Ok(())
}

/// Read the first matching attribute value by local name.
fn get_attr(e: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .and_then(|a| str::from_utf8(a.value.as_ref()).ok().map(|s| s.to_string()))
}

/// Write `<name>value</name>`.
fn write_text_element<W: std::io::Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: &str,
) -> Result<(), anyhow::Error> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

/// Capture the full `<xbrli:xbrl>…</xbrli:xbrl>` subtree as raw bytes.
///
/// `start` is the already-read (owned) start event for `xbrl`. The reader must
/// be positioned immediately after that start tag.
fn capture_xbrl<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start: BytesStart<'static>,
    buf: &mut Vec<u8>,
) -> Result<Vec<u8>, anyhow::Error> {
    let mut captured: Vec<u8> = Vec::new();
    let mut writer = Writer::new(Cursor::new(&mut captured));

    writer.write_event(Event::Start(start))?;

    let mut depth: usize = 1;

    loop {
        match reader.read_event_into(buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                writer.write_event(Event::Start(e.clone().into_owned()))?;
            }
            Ok(Event::End(e)) => {
                depth -= 1;
                writer.write_event(Event::End(e.clone().into_owned()))?;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Text(e)) => {
                writer.write_event(Event::Text(e.clone().into_owned()))?;
            }
            Ok(Event::Empty(e)) => {
                writer.write_event(Event::Empty(e.clone().into_owned()))?;
            }
            Ok(Event::Comment(e)) => {
                writer.write_event(Event::Comment(e.clone().into_owned()))?;
            }
            Ok(Event::CData(e)) => {
                writer.write_event(Event::CData(e.clone().into_owned()))?;
            }
            Ok(Event::Eof) => {
                return Err(anyhow!("Unexpected EOF inside xbrli:xbrl element"));
            }
            Err(err) => return Err(err.into()),
            _ => {}
        }
        buf.clear();
    }

    drop(writer);
    Ok(captured)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Elster xmlns="http://www.elster.de/elsterxml/schema/v11">
    <TransferHeader version="11">
        <Verfahren>ElsterBilanz</Verfahren>
        <DatenArt>Bilanz</DatenArt>
        <Vorgang>send-Auth</Vorgang>
        <Testmerker>700000004</Testmerker>
        <HerstellerID>00000</HerstellerID>
        <DatenLieferant>Datenlieferant als String</DatenLieferant>
        <Datei>
            <Verschluesselung>CMSEncryptedData</Verschluesselung>
            <Kompression>GZIP</Kompression>
            <TransportSchluessel/>
        </Datei>
        <VersionClient>abc</VersionClient>
    </TransferHeader>
    <DatenTeil>
        <Nutzdatenblock>
            <NutzdatenHeader version="11">
                <NutzdatenTicket>0001</NutzdatenTicket>
                <Empfaenger id="F">0000</Empfaenger>
                <Hersteller>
                    <ProduktName>xyz</ProduktName>
                    <ProduktVersion>1.0</ProduktVersion>
                </Hersteller>
                <DatenLieferant>Datenlieferant als String</DatenLieferant>
            </NutzdatenHeader>
            <Nutzdaten>
                <ebilanz:EBilanz version="000002" xmlns:ebilanz="http://rzf.fin-nrw.de/RMS/EBilanz/2016/XMLSchema">
                    <ebilanz:stichtag>20220630</ebilanz:stichtag>
                </ebilanz:EBilanz>
            </Nutzdaten>
        </Nutzdatenblock>
    </DatenTeil>
</Elster>"#;

    #[test]
    fn test_parse_transfer_header() {
        let report = ElsterReport::parse(MINIMAL_XML).unwrap();
        let th = &report.transfer_header;
        assert_eq!(th.version, "11");
        assert_eq!(th.procedure, "ElsterBilanz");
        assert_eq!(th.data_type, "Bilanz");
        assert_eq!(th.operation, "send-Auth");
        assert_eq!(th.test_marker.as_deref(), Some("700000004"));
        assert_eq!(th.manufacturer_id, "00000");
        assert_eq!(th.submitter.name, "Datenlieferant als String");
        assert_eq!(th.file.encryption, "CMSEncryptedData");
        assert_eq!(th.file.compression, "GZIP");
        assert_eq!(th.file.transport_key.as_deref(), Some(""));
        assert_eq!(th.client_version.as_deref(), Some("abc"));
    }

    #[test]
    fn test_parse_payload_header() {
        let report = ElsterReport::parse(MINIMAL_XML).unwrap();
        let header = &report.data_section.payload_blocks[0].payload_header;
        assert_eq!(header.version, "11");
        assert_eq!(header.payload_ticket, "0001");
        assert_eq!(header.recipient.id, "F");
        assert_eq!(header.recipient.value, "0000");
        let m = header.manufacturer.as_ref().unwrap();
        assert_eq!(m.product_name, "xyz");
        assert_eq!(m.product_version, "1.0");
        assert_eq!(
            header.submitter.as_ref().map(|s| s.name.as_str()),
            Some("Datenlieferant als String")
        );
    }

    #[test]
    fn test_parse_ebilanz() {
        let report = ElsterReport::parse(MINIMAL_XML).unwrap();
        let ebilanz = &report.data_section.payload_blocks[0].ebilanz;
        assert_eq!(ebilanz.version, "000002");
        assert_eq!(ebilanz.balance_date, 20220630);
        assert!(ebilanz.xbrl_raw.is_empty());
    }

    #[test]
    fn test_round_trip_balance_date() {
        let mut report = ElsterReport::parse(MINIMAL_XML).unwrap();
        report.data_section.payload_blocks[0].ebilanz.balance_date = 20231231;

        let xml = report.to_xml().unwrap();
        let report2 = ElsterReport::parse(&xml).unwrap();
        assert_eq!(
            report2.data_section.payload_blocks[0].ebilanz.balance_date,
            20231231
        );
        assert_eq!(report2.transfer_header.procedure, "ElsterBilanz");
        assert_eq!(report2.transfer_header.manufacturer_id, "00000");
    }
}
