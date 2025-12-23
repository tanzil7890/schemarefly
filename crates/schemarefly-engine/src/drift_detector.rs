//! Drift detection engine for comparing expected vs actual warehouse schemas
//!
//! This module implements the core drift detection logic that compares
//! schemas defined in dbt manifests/contracts against actual warehouse schemas.

use schemarefly_core::{Schema, LogicalType, Diagnostic, DiagnosticCode, Severity, Location};
use std::collections::HashSet;

/// Result of comparing expected vs actual warehouse schema
#[derive(Debug, Clone)]
pub struct DriftDetection {
    /// The table being checked
    pub table_id: String,

    /// Expected schema from manifest/contract
    pub expected: Schema,

    /// Actual schema from warehouse
    pub actual: Schema,

    /// Diagnostics produced by the comparison
    pub diagnostics: Vec<Diagnostic>,
}

impl DriftDetection {
    /// Create a new drift detection by comparing schemas
    ///
    /// This detects three types of drift:
    /// - Dropped columns: columns in expected but not in actual
    /// - Type changes: columns with different types
    /// - New columns: columns in actual but not in expected (info level)
    pub fn detect(
        table_id: impl Into<String>,
        expected: &Schema,
        actual: &Schema,
        file_path: Option<String>,
    ) -> Self {
        let table_id = table_id.into();
        let mut diagnostics = Vec::new();

        // Track which columns we've seen from the expected schema
        let mut seen_expected_cols = HashSet::new();

        // Check each column in expected schema
        for expected_col in &expected.columns {
            seen_expected_cols.insert(&expected_col.name);

            match actual.find_column(&expected_col.name) {
                Some(actual_col) => {
                    // Column exists - check for type drift
                    if !types_match(&expected_col.logical_type, &actual_col.logical_type) {
                        let message = format!(
                            "Column '{}' type changed: was {}, now {}",
                            expected_col.name,
                            expected_col.logical_type,
                            actual_col.logical_type
                        );

                        diagnostics.push(Diagnostic {
                            code: DiagnosticCode::DriftTypeChange,
                            severity: Severity::Error,
                            message,
                            location: file_path.as_ref().map(|path| Location {
                                file: path.clone(),
                                line: None,
                                column: None,
                                end_line: None,
                                end_column: None,
                            }),
                            expected: Some(expected_col.logical_type.to_string()),
                            actual: Some(actual_col.logical_type.to_string()),
                            impact: vec![],
                        });
                    }
                }
                None => {
                    // Column dropped from warehouse
                    let message = format!(
                        "Column '{}' was dropped from warehouse table (expected type: {})",
                        expected_col.name,
                        expected_col.logical_type
                    );

                    diagnostics.push(Diagnostic {
                        code: DiagnosticCode::DriftColumnDropped,
                        severity: Severity::Error,
                        message,
                        location: file_path.as_ref().map(|path| Location {
                            file: path.clone(),
                            line: None,
                            column: None,
                            end_line: None,
                            end_column: None,
                        }),
                        expected: Some(expected_col.name.clone()),
                        actual: None,
                        impact: vec![],
                    });
                }
            }
        }

        // Check for new columns in actual schema
        for actual_col in &actual.columns {
            if !seen_expected_cols.contains(&actual_col.name) {
                let message = format!(
                    "New column '{}' added to warehouse table (type: {})",
                    actual_col.name,
                    actual_col.logical_type
                );

                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::DriftColumnAdded,
                    severity: Severity::Info,
                    message,
                    location: file_path.as_ref().map(|path| Location {
                        file: path.clone(),
                        line: None,
                        column: None,
                        end_line: None,
                        end_column: None,
                    }),
                    expected: None,
                    actual: Some(actual_col.name.clone()),
                    impact: vec![],
                });
            }
        }

        Self {
            table_id,
            expected: expected.clone(),
            actual: actual.clone(),
            diagnostics,
        }
    }

    /// Check if there are any drift errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// Check if there are any drift warnings
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Warn)
    }

    /// Check if there are any drift info messages
    pub fn has_info(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Info)
    }

    /// Count error diagnostics
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    /// Count warning diagnostics
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warn).count()
    }

    /// Count info diagnostics
    pub fn info_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Info).count()
    }
}

/// Check if two types match exactly
///
/// For drift detection, we want exact matches - no lenient coercion
fn types_match(expected: &LogicalType, actual: &LogicalType) -> bool {
    match (expected, actual) {
        // Exact matches
        (LogicalType::Bool, LogicalType::Bool) => true,
        (LogicalType::Int, LogicalType::Int) => true,
        (LogicalType::Float, LogicalType::Float) => true,
        (LogicalType::String, LogicalType::String) => true,
        (LogicalType::Date, LogicalType::Date) => true,
        (LogicalType::Timestamp, LogicalType::Timestamp) => true,
        (LogicalType::Json, LogicalType::Json) => true,

        // Decimal: match if precision/scale match (or both unknown)
        (
            LogicalType::Decimal { precision: p1, scale: s1 },
            LogicalType::Decimal { precision: p2, scale: s2 },
        ) => p1 == p2 && s1 == s2,

        // Unknown matches anything (since we don't have enough info)
        (LogicalType::Unknown, _) | (_, LogicalType::Unknown) => true,

        // Everything else is a mismatch
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_core::Column;

    fn create_test_schema() -> Schema {
        Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
        ])
    }

    #[test]
    fn test_no_drift() {
        let expected = create_test_schema();
        let actual = expected.clone();

        let drift = DriftDetection::detect("test_table", &expected, &actual, None);

        assert_eq!(drift.diagnostics.len(), 0);
        assert!(!drift.has_errors());
        assert!(!drift.has_warnings());
        assert!(!drift.has_info());
    }

    #[test]
    fn test_dropped_column() {
        let expected = create_test_schema();
        let actual = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            // amount is missing
        ]);

        let drift = DriftDetection::detect("test_table", &expected, &actual, None);

        assert_eq!(drift.error_count(), 1);
        assert!(drift.has_errors());
        assert!(drift.diagnostics[0].code == DiagnosticCode::DriftColumnDropped);
        assert!(drift.diagnostics[0].message.contains("amount"));
    }

    #[test]
    fn test_type_change() {
        let expected = create_test_schema();
        let actual = Schema::from_columns(vec![
            Column::new("id", LogicalType::String), // Changed from Int
            Column::new("name", LogicalType::String),
            Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
        ]);

        let drift = DriftDetection::detect("test_table", &expected, &actual, None);

        assert_eq!(drift.error_count(), 1);
        assert!(drift.has_errors());
        assert!(drift.diagnostics[0].code == DiagnosticCode::DriftTypeChange);
        assert!(drift.diagnostics[0].message.contains("id"));
    }

    #[test]
    fn test_new_column() {
        let expected = create_test_schema();
        let actual = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
            Column::new("new_col", LogicalType::String), // New column
        ]);

        let drift = DriftDetection::detect("test_table", &expected, &actual, None);

        assert_eq!(drift.info_count(), 1);
        assert!(drift.has_info());
        assert!(!drift.has_errors());
        assert!(drift.diagnostics[0].code == DiagnosticCode::DriftColumnAdded);
        assert!(drift.diagnostics[0].message.contains("new_col"));
    }

    #[test]
    fn test_multiple_drifts() {
        let expected = create_test_schema();
        let actual = Schema::from_columns(vec![
            Column::new("id", LogicalType::String), // Type changed
            Column::new("name", LogicalType::String),
            // amount dropped
            Column::new("extra", LogicalType::Int), // New column
        ]);

        let drift = DriftDetection::detect("test_table", &expected, &actual, None);

        assert_eq!(drift.error_count(), 2); // type change + dropped column
        assert_eq!(drift.info_count(), 1);  // new column
        assert!(drift.has_errors());
        assert!(drift.has_info());
    }
}
