use std::{collections::HashMap, sync::LazyLock};

pub const GCD_ROLE_URI: &str = "http://www.xbrl.de/taxonomies/de-gcd/role/gcd";
pub const GCD_LABEL: &str = "GCD (Global Common Document)";

/// Static mapping from full eBilanz role URI to de-gcd report-element concept.
pub static ROLE_URI_TO_REPORT_ELEMENT: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet",
                "reportElements.B",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/incomeStatement",
                "reportElements.GuV",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/incomeStatementMicroBilG",
                "reportElements.GuVMicroBilG",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/managementReport",
                "reportElements.L",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/notesBelowBalanceSheet",
                "reportElements.H",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/cashFlowStatementDRS21",
                "reportElements.CFS",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/changesEquityStatement",
                "reportElements.EKE",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/changesEquityAccounts",
                "reportElements.KKE",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/OtherReportElements",
                "reportElements.SA",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncome",
                "reportElements.SGE",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncomeBusinessPartnership",
                "reportElements.SGEP",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncomeSpecialCases",
                "reportElements.SGEB",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/adjustmentOfIncome",
                "reportElements.BGWG",
            ),
            (
                "http://www.xbrl.de/taxonomies/de-gaap-ci/role/transfersTaxAssets",
                "reportElements.BVV",
            ),
        ])
    });
