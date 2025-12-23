//! Diagnostic codes and error reporting
//!
//! IMPORTANT: Diagnostic codes are versioned and stable.
//! NEVER rename or remove codes - they are part of the public API.
//! Add new codes with new names only.

use serde::{Deserialize, Serialize};

/// Diagnostic code registry (v1)
///
/// These codes are STABLE and VERSIONED.
/// Do NOT rename or remove codes - only add new ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticCode {
    // Contract violations (1xxx)
    /// A column required by the contract is missing from the inferred schema
    ContractMissingColumn,

    /// A column type does not match the contract
    ContractTypeMismatch,

    /// Extra columns present that are not in the contract
    ContractExtraColumn,

    /// Contract is missing but model references other contracts
    ContractMissing,

    // Drift detection (2xxx)
    /// Warehouse table schema has changed (column dropped)
    DriftColumnDropped,

    /// Warehouse table schema has changed (type changed)
    DriftTypeChange,

    /// Warehouse table schema has changed (new column added)
    DriftColumnAdded,

    // SQL inference issues (3xxx)
    /// SELECT * encountered but cannot expand (no catalog)
    SqlSelectStarUnexpandable,

    /// Unsupported SQL syntax encountered
    SqlUnsupportedSyntax,

    /// Failed to parse SQL
    SqlParseError,

    /// Failed to infer schema from SQL
    SqlInferenceError,

    /// Aggregate function in GROUP BY without explicit alias
    SqlGroupByAggregateUnaliased,

    // General warnings (9xxx)
    /// General informational message
    Info,

    /// General warning message
    Warning,
}

impl DiagnosticCode {
    /// Get the diagnostic code as a stable string identifier
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContractMissingColumn => "CONTRACT_MISSING_COLUMN",
            Self::ContractTypeMismatch => "CONTRACT_TYPE_MISMATCH",
            Self::ContractExtraColumn => "CONTRACT_EXTRA_COLUMN",
            Self::ContractMissing => "CONTRACT_MISSING",
            Self::DriftColumnDropped => "DRIFT_COLUMN_DROPPED",
            Self::DriftTypeChange => "DRIFT_TYPE_CHANGE",
            Self::DriftColumnAdded => "DRIFT_COLUMN_ADDED",
            Self::SqlSelectStarUnexpandable => "SQL_SELECT_STAR_UNEXPANDABLE",
            Self::SqlUnsupportedSyntax => "SQL_UNSUPPORTED_SYNTAX",
            Self::SqlParseError => "SQL_PARSE_ERROR",
            Self::SqlInferenceError => "SQL_INFERENCE_ERROR",
            Self::SqlGroupByAggregateUnaliased => "SQL_GROUP_BY_AGGREGATE_UNALIASED",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
        }
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational message
    Info,

    /// Warning - should be reviewed but not blocking
    Warn,

    /// Error - blocking issue that should fail CI
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Source location in a file
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// File path relative to project root
    pub file: String,

    /// Optional line number (1-indexed)
    pub line: Option<usize>,

    /// Optional column number (1-indexed)
    pub column: Option<usize>,

    /// Optional end line (for ranges)
    pub end_line: Option<usize>,

    /// Optional end column (for ranges)
    pub end_column: Option<usize>,
}

impl Location {
    /// Create a new location with just a file path
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        }
    }

    /// Create a location with file and line number
    pub fn with_line(file: impl Into<String>, line: usize) -> Self {
        Self {
            file: file.into(),
            line: Some(line),
            column: None,
            end_line: None,
            end_column: None,
        }
    }

    /// Create a location with file, line, and column
    pub fn with_position(file: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            file: file.into(),
            line: Some(line),
            column: Some(column),
            end_line: None,
            end_column: None,
        }
    }
}

/// A diagnostic message with structured metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable diagnostic code
    pub code: DiagnosticCode,

    /// Severity level
    pub severity: Severity,

    /// Human-readable message
    pub message: String,

    /// Source location (best-effort)
    pub location: Option<Location>,

    /// Expected value (for comparison diagnostics)
    pub expected: Option<String>,

    /// Actual value (for comparison diagnostics)
    pub actual: Option<String>,

    /// List of downstream nodes impacted by this issue
    pub impact: Vec<String>,
}

impl Diagnostic {
    /// Create a new diagnostic with minimal fields
    pub fn new(code: DiagnosticCode, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            location: None,
            expected: None,
            actual: None,
            impact: Vec::new(),
        }
    }

    /// Set the location
    pub fn with_location(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    /// Set expected/actual values
    pub fn with_comparison(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    /// Set downstream impact
    pub fn with_impact(mut self, impact: Vec<String>) -> Self {
        self.impact = impact;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_code_stability() {
        // Ensure codes are stable strings
        assert_eq!(DiagnosticCode::ContractMissingColumn.as_str(), "CONTRACT_MISSING_COLUMN");
        assert_eq!(DiagnosticCode::DriftTypeChange.as_str(), "DRIFT_TYPE_CHANGE");
    }

    #[test]
    fn diagnostic_serialization() {
        let diag = Diagnostic::new(
            DiagnosticCode::ContractMissingColumn,
            Severity::Error,
            "Column 'user_id' is missing"
        )
        .with_location(Location::with_line("models/users.sql", 42));

        let json = serde_json::to_string(&diag).unwrap();
        assert!(json.contains("CONTRACT_MISSING_COLUMN"));
        assert!(json.contains("error"));
    }
}
