//! Warehouse adapter trait for fetching table schemas

use schemarefly_core::Schema;
use std::fmt;

/// Identifies a table in a warehouse
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdentifier {
    /// Database/project name
    pub database: String,

    /// Schema/dataset name
    pub schema: String,

    /// Table name
    pub table: String,
}

impl TableIdentifier {
    /// Create a new table identifier
    pub fn new(database: impl Into<String>, schema: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            database: database.into(),
            schema: schema.into(),
            table: table.into(),
        }
    }

    /// Get fully qualified name
    pub fn fqn(&self) -> String {
        format!("{}.{}.{}", self.database, self.schema, self.table)
    }
}

impl fmt::Display for TableIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fqn())
    }
}

/// Errors that can occur when fetching schemas
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Query failed: {0}")]
    QueryError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Trait for warehouse adapters that can fetch table schemas
#[async_trait::async_trait]
pub trait WarehouseAdapter: Send + Sync {
    /// Get the adapter name (e.g., "BigQuery", "Snowflake")
    fn name(&self) -> &'static str;

    /// Fetch the schema for a specific table
    ///
    /// This should query the warehouse's INFORMATION_SCHEMA to get
    /// column names and types for the specified table.
    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError>;

    /// Test the connection to the warehouse
    ///
    /// This is useful for validating credentials before attempting
    /// to fetch schemas.
    async fn test_connection(&self) -> Result<(), FetchError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_identifier() {
        let table = TableIdentifier::new("my_project", "my_dataset", "my_table");
        assert_eq!(table.database, "my_project");
        assert_eq!(table.schema, "my_dataset");
        assert_eq!(table.table, "my_table");
        assert_eq!(table.fqn(), "my_project.my_dataset.my_table");
        assert_eq!(table.to_string(), "my_project.my_dataset.my_table");
    }
}
