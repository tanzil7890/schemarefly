//! SQL parsing using datafusion-sqlparser-rs
//!
//! Parses SQL into AST and provides error handling with diagnostics.

use sqlparser::ast::{Statement, Query};
use sqlparser::dialect::{Dialect, GenericDialect, BigQueryDialect, PostgreSqlDialect, SnowflakeDialect};
use sqlparser::parser::{Parser, ParserError};
use schemarefly_core::{Diagnostic, DiagnosticCode, Severity, Location};
use std::path::Path;

/// SQL parser with configurable dialect
pub struct SqlParser {
    dialect: Box<dyn Dialect>,
}

impl SqlParser {
    /// Create a new SQL parser with the default (generic) dialect
    pub fn new() -> Self {
        Self {
            dialect: Box::new(GenericDialect {}),
        }
    }

    /// Create a SQL parser for BigQuery
    pub fn bigquery() -> Self {
        Self {
            dialect: Box::new(BigQueryDialect {}),
        }
    }

    /// Create a SQL parser for PostgreSQL
    pub fn postgres() -> Self {
        Self {
            dialect: Box::new(PostgreSqlDialect {}),
        }
    }

    /// Create a SQL parser for Snowflake
    pub fn snowflake() -> Self {
        Self {
            dialect: Box::new(SnowflakeDialect {}),
        }
    }

    /// Create a parser from a dialect config
    pub fn from_dialect(dialect: &schemarefly_core::DialectConfig) -> Self {
        match dialect {
            schemarefly_core::DialectConfig::BigQuery => Self::bigquery(),
            schemarefly_core::DialectConfig::Snowflake => Self::snowflake(),
            schemarefly_core::DialectConfig::Postgres => Self::postgres(),
            schemarefly_core::DialectConfig::Ansi => Self::new(),
        }
    }

    /// Parse SQL string into AST
    ///
    /// Returns ParsedSql on success, or ParseError with diagnostic on failure.
    pub fn parse(&self, sql: &str, file_path: Option<&Path>) -> Result<ParsedSql, ParseError> {
        let result = Parser::parse_sql(&*self.dialect, sql);

        match result {
            Ok(statements) => Ok(ParsedSql {
                sql: sql.to_string(),
                statements,
                file_path: file_path.map(|p| p.to_path_buf()),
            }),
            Err(e) => Err(ParseError {
                sql: sql.to_string(),
                error: e,
                file_path: file_path.map(|p| p.to_path_buf()),
            }),
        }
    }

    /// Parse SQL from a file
    pub fn parse_file(&self, path: &Path) -> Result<ParsedSql, ParseError> {
        let sql = std::fs::read_to_string(path)
            .map_err(|e| ParseError {
                sql: String::new(),
                error: ParserError::ParserError(format!("Failed to read file: {}", e)),
                file_path: Some(path.to_path_buf()),
            })?;

        self.parse(&sql, Some(path))
    }

    /// Parse SQL and return diagnostic on error
    ///
    /// This is the preferred method for integration with SchemaRefly's diagnostic system.
    pub fn parse_with_diagnostic(
        &self,
        sql: &str,
        file_path: Option<&Path>,
    ) -> Result<ParsedSql, Diagnostic> {
        self.parse(sql, file_path)
            .map_err(|e| e.to_diagnostic())
    }
}

impl Default for SqlParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Successfully parsed SQL with AST
#[derive(Debug, Clone)]
pub struct ParsedSql {
    /// Original SQL string
    pub sql: String,

    /// Parsed statements
    pub statements: Vec<Statement>,

    /// Source file path (if parsed from file)
    pub file_path: Option<std::path::PathBuf>,
}

impl ParsedSql {
    /// Get the first statement (most common for dbt models)
    pub fn first_statement(&self) -> Option<&Statement> {
        self.statements.first()
    }

    /// Check if this is a SELECT statement
    pub fn is_select(&self) -> bool {
        matches!(
            self.first_statement(),
            Some(Statement::Query(_))
        )
    }

    /// Get the query if this is a SELECT statement
    pub fn as_query(&self) -> Option<&Query> {
        match self.first_statement() {
            Some(Statement::Query(query)) => Some(query.as_ref()),
            _ => None,
        }
    }

    /// Count the number of statements
    pub fn statement_count(&self) -> usize {
        self.statements.len()
    }
}

/// SQL parsing error with diagnostic information
#[derive(Debug)]
pub struct ParseError {
    /// Original SQL string
    pub sql: String,

    /// Parser error from sqlparser
    pub error: ParserError,

    /// Source file path
    pub file_path: Option<std::path::PathBuf>,
}

impl ParseError {
    /// Convert to a SchemaRefly diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        let message = format!("Failed to parse SQL: {}", self.error);

        let location = self.file_path.as_ref().map(|path| {
            // Try to extract line number from error message if available
            Location::new(path.display().to_string())
        });

        let mut diag = Diagnostic::new(
            DiagnosticCode::SqlParseError,
            Severity::Error,
            message,
        );

        if let Some(loc) = location {
            diag = diag.with_location(loc);
        }

        diag
    }

    /// Check if this is an unsupported syntax error
    pub fn is_unsupported_syntax(&self) -> bool {
        let error_msg = self.error.to_string().to_lowercase();
        error_msg.contains("expected") || error_msg.contains("unexpected")
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SQL parse error: {}", self.error)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_select() {
        let parser = SqlParser::new();
        let sql = "SELECT id, name FROM users WHERE active = true";

        let result = parser.parse(sql, None);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.statement_count(), 1);
        assert!(parsed.is_select());
    }

    #[test]
    fn parse_with_cte() {
        let parser = SqlParser::new();
        let sql = r#"
            WITH active_users AS (
                SELECT * FROM users WHERE active = true
            )
            SELECT id, name FROM active_users
        "#;

        let result = parser.parse(sql, None);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(parsed.is_select());
    }

    #[test]
    fn parse_invalid_sql() {
        let parser = SqlParser::new();
        let sql = "SELECT FROM WHERE";

        let result = parser.parse(sql, None);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.is_unsupported_syntax());

        // Can convert to diagnostic
        let diag = error.to_diagnostic();
        assert_eq!(diag.code, DiagnosticCode::SqlParseError);
        assert_eq!(diag.severity, Severity::Error);
    }

    #[test]
    fn parse_dbt_style_sql() {
        let parser = SqlParser::new();
        let sql = r#"
            SELECT
                id,
                name,
                email
            FROM {{ ref('users') }}
            WHERE deleted_at IS NULL
        "#;

        // Note: This will fail to parse because {{ }} is not standard SQL
        // But we handle it gracefully
        let result = parser.parse(sql, None);
        assert!(result.is_err());

        // We'll need to preprocess dbt templates before parsing
    }

    #[test]
    fn different_dialects() {
        let sql = "SELECT id FROM users";

        let generic = SqlParser::new();
        let bigquery = SqlParser::bigquery();
        let postgres = SqlParser::postgres();
        let snowflake = SqlParser::snowflake();

        // All should parse simple SQL
        assert!(generic.parse(sql, None).is_ok());
        assert!(bigquery.parse(sql, None).is_ok());
        assert!(postgres.parse(sql, None).is_ok());
        assert!(snowflake.parse(sql, None).is_ok());
    }

    #[test]
    fn parse_fixture_sql() {
        let parser = SqlParser::new();
        let path = Path::new("../../fixtures/mini-dbt-project/models/users.sql");

        if path.exists() {
            // Note: This will fail because of {{ }} template syntax
            // We'll handle this in the dbt_functions module
            let result = parser.parse_file(path);

            if let Err(e) = result {
                // Expected to fail on template syntax
                assert!(e.error.to_string().contains("Expected"));
            }
        }
    }
}
