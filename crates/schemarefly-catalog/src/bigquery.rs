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
//!
//! Reference: https://cloud.google.com/bigquery/docs/information-schema-columns

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, Column, LogicalType, Nullability};

#[cfg(feature = "bigquery")]
use gcp_bigquery_client::{Client as BigQueryClient, model::query_request::QueryRequest};

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
        let key_path_str = key_path.as_ref().to_string_lossy().to_string();

        let client = BigQueryClient::from_service_account_key_file(&key_path_str)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to read service account key file '{}': {}",
                key_path_str, e
            )))?;

        Ok(Self {
            project_id,
            client,
        })
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

        // Parse the JSON into ServiceAccountKey
        let sa_key: gcp_bigquery_client::yup_oauth2::ServiceAccountKey =
            serde_json::from_str(key_json)
                .map_err(|e| FetchError::ConfigError(format!(
                    "Failed to parse service account JSON: {}",
                    e
                )))?;

        let client = BigQueryClient::from_service_account_key(sa_key, false)
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
    ///
    /// Note: This is deprecated. Use async constructors instead:
    /// - `with_adc()` for Application Default Credentials
    /// - `from_service_account_file()` for service account key file
    /// - `from_service_account_json()` for inline service account JSON
    pub fn new(project_id: impl Into<String>, _credentials: impl Into<String>) -> Self {
        #[cfg(feature = "bigquery")]
        {
            panic!("BigQueryAdapter::new() is deprecated. Use async constructors: with_adc(), from_service_account_file(), or from_service_account_json()");
        }

        #[cfg(not(feature = "bigquery"))]
        {
            Self {
                project_id: project_id.into(),
                _phantom: std::marker::PhantomData,
            }
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
        let request = QueryRequest::new(query);
        let query_response = self.client
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

        // Parse results using ResultSet
        let mut columns = Vec::new();
        let mut rs = gcp_bigquery_client::model::query_response::ResultSet::new_from_query_response(query_response);

        while rs.next_row() {
            let col_name = rs.get_string_by_name("column_name")
                .map_err(|e| FetchError::InvalidResponse(format!("Failed to get column_name: {}", e)))?
                .unwrap_or_default();

            let data_type = rs.get_string_by_name("data_type")
                .map_err(|e| FetchError::InvalidResponse(format!("Failed to get data_type: {}", e)))?
                .unwrap_or_else(|| "UNKNOWN".to_string());

            let is_nullable = rs.get_string_by_name("is_nullable")
                .map_err(|e| FetchError::InvalidResponse(format!("Failed to get is_nullable: {}", e)))?
                .unwrap_or_else(|| "YES".to_string());

            let logical_type = Self::map_bigquery_type(&data_type);
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

    #[cfg(not(feature = "bigquery"))]
    async fn fetch_schema(&self, _table: &TableIdentifier) -> Result<Schema, FetchError> {
        Err(FetchError::ConfigError(
            "BigQuery support not compiled. Rebuild with: cargo build --features bigquery".to_string()
        ))
    }

    #[cfg(feature = "bigquery")]
    async fn test_connection(&self) -> Result<(), FetchError> {
        // Simple query to test connection
        let query = "SELECT 1";
        let request = QueryRequest::new(query.to_string());

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

    #[test]
    #[cfg(not(feature = "bigquery"))]
    fn test_adapter_creation() {
        let adapter = BigQueryAdapter::new("my-project", "fake-creds");
        assert_eq!(adapter.name(), "BigQuery");
        assert_eq!(adapter.project_id, "my-project");
    }

    #[test]
    #[cfg(feature = "bigquery")]
    #[should_panic(expected = "BigQueryAdapter::new() is deprecated")]
    fn test_adapter_creation_deprecated_panics() {
        // This test verifies that the deprecated new() method panics
        // when bigquery feature is enabled
        let _adapter = BigQueryAdapter::new("my-project", "fake-creds");
    }
}
