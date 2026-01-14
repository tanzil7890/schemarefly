//! Mock warehouse adapter for testing
//!
//! This adapter returns predefined schemas without connecting to any warehouse.
//! It's useful for:
//! - Unit testing drift detection logic
//! - Integration testing CI/CD pipelines
//! - Demos and examples without real credentials
//! - Simulating various error conditions
//!
//! ## Usage
//!
//! ```rust,ignore
//! use schemarefly_catalog::{MockAdapter, WarehouseAdapter, TableIdentifier};
//! use schemarefly_core::{Schema, Column, LogicalType};
//!
//! // Create a mock adapter
//! let adapter = MockAdapter::new();
//!
//! // Add a schema for a table
//! let table = TableIdentifier::new("project", "dataset", "users");
//! let schema = Schema::from_columns(vec![
//!     Column::new("id", LogicalType::Int),
//!     Column::new("name", LogicalType::String),
//! ]);
//! adapter.add_schema(table.clone(), schema).await;
//!
//! // Fetch schema (returns the predefined schema)
//! let fetched = adapter.fetch_schema(&table).await?;
//! ```
//!
//! ## Simulating Failures
//!
//! ```rust,ignore
//! // Simulate connection failure
//! let adapter = MockAdapter::new().with_connection_failure();
//! assert!(adapter.test_connection().await.is_err());
//!
//! // Simulate network latency
//! let adapter = MockAdapter::new().with_latency(100); // 100ms delay
//! ```

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::Schema;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock warehouse adapter for testing
///
/// This adapter stores schemas in memory and returns them when requested.
/// It does not connect to any real warehouse, making it perfect for testing.
///
/// # Features
///
/// - Store and retrieve schemas by table identifier
/// - Simulate connection failures
/// - Simulate network latency
/// - Simulate specific error conditions per table
/// - Thread-safe with async support
///
/// # Example
///
/// ```rust,ignore
/// let adapter = MockAdapter::new()
///     .with_latency(50)  // 50ms simulated latency
///     .with_connection_failure();  // Fail connection tests
/// ```
pub struct MockAdapter {
    /// Predefined schemas by table FQN
    schemas: Arc<RwLock<HashMap<String, Schema>>>,

    /// Errors to return for specific tables
    errors: Arc<RwLock<HashMap<String, FetchError>>>,

    /// Simulate connection failure
    fail_connection: bool,

    /// Simulate query latency (milliseconds)
    latency_ms: u64,

    /// Name to return from name() method
    adapter_name: &'static str,
}

impl MockAdapter {
    /// Create a new mock adapter with no predefined schemas
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            errors: Arc::new(RwLock::new(HashMap::new())),
            fail_connection: false,
            latency_ms: 0,
            adapter_name: "Mock",
        }
    }

    /// Add a schema for a specific table
    ///
    /// The schema will be returned when `fetch_schema` is called with a
    /// matching table identifier.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let adapter = MockAdapter::new();
    /// let table = TableIdentifier::new("db", "schema", "table");
    /// let schema = Schema::from_columns(vec![
    ///     Column::new("id", LogicalType::Int),
    /// ]);
    /// adapter.add_schema(table, schema).await;
    /// ```
    pub async fn add_schema(&self, table: TableIdentifier, schema: Schema) {
        self.schemas.write().await.insert(table.fqn(), schema);
    }

    /// Add a schema using string identifiers for convenience
    pub async fn add_schema_for(
        &self,
        database: &str,
        schema_name: &str,
        table: &str,
        schema: Schema,
    ) {
        let table_id = TableIdentifier::new(database, schema_name, table);
        self.add_schema(table_id, schema).await;
    }

    /// Configure an error to be returned for a specific table
    ///
    /// This allows simulating various error conditions like permission denied
    /// or table not found for specific tables.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// adapter.add_error_for_table(
    ///     TableIdentifier::new("db", "schema", "restricted"),
    ///     FetchError::PermissionDenied("Access denied".to_string())
    /// ).await;
    /// ```
    pub async fn add_error_for_table(&self, table: TableIdentifier, error: FetchError) {
        self.errors.write().await.insert(table.fqn(), error);
    }

    /// Configure to fail all connection tests
    ///
    /// When enabled, `test_connection()` will always return an error.
    pub fn with_connection_failure(mut self) -> Self {
        self.fail_connection = true;
        self
    }

    /// Configure simulated latency for all operations
    ///
    /// This adds a delay before returning results, useful for testing
    /// timeout handling or progress indicators.
    ///
    /// # Arguments
    ///
    /// * `latency_ms` - Delay in milliseconds before returning results
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Set a custom adapter name
    ///
    /// This is useful when mocking a specific warehouse type.
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.adapter_name = name;
        self
    }

    /// Create a mock adapter from a pre-built map of schemas
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut schemas = HashMap::new();
    /// schemas.insert(
    ///     "db.schema.table".to_string(),
    ///     Schema::from_columns(vec![Column::new("id", LogicalType::Int)])
    /// );
    /// let adapter = MockAdapter::from_schemas(schemas);
    /// ```
    pub fn from_schemas(schemas: HashMap<String, Schema>) -> Self {
        Self {
            schemas: Arc::new(RwLock::new(schemas)),
            errors: Arc::new(RwLock::new(HashMap::new())),
            fail_connection: false,
            latency_ms: 0,
            adapter_name: "Mock",
        }
    }

    /// Get the number of schemas stored in the adapter
    pub async fn schema_count(&self) -> usize {
        self.schemas.read().await.len()
    }

    /// Clear all stored schemas
    pub async fn clear_schemas(&self) {
        self.schemas.write().await.clear();
    }

    /// Clear all stored errors
    pub async fn clear_errors(&self) {
        self.errors.write().await.clear();
    }

    /// Check if a schema exists for a table
    pub async fn has_schema(&self, table: &TableIdentifier) -> bool {
        self.schemas.read().await.contains_key(&table.fqn())
    }

    /// Get all table FQNs that have schemas
    pub async fn get_table_names(&self) -> Vec<String> {
        self.schemas.read().await.keys().cloned().collect()
    }

    /// Simulate latency if configured
    async fn simulate_latency(&self) {
        if self.latency_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.latency_ms)).await;
        }
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockAdapter {
    fn clone(&self) -> Self {
        Self {
            schemas: Arc::clone(&self.schemas),
            errors: Arc::clone(&self.errors),
            fail_connection: self.fail_connection,
            latency_ms: self.latency_ms,
            adapter_name: self.adapter_name,
        }
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for MockAdapter {
    fn name(&self) -> &'static str {
        self.adapter_name
    }

    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        self.simulate_latency().await;

        // Check for configured errors first
        if let Some(error) = self.errors.read().await.get(&table.fqn()) {
            return Err(error.clone());
        }

        // Return the schema if found
        let schemas = self.schemas.read().await;
        schemas
            .get(&table.fqn())
            .cloned()
            .ok_or_else(|| FetchError::TableNotFound(table.fqn()))
    }

    async fn test_connection(&self) -> Result<(), FetchError> {
        self.simulate_latency().await;

        if self.fail_connection {
            Err(FetchError::NetworkError(
                "Simulated connection failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Builder for creating MockAdapter with multiple schemas
///
/// Provides a fluent API for building a mock adapter with predefined schemas.
///
/// # Example
///
/// ```rust,ignore
/// use schemarefly_catalog::MockAdapterBuilder;
/// use schemarefly_core::{Schema, Column, LogicalType};
///
/// let adapter = MockAdapterBuilder::new()
///     .with_schema("db", "schema", "users", Schema::from_columns(vec![
///         Column::new("id", LogicalType::Int),
///         Column::new("name", LogicalType::String),
///     ]))
///     .with_schema("db", "schema", "orders", Schema::from_columns(vec![
///         Column::new("order_id", LogicalType::Int),
///         Column::new("user_id", LogicalType::Int),
///     ]))
///     .with_latency(50)
///     .build();
/// ```
pub struct MockAdapterBuilder {
    schemas: HashMap<String, Schema>,
    errors: HashMap<String, FetchError>,
    fail_connection: bool,
    latency_ms: u64,
    adapter_name: &'static str,
}

impl MockAdapterBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
            errors: HashMap::new(),
            fail_connection: false,
            latency_ms: 0,
            adapter_name: "Mock",
        }
    }

    /// Add a schema for a table
    pub fn with_schema(
        mut self,
        database: &str,
        schema_name: &str,
        table: &str,
        schema: Schema,
    ) -> Self {
        let fqn = format!("{}.{}.{}", database, schema_name, table);
        self.schemas.insert(fqn, schema);
        self
    }

    /// Add a schema using a TableIdentifier
    pub fn with_table_schema(mut self, table: TableIdentifier, schema: Schema) -> Self {
        self.schemas.insert(table.fqn(), schema);
        self
    }

    /// Add an error for a specific table
    pub fn with_error(
        mut self,
        database: &str,
        schema_name: &str,
        table: &str,
        error: FetchError,
    ) -> Self {
        let fqn = format!("{}.{}.{}", database, schema_name, table);
        self.errors.insert(fqn, error);
        self
    }

    /// Configure connection failure
    pub fn with_connection_failure(mut self) -> Self {
        self.fail_connection = true;
        self
    }

    /// Configure latency
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Set adapter name
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.adapter_name = name;
        self
    }

    /// Build the MockAdapter
    pub fn build(self) -> MockAdapter {
        MockAdapter {
            schemas: Arc::new(RwLock::new(self.schemas)),
            errors: Arc::new(RwLock::new(self.errors)),
            fail_connection: self.fail_connection,
            latency_ms: self.latency_ms,
            adapter_name: self.adapter_name,
        }
    }
}

impl Default for MockAdapterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_core::{Column, LogicalType, Nullability};

    #[tokio::test]
    async fn test_mock_adapter_basic() {
        let adapter = MockAdapter::new();

        // Add a schema
        let table = TableIdentifier::new("project", "dataset", "users");
        let schema = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
        ]);

        adapter.add_schema(table.clone(), schema).await;

        // Fetch it back
        let fetched = adapter.fetch_schema(&table).await.unwrap();
        assert_eq!(fetched.columns.len(), 2);
        assert_eq!(fetched.columns[0].name, "id");
        assert_eq!(fetched.columns[1].name, "name");
    }

    #[tokio::test]
    async fn test_mock_adapter_table_not_found() {
        let adapter = MockAdapter::new();
        let table = TableIdentifier::new("project", "dataset", "nonexistent");

        let result = adapter.fetch_schema(&table).await;
        assert!(matches!(result, Err(FetchError::TableNotFound(_))));
    }

    #[tokio::test]
    async fn test_mock_adapter_connection_failure() {
        let adapter = MockAdapter::new().with_connection_failure();

        let result = adapter.test_connection().await;
        assert!(matches!(result, Err(FetchError::NetworkError(_))));
    }

    #[tokio::test]
    async fn test_mock_adapter_connection_success() {
        let adapter = MockAdapter::new();

        let result = adapter.test_connection().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_adapter_custom_error() {
        let adapter = MockAdapter::new();
        let table = TableIdentifier::new("project", "dataset", "restricted");

        adapter
            .add_error_for_table(
                table.clone(),
                FetchError::PermissionDenied("Access denied to restricted table".to_string()),
            )
            .await;

        let result = adapter.fetch_schema(&table).await;
        assert!(matches!(result, Err(FetchError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_mock_adapter_from_schemas() {
        let mut schemas = HashMap::new();
        schemas.insert(
            "db.schema.table".to_string(),
            Schema::from_columns(vec![Column::new("col1", LogicalType::String)]),
        );

        let adapter = MockAdapter::from_schemas(schemas);
        let table = TableIdentifier::new("db", "schema", "table");

        let fetched = adapter.fetch_schema(&table).await.unwrap();
        assert_eq!(fetched.columns.len(), 1);
        assert_eq!(fetched.columns[0].name, "col1");
    }

    #[tokio::test]
    async fn test_mock_adapter_builder() {
        let adapter = MockAdapterBuilder::new()
            .with_schema(
                "project",
                "dataset",
                "users",
                Schema::from_columns(vec![
                    Column::new("id", LogicalType::Int),
                    Column::new("email", LogicalType::String),
                ]),
            )
            .with_schema(
                "project",
                "dataset",
                "orders",
                Schema::from_columns(vec![
                    Column::new("order_id", LogicalType::Int),
                    Column::new("total", LogicalType::Float),
                ]),
            )
            .build();

        let users_table = TableIdentifier::new("project", "dataset", "users");
        let orders_table = TableIdentifier::new("project", "dataset", "orders");

        let users = adapter.fetch_schema(&users_table).await.unwrap();
        let orders = adapter.fetch_schema(&orders_table).await.unwrap();

        assert_eq!(users.columns.len(), 2);
        assert_eq!(orders.columns.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_adapter_name() {
        let adapter = MockAdapter::new();
        assert_eq!(adapter.name(), "Mock");

        let adapter = MockAdapter::new().with_name("TestBigQuery");
        assert_eq!(adapter.name(), "TestBigQuery");
    }

    #[tokio::test]
    async fn test_mock_adapter_schema_count() {
        let adapter = MockAdapter::new();

        assert_eq!(adapter.schema_count().await, 0);

        adapter
            .add_schema(
                TableIdentifier::new("db", "schema", "table1"),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        assert_eq!(adapter.schema_count().await, 1);

        adapter
            .add_schema(
                TableIdentifier::new("db", "schema", "table2"),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        assert_eq!(adapter.schema_count().await, 2);
    }

    #[tokio::test]
    async fn test_mock_adapter_clear() {
        let adapter = MockAdapter::new();

        adapter
            .add_schema(
                TableIdentifier::new("db", "schema", "table"),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        assert_eq!(adapter.schema_count().await, 1);

        adapter.clear_schemas().await;

        assert_eq!(adapter.schema_count().await, 0);
    }

    #[tokio::test]
    async fn test_mock_adapter_has_schema() {
        let adapter = MockAdapter::new();
        let table = TableIdentifier::new("db", "schema", "table");

        assert!(!adapter.has_schema(&table).await);

        adapter
            .add_schema(
                table.clone(),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        assert!(adapter.has_schema(&table).await);
    }

    #[tokio::test]
    async fn test_mock_adapter_with_nullability() {
        let adapter = MockAdapter::new();
        let table = TableIdentifier::new("db", "schema", "users");

        let schema = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int).with_nullability(Nullability::No),
            Column::new("name", LogicalType::String).with_nullability(Nullability::Yes),
            Column::new("email", LogicalType::String).with_nullability(Nullability::Unknown),
        ]);

        adapter.add_schema(table.clone(), schema).await;

        let fetched = adapter.fetch_schema(&table).await.unwrap();
        assert_eq!(fetched.columns[0].nullable, Nullability::No);
        assert_eq!(fetched.columns[1].nullable, Nullability::Yes);
        assert_eq!(fetched.columns[2].nullable, Nullability::Unknown);
    }

    #[tokio::test]
    async fn test_mock_adapter_clone() {
        let adapter = MockAdapter::new();
        let table = TableIdentifier::new("db", "schema", "table");

        adapter
            .add_schema(
                table.clone(),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        let cloned = adapter.clone();

        // Both should see the same schema (shared state)
        assert!(adapter.has_schema(&table).await);
        assert!(cloned.has_schema(&table).await);
    }

    #[tokio::test]
    async fn test_mock_adapter_get_table_names() {
        let adapter = MockAdapter::new();

        adapter
            .add_schema(
                TableIdentifier::new("db", "schema", "table1"),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        adapter
            .add_schema(
                TableIdentifier::new("db", "schema", "table2"),
                Schema::from_columns(vec![Column::new("id", LogicalType::Int)]),
            )
            .await;

        let names = adapter.get_table_names().await;
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"db.schema.table1".to_string()));
        assert!(names.contains(&"db.schema.table2".to_string()));
    }
}
