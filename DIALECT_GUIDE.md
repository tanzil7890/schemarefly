# SQL Dialect Extensibility Guide

This guide explains how to add support for new SQL dialects in SchemaRefly.

## Table of Contents

- [Overview](#overview)
- [Current Architecture](#current-architecture)
- [Adding a New Dialect](#adding-a-new-dialect)
- [Type Mapping](#type-mapping)
- [Testing](#testing)
- [Future: Plugin System](#future-plugin-system)

## Overview

SchemaRefly supports multiple SQL dialects through a configuration-driven approach. Currently supported dialects:

- **BigQuery**: Google BigQuery SQL
- **Snowflake**: Snowflake SQL
- **Postgres**: PostgreSQL
- **Ansi**: Generic ANSI SQL

## Current Architecture

### Dialect Configuration

Dialects are defined as an enum in `crates/schemarefly-core/src/config.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialectConfig {
    BigQuery,
    Snowflake,
    Postgres,
    Ansi,
}
```

### SQL Parser Integration

The dialect is used in `crates/schemarefly-sql/src/parser.rs`:

```rust
pub fn parse_sql(sql: &str, dialect: DialectConfig) -> Result<ParsedSql> {
    let sqlparser_dialect = match dialect {
        DialectConfig::BigQuery => Dialect::BigQuery,
        DialectConfig::Snowflake => Dialect::Snowflake,
        DialectConfig::Postgres => Dialect::Postgres,
        DialectConfig::Ansi => Dialect::Ansi,
    };

    let statements = sqlparser::Parser::parse_sql(&sqlparser_dialect, sql)?;
    // ... parse AST
}
```

## Adding a New Dialect

### Step 1: Add to DialectConfig Enum

Edit `crates/schemarefly-core/src/config.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialectConfig {
    BigQuery,
    Snowflake,
    Postgres,
    Ansi,
    Redshift,  // <-- Add your dialect
}
```

### Step 2: Add Parser Mapping

Edit `crates/schemarefly-sql/src/parser.rs`:

```rust
pub fn parse_sql(sql: &str, dialect: DialectConfig) -> Result<ParsedSql> {
    let sqlparser_dialect = match dialect {
        DialectConfig::BigQuery => Dialect::BigQuery,
        DialectConfig::Snowflake => Dialect::Snowflake,
        DialectConfig::Postgres => Dialect::Postgres,
        DialectConfig::Ansi => Dialect::Ansi,
        DialectConfig::Redshift => Dialect::Postgres, // <-- Redshift uses Postgres dialect
    };

    // ... rest of parsing logic
}
```

**Note**: If your dialect is not supported by `sqlparser`, use the closest match (e.g., Redshift → Postgres).

### Step 3: Add Type Mapping (if needed)

If your dialect has unique type names, add a type mapper in `crates/schemarefly-catalog/src/`:

Create `crates/schemarefly-catalog/src/redshift_types.rs`:

```rust
use crate::schema::LogicalType;

/// Map Redshift-specific types to LogicalType
pub fn map_redshift_type(type_str: &str) -> LogicalType {
    match type_str.to_uppercase().as_str() {
        "INTEGER" | "INT" | "INT4" => LogicalType::Int64,
        "BIGINT" | "INT8" => LogicalType::Int64,
        "SMALLINT" | "INT2" => LogicalType::Int64,

        "REAL" | "FLOAT4" => LogicalType::Float64,
        "DOUBLE PRECISION" | "FLOAT8" | "FLOAT" => LogicalType::Float64,

        "VARCHAR" | "CHAR" | "TEXT" | "NVARCHAR" | "BPCHAR" => LogicalType::String,

        "BOOLEAN" | "BOOL" => LogicalType::Bool,

        "TIMESTAMP" | "TIMESTAMPTZ" => LogicalType::Timestamp,
        "DATE" => LogicalType::Date,

        // Redshift-specific types
        "SUPER" => LogicalType::Json,  // Redshift's semi-structured type

        t if t.starts_with("NUMERIC") || t.starts_with("DECIMAL") => {
            parse_decimal_type(t)
        }

        unknown => LogicalType::Unknown(unknown.to_string()),
    }
}

fn parse_decimal_type(type_str: &str) -> LogicalType {
    // Parse NUMERIC(10,2) format
    // Default to (38, 0) if not specified
    LogicalType::Numeric { precision: 38, scale: 0 }
}
```

Register in `crates/schemarefly-catalog/src/lib.rs`:

```rust
mod redshift_types;
pub use redshift_types::map_redshift_type;
```

### Step 4: Update Configuration Documentation

Add to example `schemarefly.toml`:

```toml
# SQL dialect (bigquery, snowflake, postgres, ansi, redshift)
dialect = "redshift"
```

### Step 5: Update Tests

Add dialect-specific tests in `crates/schemarefly-sql/tests/`:

```rust
#[test]
fn test_redshift_syntax() {
    let sql = r#"
        SELECT
            user_id::INTEGER,
            created_at::TIMESTAMPTZ
        FROM users
        WHERE region = 'us-east-1'
    "#;

    let parsed = parse_sql(sql, DialectConfig::Redshift).unwrap();
    assert_eq!(parsed.selected_columns.len(), 2);
}
```

## Type Mapping

### Standard Types

Most dialects share common types:

| SQL Type | LogicalType |
|----------|-------------|
| INTEGER, INT, INT4 | Int64 |
| BIGINT, INT8 | Int64 |
| FLOAT, REAL | Float64 |
| DOUBLE, DOUBLE PRECISION | Float64 |
| VARCHAR, TEXT, STRING | String |
| BOOLEAN, BOOL | Bool |
| TIMESTAMP, TIMESTAMPTZ | Timestamp |
| DATE | Date |
| NUMERIC(p,s), DECIMAL(p,s) | Numeric { precision, scale } |

### Dialect-Specific Types

**BigQuery**:
- `STRUCT<...>` → `LogicalType::Struct { fields }`
- `ARRAY<...>` → `LogicalType::Array { element_type }`
- `GEOGRAPHY` → `LogicalType::Geography`

**Snowflake**:
- `VARIANT` → `LogicalType::Json`
- `OBJECT` → `LogicalType::Json`
- `ARRAY` → `LogicalType::Array { element_type }`

**Postgres**:
- `JSONB` → `LogicalType::Json`
- `UUID` → `LogicalType::Uuid`
- `INET` → `LogicalType::String`  // Network address

### Implementing Custom Type Logic

If your dialect requires complex type inference:

```rust
// In crates/schemarefly-sql/src/inference.rs

pub fn infer_column_type(
    expr: &sqlparser::ast::Expr,
    dialect: &DialectConfig,
) -> Result<LogicalType> {
    match dialect {
        DialectConfig::YourDialect => {
            // Custom type inference logic
            match expr {
                Expr::Cast { data_type, .. } => {
                    map_your_dialect_type(data_type)
                }
                // ... other cases
            }
        }
        _ => {
            // Default logic
        }
    }
}
```

## Testing

### Unit Tests

Test dialect-specific SQL parsing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_dialect_select() {
        let sql = "SELECT col1, col2 FROM table1";
        let result = parse_sql(sql, DialectConfig::YourDialect);
        assert!(result.is_ok());
    }

    #[test]
    fn test_your_dialect_join() {
        let sql = r#"
            SELECT a.id, b.name
            FROM table_a a
            JOIN table_b b ON a.id = b.id
        "#;
        let result = parse_sql(sql, DialectConfig::YourDialect);
        assert!(result.is_ok());
    }

    #[test]
    fn test_type_mapping() {
        assert_eq!(
            map_your_dialect_type("INTEGER"),
            LogicalType::Int64
        );
    }
}
```

### Integration Tests

Test against real SQL files:

```rust
#[test]
fn test_real_your_dialect_model() {
    let sql = std::fs::read_to_string("tests/fixtures/your_dialect_model.sql").unwrap();
    let parsed = parse_sql(&sql, DialectConfig::YourDialect).unwrap();

    // Verify expected columns
    assert!(parsed.selected_columns.iter().any(|c| c.name == "user_id"));
}
```

## Configuration

Users configure the dialect in `schemarefly.toml`:

```toml
# Choose dialect
dialect = "your_dialect"

# Warehouse configuration (for drift detection)
[warehouse]
type = "your_warehouse"
# ... warehouse-specific settings
```

## Limitations of Current Approach

The current enum-based approach has limitations:

1. **Not Extensible**: Adding dialects requires modifying core code
2. **No Versioning**: Can't have multiple versions of same dialect
3. **No Third-Party**: Can't distribute custom dialects as separate crates

## Future: Plugin System

A future enhancement would add a trait-based plugin system:

```rust
// Future design (not yet implemented)

pub trait DialectPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, sql: &str) -> Result<ParsedSql>;
    fn map_type(&self, type_str: &str) -> LogicalType;
}

// Users could implement custom dialects
struct MyCustomDialect;

impl DialectPlugin for MyCustomDialect {
    fn name(&self) -> &'static str { "my_custom" }
    fn parse(&self, sql: &str) -> Result<ParsedSql> { ... }
    fn map_type(&self, type_str: &str) -> LogicalType { ... }
}
```

This would enable:
- Third-party dialect crates
- Runtime dialect registration
- Versioned dialect implementations
- Proprietary dialect support without code changes

**Status**: This plugin system is not yet implemented. See `SchemaRefly Engineering Doc.md` Phase 8 for tracking.

## Examples

### Complete Example: Adding Presto Support

1. **Add enum variant**:
```rust
pub enum DialectConfig {
    // ...
    Presto,
}
```

2. **Map to sqlparser**:
```rust
DialectConfig::Presto => Dialect::Generic,  // Presto not in sqlparser, use Generic
```

3. **Add type mapper** (`crates/schemarefly-catalog/src/presto_types.rs`):
```rust
pub fn map_presto_type(type_str: &str) -> LogicalType {
    match type_str.to_uppercase().as_str() {
        "INTEGER" => LogicalType::Int64,
        "BIGINT" => LogicalType::Int64,
        "DOUBLE" => LogicalType::Float64,
        "VARCHAR" => LogicalType::String,
        "BOOLEAN" => LogicalType::Bool,
        "TIMESTAMP" => LogicalType::Timestamp,
        "DATE" => LogicalType::Date,
        "JSON" => LogicalType::Json,
        unknown => LogicalType::Unknown(unknown.to_string()),
    }
}
```

4. **Test**:
```rust
#[test]
fn test_presto_syntax() {
    let sql = "SELECT CAST(user_id AS INTEGER) FROM users";
    let parsed = parse_sql(sql, DialectConfig::Presto).unwrap();
    assert!(parsed.is_ok());
}
```

## Contributing

To contribute a new dialect:

1. Follow this guide to implement dialect support
2. Add comprehensive tests with real SQL examples
3. Document dialect-specific behavior
4. Submit pull request

## See Also

- [Warehouse Adapter Guide](WAREHOUSE_ADAPTER_GUIDE.md) - For warehouse integration
- [sqlparser-rs Documentation](https://docs.rs/sqlparser/) - SQL parser library
- [Engineering Doc](SchemaRefly%20Engineering%20Doc.md) - Overall architecture

## License

Dialect extensions inherit SchemaRefly's license (MIT OR Apache-2.0).
