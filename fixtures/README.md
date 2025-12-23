# Test Fixtures

This directory contains golden test fixtures for SchemaRefly.

## mini-dbt-project

A minimal dbt project with:
- 1 model (`users`) with a contract
- 1 source (`raw.users`)
- manifest.json in target/

Use this for testing basic contract validation.

## Usage

```bash
cd fixtures/mini-dbt-project
schemarefly check
```

Expected: No errors (empty report with 0 diagnostics)
