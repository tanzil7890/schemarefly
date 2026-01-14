//! Warehouse catalog adapters for schema drift detection
//!
//! This module provides adapters to fetch table schemas from various data warehouses
//! using their INFORMATION_SCHEMA views.
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
//!
//! ## Testing
//!
//! Use `MockAdapter` for testing without real warehouse connections:
//!
//! ```rust,ignore
//! use schemarefly_catalog::{MockAdapter, WarehouseAdapter, TableIdentifier};
//! use schemarefly_core::Schema;
//!
//! let mock = MockAdapter::new();
//! mock.add_schema(table, schema).await;
//! let fetched = mock.fetch_schema(&table).await?;
//! ```

pub mod adapter;
pub mod bigquery;
pub mod snowflake;
pub mod postgres;
pub mod mock;

pub use adapter::{WarehouseAdapter, TableIdentifier, FetchError};
pub use bigquery::BigQueryAdapter;
pub use snowflake::{SnowflakeAdapter, SnowflakeAdapterBuilder};
pub use postgres::PostgresAdapter;
pub use mock::{MockAdapter, MockAdapterBuilder};
