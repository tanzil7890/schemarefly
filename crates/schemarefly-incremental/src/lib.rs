//! Incremental computation layer using Salsa
//!
//! This crate provides incremental computation for SchemaRefly using the Salsa
//! framework. It enables fast recomputation when files change by only recomputing
//! affected queries.
//!
//! ## Architecture
//!
//! - **Inputs**: SqlFile, ManifestInput, CatalogInput, ConfigInput
//! - **Tracked Functions**: Parsing, inference, contract checking
//! - **Caching**: Warehouse metadata with TTL
//!
//! When a file changes, Salsa tracks dependencies and only recomputes affected
//! queries, making SchemaRefly fast even with large dbt projects.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use schemarefly_incremental::{db::SchemaReflyDatabase, queries};
//!
//! let mut db = SchemaReflyDatabase::default();
//!
//! // Set inputs
//! let file = queries::SqlFile::new(&db, path, contents);
//! let manifest = queries::ManifestInput::new(&db, json);
//! let config = queries::ConfigInput::new(&db, config);
//!
//! // Run queries (cached and incremental)
//! let schema = queries::infer_schema(&db, file, config, manifest);
//! ```

pub mod db;
pub mod queries;
pub mod cache;

pub use db::{Db, SchemaReflyDatabase};
pub use cache::WarehouseCache;
