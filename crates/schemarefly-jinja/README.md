# schemarefly-jinja

Jinja2 template preprocessing for dbt SQL models in SchemaRefly.

## Overview

This crate provides industry-standard Jinja2 template preprocessing for dbt SQL models using [MiniJinja](https://github.com/mitsuhiko/minijinja), created by Armin Ronacher (the original Jinja2 author). It enables SchemaRefly to parse real-world dbt projects that use Jinja templates.

## Features

- **Automatic Jinja Detection**: Detects `{{ }}`, `{% %}`, and `{# #}` patterns
- **dbt Functions**: Implements `ref()`, `source()`, `var()`, `config()`
- **Zero Overhead**: Passthrough for non-Jinja SQL (no rendering overhead)
- **Comprehensive Diagnostics**: Clear error messages with file paths and context
- **dbt Context**: Supports variables, target configuration, and project settings

## Usage

### Basic Usage

```rust
use schemarefly_jinja::JinjaPreprocessor;

let preprocessor = JinjaPreprocessor::with_defaults();
let sql = "select * from {{ ref('my_model') }}";
let result = preprocessor.preprocess(sql, None)?;

println!("Original: {}", result.original_sql);
println!("Rendered: {}", result.rendered_sql); // "select * from my_model"
```

### Integration with SQL Parser

```rust
use schemarefly_sql::SqlParser;

let parser = SqlParser::postgres();
let sql = "select * from {{ ref('customers') }} where status = '{{ var(\"status\", \"active\") }}'";

// Automatic Jinja preprocessing
let parsed = parser.parse_with_jinja(sql, None, None)?;
```

### Custom dbt Context

```rust
use schemarefly_jinja::{DbtContext, DbtContextBuilder};
use serde_json::json;

let context = DbtContextBuilder::new()
    .var("status", json!("active"))
    .var("limit", json!(100))
    .target_name("prod")
    .target_schema("analytics")
    .build();

let preprocessor = JinjaPreprocessor::new(context);
let result = preprocessor.preprocess(
    "select * from {{ source('raw', 'users') }} limit {{ var('limit') }}",
    None
)?;

// Result: "select * from raw.users limit 100"
```

## Supported dbt Functions

### `ref()`
References another dbt model:
```sql
{{ ref('model_name') }}          → model_name
{{ ref('package', 'model_name') }} → model_name
```

### `source()`
References a source table:
```sql
{{ source('source_name', 'table_name') }} → source_name.table_name
```

### `var()`
Accesses project variables:
```sql
{{ var('variable_name') }}              → Error if not defined
{{ var('variable_name', 'default') }}   → default
```

### `config()`
Model configuration (returns empty string):
```sql
{{ config(materialized='table', schema='staging') }} → (empty)
```

## Error Handling

The preprocessor provides detailed error diagnostics:

```rust
use schemarefly_jinja::PreprocessError;

let result = preprocessor.preprocess("{{ undefined_var }}", None);

match result {
    Err(PreprocessError::UndefinedVariable { name, .. }) => {
        println!("Undefined variable: {}", name);
    }
    Err(PreprocessError::RenderError { message, .. }) => {
        println!("Render error: {}", message);
    }
    Ok(result) => { /* success */ }
}
```

## Diagnostic Codes

- `JINJA_RENDER_ERROR`: Template rendering failed
- `JINJA_UNDEFINED_VARIABLE`: Variable not found in context
- `JINJA_SYNTAX_ERROR`: Invalid Jinja syntax

## Architecture

```
Raw SQL with Jinja → JinjaPreprocessor → Pure SQL → SqlParser → ParsedSql
```

1. **Detection**: Check for Jinja patterns (`{{ }}`, `{% %}`, `{# #}`)
2. **Rendering**: Use MiniJinja to render templates with dbt context
3. **Passthrough**: If no Jinja detected, return original SQL (zero overhead)

## Testing

Run the unit tests:

```bash
cargo test --package schemarefly-jinja
```

Example tests include:
- Jinja detection
- ref() and source() rendering
- Variable substitution
- Comment removal
- Passthrough for non-Jinja SQL

## Performance

- **Detection**: O(n) scan for Jinja markers
- **Rendering**: Only for files with Jinja templates
- **Passthrough**: Zero overhead for pure SQL files

## Compatibility

Compatible with dbt's Jinja implementation:
- Standard Jinja2 syntax
- dbt-specific functions (ref, source, var, config)
- Comments and whitespace control

## Dependencies

- `minijinja` 2.5: Industry-standard Jinja2 implementation
- `schemarefly-core`: Diagnostic types
- `schemarefly-dbt`: dbt types (for future integration)

## License

MIT OR Apache-2.0
