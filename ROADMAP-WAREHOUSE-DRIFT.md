# Warehouse Drift Mode - Complete Implementation Guide

## Executive Summary

This guide provides a complete, phase-by-phase implementation plan to finish the Warehouse Drift Mode feature in SchemaRefly. The feature is **80% complete** - all core logic exists, and all three warehouse adapters (BigQuery, Snowflake, PostgreSQL) are implemented.

### Current State Analysis

| Component | Status | Location |
|-----------|--------|----------|
| Drift Detection Engine | ✅ Complete | `crates/schemarefly-engine/src/drift_detector.rs` |
| CLI Command (`schemarefly drift`) | ✅ Complete | `crates/schemarefly-cli/src/main.rs` |
| Diagnostic Codes (DRIFT_*) | ✅ Complete | `crates/schemarefly-core/src/diagnostic.rs` |
| Adapter Interface | ✅ Complete | `crates/schemarefly-catalog/src/adapter.rs` |
| **BigQuery Adapter** | **✅ Complete** | `crates/schemarefly-catalog/src/bigquery.rs` |
| **Snowflake Adapter** | **✅ Complete** | `crates/schemarefly-catalog/src/snowflake.rs` |
| **PostgreSQL Adapter** | **✅ Complete** | `crates/schemarefly-catalog/src/postgres.rs` |
| Configuration Schema | ⚠️ Partial | `crates/schemarefly-core/src/config.rs` |
| Integration Tests | ❌ Missing | Needs mock adapters |
| Documentation | ❌ Missing | Needs user guide |

### Implementation Status

| Phase | Status | Commit |
|-------|--------|--------|
| **Phase 1: BigQuery Adapter** | **✅ COMPLETED** | `2de69b0` |
| **Phase 2: Snowflake Adapter** | **✅ COMPLETED** | `66ae073` |
| **Phase 3: PostgreSQL Adapter** | **✅ COMPLETED** | TBD |
| Phase 4: Enhanced Configuration | ⏳ Pending | - |
| Phase 5: Mock Adapter | ⏳ Pending | - |
| Phase 6: Integration Tests | ⏳ Pending | - |
| Phase 7: Documentation | ⏳ Pending | - |

### What Needs to Be Done

1. **Phase 1**: ~~Add SDK dependencies and implement BigQuery adapter~~ **✅ COMPLETED**
2. **Phase 2**: ~~Implement Snowflake adapter~~ **✅ COMPLETED**
3. **Phase 3**: ~~Add PostgreSQL adapter~~ **✅ COMPLETED**
4. **Phase 4**: Enhance configuration with environment variables
5. **Phase 5**: Add mock adapter for testing
6. **Phase 6**: Integration tests
7. **Phase 7**: Documentation and examples

---

## Phase 1: BigQuery Adapter Implementation

### 1.1 Add Dependencies

**File**: `Cargo.toml` (workspace root)

```toml
[workspace.dependencies]
# Add BigQuery SDK
gcp-bigquery-client = "0.18"
tokio = { version = "1.34", features = ["full"] }
```

**File**: `crates/schemarefly-catalog/Cargo.toml`

```toml
[dependencies]
schemarefly-core.workspace = true
async-trait.workspace = true
thiserror.workspace = true
tokio.workspace = true

# BigQuery SDK (optional feature)
gcp-bigquery-client = { workspace = true, optional = true }

# For environment variable parsing
dotenvy = "0.15"

[features]
default = []
bigquery = ["gcp-bigquery-client"]
snowflake = []
postgres = []
all-warehouses = ["bigquery", "snowflake", "postgres"]
```

### 1.2 Implement BigQuery Adapter

**File**: `crates/schemarefly-catalog/src/bigquery.rs`

Replace the existing file with:

```rust
//! BigQuery warehouse adapter using INFORMATION_SCHEMA.COLUMNS
//!
//! This adapter queries BigQuery's INFORMATION_SCHEMA.COLUMNS view to fetch
//! table schemas. It requires appropriate IAM permissions:
//! - bigquery.tables.get
//! - bigquery.tables.getData (for INFORMATION_SCHEMA access)
//!
//! ## Authentication
//!
//! The adapter supports multiple authentication methods:
//! 1. Service account JSON file (explicit path)
//! 2. Service account JSON content (inline)
//! 3. Application Default Credentials (ADC)
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Using ADC
//! let adapter = BigQueryAdapter::with_adc("my-project").await?;
//!
//! // Using service account file
//! let adapter = BigQueryAdapter::from_service_account_file(
//!     "my-project",
//!     "/path/to/service-account.json"
//! ).await?;
//! ```

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, Column, LogicalType, Nullability};

#[cfg(feature = "bigquery")]
use gcp_bigquery_client::Client as BigQueryClient;

/// BigQuery warehouse adapter
pub struct BigQueryAdapter {
    /// Project ID
    project_id: String,

    /// BigQuery client (only available with bigquery feature)
    #[cfg(feature = "bigquery")]
    client: BigQueryClient,

    /// Placeholder for when feature is disabled
    #[cfg(not(feature = "bigquery"))]
    _phantom: std::marker::PhantomData<()>,
}

impl BigQueryAdapter {
    /// Create a new BigQuery adapter using Application Default Credentials (ADC)
    ///
    /// ADC automatically detects credentials from:
    /// - GOOGLE_APPLICATION_CREDENTIALS environment variable
    /// - gcloud CLI default credentials
    /// - GCE/GKE metadata service
    #[cfg(feature = "bigquery")]
    pub async fn with_adc(project_id: impl Into<String>) -> Result<Self, FetchError> {
        let project_id = project_id.into();

        let client = BigQueryClient::from_application_default_credentials()
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to authenticate with ADC: {}. \
                 Ensure GOOGLE_APPLICATION_CREDENTIALS is set or run 'gcloud auth application-default login'",
                e
            )))?;

        Ok(Self {
            project_id,
            client,
        })
    }

    /// Create adapter without bigquery feature (returns error)
    #[cfg(not(feature = "bigquery"))]
    pub async fn with_adc(project_id: impl Into<String>) -> Result<Self, FetchError> {
        let _ = project_id;
        Err(FetchError::ConfigError(
            "BigQuery support not compiled. Rebuild with: cargo build --features bigquery".to_string()
        ))
    }

    /// Create a new BigQuery adapter using a service account key file
    #[cfg(feature = "bigquery")]
    pub async fn from_service_account_file(
        project_id: impl Into<String>,
        key_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, FetchError> {
        let project_id = project_id.into();
        let key_path = key_path.as_ref();

        // Read the service account key file
        let key_content = std::fs::read_to_string(key_path)
            .map_err(|e| FetchError::ConfigError(format!(
                "Failed to read service account key file '{}': {}",
                key_path.display(), e
            )))?;

        Self::from_service_account_json(project_id, &key_content).await
    }

    /// Create adapter without bigquery feature (returns error)
    #[cfg(not(feature = "bigquery"))]
    pub async fn from_service_account_file(
        project_id: impl Into<String>,
        _key_path: impl AsRef<std::path::Path>,
    ) -> Result<Self, FetchError> {
        let _ = project_id;
        Err(FetchError::ConfigError(
            "BigQuery support not compiled. Rebuild with: cargo build --features bigquery".to_string()
        ))
    }

    /// Create a new BigQuery adapter using service account JSON content
    #[cfg(feature = "bigquery")]
    pub async fn from_service_account_json(
        project_id: impl Into<String>,
        key_json: &str,
    ) -> Result<Self, FetchError> {
        let project_id = project_id.into();

        let client = BigQueryClient::from_service_account_key_json(key_json)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to authenticate with service account: {}",
                e
            )))?;

        Ok(Self {
            project_id,
            client,
        })
    }

    /// Create adapter without bigquery feature (returns error)
    #[cfg(not(feature = "bigquery"))]
    pub async fn from_service_account_json(
        project_id: impl Into<String>,
        _key_json: &str,
    ) -> Result<Self, FetchError> {
        let _ = project_id;
        Err(FetchError::ConfigError(
            "BigQuery support not compiled. Rebuild with: cargo build --features bigquery".to_string()
        ))
    }

    /// Placeholder constructor for backward compatibility
    pub fn new(project_id: impl Into<String>, _credentials: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            #[cfg(feature = "bigquery")]
            client: panic!("Use async constructors: with_adc() or from_service_account_file()"),
            #[cfg(not(feature = "bigquery"))]
            _phantom: std::marker::PhantomData,
        }
    }

    /// Convert BigQuery type to LogicalType
    pub fn map_bigquery_type(bq_type: &str) -> LogicalType {
        // Handle parameterized types like NUMERIC(10,2) or ARRAY<STRING>
        let base_type = bq_type.split('(').next()
            .unwrap_or(bq_type)
            .split('<').next()
            .unwrap_or(bq_type)
            .trim()
            .to_uppercase();

        match base_type.as_str() {
            "BOOL" | "BOOLEAN" => LogicalType::Bool,

            "INT64" | "INTEGER" | "INT" | "SMALLINT" | "TINYINT" | "BYTEINT" => LogicalType::Int,

            "FLOAT64" | "FLOAT" => LogicalType::Float,

            "NUMERIC" | "BIGNUMERIC" | "DECIMAL" => {
                // Extract precision and scale if present
                Self::parse_numeric_type(bq_type)
            }

            "STRING" => LogicalType::String,
            "BYTES" => LogicalType::String, // Map to string for compatibility

            "DATE" => LogicalType::Date,
            "DATETIME" | "TIMESTAMP" => LogicalType::Timestamp,
            "TIME" => LogicalType::Timestamp, // Map to timestamp

            "GEOGRAPHY" => LogicalType::String, // GeoJSON string
            "JSON" => LogicalType::Json,

            "ARRAY" => {
                // Extract element type from ARRAY<TYPE>
                let element_type = Self::extract_array_element_type(bq_type);
                LogicalType::Array {
                    element_type: Box::new(element_type),
                }
            }

            "STRUCT" | "RECORD" => LogicalType::Struct { fields: vec![] },

            _ => LogicalType::Unknown,
        }
    }

    /// Parse NUMERIC(precision, scale) type
    fn parse_numeric_type(type_str: &str) -> LogicalType {
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

        // Default for NUMERIC without parameters
        LogicalType::Decimal {
            precision: Some(38),
            scale: Some(9),
        }
    }

    /// Extract element type from ARRAY<TYPE>
    fn extract_array_element_type(type_str: &str) -> LogicalType {
        if let Some(start) = type_str.find('<') {
            if let Some(end) = type_str.rfind('>') {
                let element_type_str = &type_str[start + 1..end];
                return Self::map_bigquery_type(element_type_str);
            }
        }
        LogicalType::Unknown
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for BigQueryAdapter {
    fn name(&self) -> &'static str {
        "BigQuery"
    }

    #[cfg(feature = "bigquery")]
    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        use gcp_bigquery_client::model::query_request::QueryRequest;

        // Build the INFORMATION_SCHEMA query
        let query = format!(
            r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                ordinal_position
            FROM `{}.{}.INFORMATION_SCHEMA.COLUMNS`
            WHERE table_name = '{}'
            ORDER BY ordinal_position
            "#,
            table.database,
            table.schema,
            table.table
        );

        // Execute query
        let request = QueryRequest::new(&query);
        let result = self.client
            .job()
            .query(&self.project_id, request)
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("Not found") {
                    FetchError::TableNotFound(table.fqn())
                } else if err_str.contains("Access Denied") || err_str.contains("Permission") {
                    FetchError::PermissionDenied(format!(
                        "Cannot access {}: {}",
                        table.fqn(), err_str
                    ))
                } else {
                    FetchError::QueryError(err_str)
                }
            })?;

        // Parse results
        let mut columns = Vec::new();

        if let Some(rows) = result.query_response().rows.as_ref() {
            for row in rows {
                if let Some(cells) = &row.f {
                    if cells.len() >= 3 {
                        let col_name = cells[0].v.as_ref()
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let data_type = cells[1].v.as_ref()
                            .and_then(|v| v.as_str())
                            .unwrap_or("UNKNOWN");

                        let is_nullable = cells[2].v.as_ref()
                            .and_then(|v| v.as_str())
                            .unwrap_or("YES");

                        let logical_type = Self::map_bigquery_type(data_type);
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
        }

        if columns.is_empty() {
            return Err(FetchError::TableNotFound(format!(
                "Table {} not found or has no columns",
                table.fqn()
            )));
        }

        Ok(Schema::from_columns(columns))
    }

    #[cfg(not(feature = "bigquery"))]
    async fn fetch_schema(&self, _table: &TableIdentifier) -> Result<Schema, FetchError> {
        Err(FetchError::ConfigError(
            "BigQuery support not compiled. Rebuild with: cargo build --features bigquery".to_string()
        ))
    }

    #[cfg(feature = "bigquery")]
    async fn test_connection(&self) -> Result<(), FetchError> {
        use gcp_bigquery_client::model::query_request::QueryRequest;

        // Simple query to test connection
        let query = "SELECT 1";
        let request = QueryRequest::new(query);

        self.client
            .job()
            .query(&self.project_id, request)
            .await
            .map_err(|e| FetchError::QueryError(format!("Connection test failed: {}", e)))?;

        Ok(())
    }

    #[cfg(not(feature = "bigquery"))]
    async fn test_connection(&self) -> Result<(), FetchError> {
        Err(FetchError::ConfigError(
            "BigQuery support not compiled. Rebuild with: cargo build --features bigquery".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mapping() {
        assert!(matches!(BigQueryAdapter::map_bigquery_type("INT64"), LogicalType::Int));
        assert!(matches!(BigQueryAdapter::map_bigquery_type("STRING"), LogicalType::String));
        assert!(matches!(BigQueryAdapter::map_bigquery_type("BOOL"), LogicalType::Bool));
        assert!(matches!(BigQueryAdapter::map_bigquery_type("TIMESTAMP"), LogicalType::Timestamp));
        assert!(matches!(BigQueryAdapter::map_bigquery_type("JSON"), LogicalType::Json));
        assert!(matches!(BigQueryAdapter::map_bigquery_type("FLOAT64"), LogicalType::Float));
    }

    #[test]
    fn test_numeric_type_parsing() {
        match BigQueryAdapter::map_bigquery_type("NUMERIC(10,2)") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(10));
                assert_eq!(scale, Some(2));
            }
            _ => panic!("Expected Decimal type"),
        }

        match BigQueryAdapter::map_bigquery_type("BIGNUMERIC") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(38));
                assert_eq!(scale, Some(9));
            }
            _ => panic!("Expected Decimal type"),
        }
    }

    #[test]
    fn test_array_type_parsing() {
        match BigQueryAdapter::map_bigquery_type("ARRAY<STRING>") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::String));
            }
            _ => panic!("Expected Array type"),
        }

        match BigQueryAdapter::map_bigquery_type("ARRAY<INT64>") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::Int));
            }
            _ => panic!("Expected Array type"),
        }
    }
}
```

### 1.3 Update Feature Flags

**File**: `crates/schemarefly-cli/Cargo.toml`

Add feature flags:

```toml
[features]
default = []
bigquery = ["schemarefly-catalog/bigquery"]
snowflake = ["schemarefly-catalog/snowflake"]
postgres = ["schemarefly-catalog/postgres"]
all-warehouses = ["bigquery", "snowflake", "postgres"]
```

### 1.4 Test BigQuery Integration

Create a test script:

```bash
#!/bin/bash
# test-bigquery.sh

# Set credentials
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

# Build with BigQuery support
cargo build --features bigquery

# Run drift detection
./target/debug/schemarefly drift --verbose
```

---

## Phase 2: Snowflake Adapter Implementation

### 2.1 Add Dependencies

**File**: `Cargo.toml` (workspace root)

```toml
[workspace.dependencies]
# Add Snowflake SDK
snowflake-api = "0.7"
```

**File**: `crates/schemarefly-catalog/Cargo.toml`

```toml
[dependencies]
# ... existing deps

# Snowflake SDK (optional feature)
snowflake-api = { workspace = true, optional = true }

[features]
# ... existing
snowflake = ["snowflake-api"]
```

### 2.2 Implement Snowflake Adapter

**File**: `crates/schemarefly-catalog/src/snowflake.rs`

Replace with:

```rust
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
//! 2. Key-pair authentication (private key)
//! 3. OAuth (external browser or token)
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
//! .build()
//! .await?;
//! ```

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, Column, LogicalType, Nullability};

#[cfg(feature = "snowflake")]
use snowflake_api::{SnowflakeApi, QueryResult};

/// Snowflake authentication credentials
#[derive(Clone)]
pub enum SnowflakeCredentials {
    /// Password-based authentication
    Password(String),
    /// Key-pair authentication (PEM format private key)
    PrivateKey {
        private_key_pem: String,
        passphrase: Option<String>,
    },
    /// OAuth token
    OAuth(String),
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
        passphrase: Option<String>,
    ) -> Self {
        Self {
            account: account.into(),
            username: username.into(),
            credentials: SnowflakeCredentials::PrivateKey {
                private_key_pem: private_key_pem.into(),
                passphrase,
            },
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
    pub async fn build(self) -> Result<SnowflakeAdapter, FetchError> {
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
                .await
                .map_err(|e| FetchError::AuthenticationError(format!(
                    "Failed to authenticate with Snowflake: {}",
                    e
                )))?
            }
            SnowflakeCredentials::PrivateKey { private_key_pem, passphrase } => {
                SnowflakeApi::with_certificate_auth(
                    &self.account,
                    self.warehouse.as_deref(),
                    self.database.as_deref(),
                    None, // schema
                    &self.username,
                    self.role.as_deref(),
                    private_key_pem,
                    passphrase.as_deref(),
                )
                .await
                .map_err(|e| FetchError::AuthenticationError(format!(
                    "Failed to authenticate with key-pair: {}",
                    e
                )))?
            }
            SnowflakeCredentials::OAuth(_) => {
                return Err(FetchError::ConfigError(
                    "OAuth authentication not yet implemented".to_string()
                ));
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
    pub async fn build(self) -> Result<SnowflakeAdapter, FetchError> {
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
}

impl SnowflakeAdapter {
    /// Create a new Snowflake adapter with password authentication (legacy constructor)
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
        passphrase: Option<String>,
    ) -> SnowflakeAdapterBuilder {
        SnowflakeAdapterBuilder::with_key_pair(account, username, private_key_pem, passphrase)
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for SnowflakeAdapter {
    fn name(&self) -> &'static str {
        "Snowflake"
    }

    #[cfg(feature = "snowflake")]
    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        // Build the INFORMATION_SCHEMA query
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
                } else if err_str.contains("Insufficient privileges") {
                    FetchError::PermissionDenied(format!(
                        "Cannot access {}: {}",
                        table.fqn(), err_str
                    ))
                } else {
                    FetchError::QueryError(err_str)
                }
            })?;

        // Parse results
        let mut columns = Vec::new();

        for row in result.into_json_iter() {
            if let Ok(row_json) = row {
                let col_name = row_json.get("COLUMN_NAME")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let data_type = row_json.get("DATA_TYPE")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN");

                let is_nullable = row_json.get("IS_NULLABLE")
                    .and_then(|v| v.as_str())
                    .unwrap_or("YES");

                // Build full type with precision/scale for numeric types
                let full_type = if data_type == "NUMBER" {
                    let precision = row_json.get("NUMERIC_PRECISION")
                        .and_then(|v| v.as_i64());
                    let scale = row_json.get("NUMERIC_SCALE")
                        .and_then(|v| v.as_i64());

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
}
```

---

## Phase 3: PostgreSQL Adapter Implementation

### 3.1 Add Dependencies

**File**: `Cargo.toml` (workspace root)

```toml
[workspace.dependencies]
# PostgreSQL
tokio-postgres = "0.7"
native-tls = "0.2"
postgres-native-tls = "0.5"
```

**File**: `crates/schemarefly-catalog/Cargo.toml`

```toml
[dependencies]
# ... existing

# PostgreSQL (optional)
tokio-postgres = { workspace = true, optional = true }
native-tls = { workspace = true, optional = true }
postgres-native-tls = { workspace = true, optional = true }

[features]
postgres = ["tokio-postgres", "native-tls", "postgres-native-tls"]
```

### 3.2 Create PostgreSQL Adapter

**File**: `crates/schemarefly-catalog/src/postgres.rs`

```rust
//! PostgreSQL warehouse adapter using information_schema
//!
//! This adapter queries PostgreSQL's information_schema.columns view to fetch
//! table schemas. It works with:
//! - PostgreSQL 9.4+
//! - Amazon Redshift
//! - CockroachDB
//! - Other PostgreSQL-compatible databases

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, Column, LogicalType, Nullability};

#[cfg(feature = "postgres")]
use tokio_postgres::{Client, NoTls, Config as PgConfig};

/// PostgreSQL warehouse adapter
pub struct PostgresAdapter {
    #[cfg(feature = "postgres")]
    client: Client,

    host: String,
    port: u16,
    database: String,
}

impl PostgresAdapter {
    /// Create a new PostgreSQL adapter
    #[cfg(feature = "postgres")]
    pub async fn connect(
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        user: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Self, FetchError> {
        let host = host.into();
        let database = database.into();
        let user = user.into();
        let password = password.into();

        let config = format!(
            "host={} port={} dbname={} user={} password={}",
            host, port, database, user, password
        );

        let (client, connection) = tokio_postgres::connect(&config, NoTls)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to connect to PostgreSQL: {}",
                e
            )))?;

        // Spawn connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });

        Ok(Self {
            client,
            host,
            port,
            database,
        })
    }

    /// Create adapter without postgres feature
    #[cfg(not(feature = "postgres"))]
    pub async fn connect(
        _host: impl Into<String>,
        _port: u16,
        _database: impl Into<String>,
        _user: impl Into<String>,
        _password: impl Into<String>,
    ) -> Result<Self, FetchError> {
        Err(FetchError::ConfigError(
            "PostgreSQL support not compiled. Rebuild with: cargo build --features postgres".to_string()
        ))
    }

    /// Create from connection string
    #[cfg(feature = "postgres")]
    pub async fn from_connection_string(conn_str: &str) -> Result<Self, FetchError> {
        let config: PgConfig = conn_str.parse()
            .map_err(|e| FetchError::ConfigError(format!(
                "Invalid connection string: {}",
                e
            )))?;

        let host = config.get_hosts().first()
            .map(|h| format!("{:?}", h))
            .unwrap_or_else(|| "localhost".to_string());
        let port = config.get_ports().first().copied().unwrap_or(5432);
        let database = config.get_dbname().unwrap_or("postgres").to_string();

        let (client, connection) = tokio_postgres::connect(conn_str, NoTls)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to connect: {}",
                e
            )))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });

        Ok(Self {
            client,
            host,
            port,
            database,
        })
    }

    /// Convert PostgreSQL type to LogicalType
    pub fn map_postgres_type(pg_type: &str) -> LogicalType {
        let base_type = pg_type.split('(').next()
            .unwrap_or(pg_type)
            .trim()
            .to_lowercase();

        match base_type.as_str() {
            "boolean" | "bool" => LogicalType::Bool,

            "smallint" | "int2" => LogicalType::Int,
            "integer" | "int" | "int4" => LogicalType::Int,
            "bigint" | "int8" => LogicalType::Int,
            "serial" | "serial4" => LogicalType::Int,
            "bigserial" | "serial8" => LogicalType::Int,

            "real" | "float4" => LogicalType::Float,
            "double precision" | "float8" | "float" => LogicalType::Float,

            "numeric" | "decimal" => Self::parse_numeric_type(pg_type),

            "money" => LogicalType::Decimal { precision: Some(19), scale: Some(2) },

            "character varying" | "varchar" => LogicalType::String,
            "character" | "char" => LogicalType::String,
            "text" => LogicalType::String,
            "name" => LogicalType::String,

            "bytea" => LogicalType::String,

            "date" => LogicalType::Date,

            "timestamp without time zone" | "timestamp" => LogicalType::Timestamp,
            "timestamp with time zone" | "timestamptz" => LogicalType::Timestamp,
            "time without time zone" | "time" => LogicalType::Timestamp,
            "time with time zone" | "timetz" => LogicalType::Timestamp,

            "interval" => LogicalType::String,

            "json" | "jsonb" => LogicalType::Json,

            "uuid" => LogicalType::String,
            "xml" => LogicalType::String,

            "array" | "_text" | "_int4" | "_varchar" => LogicalType::Array {
                element_type: Box::new(LogicalType::Unknown),
            },

            "point" | "line" | "lseg" | "box" | "path" | "polygon" | "circle" => {
                LogicalType::String // Geometry types as string
            }

            "inet" | "cidr" | "macaddr" | "macaddr8" => LogicalType::String,

            "bit" | "bit varying" => LogicalType::String,

            "tsvector" | "tsquery" => LogicalType::String,

            _ => {
                // Handle array notation like "integer[]"
                if pg_type.ends_with("[]") {
                    let element_type_str = &pg_type[..pg_type.len() - 2];
                    LogicalType::Array {
                        element_type: Box::new(Self::map_postgres_type(element_type_str)),
                    }
                } else {
                    LogicalType::Unknown
                }
            }
        }
    }

    /// Parse numeric type with precision and scale
    fn parse_numeric_type(type_str: &str) -> LogicalType {
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

        // NUMERIC without precision has arbitrary precision
        LogicalType::Decimal {
            precision: None,
            scale: None,
        }
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for PostgresAdapter {
    fn name(&self) -> &'static str {
        "PostgreSQL"
    }

    #[cfg(feature = "postgres")]
    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        let query = r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                ordinal_position,
                numeric_precision,
                numeric_scale,
                udt_name
            FROM information_schema.columns
            WHERE table_catalog = $1
              AND table_schema = $2
              AND table_name = $3
            ORDER BY ordinal_position
        "#;

        let rows = self.client
            .query(query, &[&table.database, &table.schema, &table.table])
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("does not exist") {
                    FetchError::TableNotFound(table.fqn())
                } else if err_str.contains("permission denied") {
                    FetchError::PermissionDenied(table.fqn())
                } else {
                    FetchError::QueryError(err_str)
                }
            })?;

        let mut columns = Vec::new();

        for row in rows {
            let col_name: String = row.get(0);
            let data_type: String = row.get(1);
            let is_nullable: String = row.get(2);
            let numeric_precision: Option<i32> = row.get(4);
            let numeric_scale: Option<i32> = row.get(5);
            let udt_name: String = row.get(6);

            // Build full type string for numeric types
            let full_type = if data_type == "numeric" || data_type == "decimal" {
                match (numeric_precision, numeric_scale) {
                    (Some(p), Some(s)) => format!("numeric({},{})", p, s),
                    (Some(p), None) => format!("numeric({})", p),
                    _ => data_type.clone(),
                }
            } else if udt_name.starts_with('_') {
                // Array type
                format!("{}[]", &udt_name[1..])
            } else {
                data_type.clone()
            };

            let logical_type = Self::map_postgres_type(&full_type);
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

        if columns.is_empty() {
            return Err(FetchError::TableNotFound(format!(
                "Table {} not found or has no columns",
                table.fqn()
            )));
        }

        Ok(Schema::from_columns(columns))
    }

    #[cfg(not(feature = "postgres"))]
    async fn fetch_schema(&self, _table: &TableIdentifier) -> Result<Schema, FetchError> {
        Err(FetchError::ConfigError(
            "PostgreSQL support not compiled. Rebuild with: cargo build --features postgres".to_string()
        ))
    }

    #[cfg(feature = "postgres")]
    async fn test_connection(&self) -> Result<(), FetchError> {
        self.client
            .query("SELECT 1", &[])
            .await
            .map_err(|e| FetchError::QueryError(format!("Connection test failed: {}", e)))?;
        Ok(())
    }

    #[cfg(not(feature = "postgres"))]
    async fn test_connection(&self) -> Result<(), FetchError> {
        Err(FetchError::ConfigError(
            "PostgreSQL support not compiled. Rebuild with: cargo build --features postgres".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("integer"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("bigint"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("text"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("boolean"), LogicalType::Bool));
        assert!(matches!(PostgresAdapter::map_postgres_type("timestamp"), LogicalType::Timestamp));
        assert!(matches!(PostgresAdapter::map_postgres_type("jsonb"), LogicalType::Json));
        assert!(matches!(PostgresAdapter::map_postgres_type("double precision"), LogicalType::Float));
    }

    #[test]
    fn test_numeric_type_parsing() {
        match PostgresAdapter::map_postgres_type("numeric(10,2)") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(10));
                assert_eq!(scale, Some(2));
            }
            _ => panic!("Expected Decimal type"),
        }
    }

    #[test]
    fn test_array_type_mapping() {
        match PostgresAdapter::map_postgres_type("integer[]") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::Int));
            }
            _ => panic!("Expected Array type"),
        }
    }
}
```

### 3.3 Update lib.rs

**File**: `crates/schemarefly-catalog/src/lib.rs`

```rust
//! Warehouse catalog adapters for schema drift detection
//!
//! This module provides adapters to fetch table schemas from various data warehouses.
//!
//! ## Features
//!
//! Enable warehouse support via Cargo features:
//! - `bigquery` - Google BigQuery support
//! - `snowflake` - Snowflake support
//! - `postgres` - PostgreSQL/Redshift support
//! - `all-warehouses` - All warehouse adapters
//!
//! ## Example
//!
//! ```rust,ignore
//! use schemarefly_catalog::{BigQueryAdapter, WarehouseAdapter, TableIdentifier};
//!
//! let adapter = BigQueryAdapter::with_adc("my-project").await?;
//! let table = TableIdentifier::new("my-project", "my_dataset", "my_table");
//! let schema = adapter.fetch_schema(&table).await?;
//! ```

pub mod adapter;
pub mod bigquery;
pub mod snowflake;
pub mod postgres;

pub use adapter::{WarehouseAdapter, TableIdentifier, FetchError};
pub use bigquery::BigQueryAdapter;
pub use snowflake::{SnowflakeAdapter, SnowflakeCredentials, SnowflakeAdapterBuilder};
pub use postgres::PostgresAdapter;
```

---

## Phase 4: Enhanced Configuration

### 4.1 Update Configuration Schema

**File**: `crates/schemarefly-core/src/config.rs`

Add or update the warehouse configuration:

```rust
/// Warehouse connection configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WarehouseConfig {
    /// Warehouse type: bigquery, snowflake, postgres
    pub warehouse_type: String,

    /// Connection settings (key-value pairs)
    #[serde(default)]
    pub settings: HashMap<String, String>,

    /// Use environment variables for secrets (recommended)
    #[serde(default)]
    pub use_env_vars: bool,
}

impl WarehouseConfig {
    /// Get a setting value, checking environment variables first if enabled
    pub fn get_setting(&self, key: &str) -> Option<String> {
        if self.use_env_vars {
            // Check environment variable first
            let env_key = format!("SCHEMAREFLY_{}", key.to_uppercase());
            if let Ok(value) = std::env::var(&env_key) {
                return Some(value);
            }
        }

        self.settings.get(key).cloned()
    }

    /// Get a required setting, returning error if not found
    pub fn require_setting(&self, key: &str) -> Result<String, String> {
        self.get_setting(key).ok_or_else(|| {
            if self.use_env_vars {
                format!(
                    "Missing required setting '{}'. Set it in schemarefly.toml or via SCHEMAREFLY_{} environment variable",
                    key,
                    key.to_uppercase()
                )
            } else {
                format!("Missing required setting '{}' in warehouse configuration", key)
            }
        })
    }
}
```

### 4.2 Example Configuration File

**File**: `schemarefly.toml` (example)

```toml
# SchemaRefly Configuration
dialect = "bigquery"

# Warehouse configuration for drift detection
[warehouse]
# Warehouse type: bigquery, snowflake, postgres
warehouse_type = "bigquery"

# Use environment variables for sensitive settings (recommended)
use_env_vars = true

# Connection settings
# For BigQuery:
#   project_id = "my-gcp-project"
#   credentials = "/path/to/service-account.json"  # or use GOOGLE_APPLICATION_CREDENTIALS
#
# For Snowflake:
#   account = "xy12345.us-east-1"
#   username = "user"
#   password = "..."  # Use SCHEMAREFLY_PASSWORD env var instead
#   warehouse = "COMPUTE_WH"
#   role = "ANALYST"
#
# For PostgreSQL:
#   host = "localhost"
#   port = "5432"
#   database = "mydb"
#   username = "user"
#   password = "..."  # Use SCHEMAREFLY_PASSWORD env var instead

[warehouse.settings]
project_id = "my-gcp-project"
# credentials set via GOOGLE_APPLICATION_CREDENTIALS environment variable
```

### 4.3 Update CLI to Use Environment Variables

**File**: `crates/schemarefly-cli/src/main.rs`

Update the `drift_command` to use the new config methods:

```rust
async fn drift_command(config: &Config, output: &PathBuf, verbose: bool) -> Result<()> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // ... rest of implementation using config.warehouse.require_setting()
}
```

---

## Phase 5: Mock Adapter for Testing

### 5.1 Create Mock Adapter

**File**: `crates/schemarefly-catalog/src/mock.rs`

```rust
//! Mock warehouse adapter for testing
//!
//! This adapter returns predefined schemas without connecting to any warehouse.
//! Useful for:
//! - Unit testing
//! - CI/CD pipelines
//! - Demos and examples

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::Schema;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock warehouse adapter for testing
pub struct MockAdapter {
    /// Predefined schemas by table FQN
    schemas: Arc<RwLock<HashMap<String, Schema>>>,

    /// Simulate connection failure
    fail_connection: bool,

    /// Simulate query latency (milliseconds)
    latency_ms: u64,
}

impl MockAdapter {
    /// Create a new mock adapter
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            fail_connection: false,
            latency_ms: 0,
        }
    }

    /// Add a schema for a table
    pub async fn add_schema(&self, table: TableIdentifier, schema: Schema) {
        self.schemas.write().await.insert(table.fqn(), schema);
    }

    /// Configure to fail connection tests
    pub fn with_connection_failure(mut self) -> Self {
        self.fail_connection = true;
        self
    }

    /// Configure simulated latency
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Create from a map of schemas
    pub fn from_schemas(schemas: HashMap<String, Schema>) -> Self {
        Self {
            schemas: Arc::new(RwLock::new(schemas)),
            fail_connection: false,
            latency_ms: 0,
        }
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for MockAdapter {
    fn name(&self) -> &'static str {
        "Mock"
    }

    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        if self.latency_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.latency_ms)).await;
        }

        let schemas = self.schemas.read().await;
        schemas
            .get(&table.fqn())
            .cloned()
            .ok_or_else(|| FetchError::TableNotFound(table.fqn()))
    }

    async fn test_connection(&self) -> Result<(), FetchError> {
        if self.fail_connection {
            Err(FetchError::NetworkError("Simulated connection failure".to_string()))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_core::{Column, LogicalType};

    #[tokio::test]
    async fn test_mock_adapter() {
        let adapter = MockAdapter::new();

        // Add a schema
        let table = TableIdentifier::new("project", "dataset", "users");
        let schema = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
        ]);

        adapter.add_schema(table.clone(), schema.clone()).await;

        // Fetch it back
        let fetched = adapter.fetch_schema(&table).await.unwrap();
        assert_eq!(fetched.columns.len(), 2);
    }

    #[tokio::test]
    async fn test_table_not_found() {
        let adapter = MockAdapter::new();
        let table = TableIdentifier::new("project", "dataset", "nonexistent");

        let result = adapter.fetch_schema(&table).await;
        assert!(matches!(result, Err(FetchError::TableNotFound(_))));
    }

    #[tokio::test]
    async fn test_connection_failure() {
        let adapter = MockAdapter::new().with_connection_failure();

        let result = adapter.test_connection().await;
        assert!(matches!(result, Err(FetchError::NetworkError(_))));
    }
}
```

Update `lib.rs`:

```rust
pub mod mock;
pub use mock::MockAdapter;
```

---

## Phase 6: Integration Tests

### 6.1 Create Integration Test Suite

**File**: `crates/schemarefly-catalog/tests/integration_tests.rs`

```rust
//! Integration tests for warehouse adapters
//!
//! These tests require actual warehouse credentials and are skipped
//! in CI unless credentials are provided.

use schemarefly_catalog::*;
use schemarefly_core::{Schema, Column, LogicalType};

/// Skip test if BigQuery credentials not available
fn skip_if_no_bigquery() -> bool {
    std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_err()
        && std::env::var("SCHEMAREFLY_BIGQUERY_PROJECT").is_err()
}

/// Skip test if Snowflake credentials not available
fn skip_if_no_snowflake() -> bool {
    std::env::var("SCHEMAREFLY_SNOWFLAKE_ACCOUNT").is_err()
}

/// Skip test if PostgreSQL credentials not available
fn skip_if_no_postgres() -> bool {
    std::env::var("SCHEMAREFLY_POSTGRES_HOST").is_err()
}

#[tokio::test]
#[ignore] // Run with: cargo test --features bigquery -- --ignored
async fn test_bigquery_connection() {
    if skip_if_no_bigquery() {
        eprintln!("Skipping BigQuery test: no credentials");
        return;
    }

    let project_id = std::env::var("SCHEMAREFLY_BIGQUERY_PROJECT")
        .unwrap_or_else(|_| "test-project".to_string());

    let adapter = BigQueryAdapter::with_adc(&project_id)
        .await
        .expect("Failed to create BigQuery adapter");

    adapter.test_connection()
        .await
        .expect("Connection test failed");
}

#[tokio::test]
#[ignore]
async fn test_bigquery_fetch_schema() {
    if skip_if_no_bigquery() {
        return;
    }

    let project_id = std::env::var("SCHEMAREFLY_BIGQUERY_PROJECT").unwrap();
    let dataset = std::env::var("SCHEMAREFLY_BIGQUERY_DATASET").unwrap();
    let table_name = std::env::var("SCHEMAREFLY_BIGQUERY_TABLE").unwrap();

    let adapter = BigQueryAdapter::with_adc(&project_id)
        .await
        .expect("Failed to create adapter");

    let table = TableIdentifier::new(&project_id, &dataset, &table_name);
    let schema = adapter.fetch_schema(&table)
        .await
        .expect("Failed to fetch schema");

    assert!(!schema.columns.is_empty());
    println!("Fetched {} columns", schema.columns.len());
    for col in &schema.columns {
        println!("  {} ({})", col.name, col.logical_type);
    }
}

#[tokio::test]
async fn test_mock_adapter_drift_detection() {
    use schemarefly_engine::DriftDetection;

    // Create mock adapter with expected schema
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    let expected_schema = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("name", LogicalType::String),
        Column::new("email", LogicalType::String),
    ]);

    // Simulate actual schema with drift
    let actual_schema = Schema::from_columns(vec![
        Column::new("id", LogicalType::String), // Type changed!
        Column::new("name", LogicalType::String),
        // email is dropped!
        Column::new("phone", LogicalType::String), // New column
    ]);

    adapter.add_schema(table.clone(), actual_schema.clone()).await;

    // Fetch and compare
    let fetched = adapter.fetch_schema(&table).await.unwrap();

    // Run drift detection
    let drift = DriftDetection::detect(
        table.fqn(),
        &expected_schema,
        &fetched,
        None,
    );

    assert!(drift.has_errors());
    assert_eq!(drift.error_count(), 2); // Type change + dropped column
    assert_eq!(drift.info_count(), 1);  // New column
}
```

### 6.2 Create Test Fixtures

**File**: `crates/schemarefly-catalog/tests/fixtures/mod.rs`

```rust
//! Test fixtures for warehouse adapter tests

use schemarefly_core::{Schema, Column, LogicalType};

/// Create a typical users table schema
pub fn users_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("email", LogicalType::String),
        Column::new("name", LogicalType::String),
        Column::new("created_at", LogicalType::Timestamp),
        Column::new("is_active", LogicalType::Bool),
    ])
}

/// Create a typical orders table schema
pub fn orders_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("user_id", LogicalType::Int),
        Column::new("total_amount", LogicalType::Decimal {
            precision: Some(10),
            scale: Some(2),
        }),
        Column::new("status", LogicalType::String),
        Column::new("created_at", LogicalType::Timestamp),
    ])
}

/// Create a schema with various column types for type mapping tests
pub fn all_types_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("bool_col", LogicalType::Bool),
        Column::new("int_col", LogicalType::Int),
        Column::new("float_col", LogicalType::Float),
        Column::new("decimal_col", LogicalType::Decimal {
            precision: Some(18),
            scale: Some(4),
        }),
        Column::new("string_col", LogicalType::String),
        Column::new("date_col", LogicalType::Date),
        Column::new("timestamp_col", LogicalType::Timestamp),
        Column::new("json_col", LogicalType::Json),
        Column::new("array_col", LogicalType::Array {
            element_type: Box::new(LogicalType::String),
        }),
    ])
}
```

---

## Phase 7: Documentation & Examples

### 7.1 Update README.md

Add drift detection section:

```markdown
## Warehouse Drift Detection

SchemaRefly can detect schema drift between your dbt contracts and the actual warehouse schema.

### Configuration

Add warehouse configuration to `schemarefly.toml`:

```toml
[warehouse]
warehouse_type = "bigquery"  # or "snowflake", "postgres"
use_env_vars = true

[warehouse.settings]
project_id = "my-gcp-project"
```

### Environment Variables

Set credentials via environment variables (recommended):

```bash
# BigQuery
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

# Snowflake
export SCHEMAREFLY_ACCOUNT=xy12345.us-east-1
export SCHEMAREFLY_USERNAME=user
export SCHEMAREFLY_PASSWORD=secret

# PostgreSQL
export SCHEMAREFLY_HOST=localhost
export SCHEMAREFLY_PORT=5432
export SCHEMAREFLY_DATABASE=mydb
export SCHEMAREFLY_USERNAME=user
export SCHEMAREFLY_PASSWORD=secret
```

### Running Drift Detection

```bash
# Build with warehouse support
cargo build --features bigquery  # or snowflake, postgres, all-warehouses

# Run drift detection
schemarefly drift --verbose
```

### Output

The command generates a JSON report (`drift-report.json`) with:
- Models checked
- Drift detections (dropped columns, type changes, new columns)
- Severity levels (Error, Warning, Info)
```

### 7.2 Create Examples

**File**: `examples/drift-detection/README.md`

```markdown
# Drift Detection Example

This example demonstrates how to use SchemaRefly's drift detection feature.

## Prerequisites

1. A dbt project with contracts defined
2. Warehouse credentials configured
3. SchemaRefly built with warehouse support

## Setup

1. Copy the example configuration:
   ```bash
   cp schemarefly.toml.example schemarefly.toml
   ```

2. Edit `schemarefly.toml` with your settings

3. Set environment variables:
   ```bash
   export GOOGLE_APPLICATION_CREDENTIALS=/path/to/creds.json
   ```

## Running

```bash
# Compile dbt project
dbt compile

# Run drift detection
schemarefly drift --verbose
```

## Expected Output

```
Detecting schema drift...
Loading manifest from: target/manifest.json
Connecting to BigQuery...
✓ Connection successful
Checking models with contracts...
  Checking users...
  Checking orders...

==============================================================
Schema Drift Detection Report
==============================================================

Models checked: 10
Models with drift: 2

Drift Details:
[ERROR] Column 'legacy_id' was dropped from warehouse table users
[ERROR] Column 'amount' type changed: was DECIMAL(10, 2), now FLOAT64
[INFO] New column 'updated_at' added to warehouse table orders

Drift report saved to: drift-report.json
```
```

---

## Build & Test Commands

### Build Commands

```bash
# Build without warehouse support (default)
cargo build

# Build with specific warehouse
cargo build --features bigquery
cargo build --features snowflake
cargo build --features postgres

# Build with all warehouses
cargo build --features all-warehouses

# Release build
cargo build --release --features all-warehouses
```

### Test Commands

```bash
# Run unit tests (no credentials needed)
cargo test

# Run integration tests (requires credentials)
cargo test --features bigquery -- --ignored
cargo test --features snowflake -- --ignored
cargo test --features postgres -- --ignored

# Run all tests
cargo test --features all-warehouses -- --include-ignored
```

---

## Backward Compatibility Checklist

- [ ] Existing `schemarefly check` command works without warehouse config
- [ ] Existing configuration files work without `[warehouse]` section
- [ ] CLI builds without any warehouse features (default)
- [ ] All existing tests pass
- [ ] LSP functionality unaffected
- [ ] Report schema version unchanged for non-drift reports

---

## Summary

| Phase | Description | Files | Backward Compatible |
|-------|-------------|-------|---------------------|
| 1 | BigQuery Adapter | `bigquery.rs`, `Cargo.toml` | ✅ Yes (optional feature) |
| 2 | Snowflake Adapter | `snowflake.rs`, `Cargo.toml` | ✅ Yes (optional feature) |
| 3 | PostgreSQL Adapter | `postgres.rs`, `Cargo.toml` | ✅ Yes (optional feature) |
| 4 | Enhanced Configuration | `config.rs`, examples | ✅ Yes (new fields optional) |
| 5 | Mock Adapter | `mock.rs` | ✅ Yes (test only) |
| 6 | Integration Tests | `tests/` | ✅ Yes (test only) |
| 7 | Documentation | `README.md`, examples | ✅ Yes (docs only) |

---

## Success Criteria

1. **BigQuery**: Can fetch schema from INFORMATION_SCHEMA.COLUMNS
2. **Snowflake**: Can authenticate and query INFORMATION_SCHEMA
3. **PostgreSQL**: Can connect and query information_schema
4. **Configuration**: Environment variables work for secrets
5. **Testing**: Mock adapter enables CI/CD without credentials
6. **Backward Compatibility**: All existing functionality unchanged
7. **Documentation**: Clear setup and usage instructions

---

## Next Steps After Implementation

1. Add DuckDB adapter for local development
2. Add Databricks adapter
3. Add schema caching with TTL
4. Add parallel schema fetching for large projects
5. Add schema comparison history tracking
