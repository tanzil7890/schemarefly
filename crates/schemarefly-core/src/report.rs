//! Report schema (stable v1)
//!
//! This schema is STABLE and VERSIONED.
//! Breaking changes require a new version.

use serde::{Deserialize, Serialize};
use crate::diagnostic::Diagnostic;

/// Report schema version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportVersion {
    /// Major version (breaking changes)
    pub major: u32,

    /// Minor version (backward-compatible additions)
    pub minor: u32,
}

impl ReportVersion {
    /// Current report schema version
    pub const CURRENT: ReportVersion = ReportVersion { major: 1, minor: 0 };
}

impl std::fmt::Display for ReportVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Summary statistics for a report
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Total number of diagnostics
    pub total: usize,

    /// Number of errors
    pub errors: usize,

    /// Number of warnings
    pub warnings: usize,

    /// Number of info messages
    pub info: usize,

    /// Number of models checked
    pub models_checked: usize,

    /// Number of contracts validated
    pub contracts_validated: usize,
}

impl Default for ReportSummary {
    fn default() -> Self {
        Self {
            total: 0,
            errors: 0,
            warnings: 0,
            info: 0,
            models_checked: 0,
            contracts_validated: 0,
        }
    }
}

/// Check report (report.json v1)
///
/// This is the stable output format.
/// All fields are versioned and backward-compatible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Schema version
    pub version: ReportVersion,

    /// Timestamp (ISO 8601)
    pub timestamp: String,

    /// Summary statistics
    pub summary: ReportSummary,

    /// All diagnostics
    pub diagnostics: Vec<Diagnostic>,

    /// Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Report {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            version: ReportVersion::CURRENT,
            timestamp: chrono::Utc::now().to_rfc3339(),
            summary: ReportSummary::default(),
            diagnostics: Vec::new(),
            metadata: None,
        }
    }

    /// Create a report from diagnostics
    pub fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        use crate::diagnostic::Severity;

        let summary = ReportSummary {
            total: diagnostics.len(),
            errors: diagnostics.iter().filter(|d| d.severity == Severity::Error).count(),
            warnings: diagnostics.iter().filter(|d| d.severity == Severity::Warn).count(),
            info: diagnostics.iter().filter(|d| d.severity == Severity::Info).count(),
            models_checked: 0,
            contracts_validated: 0,
        };

        Self {
            version: ReportVersion::CURRENT,
            timestamp: chrono::Utc::now().to_rfc3339(),
            summary,
            diagnostics,
            metadata: None,
        }
    }

    /// Add a diagnostic to the report
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        use crate::diagnostic::Severity;

        match diagnostic.severity {
            Severity::Error => self.summary.errors += 1,
            Severity::Warn => self.summary.warnings += 1,
            Severity::Info => self.summary.info += 1,
        }

        self.summary.total += 1;
        self.diagnostics.push(diagnostic);
    }

    /// Check if the report has any errors
    pub fn has_errors(&self) -> bool {
        self.summary.errors > 0
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Save to file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = self.to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

// Note: chrono dependency needed for timestamp
// We'll add it to Cargo.toml

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};

    #[test]
    fn empty_report() {
        let report = Report::new();
        assert_eq!(report.version, ReportVersion::CURRENT);
        assert_eq!(report.summary.total, 0);
        assert!(!report.has_errors());
    }

    #[test]
    fn report_with_diagnostics() {
        let diagnostics = vec![
            Diagnostic::new(DiagnosticCode::ContractMissingColumn, Severity::Error, "Missing column"),
            Diagnostic::new(DiagnosticCode::Info, Severity::Info, "All good"),
        ];

        let report = Report::from_diagnostics(diagnostics);
        assert_eq!(report.summary.total, 2);
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.summary.info, 1);
        assert!(report.has_errors());
    }

    #[test]
    fn report_serialization() {
        let report = Report::new();
        let json = report.to_json().unwrap();
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"diagnostics\""));
    }
}
