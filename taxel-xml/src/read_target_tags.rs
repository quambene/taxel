use crate::TargetTags;
use csv::Reader;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    #[serde(rename = "ebilanz_key")]
    key: String,
    #[serde(rename = "ebilanz_value")]
    value: Option<String>,
}

/// Read target tags from a csv file.
///
/// If no csv file is given, use empty target tags.
pub fn read_target_tags<R>(reader: Option<&mut Reader<R>>) -> Result<TargetTags, anyhow::Error>
where
    R: std::io::Read,
{
    let mut target_tags = TargetTags::new();

    if let Some(reader) = reader {
        let records = reader.deserialize();

        for record in records {
            let row: Row = record?;
            target_tags.insert(row.key, row.value);
        }
    }

    Ok(target_tags)
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

        let res = read_target_tags(Some(&mut reader));
        assert!(res.is_ok(), "Can't read target tags: {}", res.unwrap_err());

        let target_tags = res.unwrap();
        assert_eq!(
            target_tags,
            TargetTags(HashMap::from_iter(vec![
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
