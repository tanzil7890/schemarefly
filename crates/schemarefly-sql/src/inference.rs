//! Schema inference from SQL queries
//!
//! This module implements the core schema inference engine that can
//! determine output schemas from SQL queries without executing them.

use sqlparser::ast::{
    Statement, Query, SetExpr, Select, SelectItem, Expr, DataType,
    TableFactor, JoinOperator, FunctionArg, ObjectName, Value,
};
use schemarefly_core::{Schema, Column, LogicalType, Diagnostic, DiagnosticCode, Severity};
use schemarefly_dbt::Manifest;
use std::collections::HashMap;

/// Schema inference engine
pub struct SchemaInference<'a> {
    /// Inference context with available schemas
    context: &'a InferenceContext,
}

/// Context for schema inference containing available table schemas
pub struct InferenceContext {
    /// Map of table names to their schemas
    table_schemas: HashMap<String, Schema>,

    /// Whether to use catalog for SELECT * expansion
    use_catalog: bool,
}

impl InferenceContext {
    /// Create a new inference context
    pub fn new() -> Self {
        Self {
            table_schemas: HashMap::new(),
            use_catalog: false,
        }
    }

    /// Add a table schema to the context
    pub fn add_table(&mut self, name: impl Into<String>, schema: Schema) {
        self.table_schemas.insert(name.into(), schema);
    }

    /// Load schemas from manifest
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut context = Self::new();

        // Add contract schemas from manifest
        for (node_id, node) in manifest.models() {
            if let Some(contract) = schemarefly_dbt::ContractExtractor::extract_from_node(node) {
                // Use the model name as the table name
                context.add_table(node.name.clone(), contract.schema.clone());

                // Also add with the full unique_id
                context.add_table(node_id.clone(), contract.schema.clone());

                // Add with fully qualified name
                if let (Some(database), Some(schema)) = (&node.database, &node.schema) {
                    let fqn = format!("{}.{}.{}", database, schema, node.name);
                    context.add_table(fqn, contract.schema);
                }
            }
        }

        // Add sources from manifest
        for (source_id, source) in &manifest.sources {
            if !source.columns.is_empty() {
                // Convert columns to Schema
                let columns: Vec<Column> = source.columns
                    .values()
                    .filter_map(|col| {
                        col.data_type.as_ref().map(|dt| {
                            let logical_type = schemarefly_dbt::ContractExtractor::parse_data_type(dt);
                            Column::new(col.name.clone(), logical_type)
                        })
                    })
                    .collect();

                if !columns.is_empty() {
                    let schema = Schema::from_columns(columns);

                    // Add with source name (e.g., "raw.users")
                    context.add_table(format!("{}.{}", source.source_name, source.name), schema.clone());

                    // Add with fully qualified name (e.g., "raw_db.raw.users")
                    if let Some(database) = &source.database {
                        let fqn = format!("{}.{}.{}", database, source.schema, source.name);
                        context.add_table(fqn, schema.clone());
                    }

                    // Add with unique_id
                    context.add_table(source_id.clone(), schema);
                }
            }
        }

        context
    }

    /// Get schema for a table
    pub fn get_table_schema(&self, name: &str) -> Option<&Schema> {
        self.table_schemas.get(name)
    }

    /// Enable catalog usage for SELECT * expansion
    pub fn with_catalog(mut self, use_catalog: bool) -> Self {
        self.use_catalog = use_catalog;
        self
    }
}

impl Default for InferenceContext {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> SchemaInference<'a> {
    /// Create a new schema inference engine
    pub fn new(context: &'a InferenceContext) -> Self {
        Self { context }
    }

    /// Infer schema from a parsed SQL statement
    pub fn infer_statement(&self, statement: &Statement) -> Result<Schema, InferenceError> {
        match statement {
            Statement::Query(query) => self.infer_query(query),
            _ => Err(InferenceError::UnsupportedStatement(
                "Only SELECT queries are supported".to_string()
            )),
        }
    }

    /// Infer schema from a query
    fn infer_query(&self, query: &Query) -> Result<Schema, InferenceError> {
        // For now, we only handle simple SELECT queries
        // CTEs will be handled by resolving them first

        self.infer_set_expr(&query.body)
    }

    /// Infer schema from a set expression
    fn infer_set_expr(&self, set_expr: &SetExpr) -> Result<Schema, InferenceError> {
        match set_expr {
            SetExpr::Select(select) => self.infer_select(select),
            SetExpr::Query(query) => self.infer_query(query),
            SetExpr::SetOperation { left, .. } => {
                // For UNION, INTERSECT, etc., use the left schema
                // (both should be compatible)
                self.infer_set_expr(left)
            }
            _ => Err(InferenceError::UnsupportedConstruct(
                "Unsupported set expression".to_string()
            )),
        }
    }

    /// Infer schema from a SELECT statement
    fn infer_select(&self, select: &Select) -> Result<Schema, InferenceError> {
        // First, build a map of available columns from FROM clause
        let source_schema = self.infer_from_clause(&select.from)?;

        // Check if this is a GROUP BY query
        // GroupByExpr is an enum, extract expressions from it
        let group_by_exprs: Vec<&Expr> = match &select.group_by {
            sqlparser::ast::GroupByExpr::All(_) => vec![],
            sqlparser::ast::GroupByExpr::Expressions(exprs, _) => {
                exprs.iter().collect()
            }
        };

        let has_group_by = !group_by_exprs.is_empty();
        let group_by_cols: Vec<String> = group_by_exprs.iter()
            .filter_map(|expr| {
                // Extract column name from GROUP BY expression
                match expr {
                    Expr::Identifier(ident) => Some(ident.value.clone()),
                    Expr::CompoundIdentifier(idents) => {
                        idents.last().map(|i| i.value.clone())
                    }
                    _ => None,
                }
            })
            .collect();

        // Then infer the output schema from SELECT list
        let mut columns = Vec::new();

        for item in &select.projection {
            match item {
                SelectItem::UnnamedExpr(expr) => {
                    let (col_type, col_name) = self.infer_expr(expr, &source_schema)?;

                    // If this is a GROUP BY query, check if expr is valid
                    if has_group_by {
                        let is_aggregate = Self::is_aggregate_expr(expr);
                        let is_group_key = group_by_cols.contains(&col_name);

                        if is_aggregate {
                            // Warn about missing alias on aggregate
                            return Err(InferenceError::AggregateWithoutAlias(col_name));
                        } else if !is_group_key {
                            // Column not in GROUP BY and not an aggregate
                            return Err(InferenceError::InvalidGroupByColumn(col_name));
                        }
                    }

                    columns.push(Column::new(col_name, col_type));
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    let (col_type, _) = self.infer_expr(expr, &source_schema)?;

                    // If this is a GROUP BY query, validate
                    if has_group_by {
                        let is_aggregate = Self::is_aggregate_expr(expr);
                        let col_name = alias.value.clone();

                        // Check if the underlying expression is in GROUP BY
                        let is_group_key = match expr {
                            Expr::Identifier(ident) => group_by_cols.contains(&ident.value),
                            Expr::CompoundIdentifier(idents) => {
                                idents.last().map(|i| group_by_cols.contains(&i.value)).unwrap_or(false)
                            }
                            _ => false,
                        };

                        if !is_aggregate && !is_group_key {
                            return Err(InferenceError::InvalidGroupByColumn(col_name));
                        }
                    }

                    columns.push(Column::new(alias.value.clone(), col_type));
                }
                SelectItem::Wildcard(_) => {
                    // SELECT * - expand all columns from source
                    if self.context.use_catalog {
                        columns.extend(source_schema.columns.clone());
                    } else {
                        // Warn that we can't guarantee schema
                        return Err(InferenceError::SelectStarWithoutCatalog);
                    }
                }
                SelectItem::QualifiedWildcard(name, _) => {
                    // SELECT table.* - expand columns from specific table
                    let table_name = name.to_string();
                    if let Some(schema) = self.context.get_table_schema(&table_name) {
                        columns.extend(schema.columns.clone());
                    } else {
                        return Err(InferenceError::UnknownTable(table_name));
                    }
                }
            }
        }

        Ok(Schema::from_columns(columns))
    }

    /// Check if an expression is an aggregate function
    fn is_aggregate_expr(expr: &Expr) -> bool {
        match expr {
            Expr::Function(func) => {
                let func_name = func.name.to_string().to_uppercase();
                matches!(func_name.as_str(),
                    "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" |
                    "STDDEV" | "VARIANCE" | "ARRAY_AGG" | "STRING_AGG" |
                    "LISTAGG" | "PERCENTILE_CONT" | "PERCENTILE_DISC")
            }
            _ => false,
        }
    }

    /// Infer available columns from FROM clause
    fn infer_from_clause(&self, from: &[sqlparser::ast::TableWithJoins]) -> Result<Schema, InferenceError> {
        if from.is_empty() {
            // No FROM clause (e.g., SELECT 1)
            return Ok(Schema::new());
        }

        // Start with the first table
        let mut schema = self.infer_table_factor(&from[0].relation)?;

        // Process joins
        for table_with_joins in from {
            for join in &table_with_joins.joins {
                let right_schema = self.infer_table_factor(&join.relation)?;
                schema = self.merge_schemas(schema, right_schema, &join.join_operator)?;
            }
        }

        Ok(schema)
    }

    /// Infer schema from a table factor
    fn infer_table_factor(&self, table_factor: &TableFactor) -> Result<Schema, InferenceError> {
        match table_factor {
            TableFactor::Table { name, .. } => {
                let table_name = name.to_string();

                if let Some(schema) = self.context.get_table_schema(&table_name) {
                    Ok(schema.clone())
                } else {
                    Err(InferenceError::UnknownTable(table_name))
                }
            }
            TableFactor::Derived { subquery, .. } => {
                // Recursively infer subquery schema
                self.infer_query(subquery)
            }
            _ => Err(InferenceError::UnsupportedConstruct(
                "Unsupported table factor".to_string()
            )),
        }
    }

    /// Merge schemas for JOIN operations
    fn merge_schemas(
        &self,
        left: Schema,
        right: Schema,
        _join_op: &JoinOperator,
    ) -> Result<Schema, InferenceError> {
        let mut columns = left.columns;

        // Add right columns, handling conflicts
        for right_col in right.columns {
            // Check for duplicate column names
            if columns.iter().any(|c| c.name == right_col.name) {
                // Column name collision - in a real implementation,
                // we'd handle this based on the JOIN type and constraints
                // For now, we'll keep the left column
                continue;
            }

            columns.push(right_col);
        }

        Ok(Schema::from_columns(columns))
    }

    /// Infer type and name from an expression
    fn infer_expr(&self, expr: &Expr, source_schema: &Schema) -> Result<(LogicalType, String), InferenceError> {
        match expr {
            Expr::Identifier(ident) => {
                let col_name = ident.value.clone();

                // Find column in source schema
                if let Some(col) = source_schema.find_column(&col_name) {
                    Ok((col.logical_type.clone(), col_name))
                } else {
                    Err(InferenceError::UnknownColumn(col_name))
                }
            }

            Expr::CompoundIdentifier(idents) => {
                // e.g., table.column
                let col_name = idents.last().unwrap().value.clone();

                if let Some(col) = source_schema.find_column(&col_name) {
                    Ok((col.logical_type.clone(), col_name))
                } else {
                    Err(InferenceError::UnknownColumn(col_name))
                }
            }

            Expr::Cast { expr, data_type, .. } => {
                // CAST(expr AS type)
                let logical_type = self.sqlparser_type_to_logical(data_type)?;
                let (_, name) = self.infer_expr(expr, source_schema)?;
                Ok((logical_type, name))
            }

            Expr::Value(value) => {
                // Literal value
                let (logical_type, default_name) = self.infer_literal(value)?;
                Ok((logical_type, default_name))
            }

            Expr::Function(func) => {
                // Function call - extract args from FunctionArguments
                let args_vec: Vec<FunctionArg> = match &func.args {
                    sqlparser::ast::FunctionArguments::None => vec![],
                    sqlparser::ast::FunctionArguments::Subquery(_) => vec![],
                    sqlparser::ast::FunctionArguments::List(arg_list) => arg_list.args.clone(),
                };
                self.infer_function(&func.name, &args_vec, source_schema)
            }

            Expr::BinaryOp { left, op, right } => {
                // Binary operation
                let (left_type, _) = self.infer_expr(left, source_schema)?;
                let (right_type, _) = self.infer_expr(right, source_schema)?;

                // Infer result type based on operation
                let result_type = self.infer_binary_op_type(&left_type, &right_type, op)?;
                Ok((result_type, format!("expr")))
            }

            Expr::Case { .. } => {
                // CASE expression - for now, return Unknown
                Ok((LogicalType::Unknown, "case_expr".to_string()))
            }

            _ => {
                // Other expressions - return Unknown for now
                Ok((LogicalType::Unknown, "expr".to_string()))
            }
        }
    }

    /// Convert sqlparser DataType to LogicalType
    fn sqlparser_type_to_logical(&self, data_type: &DataType) -> Result<LogicalType, InferenceError> {
        match data_type {
            DataType::SmallInt(_) | DataType::Int(_) | DataType::BigInt(_) | DataType::Integer(_) => {
                Ok(LogicalType::Int)
            }
            DataType::Float(_) | DataType::Real | DataType::Double | DataType::DoublePrecision => {
                Ok(LogicalType::Float)
            }
            DataType::Decimal(info) | DataType::Numeric(info) => {
                // ExactNumberInfo is an enum with precision and scale variants
                use sqlparser::ast::ExactNumberInfo;
                match info {
                    ExactNumberInfo::None => {
                        // Unspecified DECIMAL - no precision/scale
                        Ok(LogicalType::Decimal { precision: None, scale: None })
                    }
                    ExactNumberInfo::Precision(p) => {
                        // Precision specified, scale defaults to 0
                        let precision = Some((*p).min(u16::MAX as u64) as u16);
                        Ok(LogicalType::Decimal { precision, scale: Some(0) })
                    }
                    ExactNumberInfo::PrecisionAndScale(p, s) => {
                        // Both precision and scale specified
                        // Clamp values to u16 range
                        let precision = Some((*p).min(u16::MAX as u64) as u16);
                        // Handle scale - convert from i64/u64 to u16
                        // Note: SQL scale is typically non-negative in most systems
                        let scale = Some((*s as u64).min(u16::MAX as u64) as u16);
                        Ok(LogicalType::Decimal { precision, scale })
                    }
                }
            }
            DataType::Boolean => Ok(LogicalType::Bool),
            DataType::Char(_) | DataType::Varchar(_) | DataType::Text | DataType::String(_) => {
                Ok(LogicalType::String)
            }
            DataType::Date => Ok(LogicalType::Date),
            DataType::Timestamp(_, _) | DataType::Datetime(_) => Ok(LogicalType::Timestamp),
            DataType::JSON => Ok(LogicalType::Json),
            DataType::Array(elem_type_def) => {
                // ArrayElemTypeDef is an enum with different bracket styles
                use sqlparser::ast::ArrayElemTypeDef;
                let element_type = match elem_type_def {
                    ArrayElemTypeDef::None => {
                        // Bare ARRAY with no type specified
                        Box::new(LogicalType::Unknown)
                    }
                    ArrayElemTypeDef::AngleBracket(inner_type) => {
                        // ARRAY<type>
                        Box::new(self.sqlparser_type_to_logical(inner_type)?)
                    }
                    ArrayElemTypeDef::SquareBracket(inner_type, _size) => {
                        // ARRAY[type] or ARRAY[type, size]
                        Box::new(self.sqlparser_type_to_logical(inner_type)?)
                    }
                    ArrayElemTypeDef::Parenthesis(inner_type) => {
                        // ARRAY(type)
                        Box::new(self.sqlparser_type_to_logical(inner_type)?)
                    }
                };
                Ok(LogicalType::Array { element_type })
            }
            _ => Ok(LogicalType::Unknown),
        }
    }

    /// Infer type from a literal value
    fn infer_literal(&self, value: &Value) -> Result<(LogicalType, String), InferenceError> {
        match value {
            Value::Number(_, _) => Ok((LogicalType::Int, "literal".to_string())),
            Value::SingleQuotedString(_) | Value::DoubleQuotedString(_) => {
                Ok((LogicalType::String, "literal".to_string()))
            }
            Value::Boolean(_) => Ok((LogicalType::Bool, "literal".to_string())),
            Value::Null => Ok((LogicalType::Unknown, "null".to_string())),
            _ => Ok((LogicalType::Unknown, "literal".to_string())),
        }
    }

    /// Infer return type of a function
    fn infer_function(
        &self,
        name: &ObjectName,
        args: &[FunctionArg],
        source_schema: &Schema,
    ) -> Result<(LogicalType, String), InferenceError> {
        let func_name = name.to_string().to_uppercase();

        // Common aggregate functions
        let return_type = match func_name.as_str() {
            "COUNT" => LogicalType::Int,
            "SUM" | "AVG" | "MIN" | "MAX" => {
                // Return type depends on argument type
                // For simplicity, we'll return the argument type
                if let Some(FunctionArg::Unnamed(arg_expr)) = args.first() {
                    if let sqlparser::ast::FunctionArgExpr::Expr(expr) = arg_expr {
                        let (arg_type, _) = self.infer_expr(expr, source_schema)?;
                        arg_type
                    } else {
                        LogicalType::Unknown
                    }
                } else {
                    LogicalType::Unknown
                }
            }
            "CONCAT" | "UPPER" | "LOWER" | "TRIM" | "SUBSTRING" => LogicalType::String,
            "NOW" | "CURRENT_TIMESTAMP" | "CURRENT_DATE" => LogicalType::Timestamp,
            "COALESCE" | "IFNULL" | "NULLIF" => {
                // Return type is the type of the first argument
                if let Some(FunctionArg::Unnamed(arg_expr)) = args.first() {
                    if let sqlparser::ast::FunctionArgExpr::Expr(expr) = arg_expr {
                        let (arg_type, _) = self.infer_expr(expr, source_schema)?;
                        arg_type
                    } else {
                        LogicalType::Unknown
                    }
                } else {
                    LogicalType::Unknown
                }
            }
            _ => LogicalType::Unknown,
        };

        Ok((return_type, func_name.to_lowercase()))
    }

    /// Infer result type of binary operation
    fn infer_binary_op_type(
        &self,
        left: &LogicalType,
        _right: &LogicalType,
        op: &sqlparser::ast::BinaryOperator,
    ) -> Result<LogicalType, InferenceError> {
        use sqlparser::ast::BinaryOperator::*;

        match op {
            // Comparison operators return boolean
            Eq | NotEq | Lt | LtEq | Gt | GtEq => Ok(LogicalType::Bool),

            // Logical operators return boolean
            And | Or => Ok(LogicalType::Bool),

            // Arithmetic operators preserve numeric types
            Plus | Minus | Multiply | Divide | Modulo => {
                // Simplified: just return left type
                // In a real implementation, we'd do proper type promotion
                Ok(left.clone())
            }

            // String concatenation
            StringConcat => Ok(LogicalType::String),

            _ => Ok(LogicalType::Unknown),
        }
    }

    /// Generate diagnostic for inference warnings
    pub fn create_diagnostic(&self, error: &InferenceError) -> Diagnostic {
        match error {
            InferenceError::SelectStarWithoutCatalog => Diagnostic::new(
                DiagnosticCode::SqlSelectStarUnexpandable,
                Severity::Warn,
                "SELECT * encountered without catalog - cannot guarantee schema"
            ),
            InferenceError::UnknownTable(name) => Diagnostic::new(
                DiagnosticCode::SqlInferenceError,
                Severity::Error,
                format!("Unknown table: {}", name)
            ),
            InferenceError::UnknownColumn(name) => Diagnostic::new(
                DiagnosticCode::SqlInferenceError,
                Severity::Error,
                format!("Unknown column: {}", name)
            ),
            InferenceError::AggregateWithoutAlias(func) => Diagnostic::new(
                DiagnosticCode::SqlGroupByAggregateUnaliased,
                Severity::Warn,
                format!("Aggregate function '{}' should have an explicit alias in GROUP BY query", func)
            ),
            InferenceError::InvalidGroupByColumn(col) => Diagnostic::new(
                DiagnosticCode::SqlInferenceError,
                Severity::Error,
                format!("Column '{}' must appear in GROUP BY or be part of an aggregate function", col)
            ),
            _ => Diagnostic::new(
                DiagnosticCode::SqlInferenceError,
                Severity::Error,
                error.to_string()
            ),
        }
    }
}

/// Schema inference errors
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("Unsupported statement: {0}")]
    UnsupportedStatement(String),

    #[error("Unsupported SQL construct: {0}")]
    UnsupportedConstruct(String),

    #[error("Unknown table: {0}")]
    UnknownTable(String),

    #[error("Unknown column: {0}")]
    UnknownColumn(String),

    #[error("SELECT * without catalog")]
    SelectStarWithoutCatalog,

    #[error("Type inference error: {0}")]
    TypeError(String),

    #[error("Aggregate function without alias: {0}")]
    AggregateWithoutAlias(String),

    #[error("Column '{0}' not in GROUP BY and not an aggregate")]
    InvalidGroupByColumn(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::SqlParser;

    fn create_test_context() -> InferenceContext {
        let mut context = InferenceContext::new();

        // Add a test "users" table
        let users_schema = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
            Column::new("email", LogicalType::String),
            Column::new("age", LogicalType::Int),
        ]);

        context.add_table("users", users_schema);

        context
    }

    #[test]
    fn infer_simple_select() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT id, name FROM users";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[0].name, "id");
        assert_eq!(schema.columns[1].name, "name");
    }

    #[test]
    fn infer_with_alias() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT id, name AS user_name, email AS user_email FROM users";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[1].name, "user_name");
        assert_eq!(schema.columns[2].name, "user_email");
    }

    #[test]
    fn infer_with_cast() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT CAST(id AS VARCHAR) FROM users";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        assert_eq!(schema.columns.len(), 1);
        assert!(matches!(schema.columns[0].logical_type, LogicalType::String));
    }

    #[test]
    fn infer_select_star_without_catalog() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT * FROM users";
        let parsed = parser.parse(sql, None).unwrap();

        let result = inference.infer_statement(parsed.first_statement().unwrap());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InferenceError::SelectStarWithoutCatalog));
    }

    #[test]
    fn infer_select_star_with_catalog() {
        let mut context = create_test_context();
        context.use_catalog = true;

        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT * FROM users";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        // Should have all columns from users table
        assert_eq!(schema.columns.len(), 4);
    }

    #[test]
    fn infer_with_literal() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT id, 'active' AS status FROM users";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        assert_eq!(schema.columns.len(), 2);
        assert!(matches!(schema.columns[1].logical_type, LogicalType::String));
    }

    #[test]
    fn infer_group_by_with_aggregate() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT name, COUNT(*) AS user_count FROM users GROUP BY name";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        // Should have name (group key) and user_count (aggregate with alias)
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[0].name, "name");
        assert_eq!(schema.columns[1].name, "user_count");
        assert!(matches!(schema.columns[1].logical_type, LogicalType::Int));
    }

    #[test]
    fn infer_group_by_without_alias_errors() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT name, COUNT(*) FROM users GROUP BY name";
        let parsed = parser.parse(sql, None).unwrap();

        let result = inference.infer_statement(parsed.first_statement().unwrap());

        // Should error because COUNT(*) doesn't have an alias
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InferenceError::AggregateWithoutAlias(_)));
    }

    #[test]
    fn infer_group_by_invalid_column_errors() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT name, email FROM users GROUP BY name";
        let parsed = parser.parse(sql, None).unwrap();

        let result = inference.infer_statement(parsed.first_statement().unwrap());

        // Should error because email is not in GROUP BY and not an aggregate
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InferenceError::InvalidGroupByColumn(_)));
    }

    #[test]
    fn infer_group_by_multiple_columns() {
        let context = create_test_context();
        let inference = SchemaInference::new(&context);

        let parser = SqlParser::new();
        let sql = "SELECT name, age, COUNT(*) AS cnt, AVG(id) AS avg_id FROM users GROUP BY name, age";
        let parsed = parser.parse(sql, None).unwrap();

        let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

        // Should have 4 columns: name, age (group keys), cnt, avg_id (aggregates)
        assert_eq!(schema.columns.len(), 4);
        assert_eq!(schema.columns[0].name, "name");
        assert_eq!(schema.columns[1].name, "age");
        assert_eq!(schema.columns[2].name, "cnt");
        assert_eq!(schema.columns[3].name, "avg_id");
    }
}
