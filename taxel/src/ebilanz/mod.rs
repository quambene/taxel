mod report_elements;
mod taxonomy;

pub use report_elements::{REPORT_ELEMENT_TO_ROLE_URI, ROLE_URI_TO_REPORT_ELEMENT};
pub use taxonomy::{
    TaxonomyType, GCD_LABEL, GCD_ROLE_URI, TAXONOMY_VERSION_TO_DATE, TAXONOMY_YEAR_TO_VERSION,
};

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

    /// Replace the raw `<xbrli:xbrl>` bytes.
    pub fn set_xbrl_raw(&mut self, bytes: Vec<u8>) {
        self.xbrl_raw = bytes;
    }
}
