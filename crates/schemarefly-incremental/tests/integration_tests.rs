//! Integration tests for Salsa incremental computation

use schemarefly_incremental::{SchemaReflyDatabase, queries};
use schemarefly_core::Config;
use std::path::PathBuf;

#[test]
fn test_manifest_parsing_cached() {
    let db = SchemaReflyDatabase::default();

    let manifest_json = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    // First call - should parse
    let input = queries::ManifestInput::new(&db, manifest_json.clone());
    let manifest1 = queries::manifest(&db, input);
    assert!(manifest1.is_some());

    // Second call with same input - should return cached result
    let manifest2 = queries::manifest(&db, input);
    assert!(manifest2.is_some());

    // Verify they're the same
    assert_eq!(manifest1, manifest2);
}

#[test]
fn test_manifest_recomputes_on_change() {
    let db = SchemaReflyDatabase::default();

    let manifest_json_v1 = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    let manifest_json_v2 = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.6.0",
            "generated_at": "2024-01-02T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    // Parse first version
    let input_v1 = queries::ManifestInput::new(&db, manifest_json_v1);
    let manifest1 = queries::manifest(&db, input_v1).unwrap();
    assert_eq!(manifest1.metadata.dbt_version, "1.5.0");

    // Parse second version (different input)
    let input_v2 = queries::ManifestInput::new(&db, manifest_json_v2);
    let manifest2 = queries::manifest(&db, input_v2).unwrap();
    assert_eq!(manifest2.metadata.dbt_version, "1.6.0");
}

#[test]
fn test_sql_file_input() {
    let db = SchemaReflyDatabase::default();

    let path = PathBuf::from("models/test.sql");
    let contents = "SELECT 1 AS id, 'test' AS name".to_string();

    // Create SQL file input
    let sql_file = queries::SqlFile::new(&db, path.clone(), contents.clone());

    // Verify getters work
    assert_eq!(sql_file.path(&db), path);
    assert_eq!(sql_file.contents(&db), &contents);
}

#[test]
fn test_parse_sql_caching() {
    let db = SchemaReflyDatabase::default();

    let path = PathBuf::from("models/test.sql");
    let sql = "SELECT 1 AS id".to_string();
    let config = Config::default();

    let sql_file = queries::SqlFile::new(&db, path, sql);
    let config_input = queries::ConfigInput::new(&db, config);

    // First parse - should execute
    let parsed1 = queries::parse_sql(&db, sql_file, config_input);
    assert!(parsed1.is_ok());

    // Second parse with same inputs - should return cached result
    let parsed2 = queries::parse_sql(&db, sql_file, config_input);
    assert!(parsed2.is_ok());

    // Results should be equal
    assert_eq!(parsed1.unwrap(), parsed2.unwrap());
}

#[test]
fn test_parse_sql_recomputes_on_file_change() {
    let db = SchemaReflyDatabase::default();

    let path = PathBuf::from("models/test.sql");
    let config = Config::default();
    let config_input = queries::ConfigInput::new(&db, config);

    // First version of SQL
    let sql_v1 = "SELECT 1 AS id".to_string();
    let file_v1 = queries::SqlFile::new(&db, path.clone(), sql_v1);
    let parsed1 = queries::parse_sql(&db, file_v1, config_input);
    assert!(parsed1.is_ok());

    // Second version of SQL (modified file)
    let sql_v2 = "SELECT 1 AS id, 'test' AS name".to_string();
    let file_v2 = queries::SqlFile::new(&db, path, sql_v2);
    let parsed2 = queries::parse_sql(&db, file_v2, config_input);
    assert!(parsed2.is_ok());

    // Results should be different (different SQL)
    assert_ne!(parsed1.unwrap().sql, parsed2.unwrap().sql);
}

#[test]
fn test_infer_schema_with_valid_sql() {
    let db = SchemaReflyDatabase::default();

    let manifest_json = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    let path = PathBuf::from("models/test.sql");
    let sql = "SELECT 1 AS id, 'test' AS name".to_string();
    let config = Config::default();

    let sql_file = queries::SqlFile::new(&db, path, sql);
    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config_input = queries::ConfigInput::new(&db, config);

    // Infer schema
    let result = queries::infer_schema(&db, sql_file, config_input, manifest_input);
    assert!(result.is_ok(), "Schema inference should succeed for valid SQL");

    let schema = result.unwrap();
    assert_eq!(schema.columns.len(), 2, "Should infer 2 columns");
}

#[test]
fn test_infer_schema_with_invalid_sql() {
    let db = SchemaReflyDatabase::default();

    let manifest_json = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    let path = PathBuf::from("models/test.sql");
    let sql = "SELECT FROM WHERE".to_string(); // Invalid SQL
    let config = Config::default();

    let sql_file = queries::SqlFile::new(&db, path, sql);
    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config_input = queries::ConfigInput::new(&db, config);

    // Should fail to infer schema
    let result = queries::infer_schema(&db, sql_file, config_input, manifest_input);
    assert!(result.is_err(), "Schema inference should fail for invalid SQL");
}

#[test]
fn test_check_contract_no_contract() {
    let db = SchemaReflyDatabase::default();

    let manifest_json = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    let path = PathBuf::from("models/test.sql");
    let sql = "SELECT 1 AS id".to_string();
    let config = Config::default();

    let sql_file = queries::SqlFile::new(&db, path, sql);
    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config_input = queries::ConfigInput::new(&db, config);

    // Check contract (should return empty since no contract in manifest)
    let diagnostics = queries::check_contract(&db, sql_file, config_input, manifest_input);
    assert!(diagnostics.is_empty(), "Should have no diagnostics when no contract exists");
}

#[test]
fn test_downstream_models_empty_manifest() {
    let db = SchemaReflyDatabase::default();

    let manifest_json = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let node_id = "model.test.users".to_string();

    // Get downstream models
    let downstream = queries::downstream_models(&db, manifest_input, node_id);
    assert!(downstream.is_empty(), "Empty manifest should have no downstream models");
}

#[test]
fn test_config_input() {
    let db = SchemaReflyDatabase::default();

    let config = Config::default();
    let config_input = queries::ConfigInput::new(&db, config.clone());

    // Verify config is stored correctly
    assert_eq!(config_input.config(&db).dialect, config.dialect);
}

#[test]
fn test_catalog_input() {
    let db = SchemaReflyDatabase::default();

    let catalog_json = Some(r#"{"tables": []}"#.to_string());
    let catalog_input = queries::CatalogInput::new(&db, catalog_json.clone());

    // Verify catalog is stored correctly
    assert_eq!(catalog_input.json(&db), &catalog_json);
}

#[test]
fn test_incremental_recomputation_only_on_change() {
    let db = SchemaReflyDatabase::default();

    let manifest_json = r#"{
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
            "dbt_version": "1.5.0",
            "generated_at": "2024-01-01T00:00:00Z"
        },
        "nodes": {},
        "sources": {},
        "parent_map": {},
        "child_map": {}
    }"#.to_string();

    let path1 = PathBuf::from("models/users.sql");
    let path2 = PathBuf::from("models/orders.sql");
    let sql1 = "SELECT 1 AS user_id".to_string();
    let sql2 = "SELECT 1 AS order_id".to_string();
    let config = Config::default();

    let file1 = queries::SqlFile::new(&db, path1.clone(), sql1.clone());
    let file2 = queries::SqlFile::new(&db, path2, sql2);
    let _manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config_input = queries::ConfigInput::new(&db, config);

    // Parse both files
    let parsed1_v1 = queries::parse_sql(&db, file1, config_input);
    let parsed2 = queries::parse_sql(&db, file2, config_input);
    assert!(parsed1_v1.is_ok());
    assert!(parsed2.is_ok());

    // Modify only file1
    let sql1_modified = "SELECT 1 AS user_id, 'test' AS name".to_string();
    let file1_modified = queries::SqlFile::new(&db, path1, sql1_modified);

    // Parse file1 again (should recompute)
    let parsed1_v2 = queries::parse_sql(&db, file1_modified, config_input);
    assert!(parsed1_v2.is_ok());

    // Parse file2 again (should return cached - file2 hasn't changed)
    let parsed2_v2 = queries::parse_sql(&db, file2, config_input);
    assert!(parsed2_v2.is_ok());

    // file1 should have changed, file2 should be the same
    assert_ne!(parsed1_v1.unwrap().sql, parsed1_v2.unwrap().sql);
    assert_eq!(parsed2.unwrap(), parsed2_v2.unwrap());
}
