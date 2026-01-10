//! Snowflake warehouse adapter using INFORMATION_SCHEMA
//!
//! This adapter queries Snowflake's INFORMATION_SCHEMA.COLUMNS view to fetch
//! table schemas. It requires appropriate privileges:
//! - USAGE on the database and schema
//! - SELECT on INFORMATION_SCHEMA views
//!
//! ## Authentication Methods
//!
//! 1. Password authentication (username/password)
//! 2. Key-pair authentication (private key PEM)
//!
//! ## Usage
//!
//! ```rust,ignore
//! let adapter = SnowflakeAdapter::with_password(
//!     "xy12345.us-east-1",
//!     "username",
//!     "password"
//! )
//! .with_warehouse("COMPUTE_WH")
//! .with_role("ANALYST")
//! .build()?;
//! ```
//!
//! Reference: https://docs.snowflake.com/en/sql-reference/info-schema

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, Column, LogicalType, Nullability};

#[cfg(feature = "snowflake")]
use snowflake_api::SnowflakeApi;

#[cfg(feature = "snowflake")]
use arrow_array::cast::AsArray;

#[cfg(feature = "snowflake")]
use arrow_array::types::Int64Type;

#[cfg(feature = "snowflake")]
use arrow_array::Array;

/// Snowflake authentication credentials
#[derive(Clone)]
pub enum SnowflakeCredentials {
    /// Password-based authentication
    Password(String),
    /// Key-pair authentication (PEM format private key)
    PrivateKey(String),
}

/// Builder for SnowflakeAdapter
pub struct SnowflakeAdapterBuilder {
    account: String,
    username: String,
    credentials: SnowflakeCredentials,
    warehouse: Option<String>,
    role: Option<String>,
    database: Option<String>,
}

impl SnowflakeAdapterBuilder {
    /// Create new builder with password authentication
    pub fn with_password(
        account: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            account: account.into(),
            username: username.into(),
            credentials: SnowflakeCredentials::Password(password.into()),
            warehouse: None,
            role: None,
            database: None,
        }
    }

    /// Create new builder with key-pair authentication
    pub fn with_key_pair(
        account: impl Into<String>,
        username: impl Into<String>,
        private_key_pem: impl Into<String>,
    ) -> Self {
        Self {
            account: account.into(),
            username: username.into(),
            credentials: SnowflakeCredentials::PrivateKey(private_key_pem.into()),
            warehouse: None,
            role: None,
            database: None,
        }
    }

    /// Set the warehouse to use
    pub fn with_warehouse(mut self, warehouse: impl Into<String>) -> Self {
        self.warehouse = Some(warehouse.into());
        self
    }

    /// Set the role to use
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Set the default database
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }

    /// Build the adapter
    #[cfg(feature = "snowflake")]
    pub fn build(self) -> Result<SnowflakeAdapter, FetchError> {
        let api = match &self.credentials {
            SnowflakeCredentials::Password(password) => {
                SnowflakeApi::with_password_auth(
                    &self.account,
                    self.warehouse.as_deref(),
                    self.database.as_deref(),
                    None, // schema
                    &self.username,
                    self.role.as_deref(),
                    password,
                )
                .map_err(|e| FetchError::AuthenticationError(format!(
                    "Failed to authenticate with Snowflake: {}",
                    e
                )))?
            }
            SnowflakeCredentials::PrivateKey(private_key_pem) => {
                SnowflakeApi::with_certificate_auth(
                    &self.account,
                    self.warehouse.as_deref(),
                    self.database.as_deref(),
                    None, // schema
                    &self.username,
                    self.role.as_deref(),
                    private_key_pem,
                )
                .map_err(|e| FetchError::AuthenticationError(format!(
                    "Failed to authenticate with key-pair: {}",
                    e
                )))?
            }
        };

        Ok(SnowflakeAdapter {
            api,
            account: self.account,
            warehouse: self.warehouse,
            role: self.role,
        })
    }

    /// Build without snowflake feature
    #[cfg(not(feature = "snowflake"))]
    pub fn build(self) -> Result<SnowflakeAdapter, FetchError> {
        Err(FetchError::ConfigError(
            "Snowflake support not compiled. Rebuild with: cargo build --features snowflake".to_string()
        ))
    }
}

/// Snowflake warehouse adapter
pub struct SnowflakeAdapter {
    #[cfg(feature = "snowflake")]
    api: SnowflakeApi,

    account: String,
    warehouse: Option<String>,
    role: Option<String>,

    #[cfg(not(feature = "snowflake"))]
    _phantom: std::marker::PhantomData<()>,
}

impl SnowflakeAdapter {
    /// Create a new Snowflake adapter with password authentication (returns builder)
    pub fn new(
        account: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> SnowflakeAdapterBuilder {
        SnowflakeAdapterBuilder::with_password(account, username, password)
    }

    /// Builder pattern entry point
    pub fn builder() -> SnowflakeAdapterBuilderInit {
        SnowflakeAdapterBuilderInit
    }

    /// Convert Snowflake type to LogicalType
    pub fn map_snowflake_type(sf_type: &str) -> LogicalType {
        // Snowflake types can include precision/scale like "NUMBER(38,0)"
        let base_type = sf_type.split('(').next()
            .unwrap_or(sf_type)
            .trim()
            .to_uppercase();

        match base_type.as_str() {
            "BOOLEAN" => LogicalType::Bool,

            "NUMBER" | "DECIMAL" | "NUMERIC" => {
                // Check if it's effectively an integer (scale = 0)
                if Self::is_integer_number(sf_type) {
                    LogicalType::Int
                } else {
                    Self::parse_decimal_type(sf_type)
                }
            }

            "INT" | "INTEGER" | "BIGINT" | "SMALLINT" | "TINYINT" | "BYTEINT" => {
                LogicalType::Int
            }

            "FLOAT" | "FLOAT4" | "FLOAT8" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => {
                LogicalType::Float
            }

            "VARCHAR" | "STRING" | "TEXT" | "CHAR" | "CHARACTER" | "NVARCHAR" | "NCHAR" => {
                LogicalType::String
            }

            "BINARY" | "VARBINARY" => LogicalType::String,

            "DATE" => LogicalType::Date,

            "DATETIME" | "TIMESTAMP" | "TIMESTAMP_NTZ" | "TIMESTAMP_LTZ" | "TIMESTAMP_TZ" => {
                LogicalType::Timestamp
            }

            "TIME" => LogicalType::Timestamp,

            "VARIANT" | "OBJECT" => LogicalType::Json,

            "ARRAY" => LogicalType::Array {
                element_type: Box::new(LogicalType::Unknown),
            },

            "GEOGRAPHY" | "GEOMETRY" => LogicalType::String,

            _ => LogicalType::Unknown,
        }
    }

    /// Check if NUMBER type is effectively an integer
    fn is_integer_number(type_str: &str) -> bool {
        // NUMBER without parameters or with scale 0 is integer
        if !type_str.contains('(') {
            return false; // Bare NUMBER is treated as decimal
        }

        if let Some(params) = type_str.split('(').nth(1) {
            if let Some(params) = params.strip_suffix(')') {
                let parts: Vec<&str> = params.split(',').collect();
                if parts.len() == 2 {
                    if let Ok(scale) = parts[1].trim().parse::<i32>() {
                        return scale == 0;
                    }
                } else if parts.len() == 1 {
                    // NUMBER(precision) with no scale defaults to 0
                    return true;
                }
            }
        }

        false
    }

    /// Parse decimal type with precision and scale
    fn parse_decimal_type(type_str: &str) -> LogicalType {
        if let Some(params) = type_str.split('(').nth(1) {
            if let Some(params) = params.strip_suffix(')') {
                let parts: Vec<&str> = params.split(',').collect();
                if parts.len() == 2 {
                    let precision = parts[0].trim().parse().ok();
                    let scale = parts[1].trim().parse().ok();
                    return LogicalType::Decimal { precision, scale };
                } else if parts.len() == 1 {
                    let precision = parts[0].trim().parse().ok();
                    return LogicalType::Decimal { precision, scale: Some(0) };
                }
            }
        }

        // Default Snowflake NUMBER precision
        LogicalType::Decimal {
            precision: Some(38),
            scale: Some(0),
        }
    }
}

/// Empty struct for builder pattern initialization
pub struct SnowflakeAdapterBuilderInit;

impl SnowflakeAdapterBuilderInit {
    pub fn with_password(
        self,
        account: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> SnowflakeAdapterBuilder {
        SnowflakeAdapterBuilder::with_password(account, username, password)
    }

    pub fn with_key_pair(
        self,
        account: impl Into<String>,
        username: impl Into<String>,
        private_key_pem: impl Into<String>,
    ) -> SnowflakeAdapterBuilder {
        SnowflakeAdapterBuilder::with_key_pair(account, username, private_key_pem)
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for SnowflakeAdapter {
    fn name(&self) -> &'static str {
        "Snowflake"
    }

    #[cfg(feature = "snowflake")]
    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        use snowflake_api::QueryResult;

        // Build the INFORMATION_SCHEMA query
        // Snowflake requires uppercase for table/schema names in INFORMATION_SCHEMA
        let query = format!(
            r#"
            SELECT
                COLUMN_NAME,
                DATA_TYPE,
                IS_NULLABLE,
                ORDINAL_POSITION,
                NUMERIC_PRECISION,
                NUMERIC_SCALE
            FROM {}.INFORMATION_SCHEMA.COLUMNS
            WHERE TABLE_SCHEMA = '{}'
              AND TABLE_NAME = '{}'
            ORDER BY ORDINAL_POSITION
            "#,
            table.database,
            table.schema.to_uppercase(),
            table.table.to_uppercase()
        );

        let result = self.api.exec(&query)
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("does not exist") || err_str.contains("not found") {
                    FetchError::TableNotFound(table.fqn())
                } else if err_str.contains("Insufficient privileges") || err_str.contains("Permission") {
                    FetchError::PermissionDenied(format!(
                        "Cannot access {}: {}",
                        table.fqn(), err_str
                    ))
                } else {
                    FetchError::QueryError(err_str)
                }
            })?;

        // Parse results - handle Arrow format
        let mut columns = Vec::new();

        match result {
            QueryResult::Arrow(batches) => {
                // Process each record batch
                for batch in batches {
                    let num_rows = batch.num_rows();
                    let schema = batch.schema();

                    // Get column indices
                    let col_name_idx = schema.index_of("COLUMN_NAME")
                        .map_err(|_| FetchError::InvalidResponse("Missing COLUMN_NAME column".to_string()))?;
                    let data_type_idx = schema.index_of("DATA_TYPE")
                        .map_err(|_| FetchError::InvalidResponse("Missing DATA_TYPE column".to_string()))?;
                    let is_nullable_idx = schema.index_of("IS_NULLABLE")
                        .map_err(|_| FetchError::InvalidResponse("Missing IS_NULLABLE column".to_string()))?;
                    let precision_idx = schema.index_of("NUMERIC_PRECISION").ok();
                    let scale_idx = schema.index_of("NUMERIC_SCALE").ok();

                    // Get column arrays
                    let col_name_array = batch.column(col_name_idx).as_string::<i32>();
                    let data_type_array = batch.column(data_type_idx).as_string::<i32>();
                    let is_nullable_array = batch.column(is_nullable_idx).as_string::<i32>();

                    // Process each row
                    for row_idx in 0..num_rows {
                        let col_name = col_name_array.value(row_idx).to_string();
                        let data_type = data_type_array.value(row_idx);
                        let is_nullable = is_nullable_array.value(row_idx);

                        // Build full type with precision/scale for numeric types
                        let full_type = if data_type == "NUMBER" {
                            let precision = precision_idx
                                .and_then(|idx| batch.column(idx).as_primitive_opt::<Int64Type>())
                                .and_then(|arr| if arr.is_null(row_idx) { None } else { Some(arr.value(row_idx)) });

                            let scale = scale_idx
                                .and_then(|idx| batch.column(idx).as_primitive_opt::<Int64Type>())
                                .and_then(|arr| if arr.is_null(row_idx) { None } else { Some(arr.value(row_idx)) });

                            match (precision, scale) {
                                (Some(p), Some(s)) => format!("NUMBER({},{})", p, s),
                                (Some(p), None) => format!("NUMBER({})", p),
                                _ => data_type.to_string(),
                            }
                        } else {
                            data_type.to_string()
                        };

                        let logical_type = Self::map_snowflake_type(&full_type);
                        let nullable = match is_nullable.to_uppercase().as_str() {
                            "YES" => Nullability::Yes,
                            "NO" => Nullability::No,
                            _ => Nullability::Unknown,
                        };

                        columns.push(
                            Column::new(col_name, logical_type)
                                .with_nullability(nullable)
                        );
                    }
                }
            }
            QueryResult::Json(_) => {
                return Err(FetchError::InvalidResponse(
                    "Unexpected JSON result format".to_string()
                ));
            }
            QueryResult::Empty => {
                return Err(FetchError::TableNotFound(format!(
                    "Table {} not found or has no columns",
                    table.fqn()
                )));
            }
        }

        if columns.is_empty() {
            return Err(FetchError::TableNotFound(format!(
                "Table {} not found or has no columns",
                table.fqn()
            )));
        }

        Ok(Schema::from_columns(columns))
    }

    #[cfg(not(feature = "snowflake"))]
    async fn fetch_schema(&self, _table: &TableIdentifier) -> Result<Schema, FetchError> {
        Err(FetchError::ConfigError(
            "Snowflake support not compiled. Rebuild with: cargo build --features snowflake".to_string()
        ))
    }

    #[cfg(feature = "snowflake")]
    async fn test_connection(&self) -> Result<(), FetchError> {
        self.api.exec("SELECT 1")
            .await
            .map_err(|e| FetchError::QueryError(format!("Connection test failed: {}", e)))?;
        Ok(())
    }

    #[cfg(not(feature = "snowflake"))]
    async fn test_connection(&self) -> Result<(), FetchError> {
        Err(FetchError::ConfigError(
            "Snowflake support not compiled. Rebuild with: cargo build --features snowflake".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mapping() {
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("NUMBER(38,0)"), LogicalType::Int));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("NUMBER(10,2)"), LogicalType::Decimal { .. }));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("VARCHAR"), LogicalType::String));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("BOOLEAN"), LogicalType::Bool));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("TIMESTAMP_NTZ"), LogicalType::Timestamp));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("VARIANT"), LogicalType::Json));
    }

    #[test]
    fn test_integer_number_detection() {
        assert!(SnowflakeAdapter::is_integer_number("NUMBER(38,0)"));
        assert!(SnowflakeAdapter::is_integer_number("NUMBER(10,0)"));
        assert!(SnowflakeAdapter::is_integer_number("NUMBER(10)"));
        assert!(!SnowflakeAdapter::is_integer_number("NUMBER(10,2)"));
        assert!(!SnowflakeAdapter::is_integer_number("NUMBER")); // Bare NUMBER
    }

    #[test]
    fn test_parse_decimal_type() {
        match SnowflakeAdapter::parse_decimal_type("NUMBER(10,2)") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(10));
                assert_eq!(scale, Some(2));
            }
            _ => panic!("Expected Decimal type"),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let builder = SnowflakeAdapter::new("account", "user", "pass");
        // Builder pattern - can't test without actual credentials
    }
}
