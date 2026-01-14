//! Integration tests for warehouse adapters
//!
//! These tests validate the warehouse adapters work correctly with real
//! connections and mock adapters. Tests requiring actual warehouse credentials
//! are marked with `#[ignore]` and can be run with `cargo test -- --ignored`.
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all non-ignored tests (no credentials required)
//! cargo test -p schemarefly-catalog --test integration_tests
//!
//! # Run BigQuery integration tests
//! GOOGLE_APPLICATION_CREDENTIALS=/path/to/key.json \
//! SCHEMAREFLY_BIGQUERY_PROJECT=my-project \
//! SCHEMAREFLY_BIGQUERY_DATASET=my_dataset \
//! SCHEMAREFLY_BIGQUERY_TABLE=my_table \
//! cargo test -p schemarefly-catalog --features bigquery --test integration_tests -- --ignored
//!
//! # Run Snowflake integration tests
//! SNOWFLAKE_ACCOUNT=xy12345 \
//! SNOWFLAKE_USER=user \
//! SNOWFLAKE_PASSWORD=pass \
//! cargo test -p schemarefly-catalog --features snowflake --test integration_tests -- --ignored
//!
//! # Run PostgreSQL integration tests
//! PGHOST=localhost \
//! PGPORT=5432 \
//! PGDATABASE=mydb \
//! PGUSER=user \
//! PGPASSWORD=pass \
//! cargo test -p schemarefly-catalog --features postgres --test integration_tests -- --ignored
//! ```

mod fixtures;

use schemarefly_catalog::{FetchError, MockAdapter, TableIdentifier, WarehouseAdapter};
use schemarefly_core::{Column, LogicalType, Schema};

// =============================================================================
// Helper Functions
// =============================================================================

/// Check if BigQuery credentials are available
fn has_bigquery_credentials() -> bool {
    std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_ok()
        || std::env::var("SCHEMAREFLY_BIGQUERY_PROJECT").is_ok()
}

/// Check if Snowflake credentials are available
fn has_snowflake_credentials() -> bool {
    std::env::var("SNOWFLAKE_ACCOUNT").is_ok()
        || std::env::var("SCHEMAREFLY_ACCOUNT").is_ok()
}

/// Check if PostgreSQL credentials are available
fn has_postgres_credentials() -> bool {
    std::env::var("PGHOST").is_ok() || std::env::var("SCHEMAREFLY_HOST").is_ok()
}

// =============================================================================
// Mock Adapter Tests (No credentials required)
// =============================================================================

#[tokio::test]
async fn test_mock_adapter_basic_workflow() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    // Add a schema
    let schema = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("name", LogicalType::String),
        Column::new("email", LogicalType::String),
    ]);

    adapter.add_schema(table.clone(), schema.clone()).await;

    // Fetch it back
    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert_eq!(fetched.columns.len(), 3);
    assert_eq!(fetched.columns[0].name, "id");
    assert_eq!(fetched.columns[1].name, "name");
    assert_eq!(fetched.columns[2].name, "email");
}

#[tokio::test]
async fn test_mock_adapter_table_not_found_error() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "nonexistent");

    let result = adapter.fetch_schema(&table).await;
    assert!(matches!(result, Err(FetchError::TableNotFound(_))));

    if let Err(FetchError::TableNotFound(msg)) = result {
        assert!(msg.contains("project.dataset.nonexistent"));
    }
}

#[tokio::test]
async fn test_mock_adapter_connection_success() {
    let adapter = MockAdapter::new();
    let result = adapter.test_connection().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_adapter_connection_failure_simulation() {
    let adapter = MockAdapter::new().with_connection_failure();
    let result = adapter.test_connection().await;
    assert!(matches!(result, Err(FetchError::NetworkError(_))));
}

#[tokio::test]
async fn test_mock_adapter_custom_error() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "error_table");

    // Add a custom error for this table
    adapter
        .add_error_for_table(
            table.clone(),
            FetchError::PermissionDenied("Access denied to error_table".to_string()),
        )
        .await;

    let result = adapter.fetch_schema(&table).await;
    assert!(matches!(result, Err(FetchError::PermissionDenied(_))));
}

#[tokio::test]
async fn test_mock_adapter_latency_simulation() {
    let adapter = MockAdapter::new().with_latency(100); // 100ms latency
    let table = TableIdentifier::new("project", "dataset", "users");

    let schema = Schema::from_columns(vec![Column::new("id", LogicalType::Int)]);
    adapter.add_schema(table.clone(), schema).await;

    let start = std::time::Instant::now();
    let _ = adapter.fetch_schema(&table).await;
    let elapsed = start.elapsed();

    // Should take at least 100ms
    assert!(elapsed.as_millis() >= 100);
}

#[tokio::test]
async fn test_mock_adapter_multiple_tables() {
    let adapter = MockAdapter::new();

    // Add multiple tables
    let users_table = TableIdentifier::new("project", "dataset", "users");
    let orders_table = TableIdentifier::new("project", "dataset", "orders");
    let products_table = TableIdentifier::new("project", "dataset", "products");

    adapter
        .add_schema(
            users_table.clone(),
            fixtures::users_schema(),
        )
        .await;
    adapter
        .add_schema(
            orders_table.clone(),
            fixtures::orders_schema(),
        )
        .await;
    adapter
        .add_schema(
            products_table.clone(),
            fixtures::products_schema(),
        )
        .await;

    // Verify all tables are accessible
    assert!(adapter.fetch_schema(&users_table).await.is_ok());
    assert!(adapter.fetch_schema(&orders_table).await.is_ok());
    assert!(adapter.fetch_schema(&products_table).await.is_ok());

    // Verify counts
    assert_eq!(adapter.schema_count().await, 3);
}

#[tokio::test]
async fn test_mock_adapter_with_fixtures() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "all_types");

    adapter
        .add_schema(table.clone(), fixtures::all_types_schema())
        .await;

    let schema = adapter.fetch_schema(&table).await.unwrap();

    // Verify column types
    assert!(matches!(
        schema.find_column("bool_col").unwrap().logical_type,
        LogicalType::Bool
    ));
    assert!(matches!(
        schema.find_column("int_col").unwrap().logical_type,
        LogicalType::Int
    ));
    assert!(matches!(
        schema.find_column("float_col").unwrap().logical_type,
        LogicalType::Float
    ));
    assert!(matches!(
        schema.find_column("string_col").unwrap().logical_type,
        LogicalType::String
    ));
    assert!(matches!(
        schema.find_column("date_col").unwrap().logical_type,
        LogicalType::Date
    ));
    assert!(matches!(
        schema.find_column("timestamp_col").unwrap().logical_type,
        LogicalType::Timestamp
    ));
    assert!(matches!(
        schema.find_column("json_col").unwrap().logical_type,
        LogicalType::Json
    ));
}

#[tokio::test]
async fn test_mock_adapter_schema_update() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "evolving");

    // Add initial schema
    let v1_schema = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("name", LogicalType::String),
    ]);
    adapter.add_schema(table.clone(), v1_schema).await;

    // Verify initial schema
    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert_eq!(fetched.columns.len(), 2);

    // Update schema (simulating schema evolution)
    let v2_schema = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("name", LogicalType::String),
        Column::new("email", LogicalType::String), // New column
    ]);
    adapter.add_schema(table.clone(), v2_schema).await;

    // Verify updated schema
    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert_eq!(fetched.columns.len(), 3);
    assert!(fetched.find_column("email").is_some());
}

#[tokio::test]
async fn test_mock_adapter_clear() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    adapter
        .add_schema(table.clone(), fixtures::users_schema())
        .await;
    assert_eq!(adapter.schema_count().await, 1);

    adapter.clear_schemas().await;
    assert_eq!(adapter.schema_count().await, 0);

    // Should return not found after clearing
    assert!(matches!(
        adapter.fetch_schema(&table).await,
        Err(FetchError::TableNotFound(_))
    ));
}

#[tokio::test]
async fn test_mock_adapter_clone_shares_state() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    adapter
        .add_schema(table.clone(), fixtures::users_schema())
        .await;

    // Clone the adapter
    let cloned = adapter.clone();

    // Both should see the same schema
    assert!(adapter.fetch_schema(&table).await.is_ok());
    assert!(cloned.fetch_schema(&table).await.is_ok());

    // Adding to one should be visible in the other
    let new_table = TableIdentifier::new("project", "dataset", "new_table");
    cloned
        .add_schema(
            new_table.clone(),
            Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
        )
        .await;

    assert!(adapter.fetch_schema(&new_table).await.is_ok());
}

#[tokio::test]
async fn test_mock_adapter_from_schemas() {
    use std::collections::HashMap;

    let mut schemas = HashMap::new();
    schemas.insert(
        "project.dataset.users".to_string(),
        fixtures::users_schema(),
    );
    schemas.insert(
        "project.dataset.orders".to_string(),
        fixtures::orders_schema(),
    );

    let adapter = MockAdapter::from_schemas(schemas);

    let users = TableIdentifier::new("project", "dataset", "users");
    let orders = TableIdentifier::new("project", "dataset", "orders");

    assert!(adapter.fetch_schema(&users).await.is_ok());
    assert!(adapter.fetch_schema(&orders).await.is_ok());
}

// =============================================================================
// Drift Detection with Mock Adapter
// =============================================================================

#[tokio::test]
async fn test_drift_detection_no_drift() {
    use schemarefly_engine::DriftDetection;

    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    let expected = fixtures::users_schema();
    let actual = expected.clone();

    adapter.add_schema(table.clone(), actual).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    let drift = DriftDetection::detect(&table.fqn(), &expected, &fetched, None);

    assert!(!drift.has_errors());
    assert!(!drift.has_warnings());
    assert!(!drift.has_info());
    assert_eq!(drift.diagnostics.len(), 0);
}

#[tokio::test]
async fn test_drift_detection_dropped_column() {
    use schemarefly_engine::DriftDetection;
    use schemarefly_core::DiagnosticCode;

    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    let expected = fixtures::users_schema();
    let actual = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("email", LogicalType::String),
        // name column is missing
        Column::new("created_at", LogicalType::Timestamp),
        Column::new("is_active", LogicalType::Bool),
    ]);

    adapter.add_schema(table.clone(), actual).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    let drift = DriftDetection::detect(&table.fqn(), &expected, &fetched, None);

    assert!(drift.has_errors());
    assert_eq!(drift.error_count(), 1);

    let dropped_diagnostic = drift
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::DriftColumnDropped)
        .unwrap();
    assert!(dropped_diagnostic.message.contains("name"));
}

#[tokio::test]
async fn test_drift_detection_type_change() {
    use schemarefly_engine::DriftDetection;
    use schemarefly_core::DiagnosticCode;

    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    let expected = fixtures::users_schema();
    let actual = Schema::from_columns(vec![
        Column::new("id", LogicalType::String), // Changed from Int to String
        Column::new("email", LogicalType::String),
        Column::new("name", LogicalType::String),
        Column::new("created_at", LogicalType::Timestamp),
        Column::new("is_active", LogicalType::Bool),
    ]);

    adapter.add_schema(table.clone(), actual).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    let drift = DriftDetection::detect(&table.fqn(), &expected, &fetched, None);

    assert!(drift.has_errors());
    assert_eq!(drift.error_count(), 1);

    let type_change = drift
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::DriftTypeChange)
        .unwrap();
    assert!(type_change.message.contains("id"));
}

#[tokio::test]
async fn test_drift_detection_new_column() {
    use schemarefly_engine::DriftDetection;
    use schemarefly_core::DiagnosticCode;

    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    let expected = fixtures::users_schema();
    let actual = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("email", LogicalType::String),
        Column::new("name", LogicalType::String),
        Column::new("created_at", LogicalType::Timestamp),
        Column::new("is_active", LogicalType::Bool),
        Column::new("updated_at", LogicalType::Timestamp), // New column
    ]);

    adapter.add_schema(table.clone(), actual).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    let drift = DriftDetection::detect(&table.fqn(), &expected, &fetched, None);

    assert!(!drift.has_errors()); // New columns are info, not errors
    assert!(drift.has_info());
    assert_eq!(drift.info_count(), 1);

    let added_diagnostic = drift
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::DriftColumnAdded)
        .unwrap();
    assert!(added_diagnostic.message.contains("updated_at"));
}

#[tokio::test]
async fn test_drift_detection_multiple_issues() {
    use schemarefly_engine::DriftDetection;

    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "users");

    let expected = fixtures::users_schema();
    let actual = Schema::from_columns(vec![
        Column::new("id", LogicalType::String), // Type changed
        Column::new("email", LogicalType::String),
        // name column dropped
        Column::new("created_at", LogicalType::Timestamp),
        Column::new("is_active", LogicalType::Bool),
        Column::new("phone", LogicalType::String), // New column
    ]);

    adapter.add_schema(table.clone(), actual).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    let drift = DriftDetection::detect(&table.fqn(), &expected, &fetched, None);

    assert!(drift.has_errors());
    assert!(drift.has_info());
    assert_eq!(drift.error_count(), 2); // Type change + dropped column
    assert_eq!(drift.info_count(), 1); // New column
}

// =============================================================================
// BigQuery Integration Tests (require credentials)
// =============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --features bigquery -- --ignored
async fn test_bigquery_connection() {
    if !has_bigquery_credentials() {
        eprintln!("Skipping BigQuery test: no credentials available");
        eprintln!("Set GOOGLE_APPLICATION_CREDENTIALS or SCHEMAREFLY_BIGQUERY_PROJECT");
        return;
    }

    #[cfg(feature = "bigquery")]
    {
        use schemarefly_catalog::BigQueryAdapter;

        let project_id = std::env::var("SCHEMAREFLY_BIGQUERY_PROJECT")
            .or_else(|_| std::env::var("GCP_PROJECT"))
            .expect("SCHEMAREFLY_BIGQUERY_PROJECT or GCP_PROJECT must be set");

        let adapter = BigQueryAdapter::with_adc(&project_id)
            .await
            .expect("Failed to create BigQuery adapter");

        adapter
            .test_connection()
            .await
            .expect("Connection test failed");

        println!("BigQuery connection successful for project: {}", project_id);
    }

    #[cfg(not(feature = "bigquery"))]
    {
        eprintln!("BigQuery feature not enabled. Rebuild with --features bigquery");
    }
}

#[tokio::test]
#[ignore]
async fn test_bigquery_fetch_schema() {
    if !has_bigquery_credentials() {
        return;
    }

    #[cfg(feature = "bigquery")]
    {
        use schemarefly_catalog::BigQueryAdapter;

        let project_id = std::env::var("SCHEMAREFLY_BIGQUERY_PROJECT")
            .expect("SCHEMAREFLY_BIGQUERY_PROJECT must be set");
        let dataset = std::env::var("SCHEMAREFLY_BIGQUERY_DATASET")
            .expect("SCHEMAREFLY_BIGQUERY_DATASET must be set");
        let table_name = std::env::var("SCHEMAREFLY_BIGQUERY_TABLE")
            .expect("SCHEMAREFLY_BIGQUERY_TABLE must be set");

        let adapter = BigQueryAdapter::with_adc(&project_id)
            .await
            .expect("Failed to create adapter");

        let table = TableIdentifier::new(&project_id, &dataset, &table_name);
        let schema = adapter
            .fetch_schema(&table)
            .await
            .expect("Failed to fetch schema");

        assert!(!schema.columns.is_empty());
        println!("Fetched {} columns from BigQuery:", schema.columns.len());
        for col in &schema.columns {
            println!("  {} ({})", col.name, col.logical_type);
        }
    }
}

// =============================================================================
// Snowflake Integration Tests (require credentials)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_snowflake_connection() {
    if !has_snowflake_credentials() {
        eprintln!("Skipping Snowflake test: no credentials available");
        eprintln!("Set SNOWFLAKE_ACCOUNT, SNOWFLAKE_USER, and SNOWFLAKE_PASSWORD");
        return;
    }

    #[cfg(feature = "snowflake")]
    {
        use schemarefly_catalog::SnowflakeAdapter;

        let account = std::env::var("SNOWFLAKE_ACCOUNT")
            .or_else(|_| std::env::var("SCHEMAREFLY_ACCOUNT"))
            .expect("SNOWFLAKE_ACCOUNT must be set");
        let username = std::env::var("SNOWFLAKE_USER")
            .or_else(|_| std::env::var("SCHEMAREFLY_USERNAME"))
            .expect("SNOWFLAKE_USER must be set");
        let password = std::env::var("SNOWFLAKE_PASSWORD")
            .or_else(|_| std::env::var("SCHEMAREFLY_PASSWORD"))
            .expect("SNOWFLAKE_PASSWORD must be set");
        let warehouse = std::env::var("SNOWFLAKE_WAREHOUSE").ok();
        let role = std::env::var("SNOWFLAKE_ROLE").ok();

        let mut builder = SnowflakeAdapter::new(&account, &username, &password);
        if let Some(wh) = &warehouse {
            builder = builder.with_warehouse(wh);
        }
        if let Some(r) = &role {
            builder = builder.with_role(r);
        }

        let adapter = builder.build().expect("Failed to create Snowflake adapter");

        adapter
            .test_connection()
            .await
            .expect("Connection test failed");

        println!("Snowflake connection successful for account: {}", account);
    }

    #[cfg(not(feature = "snowflake"))]
    {
        eprintln!("Snowflake feature not enabled. Rebuild with --features snowflake");
    }
}

#[tokio::test]
#[ignore]
async fn test_snowflake_fetch_schema() {
    if !has_snowflake_credentials() {
        return;
    }

    #[cfg(feature = "snowflake")]
    {
        use schemarefly_catalog::SnowflakeAdapter;

        let account = std::env::var("SNOWFLAKE_ACCOUNT").expect("SNOWFLAKE_ACCOUNT must be set");
        let username = std::env::var("SNOWFLAKE_USER").expect("SNOWFLAKE_USER must be set");
        let password = std::env::var("SNOWFLAKE_PASSWORD").expect("SNOWFLAKE_PASSWORD must be set");
        let database = std::env::var("SCHEMAREFLY_SNOWFLAKE_DATABASE")
            .expect("SCHEMAREFLY_SNOWFLAKE_DATABASE must be set");
        let schema = std::env::var("SCHEMAREFLY_SNOWFLAKE_SCHEMA")
            .expect("SCHEMAREFLY_SNOWFLAKE_SCHEMA must be set");
        let table_name = std::env::var("SCHEMAREFLY_SNOWFLAKE_TABLE")
            .expect("SCHEMAREFLY_SNOWFLAKE_TABLE must be set");

        let adapter = SnowflakeAdapter::new(&account, &username, &password)
            .with_database(&database)
            .build()
            .expect("Failed to create adapter");

        let table = TableIdentifier::new(&database, &schema, &table_name);
        let result_schema = adapter
            .fetch_schema(&table)
            .await
            .expect("Failed to fetch schema");

        assert!(!result_schema.columns.is_empty());
        println!("Fetched {} columns from Snowflake:", result_schema.columns.len());
        for col in &result_schema.columns {
            println!("  {} ({})", col.name, col.logical_type);
        }
    }
}

// =============================================================================
// PostgreSQL Integration Tests (require credentials)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_postgres_connection() {
    if !has_postgres_credentials() {
        eprintln!("Skipping PostgreSQL test: no credentials available");
        eprintln!("Set PGHOST, PGPORT, PGDATABASE, PGUSER, and PGPASSWORD");
        return;
    }

    #[cfg(feature = "postgres")]
    {
        use schemarefly_catalog::PostgresAdapter;

        let host = std::env::var("PGHOST")
            .or_else(|_| std::env::var("SCHEMAREFLY_HOST"))
            .expect("PGHOST must be set");
        let port: u16 = std::env::var("PGPORT")
            .or_else(|_| std::env::var("SCHEMAREFLY_PORT"))
            .unwrap_or_else(|_| "5432".to_string())
            .parse()
            .expect("Invalid port");
        let database = std::env::var("PGDATABASE")
            .or_else(|_| std::env::var("SCHEMAREFLY_DATABASE"))
            .expect("PGDATABASE must be set");
        let user = std::env::var("PGUSER")
            .or_else(|_| std::env::var("SCHEMAREFLY_USERNAME"))
            .expect("PGUSER must be set");
        let password = std::env::var("PGPASSWORD")
            .or_else(|_| std::env::var("SCHEMAREFLY_PASSWORD"))
            .expect("PGPASSWORD must be set");

        let adapter = PostgresAdapter::connect(&host, port, &database, &user, &password)
            .await
            .expect("Failed to create PostgreSQL adapter");

        adapter
            .test_connection()
            .await
            .expect("Connection test failed");

        println!("PostgreSQL connection successful to {}:{}", host, port);
    }

    #[cfg(not(feature = "postgres"))]
    {
        eprintln!("PostgreSQL feature not enabled. Rebuild with --features postgres");
    }
}

#[tokio::test]
#[ignore]
async fn test_postgres_fetch_schema() {
    if !has_postgres_credentials() {
        return;
    }

    #[cfg(feature = "postgres")]
    {
        use schemarefly_catalog::PostgresAdapter;

        let host = std::env::var("PGHOST").expect("PGHOST must be set");
        let port: u16 = std::env::var("PGPORT")
            .unwrap_or_else(|_| "5432".to_string())
            .parse()
            .expect("Invalid port");
        let database = std::env::var("PGDATABASE").expect("PGDATABASE must be set");
        let user = std::env::var("PGUSER").expect("PGUSER must be set");
        let password = std::env::var("PGPASSWORD").expect("PGPASSWORD must be set");
        let schema = std::env::var("SCHEMAREFLY_POSTGRES_SCHEMA").unwrap_or_else(|_| "public".to_string());
        let table_name = std::env::var("SCHEMAREFLY_POSTGRES_TABLE")
            .expect("SCHEMAREFLY_POSTGRES_TABLE must be set");

        let adapter = PostgresAdapter::connect(&host, port, &database, &user, &password)
            .await
            .expect("Failed to create adapter");

        let table = TableIdentifier::new(&database, &schema, &table_name);
        let result_schema = adapter
            .fetch_schema(&table)
            .await
            .expect("Failed to fetch schema");

        assert!(!result_schema.columns.is_empty());
        println!("Fetched {} columns from PostgreSQL:", result_schema.columns.len());
        for col in &result_schema.columns {
            println!("  {} ({})", col.name, col.logical_type);
        }
    }
}

// =============================================================================
// Type Mapping Validation Tests
// =============================================================================

#[test]
fn test_bigquery_type_mapping() {
    use schemarefly_catalog::BigQueryAdapter;

    // Basic types
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("INT64"),
        LogicalType::Int
    ));
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("STRING"),
        LogicalType::String
    ));
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("BOOL"),
        LogicalType::Bool
    ));
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("FLOAT64"),
        LogicalType::Float
    ));
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("TIMESTAMP"),
        LogicalType::Timestamp
    ));
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("DATE"),
        LogicalType::Date
    ));
    assert!(matches!(
        BigQueryAdapter::map_bigquery_type("JSON"),
        LogicalType::Json
    ));

    // Numeric types
    match BigQueryAdapter::map_bigquery_type("NUMERIC(10,2)") {
        LogicalType::Decimal { precision, scale } => {
            assert_eq!(precision, Some(10));
            assert_eq!(scale, Some(2));
        }
        _ => panic!("Expected Decimal type"),
    }

    // Array types
    match BigQueryAdapter::map_bigquery_type("ARRAY<STRING>") {
        LogicalType::Array { element_type } => {
            assert!(matches!(*element_type, LogicalType::String));
        }
        _ => panic!("Expected Array type"),
    }
}

#[test]
fn test_snowflake_type_mapping() {
    use schemarefly_catalog::SnowflakeAdapter;

    // Basic types
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("VARCHAR"),
        LogicalType::String
    ));
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("BOOLEAN"),
        LogicalType::Bool
    ));
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("FLOAT"),
        LogicalType::Float
    ));
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("TIMESTAMP_NTZ"),
        LogicalType::Timestamp
    ));
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("DATE"),
        LogicalType::Date
    ));
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("VARIANT"),
        LogicalType::Json
    ));

    // NUMBER types
    assert!(matches!(
        SnowflakeAdapter::map_snowflake_type("NUMBER(38,0)"),
        LogicalType::Int
    ));
    match SnowflakeAdapter::map_snowflake_type("NUMBER(10,2)") {
        LogicalType::Decimal { precision, scale } => {
            assert_eq!(precision, Some(10));
            assert_eq!(scale, Some(2));
        }
        _ => panic!("Expected Decimal type"),
    }
}

#[test]
fn test_postgres_type_mapping() {
    use schemarefly_catalog::PostgresAdapter;

    // Basic types
    assert!(matches!(
        PostgresAdapter::map_postgres_type("integer"),
        LogicalType::Int
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("bigint"),
        LogicalType::Int
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("text"),
        LogicalType::String
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("boolean"),
        LogicalType::Bool
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("double precision"),
        LogicalType::Float
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("timestamp"),
        LogicalType::Timestamp
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("date"),
        LogicalType::Date
    ));
    assert!(matches!(
        PostgresAdapter::map_postgres_type("jsonb"),
        LogicalType::Json
    ));

    // Numeric types
    match PostgresAdapter::map_postgres_type("numeric(10,2)") {
        LogicalType::Decimal { precision, scale } => {
            assert_eq!(precision, Some(10));
            assert_eq!(scale, Some(2));
        }
        _ => panic!("Expected Decimal type"),
    }

    // Array types
    match PostgresAdapter::map_postgres_type("integer[]") {
        LogicalType::Array { element_type } => {
            assert!(matches!(*element_type, LogicalType::Int));
        }
        _ => panic!("Expected Array type"),
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_empty_schema() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "empty_table");

    let empty_schema = Schema::from_columns(vec![]);
    adapter.add_schema(table.clone(), empty_schema).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert!(fetched.columns.is_empty());
}

#[tokio::test]
async fn test_special_characters_in_names() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("my-project", "my_dataset", "my-table");

    let schema = Schema::from_columns(vec![
        Column::new("column-with-dashes", LogicalType::String),
        Column::new("column_with_underscores", LogicalType::Int),
        Column::new("CamelCaseColumn", LogicalType::Bool),
    ]);

    adapter.add_schema(table.clone(), schema).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert_eq!(fetched.columns.len(), 3);
    assert!(fetched.find_column("column-with-dashes").is_some());
}

#[tokio::test]
async fn test_unicode_column_names() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "unicode_table");

    let schema = Schema::from_columns(vec![
        Column::new("名前", LogicalType::String), // Japanese
        Column::new("数量", LogicalType::Int),    // Japanese
        Column::new("preis", LogicalType::Float), // German
    ]);

    adapter.add_schema(table.clone(), schema).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert_eq!(fetched.columns.len(), 3);
    assert!(fetched.find_column("名前").is_some());
}

#[tokio::test]
async fn test_large_schema() {
    let adapter = MockAdapter::new();
    let table = TableIdentifier::new("project", "dataset", "wide_table");

    // Create a schema with 100 columns
    let columns: Vec<Column> = (0..100)
        .map(|i| Column::new(format!("column_{}", i), LogicalType::String))
        .collect();

    let schema = Schema::from_columns(columns);
    adapter.add_schema(table.clone(), schema).await;

    let fetched = adapter.fetch_schema(&table).await.unwrap();
    assert_eq!(fetched.columns.len(), 100);
}

#[tokio::test]
async fn test_concurrent_access() {
    use std::sync::Arc;

    let adapter = Arc::new(MockAdapter::new());
    let table = TableIdentifier::new("project", "dataset", "concurrent");

    adapter
        .add_schema(table.clone(), fixtures::users_schema())
        .await;

    // Spawn multiple concurrent readers
    let mut handles = vec![];
    for _ in 0..10 {
        let adapter = adapter.clone();
        let table = table.clone();
        handles.push(tokio::spawn(async move {
            adapter.fetch_schema(&table).await.unwrap()
        }));
    }

    // All should succeed
    for handle in handles {
        let schema = handle.await.unwrap();
        assert_eq!(schema.columns.len(), 5);
    }
}
