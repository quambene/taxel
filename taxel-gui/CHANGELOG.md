<!-- markdownlint-disable MD041 -->

## v0.1.1 (unreleased)

- fixed
  - Fix missing decimal places for aggregated fact values that sum to zero or a whole number
  - Set/reset `test_marker` in report based on env var `TEST_MARKER`
  - Set `transfer_header` when report is saved
- added
  - Automatically aggregate calculated fact values (e.g. total assets, total equity and liabilities) from the taxonomy's calculation linkbase; aggregated facts are read-only
  - Show taxonomy info (abstract, required, tuple, calculated) in a tooltip on the fact ID
  - Export fact values as CSV file
  - Save report at location ("Save as")
  - Delete report from report list
  - Open confirmation PDF after sending the report
  - Filter income statement for report element
    `genInfo.report.id.reportElement.reportElements.GuV` based on selected
    income statement format ("Gesamtkostenverfahren" or "Umsatzkostenverfahren")
- changed
  - Add description to "Import values" modal

## v0.1.0 (2026-05-16)

- fixed
  - Fix non-nil facts with empty string as value
  - Fix unsanitized xml for `Eric::validate`
  - Fix missing `context_ref` on tuple child after editing via dropdown
  - Allow search on enabled report sections only
  - Allow jumping to fact referenced by the diagnostic issue for enabled report sections only
  - Filter out forbidden facts from created `InstanceDocument`
  - Fix missing precision or decimals attribute for numeric fact values
  - Populate concepts from selected report sections only when creating a new report
- added
  - Import report
  - Display report
  - Display single-choice tuples as dropdown
  - Display multiple-choice tuples as checkbox
  - Display required facts only
  - Display filled facts only
  - Display report sections in sidebar
  - Edit and save report
  - Close report
  - Delete report
  - Create report
  - Send and validate report
  - Import values from source report into existing report
  - Select display level
  - Select label language
  - Zoom in & out
  - Add search bar
  - Add toggle for light/dark mode
  - Add diagnostics panel
  - Persist app settings
  - Persist report list
  - Add loading spinner for import report
  - Display report list
  - Replace app and vendor data in imported report
  - Sync GCD section with `ElsterReport` and XBRL `Period`
  - Sync start and end date from `NewReportForm` to GCD section
  - Validate required facts
  - Make error message clickable and jump to relevant fact
  - Copy diagnostic message on right click
  - Send report in test mode via env var `TEST_MARKER`
  - Add modal to confirm terms of use and privacy notice
  - Print confirmation for sent report
