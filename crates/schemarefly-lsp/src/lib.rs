//! SchemaRefly Language Server Protocol implementation
//!
//! This crate provides LSP support for SchemaRefly, enabling real-time
//! diagnostics, hover information, and go-to-definition for dbt SQL files.
//!
//! ## Features
//!
//! - **Diagnostics**: Show contract violations inline without running dbt
//! - **Hover**: Display inferred schema when hovering over models
//! - **Go-to-definition**: Jump from contract columns to YAML definitions, or from refs to model files
//!
//! ## Usage
//!
//! The LSP server is started as a binary that communicates via stdin/stdout:
//!
//! ```bash
//! schemarefly-lsp
//! ```
//!
//! Configure your editor to use this binary as the language server for `.sql` files
//! in dbt projects.

mod backend;

pub use backend::Backend;
