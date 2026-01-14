//! Test fixtures for warehouse adapter integration tests
//!
//! This module provides reusable schema definitions for testing warehouse
//! adapters and drift detection. These fixtures represent common table
//! structures found in real-world data warehouses.

use schemarefly_core::{Column, LogicalType, Nullability, Schema};

/// Create a typical users table schema
///
/// Represents a common user/customer table with:
/// - Primary key (id)
/// - Contact information (email, name)
/// - Metadata (created_at, is_active)
pub fn users_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("id", LogicalType::Int).with_nullability(Nullability::No),
        Column::new("email", LogicalType::String).with_nullability(Nullability::No),
        Column::new("name", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("created_at", LogicalType::Timestamp).with_nullability(Nullability::No),
        Column::new("is_active", LogicalType::Bool).with_nullability(Nullability::No),
    ])
}

/// Create a typical orders table schema
///
/// Represents an e-commerce orders table with:
/// - Primary key (id)
/// - Foreign key (user_id)
/// - Financial data (total_amount)
/// - Status tracking
pub fn orders_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("id", LogicalType::Int).with_nullability(Nullability::No),
        Column::new("user_id", LogicalType::Int).with_nullability(Nullability::No),
        Column::new(
            "total_amount",
            LogicalType::Decimal {
                precision: Some(10),
                scale: Some(2),
            },
        )
        .with_nullability(Nullability::No),
        Column::new("status", LogicalType::String).with_nullability(Nullability::No),
        Column::new("created_at", LogicalType::Timestamp).with_nullability(Nullability::No),
    ])
}

/// Create a typical products table schema
///
/// Represents a product catalog with:
/// - Primary key (id)
/// - Product details (name, description)
/// - Pricing information
/// - Inventory tracking
pub fn products_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("id", LogicalType::Int).with_nullability(Nullability::No),
        Column::new("name", LogicalType::String).with_nullability(Nullability::No),
        Column::new("description", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new(
            "price",
            LogicalType::Decimal {
                precision: Some(10),
                scale: Some(2),
            },
        )
        .with_nullability(Nullability::No),
        Column::new("stock_quantity", LogicalType::Int).with_nullability(Nullability::No),
        Column::new("category", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("is_available", LogicalType::Bool).with_nullability(Nullability::No),
        Column::new("created_at", LogicalType::Timestamp).with_nullability(Nullability::No),
        Column::new("updated_at", LogicalType::Timestamp).with_nullability(Nullability::Yes),
    ])
}

/// Create a schema with all supported column types
///
/// This is useful for testing type mapping across different warehouses.
/// Includes all LogicalType variants to ensure comprehensive coverage.
pub fn all_types_schema() -> Schema {
    Schema::from_columns(vec![
        // Boolean
        Column::new("bool_col", LogicalType::Bool).with_nullability(Nullability::No),
        // Integer
        Column::new("int_col", LogicalType::Int).with_nullability(Nullability::No),
        // Float
        Column::new("float_col", LogicalType::Float).with_nullability(Nullability::No),
        // Decimal with precision/scale
        Column::new(
            "decimal_col",
            LogicalType::Decimal {
                precision: Some(18),
                scale: Some(4),
            },
        )
        .with_nullability(Nullability::No),
        // Decimal without precision (arbitrary)
        Column::new(
            "decimal_arbitrary_col",
            LogicalType::Decimal {
                precision: None,
                scale: None,
            },
        )
        .with_nullability(Nullability::Yes),
        // String
        Column::new("string_col", LogicalType::String).with_nullability(Nullability::No),
        // Date
        Column::new("date_col", LogicalType::Date).with_nullability(Nullability::No),
        // Timestamp
        Column::new("timestamp_col", LogicalType::Timestamp).with_nullability(Nullability::No),
        // JSON
        Column::new("json_col", LogicalType::Json).with_nullability(Nullability::Yes),
        // Array of strings
        Column::new(
            "string_array_col",
            LogicalType::Array {
                element_type: Box::new(LogicalType::String),
            },
        )
        .with_nullability(Nullability::Yes),
        // Array of integers
        Column::new(
            "int_array_col",
            LogicalType::Array {
                element_type: Box::new(LogicalType::Int),
            },
        )
        .with_nullability(Nullability::Yes),
        // Struct (nested)
        Column::new(
            "struct_col",
            LogicalType::Struct {
                fields: vec![
                    Column::new("nested_id", LogicalType::Int),
                    Column::new("nested_name", LogicalType::String),
                ],
            },
        )
        .with_nullability(Nullability::Yes),
        // Unknown type (for edge cases)
        Column::new("unknown_col", LogicalType::Unknown).with_nullability(Nullability::Yes),
    ])
}

/// Create a schema representing a fact table for analytics
///
/// Common pattern in data warehouses for storing metrics and events.
pub fn events_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("event_id", LogicalType::String).with_nullability(Nullability::No),
        Column::new("event_type", LogicalType::String).with_nullability(Nullability::No),
        Column::new("user_id", LogicalType::Int).with_nullability(Nullability::Yes),
        Column::new("session_id", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("event_timestamp", LogicalType::Timestamp).with_nullability(Nullability::No),
        Column::new("event_date", LogicalType::Date).with_nullability(Nullability::No),
        Column::new("properties", LogicalType::Json).with_nullability(Nullability::Yes),
        Column::new("page_url", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("referrer_url", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("device_type", LogicalType::String).with_nullability(Nullability::Yes),
    ])
}

/// Create a schema representing a dimension table
///
/// Common pattern for slowly changing dimensions (SCD Type 2).
pub fn customers_dim_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("customer_key", LogicalType::Int).with_nullability(Nullability::No),
        Column::new("customer_id", LogicalType::String).with_nullability(Nullability::No),
        Column::new("first_name", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("last_name", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("email", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("phone", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("address", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("city", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("state", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("country", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("postal_code", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("valid_from", LogicalType::Timestamp).with_nullability(Nullability::No),
        Column::new("valid_to", LogicalType::Timestamp).with_nullability(Nullability::Yes),
        Column::new("is_current", LogicalType::Bool).with_nullability(Nullability::No),
    ])
}

/// Create a minimal schema for basic testing
pub fn minimal_schema() -> Schema {
    Schema::from_columns(vec![Column::new("id", LogicalType::Int)])
}

/// Create an empty schema (edge case)
pub fn empty_schema() -> Schema {
    Schema::from_columns(vec![])
}

/// Create a schema with nullable columns only
pub fn all_nullable_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("optional_id", LogicalType::Int).with_nullability(Nullability::Yes),
        Column::new("optional_name", LogicalType::String).with_nullability(Nullability::Yes),
        Column::new("optional_date", LogicalType::Date).with_nullability(Nullability::Yes),
    ])
}

/// Create a schema with non-nullable columns only
pub fn all_required_schema() -> Schema {
    Schema::from_columns(vec![
        Column::new("required_id", LogicalType::Int).with_nullability(Nullability::No),
        Column::new("required_name", LogicalType::String).with_nullability(Nullability::No),
        Column::new("required_date", LogicalType::Date).with_nullability(Nullability::No),
    ])
}

/// Create a wide table schema (many columns)
///
/// Useful for testing performance with large schemas.
pub fn wide_table_schema(num_columns: usize) -> Schema {
    let columns: Vec<Column> = (0..num_columns)
        .map(|i| {
            let col_type = match i % 5 {
                0 => LogicalType::Int,
                1 => LogicalType::String,
                2 => LogicalType::Float,
                3 => LogicalType::Bool,
                _ => LogicalType::Timestamp,
            };
            Column::new(format!("column_{}", i), col_type)
        })
        .collect();

    Schema::from_columns(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_users_schema() {
        let schema = users_schema();
        assert_eq!(schema.columns.len(), 5);
        assert!(schema.find_column("id").is_some());
        assert!(schema.find_column("email").is_some());
        assert!(schema.find_column("name").is_some());
        assert!(schema.find_column("created_at").is_some());
        assert!(schema.find_column("is_active").is_some());
    }

    #[test]
    fn test_orders_schema() {
        let schema = orders_schema();
        assert_eq!(schema.columns.len(), 5);
        assert!(schema.find_column("total_amount").is_some());

        let amount_col = schema.find_column("total_amount").unwrap();
        assert!(matches!(
            amount_col.logical_type,
            LogicalType::Decimal { .. }
        ));
    }

    #[test]
    fn test_products_schema() {
        let schema = products_schema();
        assert_eq!(schema.columns.len(), 9);
        assert!(schema.find_column("price").is_some());
        assert!(schema.find_column("stock_quantity").is_some());
    }

    #[test]
    fn test_all_types_schema() {
        let schema = all_types_schema();
        assert!(schema.columns.len() >= 10);

        // Verify we have various types
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Bool)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Int)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Float)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::String)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Date)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Timestamp)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Json)));
        assert!(schema
            .columns
            .iter()
            .any(|c| matches!(c.logical_type, LogicalType::Array { .. })));
    }

    #[test]
    fn test_wide_table_schema() {
        let schema = wide_table_schema(100);
        assert_eq!(schema.columns.len(), 100);

        // Verify column naming
        assert!(schema.find_column("column_0").is_some());
        assert!(schema.find_column("column_99").is_some());
    }

    #[test]
    fn test_empty_schema() {
        let schema = empty_schema();
        assert!(schema.columns.is_empty());
    }

    #[test]
    fn test_minimal_schema() {
        let schema = minimal_schema();
        assert_eq!(schema.columns.len(), 1);
        assert_eq!(schema.columns[0].name, "id");
    }

    #[test]
    fn test_nullability() {
        let required = all_required_schema();
        assert!(required
            .columns
            .iter()
            .all(|c| c.nullable == Nullability::No));

        let nullable = all_nullable_schema();
        assert!(nullable
            .columns
            .iter()
            .all(|c| c.nullable == Nullability::Yes));
    }
}
