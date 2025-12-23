//! Warehouse catalog adapters for schema drift detection
//!
//! This module provides adapters to fetch table schemas from various data warehouses
//! using their INFORMATION_SCHEMA views.

pub mod adapter;
pub mod bigquery;
pub mod snowflake;

pub use adapter::{WarehouseAdapter, TableIdentifier, FetchError};
pub use bigquery::BigQueryAdapter;
pub use snowflake::SnowflakeAdapter;
