//! Warehouse metadata caching with TTL
//!
//! This module provides caching for warehouse metadata fetches with
//! time-to-live (TTL) expiration. This prevents redundant warehouse queries
//! and speeds up drift detection.

use schemarefly_core::Schema;
use schemarefly_catalog::TableIdentifier;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache entry for warehouse metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The cached schema
    schema: Arc<Schema>,

    /// When this entry was created
    created_at: Instant,

    /// Time-to-live for this entry
    ttl: Duration,
}

impl CacheEntry {
    /// Check if this cache entry is still valid
    fn is_valid(&self) -> bool {
        self.created_at.elapsed() < self.ttl
    }
}

/// Warehouse metadata cache with TTL support
///
/// This cache stores warehouse table schemas with configurable TTL.
/// Expired entries are automatically evicted on access.
///
/// ## Usage
///
/// ```rust,ignore
/// use std::time::Duration;
///
/// let cache = WarehouseCache::new(Duration::from_secs(300)); // 5 minute TTL
///
/// // Store schema
/// cache.insert(table_id.clone(), schema);
///
/// // Retrieve schema (if not expired)
/// if let Some(schema) = cache.get(&table_id) {
///     // Use cached schema
/// }
/// ```
pub struct WarehouseCache {
    /// Cache storage
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// Default TTL for cache entries
    default_ttl: Duration,
}

impl WarehouseCache {
    /// Create a new warehouse cache with the given default TTL
    ///
    /// # Arguments
    ///
    /// * `ttl` - Default time-to-live for cache entries (e.g., Duration::from_secs(300) for 5 minutes)
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: ttl,
        }
    }

    /// Create a cache key from table identifier
    ///
    /// Format: "database.schema.table"
    fn cache_key(table: &TableIdentifier) -> String {
        format!("{}.{}.{}", table.database, table.schema, table.table)
    }

    /// Insert a schema into the cache
    ///
    /// # Arguments
    ///
    /// * `table` - Table identifier
    /// * `schema` - Schema to cache
    pub fn insert(&self, table: TableIdentifier, schema: Schema) {
        let key = Self::cache_key(&table);
        let entry = CacheEntry {
            schema: Arc::new(schema),
            created_at: Instant::now(),
            ttl: self.default_ttl,
        };

        if let Ok(mut cache) = self.cache.write() {
            cache.insert(key, entry);
        }
    }

    /// Get a schema from the cache if it exists and is not expired
    ///
    /// Returns None if the entry doesn't exist or has expired.
    ///
    /// # Arguments
    ///
    /// * `table` - Table identifier to look up
    pub fn get(&self, table: &TableIdentifier) -> Option<Arc<Schema>> {
        let key = Self::cache_key(table);

        // Try to read from cache
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(&key) {
                if entry.is_valid() {
                    return Some(Arc::clone(&entry.schema));
                }
            }
        }

        // Entry doesn't exist or is expired - evict it
        self.evict(table);
        None
    }

    /// Evict a specific entry from the cache
    ///
    /// # Arguments
    ///
    /// * `table` - Table identifier to evict
    pub fn evict(&self, table: &TableIdentifier) {
        let key = Self::cache_key(table);

        if let Ok(mut cache) = self.cache.write() {
            cache.remove(&key);
        }
    }

    /// Clear all entries from the cache
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get the number of entries in the cache (including expired)
    pub fn len(&self) -> usize {
        if let Ok(cache) = self.cache.read() {
            cache.len()
        } else {
            0
        }
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Evict all expired entries from the cache
    ///
    /// This is called automatically on access, but can also be called
    /// manually for cleanup.
    pub fn evict_expired(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.retain(|_, entry| entry.is_valid());
        }
    }

    /// Get cache statistics
    ///
    /// Returns (total_entries, valid_entries, expired_entries)
    pub fn stats(&self) -> (usize, usize, usize) {
        if let Ok(cache) = self.cache.read() {
            let total = cache.len();
            let valid = cache.values().filter(|e| e.is_valid()).count();
            let expired = total - valid;
            (total, valid, expired)
        } else {
            (0, 0, 0)
        }
    }
}

impl Default for WarehouseCache {
    /// Create a cache with default TTL of 5 minutes
    fn default() -> Self {
        Self::new(Duration::from_secs(300))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_core::{Column, LogicalType};
    use std::thread::sleep;

    fn create_test_schema() -> Schema {
        Schema::from_columns(vec![
            Column::new("id", LogicalType::Int),
            Column::new("name", LogicalType::String),
        ])
    }

    fn create_test_table() -> TableIdentifier {
        TableIdentifier {
            database: "my_db".to_string(),
            schema: "my_schema".to_string(),
            table: "my_table".to_string(),
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = WarehouseCache::new(Duration::from_secs(60));
        let table = create_test_table();
        let schema = create_test_schema();

        // Insert schema
        cache.insert(table.clone(), schema.clone());

        // Should be able to retrieve it
        let cached = cache.get(&table);
        assert!(cached.is_some());

        let cached_schema = cached.unwrap();
        assert_eq!(cached_schema.columns.len(), 2);
    }

    #[test]
    fn test_cache_expiration() {
        let cache = WarehouseCache::new(Duration::from_millis(100));
        let table = create_test_table();
        let schema = create_test_schema();

        // Insert schema
        cache.insert(table.clone(), schema);

        // Should be available immediately
        assert!(cache.get(&table).is_some());

        // Wait for expiration
        sleep(Duration::from_millis(150));

        // Should be expired now
        assert!(cache.get(&table).is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = WarehouseCache::new(Duration::from_secs(60));
        let table = create_test_table();
        let schema = create_test_schema();

        // Insert schema
        cache.insert(table.clone(), schema);
        assert!(cache.get(&table).is_some());

        // Evict
        cache.evict(&table);
        assert!(cache.get(&table).is_none());
    }

    #[test]
    fn test_cache_clear() {
        let cache = WarehouseCache::new(Duration::from_secs(60));
        let table1 = create_test_table();
        let table2 = TableIdentifier {
            database: "db2".to_string(),
            schema: "schema2".to_string(),
            table: "table2".to_string(),
        };

        cache.insert(table1.clone(), create_test_schema());
        cache.insert(table2.clone(), create_test_schema());

        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = WarehouseCache::new(Duration::from_millis(100));
        let table1 = create_test_table();
        let table2 = TableIdentifier {
            database: "db2".to_string(),
            schema: "schema2".to_string(),
            table: "table2".to_string(),
        };

        cache.insert(table1, create_test_schema());
        cache.insert(table2, create_test_schema());

        // Both should be valid
        let (total, valid, expired) = cache.stats();
        assert_eq!(total, 2);
        assert_eq!(valid, 2);
        assert_eq!(expired, 0);

        // Wait for expiration
        sleep(Duration::from_millis(150));

        // Both should be expired
        let (total, valid, expired) = cache.stats();
        assert_eq!(total, 2);
        assert_eq!(valid, 0);
        assert_eq!(expired, 2);
    }
}
