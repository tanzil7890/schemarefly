# Drift Detection Example

This example demonstrates how to use SchemaRefly's drift detection feature to compare your dbt contracts against actual warehouse schemas.

## Overview

Schema drift occurs when the actual warehouse schema differs from what your dbt contracts expect. This can happen due to:
- Manual DDL changes
- External ETL processes
- Schema migrations
- Concurrent development

SchemaRefly's drift detection helps you catch these differences before they cause issues.

## Prerequisites

1. **dbt project with contracts** - Your models should have `contract: {enforced: true}` and column definitions
2. **Compiled dbt project** - Run `dbt compile` to generate `target/manifest.json`
3. **Warehouse credentials** - Access to your data warehouse
4. **SchemaRefly built with warehouse support**

## Setup

### 1. Build SchemaRefly with Warehouse Support

```bash
# For BigQuery
cargo build --release --features bigquery

# For Snowflake
cargo build --release --features snowflake

# For PostgreSQL
cargo build --release --features postgres

# For all warehouses
cargo build --release --features all-warehouses
```

### 2. Configure SchemaRefly

Copy the example configuration:

```bash
cp schemarefly.toml.example schemarefly.toml
```

Edit `schemarefly.toml` with your settings:

```toml
dialect = "bigquery"

[warehouse]
type = "bigquery"
use_env_vars = true

[warehouse.settings]
project_id = "your-gcp-project"
```

### 3. Set Environment Variables

#### BigQuery

```bash
# Option 1: Service account key file
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

# Option 2: Application Default Credentials (if using gcloud CLI)
gcloud auth application-default login
```

#### Snowflake

```bash
export SNOWFLAKE_ACCOUNT=your-account.region
export SNOWFLAKE_USER=your-username
export SNOWFLAKE_PASSWORD=your-password
export SNOWFLAKE_WAREHOUSE=COMPUTE_WH
export SNOWFLAKE_ROLE=YOUR_ROLE
```

#### PostgreSQL

```bash
export PGHOST=your-host
export PGPORT=5432
export PGDATABASE=your-database
export PGUSER=your-username
export PGPASSWORD=your-password
```

## Running Drift Detection

### 1. Compile Your dbt Project

```bash
cd your-dbt-project
dbt compile
```

### 2. Run SchemaRefly Drift Detection

```bash
# Basic usage
schemarefly drift

# With verbose output
schemarefly drift --verbose

# Save report to specific file
schemarefly drift --output drift-report.json
```

## Understanding the Output

### Console Output

```
Detecting schema drift...
Loading manifest from: target/manifest.json
Connecting to BigQuery...
✓ Connection successful

Checking models with contracts...
  ✓ dim_customers (8 columns)
  ✓ dim_products (6 columns)
  ✗ fct_orders (drift detected)
  ✓ dim_dates (12 columns)

==============================================================
                  Schema Drift Detection Report
==============================================================

Models checked: 4
Models with drift: 1

Drift Details:
[ERROR] Column 'legacy_order_id' was dropped from warehouse table fct_orders
        Expected type: STRING

[ERROR] Column 'total_amount' type changed in warehouse table fct_orders
        Expected: DECIMAL(10,2)
        Actual: FLOAT64

[INFO] New column 'updated_at' added to warehouse table fct_orders
       Type: TIMESTAMP

==============================================================
Summary: 2 errors, 0 warnings, 1 info
==============================================================

Drift report saved to: drift-report.json
```

### JSON Report

The generated `drift-report.json` contains:

```json
{
  "version": {
    "major": 1,
    "minor": 0
  },
  "timestamp": "2025-01-14T10:30:00Z",
  "summary": {
    "total": 3,
    "errors": 2,
    "warnings": 0,
    "info": 1,
    "models_checked": 4,
    "models_with_drift": 1
  },
  "diagnostics": [
    {
      "code": "DRIFT_COLUMN_DROPPED",
      "severity": "error",
      "message": "Column 'legacy_order_id' was dropped from warehouse table (expected type: STRING)",
      "location": {
        "file": "models/marts/fct_orders.sql"
      }
    }
  ]
}
```

## Severity Levels

| Severity | Meaning | Examples |
|----------|---------|----------|
| **Error** | Breaking changes that will cause failures | Dropped columns, type changes |
| **Warning** | Potential issues that need attention | Nullability changes |
| **Info** | Non-breaking changes for awareness | New columns added |

## CI/CD Integration

### GitHub Actions

```yaml
name: Schema Drift Check

on:
  schedule:
    - cron: '0 6 * * *'  # Daily at 6 AM
  workflow_dispatch:

jobs:
  drift-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install SchemaRefly
        run: |
          curl -fsSL https://github.com/owner/schemarefly/releases/latest/download/schemarefly-x86_64-unknown-linux-gnu.tar.gz | tar -xz
          sudo mv schemarefly-*/schemarefly /usr/local/bin/

      - name: Setup dbt
        run: pip install dbt-bigquery

      - name: Compile dbt
        run: dbt compile
        env:
          DBT_PROFILES_DIR: .

      - name: Run Drift Detection
        run: schemarefly drift --output drift-report.json
        env:
          GOOGLE_APPLICATION_CREDENTIALS: ${{ secrets.GCP_SA_KEY }}

      - name: Upload Report
        uses: actions/upload-artifact@v4
        with:
          name: drift-report
          path: drift-report.json

      - name: Fail on Errors
        run: |
          if jq -e '.summary.errors > 0' drift-report.json > /dev/null; then
            echo "Schema drift detected!"
            exit 1
          fi
```

## Troubleshooting

### Connection Issues

```
Error: Authentication failed: Could not find default credentials
```

**Solution**: Ensure `GOOGLE_APPLICATION_CREDENTIALS` is set or run `gcloud auth application-default login`

### Missing Contracts

```
Warning: No models with contracts found
```

**Solution**: Add contracts to your dbt models:

```yaml
models:
  - name: my_model
    config:
      contract:
        enforced: true
    columns:
      - name: id
        data_type: INT64
```

### Feature Not Enabled

```
Error: BigQuery support not compiled. Rebuild with: cargo build --features bigquery
```

**Solution**: Rebuild SchemaRefly with the appropriate feature flag

## Best Practices

1. **Run drift detection regularly** - Schedule daily or weekly checks
2. **Fix errors immediately** - Dropped columns and type changes are breaking
3. **Review info messages** - New columns might indicate unexpected changes
4. **Use in CI/CD** - Catch drift before it reaches production
5. **Keep contracts up to date** - Update contracts when intentional changes are made

## Related Documentation

- [SchemaRefly README](../../README.md)
- [Configuration Reference](../../schemarefly.toml)
- [Warehouse Drift Mode Roadmap](../../ROADMAP-WAREHOUSE-DRIFT.md)
