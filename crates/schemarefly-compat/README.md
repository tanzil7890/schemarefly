# SchemaRefly Compatibility Test Suite

This crate provides infrastructure to validate SchemaRefly against real dbt projects, tracking parse success rates, schema inference rates, and failure patterns across different SQL dialects.

## Purpose

As recommended in v1_extended.md section 1 ("Prove it on real dbt repos"), this compatibility suite enables:

1. **Parse Success Rate**: % of dbt models that parse successfully
2. **Schema Inference Rate**: % of models with inferred output schema
3. **Failure Analysis**: Top failure codes with samples for targeted improvements
4. **Model Type Detection**: Identifies unsupported models (Python, ephemeral, seeds, snapshots)

## Architecture

### Components

- **CompatTestHarness**: Main test runner that processes dbt projects
- **CompatMetrics**: Metrics collection (success rates, failure codes, samples)
- **ModelDetection**: Detects unsupported model types (Python, ephemeral, etc.)
- **CompatReport**: Terminal and JSON reporting with aggregate statistics

### Workflow

```
dbt project → CompatTestHarness → Per-model checks → CompatMetrics → CompatReport
                     ↓                    ↓
             Load manifest.json    Parse SQL + Infer schema
```

## Usage

### Running Compatibility Tests

Use the provided example binary:

```bash
cargo run --package schemarefly-compat --example run_compat_suite -- /path/to/dbt/project bigquery
```

Output:
```
╔══════════════════════════════════════════════════════════════════╗
║       SchemaRefly Compatibility Test Report                     ║
╚══════════════════════════════════════════════════════════════════╝

Aggregate Statistics:
  Total Projects:              1
  Total Models:                127
  Parse Success Rate:          98.4%
  Schema Inference Rate:       95.2%
  Unsupported Models:          3

Per-Project Breakdown:
  my_dbt_project (bigquery)
    Total Models:            127
    Parsed Successfully:     125 (98.4%)
    Schema Inferred:         121 (95.2%)
    Parse Failures:          2
    Inference Failures:      4
    Unsupported:             3
    Top Failure Codes:
      SR001 - 2 occurrences
        Sample 1: Unsupported function SAFE_DIVIDE in expression
      SR010 - 4 occurrences
        Sample 1: Could not infer schema for complex CASE expression
```

### Programmatic Usage

```rust
use schemarefly_compat::{CompatTestHarness, CompatReport};
use schemarefly_core::config::{Config, DialectConfig};

let config = Config {
    dialect: DialectConfig::BigQuery,
    // ... other config
};

let mut harness = CompatTestHarness::new("/path/to/dbt/project", config);
harness.load_manifest()?;

let metrics = harness.run_checks()?;
let report = CompatReport::new(vec![metrics]);

report.print_terminal_report();
report.save_json("report.json")?;
```

## Model Type Detection

The suite automatically detects unsupported dbt model types:

| Model Type | Supported | Reason |
|------------|-----------|--------|
| SQL Model (table, view, incremental) | ✅ | Standard SQL models support dbt contracts |
| Ephemeral Model | ❌ | dbt contracts not supported for ephemeral materialization |
| Python Model | ❌ | dbt contracts only available for SQL models |
| Seed (CSV) | ❌ | dbt contracts not supported for seeds |
| Snapshot | ❌ | dbt contracts not supported for snapshots |

Unsupported models are flagged with helpful diagnostic messages and don't count as failures.

## Metrics Tracked

### Per-Project Metrics

- `total_models`: All models in manifest
- `parsed_successfully`: Models that passed SQL parsing
- `schema_inferred`: Models with inferred output schema
- `parse_failures`: SQL parsing errors
- `inference_failures`: Schema inference errors (parsed but couldn't infer schema)
- `unsupported_models`: Models excluded due to type (Python, ephemeral, etc.)

### Aggregate Metrics

- `overall_parse_success_rate`: Total parsed / total models
- `overall_schema_inference_rate`: Total inferred / total models
- `top_failure_codes`: Most frequent diagnostic codes with samples

## Testing Against Real Projects

### Recommended Test Suite

Test against 10-20 real dbt projects covering:

1. **Dialect Mix**:
   - 5-7 BigQuery projects
   - 5-7 Snowflake projects
   - 3-5 Postgres projects

2. **Project Sizes**:
   - Small (10-50 models)
   - Medium (50-200 models)
   - Large (200+ models)

3. **Feature Coverage**:
   - CTEs (WITH clauses)
   - Window functions
   - UDFs/macros
   - Complex JOINs
   - SELECT * patterns
   - Dialect-specific functions (SAFE_DIVIDE, QUALIFY, etc.)

### Creating a Test Suite Directory

```
compat-test-suite/
├── bigquery/
│   ├── jaffle-shop/         # Small sample project
│   ├── analytics-prod/      # Large production project
│   └── ecommerce/           # Medium project with macros
├── snowflake/
│   ├── data-warehouse/
│   └── marketing/
├── postgres/
│   └── transactional/
└── run_all_tests.sh         # Script to run all projects
```

## Failure Code Analysis

Common failure codes you might encounter:

- **SR000**: File read errors (permissions, missing files)
- **SR001**: SQL parse errors (syntax errors, unsupported dialect features)
- **SR010**: Schema inference failures (complex queries, unsupported functions)

Use failure samples to:
1. Identify common patterns
2. Prioritize inference engine improvements
3. Create targeted test cases
4. Update documentation with known limitations

## Report Formats

### Terminal Report

Human-readable colored output with:
- Aggregate statistics
- Per-project breakdown
- Top 5 failure codes with samples
- Pass/fail summary with thresholds

### JSON Report

Machine-readable JSON with full details:
- All model results
- Complete failure samples
- Diagnostic codes
- Success/failure breakdown

Example JSON structure:
```json
{
  "projects": [{
    "project_name": "my_dbt_project",
    "dialect": "bigquery",
    "total_models": 127,
    "parsed_successfully": 125,
    "schema_inferred": 121,
    "parse_failures": 2,
    "inference_failures": 4,
    "unsupported_models": 3,
    "model_results": [
      {
        "model_name": "users",
        "file_path": "models/staging/users.sql",
        "outcome": {
          "type": "success",
          "schema_inferred": true
        }
      }
    ],
    "failure_codes": {
      "SR001": 2,
      "SR010": 4
    },
    "failure_samples": {
      "SR001": ["Sample error message..."]
    }
  }],
  "aggregate": {
    "total_projects": 1,
    "total_models": 127,
    "overall_parse_success_rate": 0.984,
    "overall_schema_inference_rate": 0.952,
    "total_unsupported": 3
  }
}
```

## Performance Thresholds

The report evaluates performance against industry standards:

| Metric | Excellent | Good | Needs Improvement |
|--------|-----------|------|-------------------|
| Parse Success Rate | ≥95% | ≥85% | <85% |
| Schema Inference Rate | ≥90% | ≥75% | <75% |

## Integration with CI/CD

Run compat tests in CI to track improvements over time:

```yaml
# .github/workflows/compat-tests.yml
name: Compatibility Tests

on: [push]

jobs:
  compat:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run compat suite
        run: |
          cargo run --package schemarefly-compat --example run_compat_suite \
            -- tests/fixtures/jaffle-shop bigquery
      - name: Upload report
        uses: actions/upload-artifact@v3
        with:
          name: compat-report
          path: tests/fixtures/jaffle-shop/schemarefly-compat-report.json
```

## Contributing

To add a new test project to the suite:

1. Add project to `tests/fixtures/<dialect>/<project-name>/`
2. Include `target/manifest.json` (generated by `dbt compile`)
3. Document expected success rates and known limitations
4. Update `run_all_tests.sh` to include new project

## See Also

- [v1_extended.md](../../v1_extended.md) - Extended roadmap section 1
- [SchemaRefly Engineering Doc.md](../../SchemaRefly%20Engineering%20Doc.md) - Overall architecture
- [DIALECT_GUIDE.md](../../DIALECT_GUIDE.md) - SQL dialect support

## License

MIT OR Apache-2.0 (same as SchemaRefly)
