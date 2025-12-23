# SchemaRefly

**Schema contract verification for dbt** - Catch breaking changes before they break production.

SchemaRefly is a Rust-based tool that validates dbt schema contracts by analyzing SQL and comparing inferred schemas against declared contracts. It provides fast, incremental checking with detailed impact analysis.

## Status

🎯 **Phase 0 COMPLETED** - Standards + interfaces are ready
🎯 **Phase 1 COMPLETED** - dbt ingestion + DAG + contracts
🎯 **Phase 2 COMPLETED** - SQL parsing layer

### Phase 0 ✅
- ✅ Diagnostic code registry (versioned, stable)
- ✅ Report schema (report.json v1)
- ✅ Config schema (schemarefly.toml)
- ✅ CLI with check command
- ✅ Golden test fixtures

### Phase 1 ✅
- ✅ Manifest.json parsing
- ✅ Dependency graph (DAG) construction
- ✅ Contract extraction from models
- ✅ Impact analysis command (`schemarefly impact <model>`)

### Phase 2 ✅
- ✅ SQL parsing with datafusion-sqlparser-rs
- ✅ Multi-dialect support (BigQuery, Snowflake, Postgres, ANSI)
- ✅ dbt template function extraction (ref, source)
- ✅ Name resolution (CTEs, aliases, tables)
- ✅ Diagnostic error reporting

See [SchemaRefly Engineering Doc.md](SchemaRefly%20Engineering%20Doc.md) for the full roadmap.

## Quick Start

### Installation

```bash
# Build from source
cargo build --release --bin schemarefly

# The binary will be in target/release/schemarefly
```

### Usage

```bash
# Run a no-op check (Phase 0)
schemarefly check

# With verbose output
schemarefly check --verbose

# Generate markdown report
schemarefly check --markdown report.md

# Use custom config
schemarefly check --config my-config.toml
```

## Configuration

Create a `schemarefly.toml` in your project root:

```toml
# SQL dialect: bigquery, snowflake, postgres, ansi
dialect = "bigquery"

[severity.overrides]
# Override severity for specific diagnostic codes
# CONTRACT_EXTRA_COLUMN = "warn"

[allowlist]
# Allow type widening for specific models (glob patterns)
allow_widening = [
    "staging.*"
]

# Allow extra columns for specific models
allow_extra_columns = [
    "staging.*"
]

# Skip checks entirely for specific models
skip_models = [
    "temp_*"
]
```

## Project Structure

```
SchemaRefly/
├── crates/
│   ├── schemarefly-core/      # Core domain model (types, diagnostics, config)
│   ├── schemarefly-dbt/       # dbt artifact parsing (Phase 1)
│   ├── schemarefly-sql/       # SQL parsing & inference (Phase 2-3)
│   ├── schemarefly-catalog/   # Warehouse metadata (Phase 5)
│   ├── schemarefly-engine/    # Salsa incremental engine (Phase 6)
│   ├── schemarefly-cli/       # CLI application
│   └── schemarefly-lsp/       # LSP server (Phase 7)
├── fixtures/                  # Test fixtures
│   └── mini-dbt-project/     # Minimal dbt project for testing
├── schemarefly.toml          # Example configuration
└── SchemaRefly Engineering Doc.md  # Detailed architecture & roadmap
```

## Architecture

SchemaRefly uses a **Rust workspace** with clear separation of concerns:

- **schemarefly-core**: Canonical schema types, stable diagnostic codes, config schema
- **schemarefly-dbt**: dbt manifest parsing, DAG construction, contract extraction
- **schemarefly-sql**: SQL parsing (datafusion-sqlparser), schema inference
- **schemarefly-catalog**: Warehouse metadata adapters (BigQuery, Snowflake)
- **schemarefly-engine**: Incremental computation using Salsa
- **schemarefly-cli**: Command-line interface
- **schemarefly-lsp**: Language Server Protocol implementation

## Diagnostic Codes

SchemaRefly uses **stable, versioned diagnostic codes** that never change:

### Contract Violations (1xxx)
- `CONTRACT_MISSING_COLUMN` - Required column missing from inferred schema
- `CONTRACT_TYPE_MISMATCH` - Column type doesn't match contract
- `CONTRACT_EXTRA_COLUMN` - Extra columns not in contract
- `CONTRACT_MISSING` - Contract missing but model references contracts

### Drift Detection (2xxx)
- `DRIFT_COLUMN_DROPPED` - Warehouse column removed
- `DRIFT_TYPE_CHANGE` - Warehouse column type changed
- `DRIFT_COLUMN_ADDED` - New column added to warehouse

### SQL Inference (3xxx)
- `SQL_SELECT_STAR_UNEXPANDABLE` - SELECT * without catalog
- `SQL_UNSUPPORTED_SYNTAX` - Unsupported SQL syntax
- `SQL_PARSE_ERROR` - Failed to parse SQL
- `SQL_INFERENCE_ERROR` - Failed to infer schema

## Report Schema

SchemaRefly generates **stable, versioned JSON reports** (v1.0):

```json
{
  "version": {
    "major": 1,
    "minor": 0
  },
  "timestamp": "2025-12-23T01:03:40.420648+00:00",
  "summary": {
    "total": 0,
    "errors": 0,
    "warnings": 0,
    "info": 0,
    "models_checked": 0,
    "contracts_validated": 0
  },
  "diagnostics": []
}
```

## Commands

### check
Validate schema contracts against inferred schemas.

```bash
schemarefly check [--output report.json] [--markdown report.md]
```

**Status**: ✅ Functional (produces empty report in Phase 0)

### impact
Show downstream dependencies for a model.

```bash
schemarefly impact <model> [--manifest target/manifest.json]

# Examples
schemarefly impact users
schemarefly impact "source.my_project.raw.users"
schemarefly impact users --manifest path/to/manifest.json --verbose
```

**Status**: ✅ Functional

Shows the complete blast radius (transitive closure) of downstream dependencies. Helps answer:
- "What will break if I change this model?"
- "Which models depend on this source?"

### drift
Detect schema drift from warehouse.

```bash
schemarefly drift [--output drift-report.json]
```

**Status**: 🚧 Planned for Phase 5

### init-contracts
Generate contracts from current schemas.

```bash
schemarefly init-contracts [models...]
```

**Status**: 🚧 Planned for Phase 4

## Development

### Prerequisites
- Rust 1.70+ (uses 2021 edition)
- Cargo

### Build
```bash
cargo build
```

### Test
```bash
cargo test
```

### Run
```bash
cargo run --bin schemarefly -- check --verbose
```

## Roadmap

- ✅ **Phase 0**: Standards + interfaces (COMPLETED)
- ✅ **Phase 1**: dbt ingestion + DAG + contracts (COMPLETED)
- ✅ **Phase 2**: SQL parsing layer (COMPLETED)
- 🚧 **Phase 3**: SQL schema inference MVP
- 🚧 **Phase 4**: Contract diff engine + CI gate
- 🚧 **Phase 5**: Warehouse drift mode
- 🚧 **Phase 6**: Incremental performance hardening
- 🚧 **Phase 7**: LSP
- 🚧 **Phase 8**: Industry standard hardening

See [SchemaRefly Engineering Doc.md](SchemaRefly%20Engineering%20Doc.md) for details.

## Why Rust?

- **Incremental computation**: Salsa enables fast, memoized recomputation
- **Type safety**: Catch errors at compile time
- **Performance**: Fast enough for large dbt projects
- **Single binary**: Easy deployment, no runtime dependencies
- **LSP support**: First-class IDE integration

## License

MIT OR Apache-2.0

## Contributing

This project is in active development. Phase 0 is complete, and we're building towards Phase 1 (dbt ingestion).

For questions or contributions, please open an issue or PR.
