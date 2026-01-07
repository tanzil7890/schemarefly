# Changelog

All notable changes to SchemaRefly are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Release workflow with multi-platform binaries (Linux, macOS, Windows)
- Artifact attestations for supply-chain security
- STABILITY.md documenting versioning and deprecation policies
- SHA-256 checksums for all release artifacts

## [0.1.0] - 2026-01-07

### Added

#### Core Features
- **Schema Contract Validation**: Validate dbt model contracts against inferred SQL schemas
- **Multi-Dialect SQL Parsing**: Support for BigQuery, Snowflake, Postgres, and ANSI SQL
- **Jinja2 Preprocessing**: Full dbt template support including `ref()`, `source()`, `var()`, `config()`
- **Impact Analysis**: Show downstream dependencies for any model with blast radius calculation

#### Slim CI Integration
- `--state <path>` flag: Compare current manifest against production state
- `--modified-only` flag: Check only modified models and their downstream dependencies
- State comparison engine detecting SQL, column, dependency, contract, and materialization changes
- Blast radius analysis for change impact assessment

#### Report System
- Stable JSON report schema (v1.0) with semantic versioning
- Deterministic content hashing for verification
- Markdown report generation for PR comments
- Slim CI metadata in reports (modified models, blast radius, etc.)

#### Diagnostic System
- Stable, immutable diagnostic codes (CONTRACT_*, DRIFT_*, SQL_*, JINJA_*)
- Severity levels: Error, Warning, Info
- Sensitive data redaction option
- Deterministic diagnostic ordering

#### Configuration
- `schemarefly.toml` configuration file
- Dialect selection (bigquery, snowflake, postgres, ansi)
- Severity overrides per diagnostic code
- Model allowlists (widening, extra columns, skip patterns)

#### Compatibility
- 100% parse success rate across 140+ real dbt models
- Tested on 13 production dbt projects
- Support for 15+ dbt_utils macros
- Custom macro fallbacks for real-world compatibility

#### Infrastructure
- GitHub Actions CI/CD pipeline
- Multi-platform testing (Ubuntu, macOS)
- Release workflow with signed binaries
- Artifact attestations for supply-chain trust

### Technical Details

#### Crates
- `schemarefly-core`: Domain model, diagnostics, report schema, configuration
- `schemarefly-dbt`: Manifest parsing, DAG construction, contract extraction
- `schemarefly-sql`: SQL parsing with sqlparser, schema inference
- `schemarefly-jinja`: Jinja2 preprocessing with MiniJinja
- `schemarefly-engine`: State comparison, contract diff, drift detection
- `schemarefly-incremental`: Salsa-based incremental computation
- `schemarefly-catalog`: Warehouse metadata adapters (BigQuery, Snowflake)
- `schemarefly-cli`: Command-line interface
- `schemarefly-lsp`: Language Server Protocol implementation
- `schemarefly-compat`: Compatibility testing infrastructure

#### Commands
- `schemarefly check`: Validate schema contracts
- `schemarefly impact <model>`: Show downstream dependencies
- `schemarefly drift`: Detect warehouse schema drift (planned)
- `schemarefly init-contracts`: Generate contracts from schemas (planned)

---

[Unreleased]: https://github.com/owner/schemarefly/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/owner/schemarefly/releases/tag/v0.1.0
