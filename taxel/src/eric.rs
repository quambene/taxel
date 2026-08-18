//! Interpretation of identifiers found in ERiC responses (validation and
//! send results), as opposed to `elster`, which builds the outbound Elster
//! envelope.

/// Extracts the XBRL concept local name from an ERiC `Feldidentifikator` value.
///
/// Input examples:
/// - `"gcd:genInfo.report.id.accountingStandard"` → `"genInfo.report.id.accountingStandard"`
/// - `"/Kontext[1]/gcd:genInfo.report.period.fiscalYearBegin[1]"` → `"genInfo.report.period.fiscalYearBegin"`
pub fn extract_fact_name(field_identifier: &str) -> &str {
    // Take the last `/`-delimited path segment.
    let segment = field_identifier
        .rsplit('/')
        .next()
        .unwrap_or(field_identifier);

    // Strip namespace prefix (everything up to and including `:`).
    let local = segment
        .split_once(':')
        .map(|(_, namespace)| namespace)
        .unwrap_or(segment);

    // Strip trailing XPath index such as `[1]`.
    local.rfind('[').map(|pos| &local[..pos]).unwrap_or(local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fact_name_bare_concept() {
        assert_eq!(
            extract_fact_name("gcd:genInfo.report.id.accountingStandard"),
            "genInfo.report.id.accountingStandard"
        );
    }

    #[test]
    fn test_extract_fact_name_xpath_with_index() {
        assert_eq!(
            extract_fact_name("/Kontext[1]/gcd:genInfo.report.period.fiscalYearBegin[1]"),
            "genInfo.report.period.fiscalYearBegin"
        );
    }

    #[test]
    fn test_extract_fact_name_no_namespace() {
        assert_eq!(extract_fact_name("fiscalYearBegin"), "fiscalYearBegin");
    }
}
