//! Benchmarks for Salsa incremental computation performance
//!
//! These benchmarks measure the performance of SchemaRefly's incremental
//! computation system with large DAGs and complex dependencies.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use schemarefly_core::Config;
use schemarefly_incremental::{queries, SchemaReflyDatabase};
use std::path::PathBuf;

/// Generate a large manifest JSON with N models
fn generate_large_manifest(num_models: usize) -> String {
    let mut nodes = Vec::new();
    let mut parent_map = Vec::new();
    let mut child_map = Vec::new();

    for i in 0..num_models {
        let model_id = format!("model.project.model_{}", i);

        // Create dependencies (each model depends on previous 2 models)
        let depends_on: Vec<String> = if i > 0 {
            let start = i.saturating_sub(2);
            (start..i)
                .map(|j| format!("model.project.model_{}", j))
                .collect()
        } else {
            vec![]
        };

        // Serialize the node to JSON
        let node_json = serde_json::json!({
            "name": format!("model_{}", i),
            "resource_type": "model",
            "package_name": "project",
            "path": format!("models/model_{}.sql", i),
            "original_file_path": format!("models/model_{}.sql", i),
            "unique_id": model_id.clone(),
            "fqn": vec!["project", &format!("model_{}", i)],
            "database": "analytics",
            "schema": "dbt_prod",
            "alias": format!("model_{}", i),
            "checksum": {
                "name": "sha256",
                "checksum": "abc123"
            },
            "config": {
                "enabled": true,
                "materialized": "table"
            },
            "tags": [],
            "meta": {},
            "depends_on": {
                "nodes": depends_on
            }
        });
        nodes.push(format!(r#""{}": {}"#, model_id, serde_json::to_string(&node_json).unwrap()));

        if !depends_on.is_empty() {
            parent_map.push(format!(r#""{}": {:?}"#, model_id, depends_on));
            for parent in &depends_on {
                child_map.push(format!(r#""{}": ["{}"]"#, parent, model_id));
            }
        }
    }

    format!(
        r#"{{
            "metadata": {{
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v10.json",
                "dbt_version": "1.5.0",
                "generated_at": "2024-01-01T00:00:00Z"
            }},
            "nodes": {{{}}},
            "sources": {{}},
            "parent_map": {{{}}},
            "child_map": {{{}}}
        }}"#,
        nodes.join(","),
        parent_map.join(","),
        child_map.join(",")
    )
}

/// Generate complex SQL for a model with N columns and joins
fn generate_complex_sql(model_num: usize, num_columns: usize, num_joins: usize) -> String {
    let mut select_cols = Vec::new();
    let mut joins = Vec::new();

    // Generate SELECT columns
    for i in 0..num_columns {
        select_cols.push(format!("    t0.col_{} AS col_{}", i, i));
    }

    // Generate JOINs (each model joins with previous models)
    for i in 1..=num_joins.min(model_num) {
        let join_model = model_num - i;
        joins.push(format!(
            "LEFT JOIN ref('model_{}') t{} ON t0.id = t{}.id",
            join_model, i, i
        ));
    }

    format!(
        "SELECT\n{}\nFROM ref('model_{}') t0\n{}",
        select_cols.join(",\n"),
        model_num.saturating_sub(1),
        joins.join("\n")
    )
}

/// Benchmark: Parse large manifest (100, 500, 1000 models)
fn bench_manifest_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("manifest_parsing");

    for num_models in [100, 500, 1000].iter() {
        let manifest_json = generate_large_manifest(*num_models);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_models),
            num_models,
            |b, _| {
                b.iter(|| {
                    let db = SchemaReflyDatabase::default();
                    let input = queries::ManifestInput::new(&db, manifest_json.clone());
                    black_box(queries::manifest(&db, input))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: SQL parsing with caching (measure cache hits)
fn bench_sql_parsing_with_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_parsing_cache");

    let db = SchemaReflyDatabase::default();
    let path = PathBuf::from("models/test.sql");
    let sql = generate_complex_sql(10, 50, 5);
    let config = Config::default();
    let config_input = queries::ConfigInput::new(&db, config);

    // First parse (cold cache)
    group.bench_function("cold_cache", |b| {
        b.iter(|| {
            let sql_file = queries::SqlFile::new(&db, path.clone(), sql.clone());
            black_box(queries::parse_sql(&db, sql_file, config_input))
        });
    });

    // Subsequent parses (warm cache)
    let sql_file = queries::SqlFile::new(&db, path.clone(), sql.clone());
    let _ = queries::parse_sql(&db, sql_file, config_input); // Prime cache

    group.bench_function("warm_cache", |b| {
        b.iter(|| black_box(queries::parse_sql(&db, sql_file, config_input)));
    });

    group.finish();
}

/// Benchmark: Schema inference on complex SQL
fn bench_schema_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema_inference");

    let db = SchemaReflyDatabase::default();
    let manifest_json = generate_large_manifest(50);
    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config = Config::default();
    let config_input = queries::ConfigInput::new(&db, config);

    for num_columns in [10, 50, 100].iter() {
        let path = PathBuf::from(format!("models/model_{}.sql", num_columns));
        let sql = generate_complex_sql(10, *num_columns, 3);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_columns),
            num_columns,
            |b, _| {
                b.iter(|| {
                    let sql_file = queries::SqlFile::new(&db, path.clone(), sql.clone());
                    black_box(queries::infer_schema(
                        &db,
                        sql_file,
                        config_input,
                        manifest_input,
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Incremental recomputation (modify one file in a DAG)
fn bench_incremental_recomputation(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_recomputation");

    let db = SchemaReflyDatabase::default();
    let manifest_json = generate_large_manifest(100);
    let _manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config = Config::default();
    let config_input = queries::ConfigInput::new(&db, config);

    // Create 100 SQL files and parse them all (populate cache)
    let mut sql_files = Vec::new();
    for i in 0..100 {
        let path = PathBuf::from(format!("models/model_{}.sql", i));
        let sql = generate_complex_sql(i, 10, 3);
        let sql_file = queries::SqlFile::new(&db, path, sql);
        let _ = queries::parse_sql(&db, sql_file, config_input);
        sql_files.push(sql_file);
    }

    // Benchmark: Modify one file and reparse (should only recompute that one file)
    group.bench_function("recompute_one_of_100", |b| {
        b.iter(|| {
            // Modify the 50th file
            let path = PathBuf::from("models/model_50.sql");
            let new_sql = generate_complex_sql(50, 15, 3); // Changed: more columns
            let modified_file = queries::SqlFile::new(&db, path, new_sql);
            black_box(queries::parse_sql(&db, modified_file, config_input))
        });
    });

    group.finish();
}

/// Benchmark: Downstream model discovery
fn bench_downstream_models(c: &mut Criterion) {
    let mut group = c.benchmark_group("downstream_models");

    for num_models in [100, 500, 1000].iter() {
        let manifest_json = generate_large_manifest(*num_models);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_models),
            num_models,
            |b, _| {
                b.iter(|| {
                    let db = SchemaReflyDatabase::default();
                    let manifest_input = queries::ManifestInput::new(&db, manifest_json.clone());
                    // Find downstream models of the first model
                    let node_id = "model.project.model_0".to_string();
                    black_box(queries::downstream_models(&db, manifest_input, node_id))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: End-to-end contract checking (parse + infer + check)
fn bench_contract_checking_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("contract_checking_e2e");

    let db = SchemaReflyDatabase::default();
    let manifest_json = generate_large_manifest(50);
    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config = Config::default();
    let config_input = queries::ConfigInput::new(&db, config);

    for num_models in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_models),
            num_models,
            |b, &models| {
                b.iter(|| {
                    // Check contracts for N models
                    for i in 0..models {
                        let path = PathBuf::from(format!("models/model_{}.sql", i));
                        let sql = generate_complex_sql(i, 10, 3);
                        let sql_file = queries::SqlFile::new(&db, path, sql);
                        black_box(queries::check_contract(
                            &db,
                            sql_file,
                            config_input,
                            manifest_input,
                        ));
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Cache efficiency (measure hit rate with varying DAG sizes)
fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    for dag_size in [50, 100, 200].iter() {
        let db = SchemaReflyDatabase::default();
        let manifest_json = generate_large_manifest(*dag_size);
        let _manifest_input = queries::ManifestInput::new(&db, manifest_json);
        let config = Config::default();
        let config_input = queries::ConfigInput::new(&db, config);

        // First pass: populate cache
        for i in 0..*dag_size {
            let path = PathBuf::from(format!("models/model_{}.sql", i));
            let sql = generate_complex_sql(i, 10, 3);
            let sql_file = queries::SqlFile::new(&db, path, sql);
            let _ = queries::parse_sql(&db, sql_file, config_input);
        }

        // Second pass: should hit cache
        group.bench_with_input(
            BenchmarkId::new("cached_queries", dag_size),
            dag_size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        let path = PathBuf::from(format!("models/model_{}.sql", i));
                        let sql = generate_complex_sql(i, 10, 3);
                        let sql_file = queries::SqlFile::new(&db, path, sql);
                        let _ = black_box(queries::parse_sql(&db, sql_file, config_input));
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_manifest_parsing,
    bench_sql_parsing_with_cache,
    bench_schema_inference,
    bench_incremental_recomputation,
    bench_downstream_models,
    bench_contract_checking_end_to_end,
    bench_cache_efficiency
);

criterion_main!(benches);
