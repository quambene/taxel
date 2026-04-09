use std::{collections::HashMap, sync::LazyLock};

pub const GCD_ROLE_URI: &str = "http://www.xbrl.de/taxonomies/de-gcd/role/gcd";
pub const GCD_LABEL: &str = "GCD (Global Common Document)";

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
