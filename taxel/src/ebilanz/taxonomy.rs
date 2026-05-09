use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::LazyLock};

pub const GCD_ROLE_URI: &str = "http://www.xbrl.de/taxonomies/de-gcd/role/gcd";
pub const GCD_LABEL: &str = "GCD (Global Common Document)";

/// Role URIs that must always be passed to `InstanceDocument::from_sections`,
/// regardless of which `reportElements.*` facts the user has activated.
///
/// ERiC requires Mussfeld facts from these sections to be present as nil facts
/// in the instance even when the corresponding report element is not selected:
/// - EV (`appropriationProfits`): `incomeUse.gainLoss.*` Mussfelder
/// - SGE (`determinationOfTaxableIncome`): `fpl.*` Mussfelder
/// - SGEP (`determinationOfTaxableIncomeBusinessPartnership`): `fplgm.*` Mussfelder
pub const BASELINE_ROLE_URIS: &[&str] = &[
    GCD_ROLE_URI,
    "http://www.xbrl.de/taxonomies/de-gaap-ci/role/appropriationProfits",
    "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncome",
    "http://www.xbrl.de/taxonomies/de-gaap-ci/role/determinationOfTaxableIncomeBusinessPartnership",
];

/// The set of facts that must be present in `ElsterReport` and `EBilanz`.
pub const REQUIRED_GCD_FACTS: &[&str] = &[
    FISCAL_YEAR_BEGIN,
    FISCAL_YEAR_END,
    CLOSING_DATE,
    COMPANY_TAX_NUMBER,
    COMPANY_STREET,
    COMPANY_HOUSE_NO,
    COMPANY_ZIP_CODE,
    COMPANY_CITY,
    COMPANY_COUNTRY,
];

pub const FISCAL_YEAR_BEGIN: &str = "genInfo.report.period.fiscalYearBegin";
pub const FISCAL_YEAR_END: &str = "genInfo.report.period.fiscalYearEnd";
pub const CLOSING_DATE: &str = "genInfo.report.period.balSheetClosingDate";
pub const COMPANY_NAME: &str = "genInfo.company.id.name";
pub const COMPANY_STREET: &str = "genInfo.company.id.location.street";
pub const COMPANY_HOUSE_NO: &str = "genInfo.company.id.location.houseNo";
pub const COMPANY_ZIP_CODE: &str = "genInfo.company.id.location.zipCode";
pub const COMPANY_CITY: &str = "genInfo.company.id.location.city";
pub const COMPANY_COUNTRY: &str = "genInfo.company.id.location.country";
pub const COMPANY_TAX_NUMBER: &str = "genInfo.company.id.idNo.type.companyId.ST13";
/// The parent fact of `COMPANY_TAX_NUMBER` is used to uniquely identify the tax
/// number fact.
pub const COMPANY_TAX_NUMBER_PARENT: &str = "genInfo.company.id.idNo";

/// The eBilanz taxonomy module to use for a new report. Each variant
/// corresponds to a specific set of schema ref URLs (plus the always-included
/// GCD module).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TaxonomyType {
    /// The core taxonomy for commercial/industrial entities (de-gaap-ci)
    #[default]
    CoreFiscal,
    /// The core taxonomy for micro entities pursuant to MicroBilG (de-gaap-ci)
    CoreFiscalMicroBilG,
    /// Supplementary taxonomy for regulated industries (de-bra)
    SupplementaryFiscal,
    /// Supplementary taxonomy for micro entities pursuant to MicroBilG (de-bra)
    SupplementaryFiscalMicroBilG,
    /// Taxonomy for financial institutions (de-fi)
    CreditInstitution,
    /// Taxonomy for payment institutions (de-pi)
    PaymentInstitution,
    /// Taxonomy for insurance companies and pension funds (de-ins)
    Insurance,
}

impl TaxonomyType {
    /// Returns the GCD module schema URL and the domain-specific schema URL for the
    /// given taxonomy date and type. These are the `link:schemaRef` entries an
    /// instance document must declare.
    pub fn schema_refs(&self, date: &str) -> Vec<String> {
        let gcd = format!("http://www.xbrl.de/taxonomies/de-gcd-{date}/de-gcd-{date}-shell.xsd");
        let domain = match self {
        TaxonomyType::CoreFiscal => format!(
            "http://www.xbrl.de/taxonomies/de-gaap-ci-{date}/de-gaap-ci-{date}-shell-fiscal.xsd"
        ),
        TaxonomyType::CoreFiscalMicroBilG => format!(
            "http://www.xbrl.de/taxonomies/de-gaap-ci-{date}/de-gaap-ci-{date}-shell-fiscal-microbilg.xsd"
        ),
        TaxonomyType::SupplementaryFiscal => format!(
            "http://www.xbrl.de/taxonomies/de-bra-{date}/de-bra-{date}-shell-fiscal.xsd"
        ),
        TaxonomyType::SupplementaryFiscalMicroBilG => format!(
            "http://www.xbrl.de/taxonomies/de-bra-{date}/de-bra-{date}-shell-fiscal-microbilg.xsd"
        ),
        TaxonomyType::CreditInstitution => format!(
            "http://www.xbrl.de/taxonomies/de-fi-{date}/de-fi-{date}-shell-staffelform-fiscal.xsd"
        ),
        TaxonomyType::PaymentInstitution => format!(
            "http://www.xbrl.de/taxonomies/de-pi-{date}/de-pi-{date}-shell-staffelform-fiscal.xsd"
        ),
        TaxonomyType::Insurance => format!(
            "http://www.xbrl.de/taxonomies/de-ins-{date}/de-ins-{date}-shell-fiscal.xsd"
        ),
    };
        vec![gcd, domain]
    }

    /// Determines the taxonomy type from a set of schema ref URLs as embedded
    /// in an XBRL instance document.
    pub fn from_schema_refs(schema_refs: &[String]) -> Option<Self> {
        schema_refs.iter().find_map(|schema_ref| {
            if schema_ref.contains("de-gaap-ci") && schema_ref.contains("microbilg") {
                Some(TaxonomyType::CoreFiscalMicroBilG)
            } else if schema_ref.contains("de-gaap-ci") {
                Some(TaxonomyType::CoreFiscal)
            } else if schema_ref.contains("de-bra") && schema_ref.contains("microbilg") {
                Some(TaxonomyType::SupplementaryFiscalMicroBilG)
            } else if schema_ref.contains("de-bra") {
                Some(TaxonomyType::SupplementaryFiscal)
            } else if schema_ref.contains("de-fi") {
                Some(TaxonomyType::CreditInstitution)
            } else if schema_ref.contains("de-pi") {
                Some(TaxonomyType::PaymentInstitution)
            } else if schema_ref.contains("de-ins") {
                Some(TaxonomyType::Insurance)
            } else {
                None
            }
        })
    }

    /// Returns the XML namespace prefix for the given taxonomy date and type.
    pub fn namespace_prefix(&self) -> &'static str {
        match self {
            TaxonomyType::CoreFiscal | TaxonomyType::CoreFiscalMicroBilG => "de-gaap-ci",
            TaxonomyType::SupplementaryFiscal | TaxonomyType::SupplementaryFiscalMicroBilG => {
                "de-bra"
            }
            TaxonomyType::CreditInstitution => "de-fi",
            TaxonomyType::PaymentInstitution => "de-pi",
            TaxonomyType::Insurance => "de-ins",
        }
    }

    /// Returns the XML namespace URI for the given taxonomy date and type.
    pub fn namespace_uri(&self, date: &str) -> String {
        match self {
            TaxonomyType::CoreFiscal | TaxonomyType::CoreFiscalMicroBilG => {
                format!("http://www.xbrl.de/taxonomies/de-gaap-ci-{date}")
            }
            TaxonomyType::SupplementaryFiscal | TaxonomyType::SupplementaryFiscalMicroBilG => {
                format!("http://www.xbrl.de/taxonomies/de-bra-{date}")
            }
            TaxonomyType::CreditInstitution => {
                format!("http://www.xbrl.de/taxonomies/de-fi-{date}")
            }
            TaxonomyType::PaymentInstitution => {
                format!("http://www.xbrl.de/taxonomies/de-pi-{date}")
            }
            TaxonomyType::Insurance => format!("http://www.xbrl.de/taxonomies/de-ins-{date}"),
        }
        .to_string()
    }

    /// Returns `true` for Supplementary (Ergänzungsbilanz / Sonderbilanz)
    /// types. For example, concepts annotated with
    /// `hgbref:onlyPermittedForSoBil_ErgBil` are allowed only for these types
    /// and must be filtered out for all others.
    pub fn is_supplementary(&self) -> bool {
        matches!(
            self,
            TaxonomyType::SupplementaryFiscal | TaxonomyType::SupplementaryFiscalMicroBilG
        )
    }

    /// Returns the human-readable label for this taxonomy type and the given
    /// language code (e.g. "en", "de"), or `None` if no label is available.
    ///
    /// Labels are sourced from the `TAXONOMY_TYPE_LABELS` static mapping, which
    /// is populated with known combinations of taxonomy type and language.
    pub fn label(&self, language: &str) -> Option<&'static str> {
        TAXONOMY_TYPE_LABELS.get(&(self, language)).copied()
    }
}

pub const TAXONOMY_TYPES: [TaxonomyType; 7] = [
    TaxonomyType::CoreFiscal,
    TaxonomyType::CoreFiscalMicroBilG,
    TaxonomyType::SupplementaryFiscal,
    TaxonomyType::SupplementaryFiscalMicroBilG,
    TaxonomyType::CreditInstitution,
    TaxonomyType::PaymentInstitution,
    TaxonomyType::Insurance,
];

/// Mapping from (taxonomy type, language) to the human-readable label for that taxonomy.
static TAXONOMY_TYPE_LABELS: LazyLock<HashMap<(&TaxonomyType, &str), &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            (
                (&TaxonomyType::CoreFiscal, "de"),
                "Kerntaxonomie (de-gaap-ci)",
            ),
            (
                (&TaxonomyType::CoreFiscal, "en"),
                "Core taxonomy (de-gaap-ci)",
            ),
            (
                (&TaxonomyType::CoreFiscalMicroBilG, "de"),
                "Kerntaxonomie für Kleinstunternehmen gemäß MicroBilG (de-gaap-ci)",
            ),
            (
                (&TaxonomyType::CoreFiscalMicroBilG, "en"),
                "Core taxonomy for micro entities pursuant to MicroBilG (de-gaap-ci)",
            ),
            (
                (&TaxonomyType::SupplementaryFiscal, "de"),
                "Ergänzungstaxonomie (de-bra)",
            ),
            (
                (&TaxonomyType::SupplementaryFiscal, "en"),
                "Supplementary taxonomy (de-bra)",
            ),
            (
                (&TaxonomyType::SupplementaryFiscalMicroBilG, "de"),
                "Ergänzungstaxonomie für Kleinstunternehmen gemäß MicroBilG (de-bra)",
            ),
            (
                (&TaxonomyType::SupplementaryFiscalMicroBilG, "en"),
                "Supplementary taxonomy for micro entities pursuant to MicroBilG (de-bra)",
            ),
            (
                (&TaxonomyType::CreditInstitution, "de"),
                "Bankentaxonomie (de-fi)",
            ),
            (
                (&TaxonomyType::CreditInstitution, "en"),
                "Banking taxonomy (de-fi)",
            ),
            (
                (&TaxonomyType::PaymentInstitution, "de"),
                "Taxonomie für Zahlungsinstitute (de-pi)",
            ),
            (
                (&TaxonomyType::PaymentInstitution, "en"),
                "Payment institution taxonomy (de-pi)",
            ),
            (
                (&TaxonomyType::Insurance, "de"),
                "Versicherungstaxonomie (de-ins)",
            ),
            (
                (&TaxonomyType::Insurance, "en"),
                "Insurance taxonomy (de-ins)",
            ),
        ])
    });

/// Maps each eBilanz taxonomy version to the release date used in schema URLs.
pub static TAXONOMY_VERSION_TO_DATE: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            ("5.0", "2011-04-14"),
            ("5.1", "2012-06-01"),
            ("5.2", "2013-04-30"),
            ("5.3", "2014-04-02"),
            ("5.4", "2015-04-03"),
            ("6.0", "2016-04-01"),
            ("6.1", "2017-04-01"),
            ("6.2", "2018-04-01"),
            ("6.3", "2019-04-01"),
            ("6.4", "2020-04-01"),
            ("6.5", "2021-04-14"),
            ("6.6", "2022-05-02"),
            ("6.7", "2023-04-01"),
            ("6.8", "2024-04-01"),
            ("6.9", "2025-04-01"),
        ])
    });

/// Maps each eBilanz taxonomy date to the corresponding version number.
pub static TAXONOMY_DATE_TO_VERSION: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        HashMap::from([
            ("2011-04-14", "5.0"),
            ("2012-06-01", "5.1"),
            ("2013-04-30", "5.2"),
            ("2014-04-02", "5.3"),
            ("2015-04-03", "5.4"),
            ("2016-04-01", "6.0"),
            ("2017-04-01", "6.1"),
            ("2018-04-01", "6.2"),
            ("2019-04-01", "6.3"),
            ("2020-04-01", "6.4"),
            ("2021-04-14", "6.5"),
            ("2022-05-02", "6.6"),
            ("2023-04-01", "6.7"),
            ("2024-04-01", "6.8"),
            ("2025-04-01", "6.9"),
        ])
    });
