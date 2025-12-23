//! Snowflake warehouse adapter using INFORMATION_SCHEMA
//!
//! This adapter queries Snowflake's INFORMATION_SCHEMA.COLUMNS view to fetch
//! table schemas. It requires appropriate privileges:
//! - USAGE on the database and schema
//! - SELECT on INFORMATION_SCHEMA views
//!
//! Reference: https://docs.snowflake.com/en/sql-reference/info-schema

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, LogicalType};

/// Snowflake warehouse adapter
pub struct SnowflakeAdapter {
    /// Account identifier (e.g., "xy12345.us-east-1")
    account: String,

    /// Username for authentication
    username: String,

    /// Password or private key for authentication
    credentials: SnowflakeCredentials,

    /// Warehouse to use for queries
    warehouse: Option<String>,

    /// Role to use
    role: Option<String>,
}

/// Snowflake authentication credentials
pub enum SnowflakeCredentials {
    /// Password-based authentication
    Password(String),

    /// Key-pair authentication (PEM format private key)
    PrivateKey(String),

    /// OAuth token
    OAuth(String),
}

impl SnowflakeAdapter {
    /// Create a new Snowflake adapter with password authentication
    pub fn new(
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

    /// Convert Snowflake type to LogicalType
    fn map_snowflake_type(sf_type: &str) -> LogicalType {
        // Snowflake types can include precision/scale like "NUMBER(38,0)"
        let base_type = sf_type.split('(').next().unwrap_or(sf_type).to_uppercase();

        match base_type.as_str() {
            "BOOLEAN" => LogicalType::Bool,
            "NUMBER" | "INT" | "INTEGER" | "BIGINT" | "SMALLINT" | "TINYINT" | "BYTEINT" => {
                // If it has scale 0, treat as Int, otherwise as Decimal
                if sf_type.contains(",0)") || !sf_type.contains('(') {
                    LogicalType::Int
                } else {
                    // Extract precision and scale if present
                    Self::parse_decimal_type(sf_type)
                }
            }
            "FLOAT" | "FLOAT4" | "FLOAT8" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => {
                LogicalType::Float
            }
            "VARCHAR" | "STRING" | "TEXT" | "CHAR" | "CHARACTER" => LogicalType::String,
            "BINARY" | "VARBINARY" => LogicalType::String, // Map to string
            "DATE" => LogicalType::Date,
            "DATETIME" | "TIMESTAMP" | "TIMESTAMP_NTZ" | "TIMESTAMP_LTZ" | "TIMESTAMP_TZ" => {
                LogicalType::Timestamp
            }
            "TIME" => LogicalType::Timestamp, // Map to timestamp
            "VARIANT" => LogicalType::Json,
            "OBJECT" => LogicalType::Struct { fields: vec![] },
            "ARRAY" => LogicalType::Array {
                element_type: Box::new(LogicalType::Unknown),
            },
            "GEOGRAPHY" | "GEOMETRY" => LogicalType::String, // Map to string
            _ => LogicalType::Unknown,
        }
    }

    /// Parse decimal type with precision and scale
    fn parse_decimal_type(type_str: &str) -> LogicalType {
        // Extract precision and scale from "NUMBER(38,2)" format
        if let Some(params) = type_str.split('(').nth(1) {
            if let Some(params) = params.strip_suffix(')') {
                let parts: Vec<&str> = params.split(',').collect();
                if parts.len() == 2 {
                    let precision = parts[0].trim().parse().ok();
                    let scale = parts[1].trim().parse().ok();
                    return LogicalType::Decimal { precision, scale };
                }
            }
        }
        LogicalType::Decimal {
            precision: Some(38),
            scale: Some(0),
        }
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for SnowflakeAdapter {
    fn name(&self) -> &'static str {
        "Snowflake"
    }

    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        // Build the INFORMATION_SCHEMA query
        let _query = format!(
            r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                ordinal_position
            FROM {}.INFORMATION_SCHEMA.COLUMNS
            WHERE table_schema = '{}'
              AND table_name = '{}'
            ORDER BY ordinal_position
            "#,
            table.database,
            table.schema,
            table.table
        );

        // In a real implementation, this would:
        // 1. Create a Snowflake connection using credentials
        // 2. Execute the query
        // 3. Parse results into Schema
        //
        // For now, we return a placeholder error indicating this needs
        // actual Snowflake driver integration

        Err(FetchError::ConfigError(
            "Snowflake adapter requires snowflake-connector dependency. \
             Install with: cargo add snowflake-api".to_string()
        ))

        // Example of what the real implementation would look like:
        //
        // use snowflake_api::{SnowflakeApi, QueryResult};
        //
        // let mut conn = SnowflakeApi::with_password_auth(
        //     &self.account,
        //     &self.username,
        //     match &self.credentials {
        //         SnowflakeCredentials::Password(p) => p,
        //         _ => return Err(FetchError::AuthenticationError(
        //             "Only password auth implemented".to_string()
        //         )),
        //     },
        //     self.warehouse.as_deref(),
        //     self.role.as_deref(),
        // )
        // .await
        // .map_err(|e| FetchError::AuthenticationError(e.to_string()))?;
        //
        // let result = conn.exec(&query).await
        //     .map_err(|e| FetchError::QueryError(e.to_string()))?;
        //
        // let mut columns = Vec::new();
        // for row in result.rows() {
        //     let col_name = row.get::<String>(0)?;
        //     let data_type = row.get::<String>(1)?;
        //
        //     let logical_type = Self::map_snowflake_type(&data_type);
        //     columns.push(Column::new(col_name, logical_type));
        // }
        //
        // Ok(Schema::from_columns(columns))
    }

    async fn test_connection(&self) -> Result<(), FetchError> {
        // In a real implementation, this would test the Snowflake connection
        // For now, return config error
        Err(FetchError::ConfigError(
            "Snowflake adapter requires snowflake-api dependency".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mapping() {
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("NUMBER(38,0)"), LogicalType::Int));
        assert!(matches!(
            SnowflakeAdapter::map_snowflake_type("NUMBER(38,2)"),
            LogicalType::Decimal { .. }
        ));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("VARCHAR"), LogicalType::String));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("BOOLEAN"), LogicalType::Bool));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("TIMESTAMP"), LogicalType::Timestamp));
        assert!(matches!(SnowflakeAdapter::map_snowflake_type("VARIANT"), LogicalType::Json));
    }

    #[test]
    fn test_parse_decimal_type() {
        let decimal = SnowflakeAdapter::parse_decimal_type("NUMBER(10,2)");
        assert!(matches!(
            decimal,
            LogicalType::Decimal {
                precision: Some(10),
                scale: Some(2)
            }
        ));

        let decimal2 = SnowflakeAdapter::parse_decimal_type("NUMBER(38,4)");
        assert!(matches!(
            decimal2,
            LogicalType::Decimal {
                precision: Some(38),
                scale: Some(4)
            }
        ));
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = SnowflakeAdapter::new("my-account.us-east-1", "user", "pass")
            .with_warehouse("COMPUTE_WH")
            .with_role("ANALYST");

        assert_eq!(adapter.name(), "Snowflake");
        assert_eq!(adapter.account, "my-account.us-east-1");
        assert_eq!(adapter.username, "user");
        assert_eq!(adapter.warehouse, Some("COMPUTE_WH".to_string()));
        assert_eq!(adapter.role, Some("ANALYST".to_string()));
    }
}
