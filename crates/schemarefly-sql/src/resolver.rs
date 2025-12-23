//! Name resolution for CTEs, aliases, and table references
//!
//! Resolves names in SQL queries to their definitions.

use sqlparser::ast::{Statement, Query, SetExpr, Select, TableFactor, Cte, SelectItem, Expr};
use std::collections::HashMap;

/// A resolved name in SQL
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedName {
    /// A CTE (Common Table Expression)
    Cte {
        name: String,
        /// Position in the query where it's defined
        definition_index: usize,
    },

    /// A column alias in SELECT
    ColumnAlias {
        alias: String,
        /// Original expression
        original: String,
    },

    /// A table reference
    Table {
        name: String,
        /// Alias if present
        alias: Option<String>,
    },
}

/// Name resolver for SQL queries
pub struct NameResolver {
    /// Map of CTE names to their definitions
    ctes: HashMap<String, usize>,

    /// Map of table names/aliases
    tables: HashMap<String, ResolvedName>,

    /// Map of column aliases
    columns: HashMap<String, ResolvedName>,
}

impl NameResolver {
    /// Create a new name resolver
    pub fn new() -> Self {
        Self {
            ctes: HashMap::new(),
            tables: HashMap::new(),
            columns: HashMap::new(),
        }
    }

    /// Resolve names in a parsed SQL statement
    pub fn resolve(&mut self, statement: &Statement) -> Result<(), ResolverError> {
        match statement {
            Statement::Query(query) => {
                self.resolve_query(query)?;
            }
            _ => {
                // Other statement types not yet supported
            }
        }

        Ok(())
    }

    /// Resolve names in a query
    fn resolve_query(&mut self, query: &Query) -> Result<(), ResolverError> {
        // First, resolve CTEs
        if let Some(with) = &query.with {
            for (i, cte) in with.cte_tables.iter().enumerate() {
                self.resolve_cte(cte, i)?;
            }
        }

        // Then resolve the main query body
        self.resolve_set_expr(&query.body)?;

        Ok(())
    }

    /// Resolve a CTE
    fn resolve_cte(&mut self, cte: &Cte, index: usize) -> Result<(), ResolverError> {
        let name = cte.alias.name.value.clone();

        // Check for duplicate CTE names
        if self.ctes.contains_key(&name) {
            return Err(ResolverError::DuplicateCte(name));
        }

        self.ctes.insert(name.clone(), index);

        // Resolve the CTE's query
        self.resolve_query(&cte.query)?;

        Ok(())
    }

    /// Resolve names in a set expression
    fn resolve_set_expr(&mut self, set_expr: &SetExpr) -> Result<(), ResolverError> {
        match set_expr {
            SetExpr::Select(select) => {
                self.resolve_select(select)?;
            }
            SetExpr::Query(query) => {
                self.resolve_query(query)?;
            }
            SetExpr::SetOperation { left, right, .. } => {
                self.resolve_set_expr(left)?;
                self.resolve_set_expr(right)?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Resolve names in a SELECT statement
    fn resolve_select(&mut self, select: &Select) -> Result<(), ResolverError> {
        // Resolve FROM clause first (tables and CTEs)
        for table_with_joins in &select.from {
            self.resolve_table_factor(&table_with_joins.relation)?;

            for join in &table_with_joins.joins {
                self.resolve_table_factor(&join.relation)?;
            }
        }

        // Resolve SELECT list (column aliases)
        for item in &select.projection {
            self.resolve_select_item(item)?;
        }

        Ok(())
    }

    /// Resolve a table factor (table, subquery, CTE reference)
    fn resolve_table_factor(&mut self, table_factor: &TableFactor) -> Result<(), ResolverError> {
        match table_factor {
            TableFactor::Table { name, alias, .. } => {
                let table_name = name.to_string();

                // Check if this is a CTE reference
                if self.ctes.contains_key(&table_name) {
                    // It's a CTE - already resolved
                } else {
                    // It's a table reference
                    let resolved = ResolvedName::Table {
                        name: table_name.clone(),
                        alias: alias.as_ref().map(|a| a.name.value.clone()),
                    };

                    let key = alias
                        .as_ref()
                        .map(|a| a.name.value.clone())
                        .unwrap_or_else(|| table_name.clone());

                    self.tables.insert(key, resolved);
                }
            }
            TableFactor::Derived { alias, subquery, .. } => {
                // Resolve subquery
                self.resolve_query(subquery)?;

                if let Some(alias) = alias {
                    let resolved = ResolvedName::Table {
                        name: format!("(subquery)"),
                        alias: Some(alias.name.value.clone()),
                    };

                    self.tables.insert(alias.name.value.clone(), resolved);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Resolve a select item (column or alias)
    fn resolve_select_item(&mut self, item: &SelectItem) -> Result<(), ResolverError> {
        match item {
            SelectItem::UnnamedExpr(expr) => {
                // No alias - just resolve the expression
                self.resolve_expr(expr)?;
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                // Resolve expression and register alias
                let original = Self::expr_to_string(expr);

                let resolved = ResolvedName::ColumnAlias {
                    alias: alias.value.clone(),
                    original,
                };

                self.columns.insert(alias.value.clone(), resolved);
                self.resolve_expr(expr)?;
            }
            SelectItem::Wildcard(_) => {
                // SELECT * - no specific resolution needed
            }
            SelectItem::QualifiedWildcard(name, _) => {
                // SELECT table.* - verify table exists
                let table_name = name.to_string();
                if !self.tables.contains_key(&table_name) && !self.ctes.contains_key(&table_name) {
                    return Err(ResolverError::UnresolvedTable(table_name));
                }
            }
        }

        Ok(())
    }

    /// Resolve an expression (basic implementation)
    fn resolve_expr(&self, _expr: &Expr) -> Result<(), ResolverError> {
        // For now, we don't deeply resolve expressions
        // This would be expanded in future phases for type inference
        Ok(())
    }

    /// Convert expression to string for diagnostics
    fn expr_to_string(expr: &Expr) -> String {
        format!("{}", expr)
    }

    /// Get all resolved CTEs
    pub fn get_ctes(&self) -> &HashMap<String, usize> {
        &self.ctes
    }

    /// Get all resolved tables
    pub fn get_tables(&self) -> &HashMap<String, ResolvedName> {
        &self.tables
    }

    /// Get all resolved column aliases
    pub fn get_column_aliases(&self) -> &HashMap<String, ResolvedName> {
        &self.columns
    }

    /// Check if a name is a CTE
    pub fn is_cte(&self, name: &str) -> bool {
        self.ctes.contains_key(name)
    }

    /// Check if a name is a table
    pub fn is_table(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    /// Check if a name is a column alias
    pub fn is_column_alias(&self, name: &str) -> bool {
        self.columns.contains_key(name)
    }
}

impl Default for NameResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Name resolution errors
#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("Duplicate CTE name: {0}")]
    DuplicateCte(String),

    #[error("Unresolved table reference: {0}")]
    UnresolvedTable(String),

    #[error("Ambiguous reference: {0}")]
    AmbiguousReference(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::SqlParser;

    #[test]
    fn resolve_simple_select() {
        let parser = SqlParser::new();
        let sql = "SELECT id, name AS user_name FROM users";

        let parsed = parser.parse(sql, None).unwrap();
        let mut resolver = NameResolver::new();

        resolver.resolve(parsed.first_statement().unwrap()).unwrap();

        // Should have one table
        assert_eq!(resolver.get_tables().len(), 1);
        assert!(resolver.is_table("users"));

        // Should have one column alias
        assert_eq!(resolver.get_column_aliases().len(), 1);
        assert!(resolver.is_column_alias("user_name"));
    }

    #[test]
    fn resolve_with_cte() {
        let parser = SqlParser::new();
        let sql = r#"
            WITH active_users AS (
                SELECT * FROM users WHERE active = true
            )
            SELECT id, name FROM active_users
        "#;

        let parsed = parser.parse(sql, None).unwrap();
        let mut resolver = NameResolver::new();

        resolver.resolve(parsed.first_statement().unwrap()).unwrap();

        // Should have one CTE
        assert_eq!(resolver.get_ctes().len(), 1);
        assert!(resolver.is_cte("active_users"));
    }

    #[test]
    fn resolve_multiple_ctes() {
        let parser = SqlParser::new();
        let sql = r#"
            WITH
                base AS (SELECT * FROM users),
                filtered AS (SELECT * FROM base WHERE active = true)
            SELECT * FROM filtered
        "#;

        let parsed = parser.parse(sql, None).unwrap();
        let mut resolver = NameResolver::new();

        resolver.resolve(parsed.first_statement().unwrap()).unwrap();

        // Should have two CTEs
        assert_eq!(resolver.get_ctes().len(), 2);
        assert!(resolver.is_cte("base"));
        assert!(resolver.is_cte("filtered"));
    }

    #[test]
    fn resolve_with_aliases() {
        let parser = SqlParser::new();
        let sql = "SELECT id, name AS user_name, email AS user_email FROM users AS u";

        let parsed = parser.parse(sql, None).unwrap();
        let mut resolver = NameResolver::new();

        resolver.resolve(parsed.first_statement().unwrap()).unwrap();

        // Should have one table with alias
        assert_eq!(resolver.get_tables().len(), 1);
        assert!(resolver.is_table("u"));

        // Should have two column aliases
        assert_eq!(resolver.get_column_aliases().len(), 2);
        assert!(resolver.is_column_alias("user_name"));
        assert!(resolver.is_column_alias("user_email"));
    }
}
