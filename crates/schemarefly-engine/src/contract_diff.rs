//! Contract diff engine for comparing inferred schemas against contracts
//!
//! This module implements the core contract validation logic that compares
//! inferred SQL output schemas against declared dbt contracts.

use schemarefly_core::{Schema, LogicalType, Diagnostic, DiagnosticCode, Severity, Location, Contract};
use std::collections::HashSet;

/// Result of comparing an inferred schema against a contract
#[derive(Debug, Clone)]
pub struct ContractDiff {
    /// The model being checked
    pub model_id: String,

    /// Expected schema from contract
    pub expected: Schema,

    /// Actual inferred schema
    pub actual: Schema,

    /// Diagnostics produced by the comparison
    pub diagnostics: Vec<Diagnostic>,
}

impl ContractDiff {
    /// Create a new contract diff by comparing inferred schema to contract
    pub fn compare(
        model_id: impl Into<String>,
        contract: &Contract,
        inferred: &Schema,
        file_path: Option<String>,
    ) -> Self {
        let model_id = model_id.into();
        let mut diagnostics = Vec::new();

        // Track which columns we've seen from the contract
        let mut seen_contract_cols = HashSet::new();

        // Check each column in the contract
        for expected_col in &contract.schema.columns {
            seen_contract_cols.insert(&expected_col.name);

            match inferred.find_column(&expected_col.name) {
                Some(actual_col) => {
                    // Column exists - check type match
                    if !types_compatible(&expected_col.logical_type, &actual_col.logical_type) {
                        let message = format!(
                            "Column '{}' type mismatch: expected {}, got {}",
                            expected_col.name,
                            expected_col.logical_type,
                            actual_col.logical_type
                        );

                        let mut diag = Diagnostic::new(
                            DiagnosticCode::ContractTypeMismatch,
                            Severity::Error,
                            message,
                        );

                        if let Some(ref path) = file_path {
                            diag = diag.with_location(Location::new(path.clone()));
                        }

                        diagnostics.push(diag);
                    }
                }
                None => {
                    // Column missing from inferred schema
                    let message = format!(
                        "Column '{}' required by contract but missing from inferred schema",
                        expected_col.name
                    );

                    let mut diag = Diagnostic::new(
                        DiagnosticCode::ContractMissingColumn,
                        Severity::Error,
                        message,
                    );

                    if let Some(ref path) = file_path {
                        diag = diag.with_location(Location::new(path.clone()));
                    }

                    diagnostics.push(diag);
                }
            }
        }

        // Check for extra columns in inferred schema
        for actual_col in &inferred.columns {
            if !seen_contract_cols.contains(&actual_col.name) {
                let message = format!(
                    "Column '{}' present in inferred schema but not declared in contract",
                    actual_col.name
                );

                let mut diag = Diagnostic::new(
                    DiagnosticCode::ContractExtraColumn,
                    Severity::Warn,
                    message,
                );

                if let Some(ref path) = file_path {
                    diag = diag.with_location(Location::new(path.clone()));
                }

                diagnostics.push(diag);
            }
        }

        Self {
            model_id,
            expected: contract.schema.clone(),
            actual: inferred.clone(),
            diagnostics,
        }
    }

    /// Check if the diff has any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// Check if the diff has any warnings
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Warn)
    }

    /// Get count of errors
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    /// Get count of warnings
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warn).count()
    }
}

/// Check if two types are compatible
///
/// This implements a lenient type compatibility check that allows for
/// certain type coercions common in SQL systems.
fn types_compatible(expected: &LogicalType, actual: &LogicalType) -> bool {
    match (expected, actual) {
        // Exact match
        (a, b) if a == b => true,

        // Unknown types are compatible with anything (inference limitation)
        (_, LogicalType::Unknown) | (LogicalType::Unknown, _) => true,

        // Numeric type compatibility
        (LogicalType::Int, LogicalType::Float) | (LogicalType::Float, LogicalType::Int) => true,

        // Decimal compatibility - allow if precision/scale differ but are both decimals
        (LogicalType::Decimal { .. }, LogicalType::Decimal { .. }) => true,

        // Int can be decimal
        (LogicalType::Decimal { .. }, LogicalType::Int) | (LogicalType::Int, LogicalType::Decimal { .. }) => true,

        // Array element type compatibility
        (LogicalType::Array { element_type: e1 }, LogicalType::Array { element_type: e2 }) => {
            types_compatible(e1, e2)
        }

        // Struct field compatibility (not implemented in detail yet)
        (LogicalType::Struct { .. }, LogicalType::Struct { .. }) => true,

        // No other implicit conversions
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_core::Column;

    fn create_test_contract() -> Contract {
        Contract::new(Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
        ]))
    }

    #[test]
    fn test_exact_match() {
        let contract = create_test_contract();
        let inferred = contract.schema.clone();

        let diff = ContractDiff::compare("test_model", &contract, &inferred, None);

        assert_eq!(diff.diagnostics.len(), 0);
        assert!(!diff.has_errors());
        assert!(!diff.has_warnings());
    }

    #[test]
    fn test_missing_column() {
        let contract = create_test_contract();
        let inferred = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            // amount is missing
        ]);

        let diff = ContractDiff::compare("test_model", &contract, &inferred, None);

        assert_eq!(diff.error_count(), 1);
        assert!(diff.has_errors());
        assert!(diff.diagnostics[0].code == DiagnosticCode::ContractMissingColumn);
    }

    #[test]
    fn test_type_mismatch() {
        let contract = create_test_contract();
        let inferred = Schema::from_columns(vec![
            Column::new("id", LogicalType::String), // Wrong type
            Column::new("name", LogicalType::String),
            Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
        ]);

        let diff = ContractDiff::compare("test_model", &contract, &inferred, None);

        assert_eq!(diff.error_count(), 1);
        assert!(diff.has_errors());
        assert!(diff.diagnostics[0].code == DiagnosticCode::ContractTypeMismatch);
    }

    #[test]
    fn test_extra_column() {
        let contract = create_test_contract();
        let inferred = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
            Column::new("extra_col", LogicalType::String), // Extra column
        ]);

        let diff = ContractDiff::compare("test_model", &contract, &inferred, None);

        assert_eq!(diff.warning_count(), 1);
        assert!(diff.has_warnings());
        assert!(!diff.has_errors());
        assert!(diff.diagnostics[0].code == DiagnosticCode::ContractExtraColumn);
    }

    #[test]
    fn test_type_compatibility() {
        // Int and Float are compatible
        assert!(types_compatible(&LogicalType::Int, &LogicalType::Float));
        assert!(types_compatible(&LogicalType::Float, &LogicalType::Int));

        // Decimals are compatible
        let dec1 = LogicalType::Decimal { precision: Some(10), scale: Some(2) };
        let dec2 = LogicalType::Decimal { precision: Some(20), scale: Some(4) };
        assert!(types_compatible(&dec1, &dec2));

        // Unknown is compatible with everything
        assert!(types_compatible(&LogicalType::Unknown, &LogicalType::Int));
        assert!(types_compatible(&LogicalType::String, &LogicalType::Unknown));

        // String and Int are not compatible
        assert!(!types_compatible(&LogicalType::String, &LogicalType::Int));
    }
}
