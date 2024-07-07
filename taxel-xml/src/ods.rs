use anyhow::anyhow;
use spreadsheet_ods::{Sheet, Value};
use std::{collections::HashMap, path::Path};

#[derive(Debug)]
struct Cell {
    row: u32,
    col: u32,
}

impl Cell {
    pub fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }
}

#[derive(Debug)]
struct SourceCell {
    key: Cell,
    value: Cell,
}

impl SourceCell {
    pub fn new(key: Cell, value: Cell) -> Self {
        Self { key, value }
    }
}

const NUM_ROWS: usize = 500;

/// Read target tags from ods file.
#[allow(dead_code)]
pub fn read_target_tags_ods(path: &Path) -> Result<HashMap<String, Value>, anyhow::Error> {
    let mut target_tags = HashMap::new();
    let workbook = if path.exists() {
        spreadsheet_ods::read_ods(path)?
    } else {
        return Err(anyhow!("Invalid path: {}", path.display()));
    };
    let num_sheets = workbook.num_sheets();
    let mut sheets = vec![];

    for i in 0..num_sheets {
        let sheet = workbook.sheet(i);
        sheets.push(sheet);
    }

    for sheet in sheets {
        let source_cell = find_source_cell(sheet)?;
        let key_column = source_cell.key.col;
        let value_column = source_cell.value.col;

        for i in 1..NUM_ROWS {
            let key = sheet.value(i as u32, key_column);
            let value = sheet.value(i as u32, value_column);

            match (key, value) {
                (Value::Empty, Value::Empty) => continue,
                (key, value) => {
                    if let Some(key) = key.as_str_opt() {
                        target_tags.insert(key.to_owned(), value.to_owned());
                    }
                }
            }
        }
    }

    Ok(target_tags)
}

fn find_source_cell(sheet: &Sheet) -> Result<SourceCell, anyhow::Error> {
    let mut key_cell = None;
    let mut value_cell = None;

    for i in 0..NUM_ROWS {
        for j in 0..NUM_ROWS {
            let sheet_value = sheet.value(i as u32, j as u32);

            if sheet_value.as_str_opt() == Some("ebilanz_key") {
                key_cell = Some(Cell::new(i as u32, j as u32));
            }

            if sheet_value.as_str_opt() == Some("ebilanz_value") {
                value_cell = Some(Cell::new(i as u32, j as u32));
            }
        }
    }

    match (key_cell, value_cell) {
        (Some(key), Some(value)) => {
            if key.row != value.row {
                Err(anyhow!(
                    "Invalid rows for `ebilanz_key` and `ebilanz_value`"
                ))
            } else {
                Ok(SourceCell::new(key, value))
            }
        }
        (Some(_), None) => Err(anyhow!("Missing column `ebilanz_key`")),
        (None, Some(_)) => Err(anyhow!("Missing column `ebilanz_value`")),
        (None, None) => Err(anyhow!("Missing columns `ebilanz_key` and `ebilanz_value`")),
    }
}
