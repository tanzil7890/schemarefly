//! Salsa database definition for incremental computation
//!
//! This module defines the Salsa database that powers incremental computation
//! in SchemaRefly. The database tracks inputs (files, manifest, catalog) and
//! provides tracked functions for derived computations.

/// Database trait for SchemaRefly incremental computation
///
/// All tracked functions take `&dyn Db` as their first parameter.
pub trait Db: salsa::Database {}

/// Main Salsa database implementation
///
/// This database tracks all inputs and provides derived queries for:
/// - SQL parsing
/// - Schema inference
/// - Contract validation
/// - Dependency analysis
///
/// ## Usage
///
/// ```rust,ignore
/// use schemarefly_incremental::{SchemaReflyDatabase, queries};
///
/// let mut db = SchemaReflyDatabase::default();
///
/// // Set inputs
/// let file = queries::SqlFile::new(&db, path, contents);
/// let manifest = queries::ManifestInput::new(&db, json);
/// let config = queries::ConfigInput::new(&db, config);
///
/// // Run queries (cached and incremental)
/// let schema = queries::infer_schema(&db, file, config, manifest);
/// ```
#[salsa::db]
#[derive(Default, Clone)]
pub struct SchemaReflyDatabase {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for SchemaReflyDatabase {}

impl Db for SchemaReflyDatabase {}
