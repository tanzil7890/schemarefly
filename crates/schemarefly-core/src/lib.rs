//! SchemaRefly Core
//!
//! Core domain model with stable, versioned types.
//! Never rename diagnostic codes - they are part of the public API.

pub mod diagnostic;
pub mod schema;
pub mod report;
pub mod config;

pub use diagnostic::{Diagnostic, DiagnosticCode, Severity, Location};
pub use schema::{LogicalType, Column, Schema, Contract, Nullability, ColumnRef, EnforcementPolicy};
pub use report::{Report, ReportVersion};
pub use config::{Config, DialectConfig, SeverityThreshold, AllowlistRules};
