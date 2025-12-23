//! Schema types and canonical type system

use serde::{Deserialize, Serialize};

/// Portable logical type system
///
/// Maps warehouse-specific types to a common representation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LogicalType {
    /// Boolean type
    Bool,

    /// Integer type (any precision)
    Int,

    /// Floating point (any precision)
    Float,

    /// Decimal with precision and scale
    Decimal {
        precision: Option<u16>,
        scale: Option<u16>,
    },

    /// String/text type
    String,

    /// Date (no time component)
    Date,

    /// Timestamp (with time component)
    Timestamp,

    /// JSON/Variant type
    Json,

    /// Structured type with named fields
    Struct {
        fields: Vec<Column>,
    },

    /// Array type
    Array {
        element_type: Box<LogicalType>,
    },

    /// Unknown type (cannot infer)
    Unknown,
}

impl std::fmt::Display for LogicalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool => write!(f, "BOOL"),
            Self::Int => write!(f, "INT"),
            Self::Float => write!(f, "FLOAT"),
            Self::Decimal { precision, scale } => {
                match (precision, scale) {
                    (Some(p), Some(s)) => write!(f, "DECIMAL({}, {})", p, s),
                    (Some(p), None) => write!(f, "DECIMAL({})", p),
                    _ => write!(f, "DECIMAL"),
                }
            }
            Self::String => write!(f, "STRING"),
            Self::Date => write!(f, "DATE"),
            Self::Timestamp => write!(f, "TIMESTAMP"),
            Self::Json => write!(f, "JSON"),
            Self::Struct { .. } => write!(f, "STRUCT"),
            Self::Array { .. } => write!(f, "ARRAY"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Nullability state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Nullability {
    /// Definitely nullable
    Yes,

    /// Definitely not nullable
    No,

    /// Cannot determine nullability
    Unknown,
}

/// Reference to where a column comes from
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnRef {
    /// Source table/model name
    pub source: String,

    /// Original column name
    pub column: String,
}

/// A column in a schema
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Column {
    /// Column name
    pub name: String,

    /// Logical type
    pub logical_type: LogicalType,

    /// Nullability
    pub nullable: Nullability,

    /// Provenance - where this column comes from
    pub provenance: Vec<ColumnRef>,
}

impl Column {
    /// Create a new column with unknown provenance
    pub fn new(name: impl Into<String>, logical_type: LogicalType) -> Self {
        Self {
            name: name.into(),
            logical_type,
            nullable: Nullability::Unknown,
            provenance: Vec::new(),
        }
    }

    /// Set nullability
    pub fn with_nullability(mut self, nullable: Nullability) -> Self {
        self.nullable = nullable;
        self
    }

    /// Set provenance
    pub fn with_provenance(mut self, provenance: Vec<ColumnRef>) -> Self {
        self.provenance = provenance;
        self
    }
}

/// An ordered collection of columns
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Ordered list of columns
    pub columns: Vec<Column>,
}

impl Schema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
        }
    }

    /// Create a schema from columns
    pub fn from_columns(columns: Vec<Column>) -> Self {
        Self { columns }
    }

    /// Find a column by name
    pub fn find_column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Get column names
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

/// Enforcement policy for contracts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnforcementPolicy {
    /// Allow extra columns not in contract
    pub allow_extra_columns: bool,

    /// Allow type widening (e.g., INT -> BIGINT)
    pub allow_widening: bool,
}

impl Default for EnforcementPolicy {
    fn default() -> Self {
        Self {
            allow_extra_columns: false,
            allow_widening: false,
        }
    }
}

/// A contract defines expected schema with enforcement policy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    /// Expected schema
    pub schema: Schema,

    /// Enforcement policy
    pub policy: EnforcementPolicy,

    /// Whether this contract is enforced
    pub enforced: bool,
}

impl Contract {
    /// Create a new enforced contract with default policy
    pub fn new(schema: Schema) -> Self {
        Self {
            schema,
            policy: EnforcementPolicy::default(),
            enforced: true,
        }
    }

    /// Set enforcement policy
    pub fn with_policy(mut self, policy: EnforcementPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Set whether enforced
    pub fn with_enforced(mut self, enforced: bool) -> Self {
        self.enforced = enforced;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_type_display() {
        assert_eq!(LogicalType::Bool.to_string(), "BOOL");
        assert_eq!(
            LogicalType::Decimal { precision: Some(10), scale: Some(2) }.to_string(),
            "DECIMAL(10, 2)"
        );
    }

    #[test]
    fn schema_operations() {
        let schema = Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
        ]);

        assert_eq!(schema.column_names(), vec!["id", "name"]);
        assert!(schema.find_column("id").is_some());
        assert!(schema.find_column("nonexistent").is_none());
    }

    #[test]
    fn contract_creation() {
        let schema = Schema::from_columns(vec![
            Column::new("user_id", LogicalType::Int),
        ]);

        let contract = Contract::new(schema)
            .with_policy(EnforcementPolicy {
                allow_extra_columns: true,
                allow_widening: false,
            });

        assert!(contract.enforced);
        assert!(contract.policy.allow_extra_columns);
        assert!(!contract.policy.allow_widening);
    }
}
