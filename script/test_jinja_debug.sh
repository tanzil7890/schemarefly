#!/bin/bash
# Debug script to test Jinja preprocessing

cd /Users/mohammadtanzilidrisi/Tanzil/OUTHAD_MAIN/AI-Projects/SchemaRefly

# Create a simple Rust test program
cat > /tmp/test_jinja.rs << 'RUST_EOF'
fn main() {
    use schemarefly_jinja::JinjaPreprocessor;

    let preprocessor = JinjaPreprocessor::with_defaults();

    // Test 1: config function
    println!("=== Test 1: config ===");
    let sql1 = "{{ config(materialized='view') }}\nSELECT * FROM table";
    match preprocessor.preprocess(sql1, None) {
        Ok(result) => println!("✓ Rendered: {}", result.rendered_sql),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Test 2: dynamic_partition function
    println!("\n=== Test 2: dynamic_partition ===");
    let sql2 = "SELECT {{ dynamic_partition('order_date', 'MONTH') }} FROM table";
    match preprocessor.preprocess(sql2, None) {
        Ok(result) => println!("✓ Rendered: {}", result.rendered_sql),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Test 3: dbt_utils.surrogate_key
    println!("\n=== Test 3: dbt_utils.surrogate_key ===");
    let sql3 = "SELECT {{ dbt_utils.surrogate_key(['col1', 'col2']) }} FROM table";
    match preprocessor.preprocess(sql3, None) {
        Ok(result) => println!("✓ Rendered: {}", result.rendered_sql),
        Err(e) => println!("✗ Error: {}", e),
    }
}
RUST_EOF

# Compile and run
rustc --edition 2021 -L target/debug/deps \
    --extern schemarefly_jinja=target/debug/libschemarefly_jinja.rlib \
    --extern schemarefly_core=target/debug/libschemarefly_core.rlib \
    --extern minijinja=target/debug/deps/libminijinja-*.rlib \
    --extern serde_json=target/debug/deps/libserde_json-*.rlib \
    /tmp/test_jinja.rs -o /tmp/test_jinja 2>&1 | head -20

if [ -f /tmp/test_jinja ]; then
    /tmp/test_jinja
else
    echo "Compilation failed"
fi
