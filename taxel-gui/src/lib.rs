use anyhow::Result;
use eframe::{
    egui::{CentralPanel, Context, Grid, ScrollArea},
    App, Frame,
};
use quick_xml::events::BytesStart;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct TableRow {
    pub concept: String, // de-gaap-ci:Assets
    pub context: String, // Context ID
    pub unit: Option<String>,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct XbrlTable {
    pub rows: Vec<TableRow>,
}

pub struct XbrlApp {
    table: XbrlTable,
}

impl XbrlApp {
    pub fn new(table: XbrlTable) -> XbrlApp {
        Self { table }
    }
}

impl App for XbrlApp {
    fn update(&mut self, ctx: &Context, _: &mut Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.heading("eBilanz");

                    Grid::new("xbrl_table").striped(true).show(ui, |ui| {
                        ui.label("Key");
                        ui.label("Context");
                        ui.label("Unit");
                        ui.label("Value");
                        ui.end_row();

                        for row in &mut self.table.rows {
                            ui.label(&row.concept);
                            ui.label(&row.context);
                            ui.label(row.unit.as_deref().unwrap_or("-"));
                            ui.text_edit_singleline(&mut row.value);
                            ui.end_row();
                        }
                    });
                });
        });
    }
}

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
                context: ctx,
                unit,
                value: t.unescape()?.to_string(),
            });
        }
    }

    Ok(())
}
