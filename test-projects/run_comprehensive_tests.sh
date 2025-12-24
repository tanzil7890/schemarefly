#!/bin/bash
cd /Users/mohammadtanzilidrisi/Tanzil/OUTHAD_MAIN/AI-Projects/SchemaRefly

echo "=== COMPREHENSIVE TEST SUITE - DECEMBER 23, 2025 ==="
echo ""
echo "Testing all 13 real dbt projects..."
echo ""

declare -A results

# Test each project
test_project() {
    local name=$1
    local path=$2
    local dialect=$3
    
    echo "Testing: $name ($dialect)"
    result=$(cargo run --release --package schemarefly-compat --example run_compat_suite -- "$path" "$dialect" 2>&1 | grep -E "Parse Success Rate|Total Models:")
    parse_rate=$(echo "$result" | grep "Parse Success Rate" | awk '{print $4}')
    total_models=$(echo "$result" | grep "Total Models:" | awk '{print $3}')
    
    results["$name"]="$parse_rate ($total_models models)"
    echo "  ✓ Result: $parse_rate"
    echo ""
}

# Postgres projects (with corrected paths)
test_project "jaffle_shop_classic" "test-projects/postgres/jaffle_shop_classic" "postgres"
test_project "dbt_postgres_demo" "test-projects/postgres/dbt-postgres-demo" "postgres"
test_project "dbt_local_postgres" "test-projects/postgres/dbt-local-postgresql-tutorial" "postgres"
test_project "dbt_postgres_tutorial" "test-projects/postgres/dbt-postgres-tutorial/jaffle_shop" "postgres"
test_project "dbt_slamco_project" "test-projects/postgres/dbt-project/slamco" "postgres"

# BigQuery projects (with corrected paths)
test_project "dbt_tutorial" "test-projects/bigquery/dbt_tutorial" "bigquery"
test_project "dbt_bigquery_example" "test-projects/bigquery/dbt_bigquery_example" "bigquery"
test_project "dbt_sales_analytics" "test-projects/bigquery/dbt_bigquery_sales_analytics_project" "bigquery"
test_project "dbt-bigquery" "test-projects/bigquery/dbt-bigquery" "bigquery"

# Snowflake projects (with corrected paths)
test_project "tasty_bytes_demo" "test-projects/snowflake/getting-started-with-dbt-on-snowflake/tasty_bytes_dbt_demo" "snowflake"
test_project "dbt_snowflake_public" "test-projects/snowflake/dbt-snowflake-public" "snowflake"
test_project "snowflake_summit_2025" "test-projects/snowflake/snowflake_summit_hol_2025" "snowflake"
test_project "snowflake_demo" "test-projects/snowflake/snowflake-dbt-demo-project" "snowflake"

echo "=== SUMMARY ==="
echo ""
for project in "${!results[@]}"; do
    echo "$project: ${results[$project]}"
done | sort

echo ""
echo "Test complete at $(date)"
