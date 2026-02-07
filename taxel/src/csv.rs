use crate::{Tag, Tags};
pub use csv::{Reader, ReaderBuilder, Trim, Writer, WriterBuilder};
use log::{debug, info};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CsvRow {
    #[serde(rename = "ebilanz_key")]
    key: String,
    #[serde(rename = "ebilanz_value")]
    value: Option<String>,
}

impl CsvRow {
    pub fn new(key: String, value: Option<String>) -> Self {
        Self { key, value }
    }
}

/// Read target tags from a csv file.
///
/// If no csv file is given, use empty target tags.
pub fn read_tags<R>(reader: Option<&mut Reader<R>>) -> Result<Tags, anyhow::Error>
where
    R: std::io::Read,
{
    info!("Read target tags");

    let mut target_tags = Tags::new();

    if let Some(reader) = reader {
        let records = reader.deserialize();

        for record in records {
            let row: CsvRow = record?;
            let value = row.value.filter(|value| !value.is_empty());
            target_tags.insert(row.key, value);
        }
    }

    debug!("Target tags read: {target_tags:#?}");

    Ok(target_tags)
}

/// Write target tags to a csv file.
pub fn write_tags<W>(writer: &mut Writer<W>, extracted_tags: Vec<Tag>) -> Result<(), anyhow::Error>
where
    W: std::io::Write,
{
    info!("Write tags to csv file");

    for tag in extracted_tags {
        let row = CsvRow::new(tag.name, tag.value);
        writer.serialize(row)?;
    }

    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv::{ReaderBuilder, Trim};
    use std::collections::HashMap;

    #[test]
    fn test_read_target_tags() {
        let data = r#"ebilanz_key,ebilanz_value
        Empfaenger,1111
        ebilanz:stichtag,20201231
        de-gcd:genInfo.report.period.fiscalYearBegin,2020-01-01
        de-gcd:genInfo.report.period.fiscalYearEnd,2020-12-31
        de-gcd:genInfo.report.period.balSheetClosingDate,2020-12-31
        de-gcd:genInfo.company.id.location.country,Deutschland
        de-gcd:genInfo.company.id.location.country.isoCode,DE"#;
        let mut reader = ReaderBuilder::new()
            .delimiter(b',')
            .has_headers(true)
            .trim(Trim::All)
            .from_reader(data.as_bytes());

        let res = read_tags(Some(&mut reader));
        assert!(res.is_ok(), "Can't read target tags: {}", res.unwrap_err());

        let target_tags = res.unwrap();
        assert_eq!(
            target_tags,
            Tags(HashMap::from_iter(vec![
                (String::from("Empfaenger"), Some(String::from("1111"))),
                (
                    String::from("ebilanz:stichtag"),
                    Some(String::from("20201231"))
                ),
                (
                    String::from("de-gcd:genInfo.report.period.fiscalYearBegin"),
                    Some(String::from("2020-01-01"))
                ),
                (
                    String::from("de-gcd:genInfo.report.period.fiscalYearEnd"),
                    Some(String::from("2020-12-31"))
                ),
                (
                    String::from("de-gcd:genInfo.report.period.balSheetClosingDate"),
                    Some(String::from("2020-12-31"))
                ),
                (
                    String::from("de-gcd:genInfo.company.id.location.country"),
                    Some(String::from("Deutschland"))
                ),
                (
                    String::from("de-gcd:genInfo.company.id.location.country.isoCode"),
                    Some(String::from("DE"))
                ),
            ]))
        )
    }
}
