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
# Run a no-op check ()
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



Shows the complete blast radius (transitive closure) of downstream dependencies. Helps answer:
- "What will break if I change this model?"
- "Which models depend on this source?"

### drift
Detect schema drift between your dbt contracts and the actual warehouse schema.

```bash
# Run drift detection
schemarefly drift [--output drift-report.json] [--verbose]

# Example with verbose output
schemarefly drift --verbose
```

**Note**: Requires warehouse feature flags and credentials. See [Warehouse Drift Detection](#warehouse-drift-detection) for setup.

## Warehouse Drift Detection

SchemaRefly can detect schema drift between your dbt contracts and the actual warehouse schema. This helps catch unexpected schema changes before they cause issues in production.

### Building with Warehouse Support

Warehouse adapters are optional features to keep the default binary small:

```bash
# Build with specific warehouse support
cargo build --release --features bigquery
cargo build --release --features snowflake
cargo build --release --features postgres

# Build with all warehouse adapters
cargo build --release --features all-warehouses
```

### Configuration

Add warehouse configuration to your `schemarefly.toml`:

```toml
# SQL dialect (required)
dialect = "bigquery"

[warehouse]
type = "bigquery"  # or "snowflake", "postgres"
use_env_vars = true  # Recommended for security

[warehouse.settings]
# BigQuery
project_id = "my-gcp-project"

# Snowflake (alternative)
# account = "xy12345.us-east-1"
# username = "user"
# warehouse = "COMPUTE_WH"
# database = "MY_DB"

# PostgreSQL (alternative)
# host = "localhost"
# port = "5432"
# database = "mydb"
# username = "user"
```

### Environment Variables

For security, credentials should be set via environment variables:

```bash
# BigQuery (uses Application Default Credentials)
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

# Or use GCP project environment variable
export GCP_PROJECT=my-gcp-project
```

```bash
# Snowflake
export SNOWFLAKE_ACCOUNT=xy12345.us-east-1
export SNOWFLAKE_USER=username
export SNOWFLAKE_PASSWORD=secret
export SNOWFLAKE_WAREHOUSE=COMPUTE_WH  # Optional
export SNOWFLAKE_ROLE=MY_ROLE          # Optional
```

```bash
# PostgreSQL
export PGHOST=localhost
export PGPORT=5432
export PGDATABASE=mydb
export PGUSER=user
export PGPASSWORD=secret
```

SchemaRefly also supports `SCHEMAREFLY_*` prefixed environment variables (e.g., `SCHEMAREFLY_PASSWORD`) which take precedence.

### Running Drift Detection

```bash
# Compile your dbt project first
dbt compile

# Run drift detection
schemarefly drift --verbose

# Output to specific file
schemarefly drift --output my-drift-report.json
```

### Drift Report Output

The drift command generates a JSON report with:

- **Models checked**: Number of models with contracts
- **Drift detections**: Specific schema differences found
- **Severity levels**:
  - `Error`: Dropped columns, breaking type changes
  - `Warning`: Potential issues
  - `Info`: New columns added (non-breaking)

Example output:
```
Detecting schema drift...
Loading manifest from: target/manifest.json
Connecting to BigQuery...
✓ Connection successful

Checking models with contracts...
  ✓ users (5 columns)
  ✗ orders (drift detected)

Drift Report:
[ERROR] Column 'legacy_id' was dropped from warehouse table orders
[ERROR] Column 'amount' type changed: was DECIMAL(10,2), now FLOAT64
[INFO] New column 'updated_at' added to warehouse table orders

Drift report saved to: drift-report.json
```

### Supported Warehouses

| Warehouse | Feature Flag | Authentication |
|-----------|-------------|----------------|
| BigQuery | `bigquery` | Application Default Credentials, Service Account JSON |
| Snowflake | `snowflake` | Password, Key-Pair |
| PostgreSQL | `postgres` | Password, TLS |

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

This project is in active development.  is complete, and we're building towards  (dbt ingestion).

For questions or contributions, please open an issue or PR.
