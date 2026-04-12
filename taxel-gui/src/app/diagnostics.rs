/// Indicates the level of a diagnostics message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Success,
}

/// Groups diagnostics by their source domain so they can be cleared
/// independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCategory {
    /// Issues related to the application itself, such as unexpected panics or
    /// unhandled errors that should never occur during normal operation.
    App,
    /// Issues related to creating or importing a report, such as file I/O
    /// errors or parsing errors.
    Import,
    /// Issues related to validating the report against the taxonomy.
    Validation,
    /// Issues related to sending the report to the tax authority's API.
    Send,
}

/// Collects all information about a diagnostics message to display in the
/// diagnostics panel and diagnostics summary in the header.
#[derive(Clone, Debug)]
pub struct AppDiagnostic {
    pub level: DiagnosticLevel,
    pub category: DiagnosticCategory,
    pub message: String,
}

impl AppDiagnostic {
    pub fn new_warning(category: DiagnosticCategory, message: String) -> Self {
        AppDiagnostic {
            level: DiagnosticLevel::Warning,
            category,
            message,
        }
    }

    pub fn new_error(category: DiagnosticCategory, message: String) -> Self {
        AppDiagnostic {
            level: DiagnosticLevel::Error,
            category,
            message,
        }
    }

    pub fn new_success(category: DiagnosticCategory, message: String) -> Self {
        AppDiagnostic {
            level: DiagnosticLevel::Success,
            category,
            message,
        }
    }

    pub fn taxonomy_version_error(category: DiagnosticCategory) -> Self {
        AppDiagnostic::new_error(category, "Failed to determine taxonomy version".to_string())
    }
}
