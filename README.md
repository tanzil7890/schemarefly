# SchemaRefly

**Schema contract verification for dbt** - Catch breaking changes before they break production.

SchemaRefly is a Rust-based static analysis tool that validates dbt schema contracts by analyzing SQL and comparing inferred schemas against declared contracts. It provides fast, incremental checking with detailed impact analysis and Slim CI integration for modern dbt workflows.

## Features

- **Schema Contract Validation** - Validate dbt contracts before deployment
- **Slim CI Integration** - Compare against production state, check only modified models
- **Blast Radius Analysis** - See downstream impact of changes
- **Multi-Dialect SQL** - BigQuery, Snowflake, Postgres, ANSI
- **Jinja2 Support** - Full dbt template preprocessing (ref, source, var, config)
- **100% Compatibility** - Tested on 140+ real dbt models across 13 projects

## Status

🎯 **Production Ready** - All core features implemented and tested

### Completed Phases ✅
- **Phase 0**: Standards + interfaces (diagnostic codes, report schema, config)
- **Phase 1**: dbt ingestion + DAG + contracts + impact analysis
- **Phase 2**: SQL parsing layer (multi-dialect, Jinja2 preprocessing)
- **Slim CI**: State comparison + modified-only checks + blast radius
- **Phase 3**: Release toolchain (signed binaries, attestations, stability contract)
- **Phase 4**: Frictionless adoption (init, init-contracts, PR comment mode)
- **Phase 5**: VS Code extension packaging

### In Progress 🔄
- **Phase 6**: Warehouse drift mode
- **Phase 7**: Incremental performance hardening

See [v1_extended.md](v1_extended.md) for the detailed roadmap.

## Quick Start

### Installation

#### Pre-built Binaries (Recommended)

Download the latest release from [GitHub Releases](https://github.com/owner/schemarefly/releases):

```bash
# Linux x86_64
curl -fsSL https://github.com/owner/schemarefly/releases/latest/download/schemarefly-x86_64-unknown-linux-gnu.tar.gz | tar -xz

# macOS Apple Silicon
curl -fsSL https://github.com/owner/schemarefly/releases/latest/download/schemarefly-aarch64-apple-darwin.tar.gz | tar -xz

# macOS Intel
curl -fsSL https://github.com/owner/schemarefly/releases/latest/download/schemarefly-x86_64-apple-darwin.tar.gz | tar -xz

# Move to PATH
sudo mv schemarefly-*/schemarefly /usr/local/bin/
```

#### Verify Download

All binaries include SHA-256 checksums and GitHub artifact attestations:

```bash
# Verify checksum
shasum -a 256 -c schemarefly-*.sha256

# Verify attestation (requires GitHub CLI)
gh attestation verify schemarefly-*.tar.gz --repo owner/schemarefly
```

#### Build from Source

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
│   ├── schemarefly-dbt/       # dbt artifact parsing
│   ├── schemarefly-sql/       # SQL parsing & inference
│   ├── schemarefly-catalog/   # Warehouse metadata adapters
│   ├── schemarefly-engine/    # State comparison & drift detection
│   ├── schemarefly-incremental/ # Salsa incremental engine
│   ├── schemarefly-cli/       # CLI application
│   └── schemarefly-lsp/       # LSP server
├── editors/
│   └── vscode/               # VS Code extension
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

### init
Initialize SchemaRefly in a dbt project.

```bash
schemarefly init [--dialect bigquery] [--path .]

# Examples
schemarefly init --dialect snowflake
schemarefly init --skip-workflow  # Skip GitHub workflow creation
schemarefly init --force          # Overwrite existing files
```

**Status**: ✅ Functional

Creates `schemarefly.toml` and `.github/workflows/schemarefly.yml` with best-practice defaults.

### check
Validate schema contracts against inferred schemas.

```bash
schemarefly check [--output report.json] [--markdown report.md]

# Slim CI mode
schemarefly check --state prod/manifest.json --modified-only

# PR comment mode (outputs GitHub-optimized markdown)
schemarefly check --pr-comment > pr-comment.md
```

**Status**: ✅ Functional

### init-contracts
Generate contract stubs from current schemas.

```bash
schemarefly init-contracts [models...] [--output-dir contracts]

# Examples
schemarefly init-contracts                        # All models
schemarefly init-contracts users orders           # Specific models
schemarefly init-contracts --catalog target/catalog.json  # Use catalog for types
schemarefly init-contracts --enforced-only        # Only models with enforced contracts
```

**Status**: ✅ Functional

Generates YAML contract stubs ready to copy into your dbt schema.yml files.

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

**Status**: 🚧 Planned for Phase 6

## VS Code Extension

Real-time schema contract verification in your editor.

### Features
- **Diagnostics on save** - Contract violations shown inline
- **Hover for schema** - See inferred column types
- **Go-to-definition** - Jump to contract definitions
- **Offline mode** - Works without warehouse connection

### Installation

```bash
# Install from VS Code Marketplace
code --install-extension schemarefly.schemarefly

# Or install from VSIX
code --install-extension schemarefly-0.1.0.vsix
```

The extension requires the `schemarefly-lsp` binary. See [editors/vscode/README.md](editors/vscode/README.md) for details.

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
- ✅ **Slim CI**: State comparison + modified-only checks (COMPLETED)
- ✅ **Phase 3**: Release toolchain (COMPLETED)
- ✅ **Phase 4**: Frictionless adoption (COMPLETED)
- ✅ **Phase 5**: VS Code extension (COMPLETED)
- 🚧 **Phase 6**: Warehouse drift mode
- 🚧 **Phase 7**: Incremental performance hardening

See [v1_extended.md](v1_extended.md) for detailed roadmap.

## Stability

SchemaRefly follows semantic versioning with documented stability guarantees:

- **Report schema**: Versioned (v1.0), backward-compatible
- **Diagnostic codes**: Immutable, never renamed or removed
- **CLI exit codes**: Stable and documented
- **Configuration**: Forward-compatible

See [STABILITY.md](STABILITY.md) for the complete stability contract.

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
