//! Salsa inputs and tracked functions for incremental computation
//!
//! This module defines all Salsa inputs (base data that can change) and
//! tracked functions (derived computations) for incremental recomputation.

use schemarefly_core::{Schema, Diagnostic, Config};
use schemarefly_dbt::Manifest;
use schemarefly_sql::ParsedSql;
use std::path::PathBuf;

/// Input: SQL file with its path and contents
///
/// This is a base input that changes when a user edits a SQL file.
/// Salsa tracks changes to file contents and only recomputes affected queries.
#[salsa::input]
pub struct SqlFile {
    /// Path to the SQL file
    pub path: PathBuf,

    /// Text content of the file
    #[returns(ref)]
    pub contents: String,
}

/// Input: dbt manifest JSON
///
/// This is a base input that changes when dbt is recompiled.
/// Contains model definitions, dependencies, and contracts.
#[salsa::input]
pub struct ManifestInput {
    /// Raw JSON content of manifest.json
    #[returns(ref)]
    pub json: String,
}

/// Input: dbt catalog JSON (optional)
///
/// This is a base input that contains actual table schemas from the warehouse.
/// Used for SELECT * expansion and type information.
#[salsa::input]
pub struct CatalogInput {
    /// Raw JSON content of catalog.json (if available)
    #[returns(ref)]
    pub json: Option<String>,
}

/// Input: Configuration
///
/// This is a base input that changes when schemarefly.toml is modified.
#[salsa::input]
pub struct ConfigInput {
    /// Configuration settings
    #[returns(ref)]
    pub config: Config,
}

/// Tracked function: Parse manifest JSON into Manifest struct
///
/// This is memoized and only recomputed when the manifest JSON changes.
/// Returns `None` if parsing fails.
#[salsa::tracked]
pub fn manifest(db: &dyn salsa::Database, input: ManifestInput) -> Option<Manifest> {
    let json = input.json(db);

    match serde_json::from_str(json) {
        Ok(manifest) => Some(manifest),
        Err(e) => {
            eprintln!("Failed to parse manifest: {}", e);
            None
        }
    }
}

/// Tracked function: Parse a SQL file into an AST
///
/// This is memoized per file and only recomputed when:
/// - The file contents change
/// - The config changes (dialect affects parsing)
#[salsa::tracked]
pub fn parse_sql(
    db: &dyn salsa::Database,
    file: SqlFile,
    config: ConfigInput,
) -> Result<ParsedSql, String> {
    use schemarefly_sql::SqlParser;

    let contents = file.contents(db);
    let config_val = config.config(db);
    let path = file.path(db);

    // Create parser based on config dialect
    let parser = SqlParser::from_dialect(&config_val.dialect);

    // Parse SQL
    parser
        .parse(contents, Some(&path))
        .map_err(|e| format!("Parse error: {}", e))
}

/// Tracked function: Infer schema for a SQL file
///
/// This is memoized and only recomputed when:
/// - The parsed SQL changes (which depends on file contents + config)
/// - The manifest changes (affects ref() resolution and type information)
#[salsa::tracked]
pub fn infer_schema(
    db: &dyn salsa::Database,
    file: SqlFile,
    config: ConfigInput,
    manifest_input: ManifestInput,
) -> Result<Schema, String> {
    use schemarefly_sql::{SchemaInference, InferenceContext};

    // Get parsed SQL (cached)
    let parsed = parse_sql(db, file, config)
        .map_err(|e| format!("Cannot infer schema - {}", e))?;

    // Get manifest (cached)
    let manifest_val = manifest(db, manifest_input)
        .ok_or_else(|| "Failed to parse manifest".to_string())?;

    // Create inference context from manifest
    let context = InferenceContext::from_manifest(&manifest_val);

    // Infer schema
    let inference = SchemaInference::new(&context);

    if let Some(stmt) = parsed.first_statement() {
        inference
            .infer_statement(stmt)
            .map_err(|e| format!("Inference error: {}", e))
    } else {
        Err("No SQL statement found".to_string())
    }
}

/// Tracked function: Check contract for a model
///
/// This is memoized and only recomputed when:
/// - The inferred schema changes
/// - The manifest changes (contract definition might change)
///
/// Returns diagnostics for contract violations.
#[salsa::tracked]
pub fn check_contract(
    db: &dyn salsa::Database,
    file: SqlFile,
    config: ConfigInput,
    manifest_input: ManifestInput,
) -> Vec<Diagnostic> {
    use schemarefly_engine::ContractDiff;
    use schemarefly_dbt::ContractExtractor;

    // Get inferred schema (cached)
    let inferred = match infer_schema(db, file, config, manifest_input) {
        Ok(schema) => schema,
        Err(_) => return Vec::new(), // Can't check contract if inference failed
    };

    // Get manifest (cached)
    let manifest_val = match manifest(db, manifest_input) {
        Some(m) => m,
        None => return Vec::new(), // Can't check contract if manifest is invalid
    };

    // Get file path
    let path = file.path(db);
    let path_str = path.to_string_lossy().to_string();

    // Find model in manifest by path
    for (node_id, node) in manifest_val.models() {
        if node.original_file_path == path_str {
            // Check if model has a contract
            if let Some(contract) = ContractExtractor::extract_from_node(node) {
                // Compare contract to inferred schema
                let diff = ContractDiff::compare(node_id, &contract, &inferred, Some(path_str));

                return diff.diagnostics;
            }
        }
    }

    // No contract found for this model
    Vec::new()
}

/// Tracked function: Get downstream dependencies for a model
///
/// This is memoized and only recomputed when the manifest changes.
#[salsa::tracked]
pub fn downstream_models(
    db: &dyn salsa::Database,
    manifest_input: ManifestInput,
    node_id: String,
) -> Vec<String> {
    use schemarefly_dbt::DependencyGraph;

    // Get manifest (cached)
    let manifest_val = match manifest(db, manifest_input) {
        Some(m) => m,
        None => return Vec::new(), // No dependencies if manifest is invalid
    };

    // Build dependency graph (could also be cached as a tracked function)
    let dag = DependencyGraph::from_manifest(&manifest_val);

    // Get downstream dependencies
    dag.downstream(&node_id)
}
