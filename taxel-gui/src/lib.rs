use anyhow::Result;
use quick_xml::events::BytesStart;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct TableRow {
    pub concept: String,
    // Human-readable label
    pub label: Option<String>,
    pub context: String,
    pub unit: Option<String>,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct XbrlTable {
    pub rows: Vec<TableRow>,
}

// TODO: replace by XBRL parser
/// Read XBRL in table format.
pub fn read_xbrl(xml: &str) -> Result<XbrlTable> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut table = XbrlTable::default();
    let mut inside_xbrl = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) => {
                let name = event.name();

                // detect start of XBRL instance
                if name.as_ref().ends_with(b"xbrl") {
                    inside_xbrl = true;
                }

                if inside_xbrl {
                    parse_xbrl_fact(&mut reader, &event, &mut table)?;
                }
            }

            Event::End(event) => {
                if event.name().as_ref().ends_with(b"xbrl") {
                    break;
                }
            }

            Event::Eof => break,
            _ => {}
        }

        buf.clear();
    }

    Ok(table)
}

fn parse_xbrl_fact(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    table: &mut XbrlTable,
) -> Result<()> {
    let mut context = None;
    let mut unit = None;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"contextRef" => context = Some(attr.unescape_value()?.to_string()),
            b"unitRef" => unit = Some(attr.unescape_value()?.to_string()),
            _ => {}
        }
    }

    if let Some(ctx) = context {
        let concept = String::from_utf8_lossy(e.name().as_ref()).to_string();

        if let Event::Text(t) = reader.read_event()? {
            table.rows.push(TableRow {
                concept,
                label: None,
                context: ctx,
                unit,
                value: t.unescape()?.to_string(),
            });
        }
    }

    Ok(())
}
