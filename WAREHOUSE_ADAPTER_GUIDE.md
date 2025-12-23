# Warehouse Adapter Implementation Guide

This guide explains how to implement custom warehouse adapters for SchemaRefly to support additional data warehouses beyond BigQuery and Snowflake.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Implementing a Custom Adapter](#implementing-a-custom-adapter)
- [Step-by-Step Example: Redshift](#step-by-step-example-redshift)
- [Testing Your Adapter](#testing-your-adapter)
- [Security Considerations](#security-considerations)
- [Performance Best Practices](#performance-best-practices)

## Overview

SchemaRefly uses warehouse adapters to fetch table metadata from data warehouses for drift detection. Adapters follow a trait-based design that ensures:

- **Read-only access**: Adapters only query INFORMATION_SCHEMA
- **Type safety**: Rust's type system prevents row-level data access
- **Async/await**: Non-blocking I/O for concurrent operations
- **Error handling**: Structured errors with context

## Architecture

### WarehouseAdapter Trait

Located in `crates/schemarefly-catalog/src/adapter.rs`:

```rust
#[async_trait::async_trait]
pub trait WarehouseAdapter: Send + Sync {
    /// Adapter name (e.g., "bigquery", "snowflake", "redshift")
    fn name(&self) -> &'static str;

    /// Fetch table schema from INFORMATION_SCHEMA
    async fn fetch_schema(
        &self,
        table: &TableIdentifier,
    ) -> Result<Schema, FetchError>;

    /// Test warehouse connection (used for validation)
    async fn test_connection(&self) -> Result<(), FetchError>;
}
```

### Core Types

**TableIdentifier**: Fully-qualified table name
```rust
pub struct TableIdentifier {
    pub database: Option<String>,  // e.g., "analytics"
    pub schema: Option<String>,    // e.g., "public"
    pub table: String,              // e.g., "users"
}
```

**Schema**: Inferred table schema
```rust
pub struct Schema {
    pub columns: Vec<Column>,
}

pub struct Column {
    pub name: String,
    pub logical_type: LogicalType,
    pub nullable: bool,
    pub provenance: Provenance,
}
```

**LogicalType**: Platform-agnostic type system
```rust
pub enum LogicalType {
    Int64, Float64, String, Bool, Timestamp,
    Numeric { precision: u8, scale: u8 },
    // ... more types
}
```

## Implementing a Custom Adapter

### 1. Create Adapter Struct

Create a new file `crates/schemarefly-catalog/src/your_warehouse.rs`:

```rust
use crate::adapter::{WarehouseAdapter, FetchError};
use crate::schema::{Schema, Column, LogicalType, Provenance, TableIdentifier};
use async_trait::async_trait;

pub struct YourWarehouseAdapter {
    // Connection credentials
    host: String,
    port: u16,
    database: String,
    credentials: YourCredentials,
}

pub enum YourCredentials {
    UsernamePassword { username: String, password: String },
    OAuth { token: String },
    // ... other auth methods
}

impl YourWarehouseAdapter {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        credentials: YourCredentials,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            database: database.into(),
            credentials,
        }
    }
}
```

### 2. Implement WarehouseAdapter Trait

```rust
#[async_trait]
impl WarehouseAdapter for YourWarehouseAdapter {
    fn name(&self) -> &'static str {
        "your_warehouse"
    }

    async fn fetch_schema(
        &self,
        table: &TableIdentifier,
    ) -> Result<Schema, FetchError> {
        // 1. Build INFORMATION_SCHEMA query
        let query = self.build_schema_query(table)?;

        // 2. Execute query (use your warehouse's client library)
        let rows = self.execute_query(&query).await?;

        // 3. Parse rows into Schema
        let columns = rows
            .into_iter()
            .map(|row| self.parse_column_row(row))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Schema { columns })
    }

    async fn test_connection(&self) -> Result<(), FetchError> {
        // Simple query to verify connectivity
        let query = "SELECT 1";
        self.execute_query(query).await?;
        Ok(())
    }
}
```

### 3. Helper Methods

```rust
impl YourWarehouseAdapter {
    /// Build INFORMATION_SCHEMA query for table metadata
    fn build_schema_query(&self, table: &TableIdentifier) -> Result<String, FetchError> {
        let database = table.database.as_deref()
            .or(Some(self.database.as_str()))
            .ok_or_else(|| FetchError::Config("Database not specified".into()))?;

        let schema = table.schema.as_deref()
            .unwrap_or("public");

        Ok(format!(
            r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                ordinal_position
            FROM {}.information_schema.columns
            WHERE table_schema = '{}'
              AND table_name = '{}'
            ORDER BY ordinal_position
            "#,
            database, schema, table.table
        ))
    }

    /// Execute query using warehouse client
    async fn execute_query(&self, query: &str) -> Result<Vec<Row>, FetchError> {
        // Use your warehouse's client library here
        // Example (pseudo-code):
        //
        // let client = YourWarehouseClient::new(&self.host, self.port, &self.credentials)?;
        // let rows = client.execute(query).await?;
        // Ok(rows)

        Err(FetchError::NotImplemented)
    }

    /// Parse row from INFORMATION_SCHEMA into Column
    fn parse_column_row(&self, row: Row) -> Result<Column, FetchError> {
        Ok(Column {
            name: row.get_string("column_name")?,
            logical_type: self.map_data_type(&row.get_string("data_type")?)?,
            nullable: row.get_string("is_nullable")? == "YES",
            provenance: Provenance::Warehouse,
        })
    }

    /// Map warehouse-specific type to LogicalType
    fn map_data_type(&self, type_str: &str) -> Result<LogicalType, FetchError> {
        match type_str.to_uppercase().as_str() {
            "INTEGER" | "INT" | "INT4" => Ok(LogicalType::Int64),
            "BIGINT" | "INT8" => Ok(LogicalType::Int64),
            "FLOAT" | "REAL" | "FLOAT4" => Ok(LogicalType::Float64),
            "DOUBLE" | "DOUBLE PRECISION" | "FLOAT8" => Ok(LogicalType::Float64),
            "VARCHAR" | "TEXT" | "CHAR" => Ok(LogicalType::String),
            "BOOLEAN" | "BOOL" => Ok(LogicalType::Bool),
            "TIMESTAMP" | "TIMESTAMPTZ" => Ok(LogicalType::Timestamp),

            // Handle NUMERIC/DECIMAL with precision/scale
            t if t.starts_with("NUMERIC") || t.starts_with("DECIMAL") => {
                self.parse_numeric_type(t)
            }

            unknown => Ok(LogicalType::Unknown(unknown.to_string())),
        }
    }

    fn parse_numeric_type(&self, type_str: &str) -> Result<LogicalType, FetchError> {
        // Parse NUMERIC(10,2) format
        // Implementation depends on your warehouse's format
        Ok(LogicalType::Numeric { precision: 10, scale: 2 })
    }
}
```

## Step-by-Step Example: Redshift

Let's implement a complete Redshift adapter:

```rust
// crates/schemarefly-catalog/src/redshift.rs

use crate::adapter::{WarehouseAdapter, FetchError};
use crate::schema::{Schema, Column, LogicalType, Provenance, TableIdentifier};
use async_trait::async_trait;

pub struct RedshiftAdapter {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: String,
}

impl RedshiftAdapter {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            database: database.into(),
            username: username.into(),
            password: password.into(),
        }
    }
}

#[async_trait]
impl WarehouseAdapter for RedshiftAdapter {
    fn name(&self) -> &'static str {
        "redshift"
    }

    async fn fetch_schema(
        &self,
        table: &TableIdentifier,
    ) -> Result<Schema, FetchError> {
        let schema_name = table.schema.as_deref().unwrap_or("public");

        let query = format!(
            r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                ordinal_position
            FROM information_schema.columns
            WHERE table_schema = '{}'
              AND table_name = '{}'
            ORDER BY ordinal_position
            "#,
            schema_name, table.table
        );

        // TODO: Execute query with Redshift client
        // For now, return error
        Err(FetchError::NotImplemented)
    }

    async fn test_connection(&self) -> Result<(), FetchError> {
        // TODO: Test connection
        Err(FetchError::NotImplemented)
    }
}
```

## Testing Your Adapter

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mapping() {
        let adapter = YourWarehouseAdapter::new(...);

        assert_eq!(
            adapter.map_data_type("INTEGER").unwrap(),
            LogicalType::Int64
        );
        assert_eq!(
            adapter.map_data_type("VARCHAR").unwrap(),
            LogicalType::String
        );
    }

    #[tokio::test]
    async fn test_schema_query_format() {
        let adapter = YourWarehouseAdapter::new(...);
        let table = TableIdentifier {
            database: Some("test_db".to_string()),
            schema: Some("public".to_string()),
            table: "users".to_string(),
        };

        let query = adapter.build_schema_query(&table).unwrap();
        assert!(query.contains("information_schema.columns"));
        assert!(query.contains("users"));
    }
}
```

### Integration Tests

Create `crates/schemarefly-catalog/tests/your_warehouse_integration.rs`:

```rust
#[tokio::test]
#[ignore] // Only run with --ignored flag
async fn test_real_warehouse_connection() {
    // Requires real credentials (use env vars)
    let adapter = YourWarehouseAdapter::new(
        env::var("WAREHOUSE_HOST").unwrap(),
        env::var("WAREHOUSE_PORT").unwrap().parse().unwrap(),
        env::var("WAREHOUSE_DB").unwrap(),
        YourCredentials::from_env(),
    );

    adapter.test_connection().await.expect("Connection failed");
}
```

## Security Considerations

### 1. Read-Only Access

**ALWAYS** use read-only queries:

```rust
✅ SELECT column_name FROM information_schema.columns
❌ INSERT INTO ...
❌ DELETE FROM ...
❌ DROP TABLE ...
```

### 2. No Row-Level Data

**NEVER** query actual table data:

```rust
✅ SELECT * FROM information_schema.columns WHERE table_name = 'users'
❌ SELECT * FROM users
❌ SELECT email FROM users LIMIT 1
```

### 3. Credential Security

- **Never log credentials**: Redact passwords/tokens in error messages
- **Use environment variables**: Don't hardcode credentials
- **Support credential vaults**: Allow integration with secrets managers

```rust
// Bad: Credentials in error messages
Err(format!("Failed to connect with password {}", password))

// Good: Redacted credentials
Err("Failed to connect (credentials redacted)".into())
```

### 4. SQL Injection Prevention

Always use parameterized queries or proper escaping:

```rust
// Bad: String interpolation
let query = format!("SELECT * FROM {}", user_input);

// Good: Validated identifiers
let table_name = validate_identifier(user_input)?;
let query = format!("SELECT column_name FROM information_schema.columns WHERE table_name = '{}'", table_name);
```

## Performance Best Practices

### 1. Connection Pooling

Reuse connections when possible:

```rust
pub struct YourWarehouseAdapter {
    connection_pool: Arc<ConnectionPool>,
}
```

### 2. Caching

Use SchemaRefly's built-in caching:

```rust
use schemarefly_incremental::cache::WarehouseMetadataCache;

let cache = WarehouseMetadataCache::new(Duration::from_secs(300)); // 5 min TTL
```

### 3. Concurrent Fetches

Fetch multiple tables concurrently:

```rust
use futures::future::join_all;

let futures = tables.iter().map(|table| adapter.fetch_schema(table));
let schemas = join_all(futures).await;
```

### 4. Timeout Configuration

Always set query timeouts:

```rust
async fn execute_query_with_timeout(&self, query: &str) -> Result<Vec<Row>, FetchError> {
    tokio::time::timeout(
        Duration::from_secs(30),
        self.execute_query(query)
    )
    .await
    .map_err(|_| FetchError::Timeout)?
}
```

## Registering Your Adapter

### 1. Add to `lib.rs`

In `crates/schemarefly-catalog/src/lib.rs`:

```rust
mod your_warehouse;
pub use your_warehouse::YourWarehouseAdapter;
```

### 2. Update CLI Factory

In `crates/schemarefly-cli/src/main.rs`, add to adapter factory:

```rust
let adapter: Box<dyn WarehouseAdapter> = match warehouse_type.as_str() {
    "bigquery" => Box::new(BigQueryAdapter::new(...)),
    "snowflake" => Box::new(SnowflakeAdapter::new(...)),
    "your_warehouse" => Box::new(YourWarehouseAdapter::new(...)),
    _ => return Err(anyhow!("Unsupported warehouse: {}", warehouse_type)),
};
```

### 3. Update Configuration Schema

Document in `schemarefly.toml`:

```toml
[warehouse]
type = "your_warehouse"
host = "warehouse.example.com"
port = 5439
database = "analytics"
username = "${WAREHOUSE_USER}"
password = "${WAREHOUSE_PASSWORD}"
```

## Example Adapters

See existing implementations:

- **BigQuery**: `crates/schemarefly-catalog/src/bigquery.rs`
- **Snowflake**: `crates/schemarefly-catalog/src/snowflake.rs`

## Troubleshooting

### Common Issues

**Issue**: "NotImplemented" error
- **Solution**: Implement actual query execution logic

**Issue**: Type mapping errors
- **Solution**: Add warehouse-specific types to `map_data_type()`

**Issue**: Connection timeouts
- **Solution**: Check network, credentials, and firewall rules

### Debug Logging

Enable trace logging:

```bash
RUST_LOG=schemarefly_catalog=trace schemarefly drift check
```

## Contributing

To contribute your adapter to SchemaRefly:

1. Implement adapter following this guide
2. Add comprehensive tests
3. Document warehouse-specific configuration
4. Submit pull request to [SchemaRefly repository]

## License

Custom adapters inherit SchemaRefly's license (MIT OR Apache-2.0).
