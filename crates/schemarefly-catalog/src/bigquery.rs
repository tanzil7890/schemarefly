//! BigQuery warehouse adapter using INFORMATION_SCHEMA.COLUMNS
//!
//! This adapter queries BigQuery's INFORMATION_SCHEMA.COLUMNS view to fetch
//! table schemas. It requires appropriate IAM permissions:
//! - bigquery.tables.get
//! - bigquery.tables.getData (for INFORMATION_SCHEMA access)
//!
//! Reference: https://cloud.google.com/bigquery/docs/information-schema-columns

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, LogicalType};

/// BigQuery warehouse adapter
pub struct BigQueryAdapter {
    /// Project ID
    project_id: String,

    /// Authentication credentials (service account key JSON or ADC)
    credentials: Option<String>,
}

impl BigQueryAdapter {
    /// Create a new BigQuery adapter with explicit credentials
    pub fn new(project_id: impl Into<String>, credentials: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            credentials: Some(credentials.into()),
        }
    }

    /// Create a new BigQuery adapter using Application Default Credentials (ADC)
    pub fn with_adc(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            credentials: None,
        }
    }

    /// Convert BigQuery type to LogicalType
    fn map_bigquery_type(bq_type: &str) -> LogicalType {
        match bq_type.to_uppercase().as_str() {
            "BOOL" | "BOOLEAN" => LogicalType::Bool,
            "INT64" | "INTEGER" | "INT" | "SMALLINT" | "TINYINT" | "BYTEINT" => LogicalType::Int,
            "FLOAT64" | "FLOAT" | "NUMERIC" => LogicalType::Float,
            "BIGNUMERIC" => LogicalType::Decimal { precision: Some(76), scale: Some(38) },
            "STRING" => LogicalType::String,
            "BYTES" => LogicalType::String, // Map to string for simplicity
            "DATE" => LogicalType::Date,
            "DATETIME" | "TIMESTAMP" => LogicalType::Timestamp,
            "TIME" => LogicalType::Timestamp, // Map to timestamp
            "GEOGRAPHY" => LogicalType::String, // Map to string
            "JSON" => LogicalType::Json,
            "ARRAY" => LogicalType::Array {
                element_type: Box::new(LogicalType::Unknown),
            },
            "STRUCT" | "RECORD" => LogicalType::Struct { fields: vec![] },
            _ => LogicalType::Unknown,
        }
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for BigQueryAdapter {
    fn name(&self) -> &'static str {
        "BigQuery"
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
            FROM `{}.{}.INFORMATION_SCHEMA.COLUMNS`
            WHERE table_name = '{}'
            ORDER BY ordinal_position
            "#,
            table.database,
            table.schema,
            table.table
        );

        // In a real implementation, this would:
        // 1. Create a BigQuery client using credentials
        // 2. Execute the query
        // 3. Parse results into Schema
        //
        // For now, we return a placeholder error indicating this needs
        // actual BigQuery SDK integration

        Err(FetchError::ConfigError(
            "BigQuery adapter requires google-cloud-bigquery dependency. \
             Install with: cargo add google-cloud-bigquery@0.4".to_string()
        ))

        // Example of what the real implementation would look like:
        //
        // use google_cloud_bigquery::http::bigquery::BigqueryClient;
        //
        // let client = if let Some(creds) = &self.credentials {
        //     BigqueryClient::from_credentials(creds).await
        //         .map_err(|e| FetchError::AuthenticationError(e.to_string()))?
        // } else {
        //     BigqueryClient::new().await
        //         .map_err(|e| FetchError::AuthenticationError(e.to_string()))?
        // };
        //
        // let result = client.query(&query).await
        //     .map_err(|e| FetchError::QueryError(e.to_string()))?;
        //
        // let mut columns = Vec::new();
        // for row in result.rows {
        //     let col_name = row.get::<String>("column_name")?;
        //     let data_type = row.get::<String>("data_type")?;
        //     let is_nullable = row.get::<String>("is_nullable")?;
        //
        //     let logical_type = Self::map_bigquery_type(&data_type);
        //     let mut column = Column::new(col_name, logical_type);
        //
        //     // Set nullable based on IS_NULLABLE
        //     // (this would require extending Column to support nullable flag)
        //
        //     columns.push(column);
        // }
        //
        // Ok(Schema::from_columns(columns))
    }

    async fn test_connection(&self) -> Result<(), FetchError> {
        // In a real implementation, this would test the BigQuery connection
        // For now, return config error
        Err(FetchError::ConfigError(
            "BigQuery adapter requires google-cloud-bigquery dependency".to_string()
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
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = BigQueryAdapter::with_adc("my-project");
        assert_eq!(adapter.name(), "BigQuery");
        assert_eq!(adapter.project_id, "my-project");
        assert!(adapter.credentials.is_none());

        let adapter_with_creds = BigQueryAdapter::new("my-project", "fake-creds");
        assert!(adapter_with_creds.credentials.is_some());
    }
}
