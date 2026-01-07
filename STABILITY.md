# SchemaRefly Stability Contract

This document defines the stability guarantees for SchemaRefly. We take backward compatibility seriously to ensure your CI pipelines and integrations don't break unexpectedly.

## Version Scheme

SchemaRefly follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH[-PRERELEASE]
```

- **MAJOR**: Breaking changes to stable APIs
- **MINOR**: New features, backward-compatible
- **PATCH**: Bug fixes, backward-compatible
- **PRERELEASE**: Alpha/beta releases (e.g., `1.0.0-beta.1`)

## Stability Tiers

### Tier 1: Stable (Breaking changes only in MAJOR versions)

These interfaces are production-ready and will not change without a major version bump:

| Interface | Description | Since |
|-----------|-------------|-------|
| `report.json` schema | Output report format | v0.1.0 |
| Diagnostic codes | Error/warning identifiers | v0.1.0 |
| CLI exit codes | Process return values | v0.1.0 |
| `schemarefly.toml` config | Configuration file format | v0.1.0 |

### Tier 2: Stable with Extensions (Additions in MINOR versions)

New fields may be added in minor versions, but existing fields won't change:

| Interface | Description |
|-----------|-------------|
| Report metadata | `report.json` metadata section |
| Slim CI output | State comparison results |

### Tier 3: Experimental (May change in any version)

These interfaces are under active development:

| Interface | Description |
|-----------|-------------|
| LSP protocol extensions | IDE integration features |
| Internal crate APIs | Rust library interfaces |

---

## Report Schema Stability (v1.0)

The `report.json` output format is **Tier 1 Stable**.

### Schema Version

```json
{
  "version": {
    "major": 1,
    "minor": 0
  }
}
```

### Versioning Rules

1. **Major version bump (1.x → 2.x)**: Breaking changes
   - Removing fields
   - Renaming fields
   - Changing field types
   - Changing field semantics

2. **Minor version bump (1.0 → 1.1)**: Backward-compatible additions
   - Adding new optional fields
   - Adding new diagnostic codes
   - Adding new metadata sections

3. **Patch version**: No schema changes
   - Bug fixes only
   - Documentation updates

### Backward Compatibility Promise

When parsing `report.json`:

```python
# Your code should handle unknown fields gracefully
report = json.load(open("report.json"))
version = report["version"]

if version["major"] == 1:
    # All v1.x reports are compatible
    diagnostics = report["diagnostics"]
    # Unknown fields should be ignored, not cause errors
```

### Current Schema (v1.0)

```json
{
  "version": { "major": 1, "minor": 0 },
  "timestamp": "2025-01-07T12:00:00Z",
  "content_hash": "sha256:...",
  "summary": {
    "total": 0,
    "errors": 0,
    "warnings": 0,
    "info": 0,
    "models_checked": 0,
    "contracts_validated": 0
  },
  "diagnostics": [
    {
      "code": "CONTRACT_MISSING_COLUMN",
      "severity": "error",
      "message": "...",
      "location": { "file": "...", "line": 1 },
      "expected": "...",
      "actual": "...",
      "impact": ["downstream_model"]
    }
  ],
  "metadata": { }
}
```

---

## Diagnostic Code Stability

Diagnostic codes are **Tier 1 Stable** and follow strict rules:

### Immutability Rules

1. **Never rename codes**: Once a code is published, its string identifier never changes
2. **Never remove codes**: Deprecated codes remain valid indefinitely
3. **Never change semantics**: A code's meaning is fixed at introduction
4. **Add new codes only**: New diagnostics get new unique codes

### Code Registry

| Code | Category | Introduced | Status |
|------|----------|------------|--------|
| `CONTRACT_MISSING_COLUMN` | Contract | v0.1.0 | Stable |
| `CONTRACT_TYPE_MISMATCH` | Contract | v0.1.0 | Stable |
| `CONTRACT_EXTRA_COLUMN` | Contract | v0.1.0 | Stable |
| `CONTRACT_MISSING` | Contract | v0.1.0 | Stable |
| `DRIFT_COLUMN_DROPPED` | Drift | v0.1.0 | Stable |
| `DRIFT_TYPE_CHANGE` | Drift | v0.1.0 | Stable |
| `DRIFT_COLUMN_ADDED` | Drift | v0.1.0 | Stable |
| `SQL_SELECT_STAR_UNEXPANDABLE` | SQL | v0.1.0 | Stable |
| `SQL_UNSUPPORTED_SYNTAX` | SQL | v0.1.0 | Stable |
| `SQL_PARSE_ERROR` | SQL | v0.1.0 | Stable |
| `SQL_INFERENCE_ERROR` | SQL | v0.1.0 | Stable |
| `SQL_GROUP_BY_AGGREGATE_UNALIASED` | SQL | v0.1.0 | Stable |
| `JINJA_RENDER_ERROR` | Jinja | v0.1.0 | Stable |
| `JINJA_UNDEFINED_VARIABLE` | Jinja | v0.1.0 | Stable |
| `JINJA_SYNTAX_ERROR` | Jinja | v0.1.0 | Stable |
| `INTERNAL_ERROR` | Internal | v0.1.0 | Stable |
| `INFO` | General | v0.1.0 | Stable |
| `WARNING` | General | v0.1.0 | Stable |

### Code Numbering Convention

- `1xxx`: Contract violations
- `2xxx`: Drift detection
- `3xxx`: SQL inference issues
- `4xxx`: Jinja template issues
- `8xxx`: Internal errors
- `9xxx`: General messages

---

## CLI Exit Codes

Exit codes are **Tier 1 Stable**:

| Code | Meaning |
|------|---------|
| `0` | Success, no errors |
| `1` | Contract violations found (errors) |
| `2` | Invalid arguments or configuration |
| `3` | IO error (file not found, permission denied) |
| `4` | Internal error |

### Usage in CI

```yaml
# GitHub Actions example
- name: Check contracts
  run: schemarefly check
  continue-on-error: false  # Fails on exit code != 0

# Or capture specific exit codes
- name: Check contracts (allow warnings)
  run: |
    schemarefly check || exit_code=$?
    if [ "$exit_code" -eq 1 ]; then
      echo "Warnings found, but continuing"
    elif [ "$exit_code" -ne 0 ]; then
      exit $exit_code
    fi
```

---

## Configuration Stability

The `schemarefly.toml` configuration format is **Tier 1 Stable**:

### Current Schema

```toml
# SQL dialect (required)
dialect = "bigquery"  # bigquery | snowflake | postgres | ansi

# Severity overrides (optional)
[severity.overrides]
CONTRACT_EXTRA_COLUMN = "warn"

# Allowlist rules (optional)
[allowlist]
allow_widening = ["staging.*"]
allow_extra_columns = ["staging.*"]
skip_models = ["temp_*"]
```

### Compatibility Rules

1. New configuration keys may be added in minor versions
2. Existing keys retain their meaning and default values
3. Unknown keys are ignored (forward compatibility)
4. Invalid values produce clear error messages

---

## Deprecation Policy

When we need to deprecate functionality:

### Timeline

1. **Deprecation announcement**: Feature marked deprecated in release notes
2. **Warning period**: 2 minor versions with deprecation warnings
3. **Removal**: Feature removed in next major version

### Example Timeline

```
v1.2.0 - Feature X deprecated (announcement)
v1.3.0 - Deprecation warning when using Feature X
v1.4.0 - Deprecation warning continues
v2.0.0 - Feature X removed
```

### Deprecation Markers

Deprecated features are marked in:
- Release notes
- CLI output (warning message)
- Documentation
- Code comments

---

## Supply Chain Security

### Artifact Verification

All release binaries include:

1. **SHA-256 checksums**: Verify file integrity
   ```bash
   shasum -a 256 -c schemarefly-*.sha256
   ```

2. **GitHub artifact attestations**: Verify provenance
   ```bash
   gh attestation verify schemarefly-*.tar.gz --repo owner/schemarefly
   ```

### Reproducible Builds

Reports include a `content_hash` field for deterministic verification:
- Same input produces same hash (excluding timestamp)
- Useful for caching and verification

---

## Support Policy

| Version | Status | Support |
|---------|--------|---------|
| 0.x.x | Current | Active development |
| Future 1.x.x | Planned | LTS when released |

### Getting Help

- GitHub Issues: Bug reports and feature requests
- Discussions: Questions and community support

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for detailed release history.

---

*Last updated: January 2026*
