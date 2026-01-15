//! PostgreSQL warehouse adapter using information_schema
//!
//! This adapter queries PostgreSQL's information_schema.columns view to fetch
//! table schemas. It works with:
//! - PostgreSQL 9.4+
//! - Amazon Redshift
//! - CockroachDB
//! - Other PostgreSQL-compatible databases
//!
//! ## Authentication
//!
//! The adapter supports multiple authentication methods:
//! 1. Direct password authentication
//! 2. Connection string (PostgreSQL URL format)
//! 3. TLS/SSL connections via native-tls
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Using direct credentials
//! let adapter = PostgresAdapter::connect(
//!     "localhost",
//!     5432,
//!     "mydb",
//!     "username",
//!     "password"
//! ).await?;
//!
//! // Using connection string
//! let adapter = PostgresAdapter::from_connection_string(
//!     "host=localhost port=5432 dbname=mydb user=username password=password"
//! ).await?;
//!
//! // Using connection string with SSL
//! let adapter = PostgresAdapter::from_connection_string_with_tls(
//!     "host=localhost port=5432 dbname=mydb user=username password=password sslmode=require"
//! ).await?;
//! ```
//!
//! Reference: https://www.postgresql.org/docs/current/information-schema-columns.html

use crate::adapter::{WarehouseAdapter, TableIdentifier, FetchError};
use schemarefly_core::{Schema, LogicalType};

#[cfg(feature = "postgres")]
use tokio_postgres::{Client, NoTls, Config as PgConfig};

#[cfg(feature = "postgres")]
use postgres_native_tls::MakeTlsConnector;

#[cfg(feature = "postgres")]
use native_tls::TlsConnector;

/// PostgreSQL warehouse adapter
///
/// This adapter connects to PostgreSQL databases and fetches table schemas
/// from information_schema.columns. It supports both plain and TLS connections.
pub struct PostgresAdapter {
    /// PostgreSQL client (only available with postgres feature)
    #[cfg(feature = "postgres")]
    client: Client,

    /// Connection host
    host: String,

    /// Connection port
    port: u16,

    /// Database name
    database: String,

    /// Placeholder for when feature is disabled
    #[cfg(not(feature = "postgres"))]
    _phantom: std::marker::PhantomData<()>,
}

impl PostgresAdapter {
    /// Create a new PostgreSQL adapter with direct credentials
    ///
    /// This method establishes a connection using host, port, database, username,
    /// and password. For TLS connections, use `connect_with_tls` instead.
    ///
    /// # Arguments
    ///
    /// * `host` - PostgreSQL server hostname or IP
    /// * `port` - PostgreSQL server port (usually 5432)
    /// * `database` - Database name to connect to
    /// * `user` - Username for authentication
    /// * `password` - Password for authentication
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let adapter = PostgresAdapter::connect(
    ///     "localhost", 5432, "mydb", "postgres", "password"
    /// ).await?;
    /// ```
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
                "Failed to connect to PostgreSQL at {}:{}: {}",
                host, port, e
            )))?;

        // Spawn connection handler in background
        let host_clone = host.clone();
        let port_clone = port;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error ({}:{}): {}", host_clone, port_clone, e);
            }
        });

        Ok(Self {
            client,
            host,
            port,
            database,
        })
    }

    /// Create adapter without postgres feature (returns error)
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

    /// Create a PostgreSQL adapter with TLS support
    ///
    /// This method establishes a secure TLS connection to PostgreSQL.
    /// Use this for production environments where data encryption is required.
    ///
    /// # Arguments
    ///
    /// * `host` - PostgreSQL server hostname or IP
    /// * `port` - PostgreSQL server port (usually 5432)
    /// * `database` - Database name to connect to
    /// * `user` - Username for authentication
    /// * `password` - Password for authentication
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let adapter = PostgresAdapter::connect_with_tls(
    ///     "db.example.com", 5432, "mydb", "postgres", "password"
    /// ).await?;
    /// ```
    #[cfg(feature = "postgres")]
    pub async fn connect_with_tls(
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

        // Create TLS connector
        let connector = TlsConnector::builder()
            .build()
            .map_err(|e| FetchError::ConfigError(format!(
                "Failed to create TLS connector: {}", e
            )))?;

        let tls = MakeTlsConnector::new(connector);

        let (client, connection) = tokio_postgres::connect(&config, tls)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to connect to PostgreSQL at {}:{} with TLS: {}",
                host, port, e
            )))?;

        // Spawn connection handler in background
        let host_clone = host.clone();
        let port_clone = port;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL TLS connection error ({}:{}): {}", host_clone, port_clone, e);
            }
        });

        Ok(Self {
            client,
            host,
            port,
            database,
        })
    }

    /// Create adapter without postgres feature (returns error)
    #[cfg(not(feature = "postgres"))]
    pub async fn connect_with_tls(
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

    /// Create adapter from a PostgreSQL connection string
    ///
    /// Supports standard PostgreSQL connection string format:
    /// `host=localhost port=5432 dbname=mydb user=postgres password=secret`
    ///
    /// # Arguments
    ///
    /// * `conn_str` - PostgreSQL connection string
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let adapter = PostgresAdapter::from_connection_string(
    ///     "host=localhost port=5432 dbname=mydb user=postgres password=secret"
    /// ).await?;
    /// ```
    #[cfg(feature = "postgres")]
    pub async fn from_connection_string(conn_str: &str) -> Result<Self, FetchError> {
        let config: PgConfig = conn_str.parse()
            .map_err(|e| FetchError::ConfigError(format!(
                "Invalid connection string: {}", e
            )))?;

        // Extract connection info for logging
        let host = config.get_hosts()
            .first()
            .map(|h| format!("{:?}", h))
            .unwrap_or_else(|| "localhost".to_string());
        let port = config.get_ports()
            .first()
            .copied()
            .unwrap_or(5432);
        let database = config.get_dbname()
            .unwrap_or("postgres")
            .to_string();

        let (client, connection) = tokio_postgres::connect(conn_str, NoTls)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to connect: {}", e
            )))?;

        // Spawn connection handler in background
        let host_clone = host.clone();
        let port_clone = port;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error ({}:{}): {}", host_clone, port_clone, e);
            }
        });

        Ok(Self {
            client,
            host,
            port,
            database,
        })
    }

    /// Create adapter without postgres feature (returns error)
    #[cfg(not(feature = "postgres"))]
    pub async fn from_connection_string(_conn_str: &str) -> Result<Self, FetchError> {
        Err(FetchError::ConfigError(
            "PostgreSQL support not compiled. Rebuild with: cargo build --features postgres".to_string()
        ))
    }

    /// Create adapter from a PostgreSQL connection string with TLS
    ///
    /// Use this for secure connections to remote PostgreSQL servers.
    ///
    /// # Arguments
    ///
    /// * `conn_str` - PostgreSQL connection string (sslmode setting is ignored, TLS is always used)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let adapter = PostgresAdapter::from_connection_string_with_tls(
    ///     "host=db.example.com port=5432 dbname=mydb user=postgres password=secret"
    /// ).await?;
    /// ```
    #[cfg(feature = "postgres")]
    pub async fn from_connection_string_with_tls(conn_str: &str) -> Result<Self, FetchError> {
        let config: PgConfig = conn_str.parse()
            .map_err(|e| FetchError::ConfigError(format!(
                "Invalid connection string: {}", e
            )))?;

        // Extract connection info for logging
        let host = config.get_hosts()
            .first()
            .map(|h| format!("{:?}", h))
            .unwrap_or_else(|| "localhost".to_string());
        let port = config.get_ports()
            .first()
            .copied()
            .unwrap_or(5432);
        let database = config.get_dbname()
            .unwrap_or("postgres")
            .to_string();

        // Create TLS connector
        let connector = TlsConnector::builder()
            .build()
            .map_err(|e| FetchError::ConfigError(format!(
                "Failed to create TLS connector: {}", e
            )))?;

        let tls = MakeTlsConnector::new(connector);

        let (client, connection) = tokio_postgres::connect(conn_str, tls)
            .await
            .map_err(|e| FetchError::AuthenticationError(format!(
                "Failed to connect with TLS: {}", e
            )))?;

        // Spawn connection handler in background
        let host_clone = host.clone();
        let port_clone = port;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL TLS connection error ({}:{}): {}", host_clone, port_clone, e);
            }
        });

        Ok(Self {
            client,
            host,
            port,
            database,
        })
    }

    /// Create adapter without postgres feature (returns error)
    #[cfg(not(feature = "postgres"))]
    pub async fn from_connection_string_with_tls(_conn_str: &str) -> Result<Self, FetchError> {
        Err(FetchError::ConfigError(
            "PostgreSQL support not compiled. Rebuild with: cargo build --features postgres".to_string()
        ))
    }

    /// Placeholder constructor for backward compatibility when feature is disabled
    #[cfg(not(feature = "postgres"))]
    pub fn new_disabled() -> Self {
        Self {
            host: String::new(),
            port: 0,
            database: String::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Convert PostgreSQL type to LogicalType
    ///
    /// This method handles all standard PostgreSQL data types and maps them
    /// to the appropriate LogicalType for schema comparison.
    ///
    /// # Supported Types
    ///
    /// - **Boolean**: `boolean`, `bool`
    /// - **Integer**: `smallint`, `integer`, `bigint`, `serial`, `bigserial`
    /// - **Floating Point**: `real`, `double precision`
    /// - **Numeric**: `numeric(p,s)`, `decimal(p,s)`, `money`
    /// - **String**: `varchar`, `char`, `text`, `name`
    /// - **Binary**: `bytea`
    /// - **Date/Time**: `date`, `timestamp`, `timestamptz`, `time`, `interval`
    /// - **JSON**: `json`, `jsonb`
    /// - **Array**: `type[]` notation
    /// - **UUID**: `uuid`
    /// - **Network**: `inet`, `cidr`, `macaddr`
    /// - **Geometry**: `point`, `line`, `polygon`, etc.
    pub fn map_postgres_type(pg_type: &str) -> LogicalType {
        let base_type = pg_type.split('(').next()
            .unwrap_or(pg_type)
            .trim()
            .to_lowercase();

        match base_type.as_str() {
            // Boolean types
            "boolean" | "bool" => LogicalType::Bool,

            // Integer types
            "smallint" | "int2" => LogicalType::Int,
            "integer" | "int" | "int4" => LogicalType::Int,
            "bigint" | "int8" => LogicalType::Int,
            "serial" | "serial4" => LogicalType::Int,
            "bigserial" | "serial8" => LogicalType::Int,
            "smallserial" | "serial2" => LogicalType::Int,

            // Floating point types
            "real" | "float4" => LogicalType::Float,
            "double precision" | "float8" | "float" => LogicalType::Float,

            // Numeric/Decimal types
            "numeric" | "decimal" => Self::parse_numeric_type(pg_type),

            // Money type (fixed precision)
            "money" => LogicalType::Decimal {
                precision: Some(19),
                scale: Some(2),
            },

            // String types
            "character varying" | "varchar" => LogicalType::String,
            "character" | "char" | "bpchar" => LogicalType::String,
            "text" => LogicalType::String,
            "name" => LogicalType::String,
            "citext" => LogicalType::String, // Case-insensitive text extension

            // Binary data
            "bytea" => LogicalType::String, // Mapped to string for compatibility

            // Date/Time types
            "date" => LogicalType::Date,
            "timestamp without time zone" | "timestamp" => LogicalType::Timestamp,
            "timestamp with time zone" | "timestamptz" => LogicalType::Timestamp,
            "time without time zone" | "time" => LogicalType::Timestamp,
            "time with time zone" | "timetz" => LogicalType::Timestamp,
            "interval" => LogicalType::String, // Interval as string

            // JSON types
            "json" | "jsonb" => LogicalType::Json,

            // UUID
            "uuid" => LogicalType::String,

            // XML
            "xml" => LogicalType::String,

            // Generic array type (without element type)
            "array" => LogicalType::Array {
                element_type: Box::new(LogicalType::Unknown),
            },

            // Geometric types
            "point" | "line" | "lseg" | "box" | "path" | "polygon" | "circle" => {
                LogicalType::String // Geometry types represented as string
            }

            // Network address types
            "inet" | "cidr" | "macaddr" | "macaddr8" => LogicalType::String,

            // Bit string types
            "bit" | "bit varying" | "varbit" => LogicalType::String,

            // Text search types
            "tsvector" | "tsquery" => LogicalType::String,

            // Range types
            "int4range" | "int8range" | "numrange" | "tsrange" | "tstzrange" | "daterange" => {
                LogicalType::String // Range types as string
            }

            // Object identifiers
            "oid" | "regclass" | "regproc" | "regtype" | "regnamespace" => LogicalType::Int,

            // PostgreSQL internal types
            "pg_lsn" | "pg_snapshot" => LogicalType::String,

            _ => {
                // Handle array notation like "integer[]" or "_int4"
                if pg_type.ends_with("[]") {
                    let element_type_str = &pg_type[..pg_type.len() - 2];
                    LogicalType::Array {
                        element_type: Box::new(Self::map_postgres_type(element_type_str)),
                    }
                } else if pg_type.starts_with('_') {
                    // PostgreSQL internal array notation (e.g., _int4 for int4[])
                    let element_type_str = &pg_type[1..];
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
    ///
    /// Handles types like:
    /// - `numeric` - arbitrary precision
    /// - `numeric(10)` - precision 10, scale 0
    /// - `numeric(10,2)` - precision 10, scale 2
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

    /// Get the connection host
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Get the connection port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the database name
    pub fn database(&self) -> &str {
        &self.database
    }
}

#[async_trait::async_trait]
impl WarehouseAdapter for PostgresAdapter {
    fn name(&self) -> &'static str {
        "PostgreSQL"
    }

    #[cfg(feature = "postgres")]
    async fn fetch_schema(&self, table: &TableIdentifier) -> Result<Schema, FetchError> {
        // Query information_schema.columns for the table schema
        // Using parameterized query to prevent SQL injection
        let query = r#"
            SELECT
                column_name,
                data_type,
                is_nullable,
                ordinal_position,
                numeric_precision,
                numeric_scale,
                udt_name,
                character_maximum_length
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
                    FetchError::PermissionDenied(format!(
                        "Cannot access {}: {}",
                        table.fqn(), err_str
                    ))
                } else {
                    FetchError::QueryError(err_str)
                }
            })?;

        let mut columns = Vec::new();

        for row in rows {
            // Get column values
            let col_name: String = row.get(0);
            let data_type: String = row.get(1);
            let is_nullable: String = row.get(2);
            let numeric_precision: Option<i32> = row.get(4);
            let numeric_scale: Option<i32> = row.get(5);
            let udt_name: String = row.get(6);

            // Build full type string for numeric types with precision/scale
            let full_type = if data_type == "numeric" || data_type == "decimal" {
                match (numeric_precision, numeric_scale) {
                    (Some(p), Some(s)) => format!("numeric({},{})", p, s),
                    (Some(p), None) => format!("numeric({})", p),
                    _ => data_type.clone(),
                }
            } else if udt_name.starts_with('_') {
                // Array type - convert _int4 to int4[]
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
        // Simple query to test connection
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
    fn test_basic_type_mapping() {
        // Boolean
        assert!(matches!(PostgresAdapter::map_postgres_type("boolean"), LogicalType::Bool));
        assert!(matches!(PostgresAdapter::map_postgres_type("bool"), LogicalType::Bool));

        // Integer types
        assert!(matches!(PostgresAdapter::map_postgres_type("integer"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("int"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("int4"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("bigint"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("int8"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("smallint"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("serial"), LogicalType::Int));
        assert!(matches!(PostgresAdapter::map_postgres_type("bigserial"), LogicalType::Int));

        // Float types
        assert!(matches!(PostgresAdapter::map_postgres_type("real"), LogicalType::Float));
        assert!(matches!(PostgresAdapter::map_postgres_type("float4"), LogicalType::Float));
        assert!(matches!(PostgresAdapter::map_postgres_type("double precision"), LogicalType::Float));
        assert!(matches!(PostgresAdapter::map_postgres_type("float8"), LogicalType::Float));
    }

    #[test]
    fn test_string_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("text"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("varchar"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("character varying"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("char"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("character"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("name"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("uuid"), LogicalType::String));
    }

    #[test]
    fn test_datetime_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("date"), LogicalType::Date));
        assert!(matches!(PostgresAdapter::map_postgres_type("timestamp"), LogicalType::Timestamp));
        assert!(matches!(PostgresAdapter::map_postgres_type("timestamp without time zone"), LogicalType::Timestamp));
        assert!(matches!(PostgresAdapter::map_postgres_type("timestamptz"), LogicalType::Timestamp));
        assert!(matches!(PostgresAdapter::map_postgres_type("timestamp with time zone"), LogicalType::Timestamp));
        assert!(matches!(PostgresAdapter::map_postgres_type("time"), LogicalType::Timestamp));
    }

    #[test]
    fn test_json_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("json"), LogicalType::Json));
        assert!(matches!(PostgresAdapter::map_postgres_type("jsonb"), LogicalType::Json));
    }

    #[test]
    fn test_numeric_type_parsing() {
        // Basic numeric without precision
        match PostgresAdapter::map_postgres_type("numeric") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, None);
                assert_eq!(scale, None);
            }
            _ => panic!("Expected Decimal type"),
        }

        // Numeric with precision and scale
        match PostgresAdapter::map_postgres_type("numeric(10,2)") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(10));
                assert_eq!(scale, Some(2));
            }
            _ => panic!("Expected Decimal type"),
        }

        // Numeric with precision only
        match PostgresAdapter::map_postgres_type("numeric(10)") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(10));
                assert_eq!(scale, Some(0));
            }
            _ => panic!("Expected Decimal type"),
        }

        // Money type
        match PostgresAdapter::map_postgres_type("money") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(19));
                assert_eq!(scale, Some(2));
            }
            _ => panic!("Expected Decimal type"),
        }
    }

    #[test]
    fn test_array_type_mapping() {
        // Standard array notation
        match PostgresAdapter::map_postgres_type("integer[]") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::Int));
            }
            _ => panic!("Expected Array type"),
        }

        match PostgresAdapter::map_postgres_type("text[]") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::String));
            }
            _ => panic!("Expected Array type"),
        }

        // PostgreSQL internal array notation
        match PostgresAdapter::map_postgres_type("_int4") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::Int));
            }
            _ => panic!("Expected Array type"),
        }

        match PostgresAdapter::map_postgres_type("_text") {
            LogicalType::Array { element_type } => {
                assert!(matches!(*element_type, LogicalType::String)); // _text -> text[]
            }
            _ => panic!("Expected Array type"),
        }
    }

    #[test]
    fn test_network_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("inet"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("cidr"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("macaddr"), LogicalType::String));
    }

    #[test]
    fn test_geometric_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("point"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("line"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("polygon"), LogicalType::String));
        assert!(matches!(PostgresAdapter::map_postgres_type("circle"), LogicalType::String));
    }

    #[test]
    fn test_unknown_type_mapping() {
        assert!(matches!(PostgresAdapter::map_postgres_type("custom_type"), LogicalType::Unknown));
        assert!(matches!(PostgresAdapter::map_postgres_type("some_extension_type"), LogicalType::Unknown));
    }
}
