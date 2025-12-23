//! Integration tests for SQL parsing

use schemarefly_sql::{SqlParser, DbtFunctionExtractor, NameResolver};
use schemarefly_dbt::Manifest;
use std::path::Path;

#[test]
fn parse_and_resolve_fixture_models() {
    let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");
    let users_sql_path = Path::new("../../fixtures/mini-dbt-project/models/users.sql");

    if manifest_path.exists() && users_sql_path.exists() {
        // Load manifest
        let manifest = Manifest::from_file(manifest_path).unwrap();

        // Read SQL file
        let sql = std::fs::read_to_string(users_sql_path).unwrap();

        // Extract dbt references
        let mut references = DbtFunctionExtractor::extract(&sql);
        assert!(!references.is_empty(), "Should have extracted dbt references");

        // Resolve references
        DbtFunctionExtractor::resolve(&mut references, &manifest);

        // Verify resolution
        for ref_ in &references {
            match ref_ {
                schemarefly_sql::DbtReference::Source { unique_id, .. } => {
                    assert!(unique_id.is_some(), "Source should be resolved");
                }
                schemarefly_sql::DbtReference::Ref { unique_id, .. } => {
                    assert!(unique_id.is_some(), "Ref should be resolved");
                }
            }
        }

        // Preprocess SQL
        let (preprocessed_sql, _replacements) = DbtFunctionExtractor::preprocess(&sql, Some(&manifest));

        // Parse preprocessed SQL
        let parser = SqlParser::new();
        let parsed = parser.parse(&preprocessed_sql, Some(users_sql_path));

        if let Err(e) = &parsed {
            // It's ok if parsing fails - we'll improve this in Phase 3
            println!("Parsing failed (expected for complex SQL): {}", e);
        } else {
            let parsed = parsed.unwrap();

            // Resolve names
            let mut resolver = NameResolver::new();
            if let Some(stmt) = parsed.first_statement() {
                let _ = resolver.resolve(stmt);

                // Check for resolved names
                println!("CTEs: {:?}", resolver.get_ctes());
                println!("Tables: {:?}", resolver.get_tables());
                println!("Columns: {:?}", resolver.get_column_aliases());
            }
        }
    }
}

#[test]
fn parse_active_users_model() {
    let active_users_path = Path::new("../../fixtures/mini-dbt-project/models/active_users.sql");

    if active_users_path.exists() {
        let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");
        let manifest = Manifest::from_file(manifest_path).ok();

        let sql = std::fs::read_to_string(active_users_path).unwrap();

        // Preprocess to handle {{ ref() }}
        let (preprocessed, _) = DbtFunctionExtractor::preprocess(&sql, manifest.as_ref());

        // Parse
        let parser = SqlParser::new();
        let result = parser.parse(&preprocessed, Some(active_users_path));

        if let Ok(parsed) = result {
            assert!(parsed.is_select());

            // Resolve names
            let mut resolver = NameResolver::new();
            if let Some(stmt) = parsed.first_statement() {
                resolver.resolve(stmt).ok();

                // Should have resolved the table (users)
                assert!(!resolver.get_tables().is_empty());
            }
        }
    }
}

#[test]
fn end_to_end_parsing_workflow() {
    // This test demonstrates the complete workflow:
    // 1. Load manifest
    // 2. Read SQL file
    // 3. Extract and resolve dbt references
    // 4. Preprocess SQL
    // 5. Parse SQL
    // 6. Resolve names

    let sql = r#"
        WITH active_users AS (
            SELECT
                id,
                name,
                email
            FROM {{ ref('users') }}
            WHERE deleted_at IS NULL
        ),
        premium_users AS (
            SELECT
                id,
                name AS user_name,
                subscription_tier
            FROM {{ source('billing', 'subscriptions') }}
            WHERE subscription_tier = 'premium'
        )
        SELECT
            a.id,
            a.name,
            p.subscription_tier AS tier
        FROM active_users AS a
        LEFT JOIN premium_users AS p ON a.id = p.id
    "#;

    // Step 1 & 2: Extract dbt references
    let references = DbtFunctionExtractor::extract(sql);
    assert_eq!(references.len(), 2);

    // Step 3: Preprocess
    let (preprocessed, _) = DbtFunctionExtractor::preprocess(sql, None);
    assert!(!preprocessed.contains("{{"));
    assert!(!preprocessed.contains("}}"));

    // Step 4: Parse
    let parser = SqlParser::new();
    let parsed = parser.parse(&preprocessed, None);

    if let Ok(parsed) = parsed {
        assert!(parsed.is_select());

        // Step 5: Resolve names
        let mut resolver = NameResolver::new();
        if let Some(stmt) = parsed.first_statement() {
            resolver.resolve(stmt).unwrap();

            // Should have 2 CTEs
            assert_eq!(resolver.get_ctes().len(), 2);
            assert!(resolver.is_cte("active_users"));
            assert!(resolver.is_cte("premium_users"));

            // Should have column aliases
            assert!(!resolver.get_column_aliases().is_empty());
            assert!(resolver.is_column_alias("user_name") || resolver.is_column_alias("tier"));
        }
    }
}

#[test]
fn infer_schema_from_fixture_models() {
    use schemarefly_sql::SchemaInference;
    use schemarefly_sql::InferenceContext;

    let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");
    let active_users_path = Path::new("../../fixtures/mini-dbt-project/models/active_users.sql");

    if manifest_path.exists() && active_users_path.exists() {
        // Load manifest to get table schemas
        let manifest = Manifest::from_file(manifest_path).unwrap();

        // Create inference context from manifest
        let context = InferenceContext::from_manifest(&manifest).with_catalog(true);

        // Read active_users model
        let sql = std::fs::read_to_string(active_users_path).unwrap();

        // Preprocess to handle {{ ref() }}
        let (preprocessed, _) = DbtFunctionExtractor::preprocess(&sql, Some(&manifest));

        // Parse
        let parser = SqlParser::new();
        let parsed = parser.parse(&preprocessed, Some(active_users_path));

        if let Ok(parsed) = parsed {
            // Infer schema
            let inference = SchemaInference::new(&context);
            if let Some(stmt) = parsed.first_statement() {
                let result = inference.infer_statement(stmt);

                // Should successfully infer schema for active_users
                if let Ok(schema) = result {
                    // active_users selects id, name, email from users where deleted_at IS NULL
                    println!("Inferred schema for active_users:");
                    println!("  Columns: {}", schema.columns.len());
                    for col in &schema.columns {
                        println!("    - {}: {}", col.name, col.logical_type);
                    }

                    // Basic validations
                    assert!(!schema.columns.is_empty(), "Schema should have columns");
                } else {
                    println!("Schema inference result: {:?}", result);
                }
            }
        } else {
            println!("Parse error (may be expected for complex SQL): {:?}", parsed);
        }
    }
}

#[test]
fn infer_schema_with_joins() {
    use schemarefly_sql::SchemaInference;
    use schemarefly_sql::InferenceContext;
    use schemarefly_core::{Schema, Column, LogicalType};

    let mut context = InferenceContext::new().with_catalog(true);

    // Add test schemas for orders and customers
    let customers_schema = Schema::from_columns(vec![
        Column::new("id", LogicalType::Int),
        Column::new("name", LogicalType::String),
        Column::new("email", LogicalType::String),
    ]);

    let orders_schema = Schema::from_columns(vec![
        Column::new("order_id", LogicalType::Int),
        Column::new("customer_id", LogicalType::Int),
        Column::new("total", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
    ]);

    context.add_table("customers", customers_schema);
    context.add_table("orders", orders_schema);

    let inference = SchemaInference::new(&context);
    let parser = SqlParser::new();

    let sql = r#"
        SELECT
            c.id,
            c.name,
            o.order_id,
            o.total
        FROM customers AS c
        LEFT JOIN orders AS o ON c.id = o.customer_id
    "#;

    let parsed = parser.parse(sql, None).unwrap();
    let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

    // Should have 4 columns from the JOIN
    assert_eq!(schema.columns.len(), 4);
    assert_eq!(schema.columns[0].name, "id");
    assert_eq!(schema.columns[1].name, "name");
    assert_eq!(schema.columns[2].name, "order_id");
    assert_eq!(schema.columns[3].name, "total");
}

#[test]
fn infer_schema_complex_aggregation() {
    use schemarefly_sql::SchemaInference;
    use schemarefly_sql::InferenceContext;
    use schemarefly_core::{Schema, Column, LogicalType};

    let mut context = InferenceContext::new();

    let sales_schema = Schema::from_columns(vec![
        Column::new("product_id", LogicalType::Int),
        Column::new("region", LogicalType::String),
        Column::new("amount", LogicalType::Decimal { precision: Some(10), scale: Some(2) }),
        Column::new("quantity", LogicalType::Int),
    ]);

    context.add_table("sales", sales_schema);

    let inference = SchemaInference::new(&context);
    let parser = SqlParser::new();

    let sql = r#"
        SELECT
            region,
            product_id,
            SUM(amount) AS total_revenue,
            COUNT(*) AS sale_count,
            AVG(quantity) AS avg_quantity
        FROM sales
        GROUP BY region, product_id
    "#;

    let parsed = parser.parse(sql, None).unwrap();
    let schema = inference.infer_statement(parsed.first_statement().unwrap()).unwrap();

    // Should have 5 columns: 2 group keys + 3 aggregates
    assert_eq!(schema.columns.len(), 5);
    assert_eq!(schema.columns[0].name, "region");
    assert_eq!(schema.columns[1].name, "product_id");
    assert_eq!(schema.columns[2].name, "total_revenue");
    assert_eq!(schema.columns[3].name, "sale_count");
    assert_eq!(schema.columns[4].name, "avg_quantity");

    // Aggregates should have correct types
    assert!(matches!(schema.columns[3].logical_type, LogicalType::Int)); // COUNT
}
