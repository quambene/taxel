use std::{collections::HashMap, sync::LazyLock};

pub const GCD_ROLE_URI: &str = "http://www.xbrl.de/taxonomies/de-gcd/role/gcd";
pub const GCD_LABEL: &str = "GCD (Global Common Document)";

pub static TAXONOMY_YEAR_TO_VERSION: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            ("2020", "6.4"),
            ("2021", "6.5"),
            ("2022", "6.6"),
            ("2023", "6.7"),
            ("2024", "6.8"),
            ("2025", "6.9"),
            ("2026", "6.10"),
        ])
    });

/// Static mapping from full eBilanz role URI to de-gcd report-element concept.
pub static ROLE_URI_TO_REPORT_ELEMENT: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet",
                "genInfo.report.id.reportElement.reportElements.B",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/incomeStatement",
                "genInfo.report.id.reportElement.reportElements.GuV",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/incomeStatementMicroBilG",
                "genInfo.report.id.reportElement.reportElements.GuVMicroBilG",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/managementReport",
                "genInfo.report.id.reportElement.reportElements.L",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/notesBelowBalanceSheet",
                "genInfo.report.id.reportElement.reportElements.H",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/cashFlowStatementDRS21",
                "genInfo.report.id.reportElement.reportElements.CFS",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/changesEquityStatement",
                "genInfo.report.id.reportElement.reportElements.EKE",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/changesEquityAccounts",
                "genInfo.report.id.reportElement.reportElements.KKE",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/OtherReportElements",
                "genInfo.report.id.reportElement.reportElements.SA",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncome",
                "genInfo.report.id.reportElement.reportElements.SGE",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncomeBusinessPartnership",
                "genInfo.report.id.reportElement.reportElements.SGEP",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncomeSpecialCases",
                "genInfo.report.id.reportElement.reportElements.SGEB",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/adjustmentOfIncome",
                "genInfo.report.id.reportElement.reportElements.BGWG",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/transfersTaxAssets",
                "genInfo.report.id.reportElement.reportElements.BVV",
            ),
        ])
    });

/// The eBilanz payload header within a Nutzdatenblock.
///
/// The `xbrli:xbrl` subtree is handled separately by `xbrl-rs`; it is
/// captured verbatim during parsing and re-emitted unchanged during
/// serialization.
#[derive(Debug)]
pub struct EBilanz {
    /// Schema version, e.g. `"000002"`.
    pub version: String,
    /// Reporting cut-off date in `YYYYMMDD` format.
    pub balance_date: u32,
    /// Raw bytes of the `<xbrli:xbrl>…</xbrli:xbrl>` subtree.
    /// Private: populated by the parser, emitted verbatim by `to_xml`.
    pub(crate) xbrl_raw: Vec<u8>,
}

impl EBilanz {
    pub fn new(version: impl Into<String>, balance_date: u32) -> Self {
        Self {
            version: version.into(),
            balance_date,
            xbrl_raw: Vec::new(),
        }
    }
}
