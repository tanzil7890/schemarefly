# SchemaRefly - Real-Life Use Cases

## 1. Preventing Production Outages

**Scenario:** A data engineer modifies a staging model and removes a column that's used by 5 downstream tables and 3 dashboards.

**How SchemaRefly helps:**
- Detects the missing column during the CI/CD pipeline
- Shows the blast radius: "This change affects 8 downstream models"
- Blocks the deployment automatically
- Prevents the incident entirely

**Impact:** Saves thousands in incident response costs and prevents data pipeline failures.

---

## 2. Fast CI/CD with Slim CI

**Scenario:** You have 1,000+ dbt models and make a small change to one model.

**How SchemaRefly helps:**
- Instead of checking all 1,000 models, checks only the 15 modified ones
- Compares against a saved production state
- Runs in seconds instead of minutes
- Makes iterative development faster

**Impact:** Reduces CI/CD time from 20+ minutes to under 1 minute for most changes.

---

## 3. Type Safety in Data Pipelines

**Scenario:** A source system changes a column from `DECIMAL(10,2)` to `FLOAT`, breaking your downstream calculations.

**How SchemaRefly helps:**
- Detects the type change via warehouse drift detection
- Alerts you immediately
- Prevents bad data from flowing into analytics/ML pipelines
- Enforces data quality guarantees

**Impact:** Prevents silent data quality issues that could corrupt downstream analytics.

---

## 4. Contract-First Data Development

**Scenario:** Teams want to enforce that every dbt model has an explicit schema contract (like contracts in software).

**How SchemaRefly helps:**
- Validates that declared contracts match reality
- Catches extra columns creeping in
- Ensures backward compatibility when widening types
- Makes schema evolution intentional, not accidental

**Impact:** Creates a culture of intentional data design and reduces technical debt.

---

## 5. Debugging Cross-Team Dependencies

**Scenario:** "If I change this raw source table, what models will break?"

**How SchemaRefly helps:**
- Runs: `schemarefly impact raw_events`
- Shows complete dependency graph (transitive closure)
- Identifies all downstream impacts
- Prevents surprise breaking changes

**Impact:** Enables safe refactoring and makes data lineage transparent.

---

## 6. IDE-Assisted Development

**Scenario:** Data engineer wants immediate feedback while editing SQL in VS Code.

**How SchemaRefly helps:**
- VS Code extension shows contract violations inline
- Hover over columns to see inferred types
- Jump to contract definitions
- No need to leave the editor

**Impact:** Developer experience similar to traditional software development with instant feedback.

---

## Who Benefits Most

| Role | Benefit |
|------|---------|
| **Data Engineers** | Catch schema bugs before production, faster CI builds |
| **Analytics Engineers** | Ensure contracts on transformation models, prevent dashboard breaks |
| **Data Platform Teams** | Enforce data quality standards, reduce operational incidents |
| **ML Engineers** | Detect upstream schema drift affecting model features |
| **Data Product Managers** | Reliable, stable data contracts for APIs/exports |

---

## Why It's Built in Rust

- **Speed**: Incremental computation (using Salsa) means only changed models are reanalyzed
- **Reliability**: Type safety catches bugs at compile time
- **Easy Distribution**: Single binary, no runtime dependencies needed
- **IDE Integration**: Native LSP support for editor integration

SchemaRefly brings **contract-driven development** to dbt, similar to type systems in traditional software engineering—catching errors before they reach production. 🎯

---

## Getting Started with These Use Cases

### For Preventing Outages
```bash
# Add to your CI/CD pipeline
schemarefly check --output report.json