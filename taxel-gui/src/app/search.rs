use crate::domain::ReportSection;
use std::time::Instant;

/// Transient highlight for a row that was jumped to via search results, cleared
/// after a short duration.
pub struct RowHighlight {
    pub section_idx: usize,
    pub row_idx: usize,
    pub until: Instant,
}

/// A single search result pointing to a specific row in a specific section.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Index into `FactTable::sections`.
    pub section_idx: usize,
    /// Raw index into `FactSection::rows`.
    pub row_idx: usize,
    /// The concept name of the matched row.
    pub concept: String,
    /// The resolved label of the matched row.
    pub label: String,
    /// The section role (short name) for display.
    pub section_name: String,
}

/// Grouped search state.
#[derive(Default)]
pub struct Search {
    /// The current search query text.
    pub query: String,
    /// Cached search results, updated when the query or language changes.
    pub results: Vec<SearchHit>,
    /// Visible row index to scroll to after a search result click. Consumed
    /// after one frame.
    pub scroll_to_row: Option<usize>,
    /// Transient highlight for the row selected via search results.
    pub row_highlight: Option<RowHighlight>,
}

impl Search {
    /// Search all sections for rows matching the current query (case-insensitive substring
    /// match on concept, label, or value).
    pub fn search(&mut self, sections: &[ReportSection], lang: &str) {
        let query = self.query.trim().to_lowercase();

        if query.is_empty() {
            self.results.clear();
            return;
        }

        let mut hits = Vec::new();

        for (section_idx, section) in sections.iter().enumerate() {
            let section_name = section
                .labels
                .get(lang)
                .map(|lang| lang.as_str())
                .unwrap_or_else(|| section.role.rsplit('/').next().unwrap_or(&section.role));

            for (row_idx, row) in section.rows.iter().enumerate() {
                let label = row
                    .labels
                    .get(lang)
                    .map(|label| label.as_str())
                    .unwrap_or("");

                if row.concept.to_lowercase().contains(&query)
                    || label.to_lowercase().contains(&query)
                    || row.value.to_lowercase().contains(&query)
                {
                    hits.push(SearchHit {
                        section_idx,
                        row_idx,
                        concept: row.concept.clone(),
                        label: label.to_string(),
                        section_name: section_name.to_owned(),
                    });
                }
            }
        }

        self.results = hits;
    }
}
