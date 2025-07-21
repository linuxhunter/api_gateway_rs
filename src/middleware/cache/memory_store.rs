use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;

use crate::error::CacheError;
use crate::middleware::cache::models::CachedResponse;
use crate::middleware::cache::CacheStore;

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cache hits
    pub hits: u64,
    /// Total number of cache misses
    pub misses: u64,
    /// Total number of cache evictions
    pub evictions: u64,
    /// Total number of cache insertions
    pub insertions: u64,
    /// Total number of cache deletions
    pub deletions: u64,
    /// Total number of expired items removed
    pub expirations: u64,
    /// Cache hit ratio (hits / (hits + misses))
    pub hit_ratio: f64,
    /// Last time stats were reset
    pub last_reset: Instant,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            insertions: 0,
            deletions: 0,
            expirations: 0,
            hit_ratio: 0.0,
            last_reset: Instant::now(),
        }
    }
}

impl CacheStats {
    /// Create new cache statistics
    pub fn new() -> Self {
        Self {
            last_reset: Instant::now(),
            ..Default::default()
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = CacheStats::new();
    }

    /// Update hit ratio
    pub fn update_hit_ratio(&mut self) {
        let total = self.hits + self.misses;
        if total > 0 {
            self.hit_ratio = self.hits as f64 / total as f64;
        } else {
            self.hit_ratio = 0.0;
        }
    }

    /// Record a cache hit
    pub fn record_hit(&mut self) {
        self.hits += 1;
        self.update_hit_ratio();
    }

    /// Record a cache miss
    pub fn record_miss(&mut self) {
        self.misses += 1;
        self.update_hit_ratio();
    }

    /// Record a cache eviction
    pub fn record_eviction(&mut self) {
        self.evictions += 1;
    }

    /// Record a cache insertion
    pub fn record_insertion(&mut self) {
        self.insertions += 1;
    }

    /// Record a cache deletion
    pub fn record_deletion(&mut self) {
        self.deletions += 1;
    }

    /// Record an expired item removal
    pub fn record_expiration(&mut self) {
        self.expirations += 1;
    }
}

/// LRU cache entry with access tracking
struct LruCacheEntry {
    /// The cached response
    response: CachedResponse,
    /// Last access time for LRU tracking
    last_accessed: Instant,
}

/// Enhanced in-memory cache store implementation with LRU eviction
pub struct MemoryStore {
    /// The cache data
    cache: Arc<RwLock<HashMap<String, LruCacheEntry>>>,
    /// LRU tracking queue
    lru_queue: Arc<RwLock<VecDeque<String>>>,
    /// Maximum number of entries in the cache
    max_entries: usize,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Automatic cleanup interval in seconds
    cleanup_interval: u64,
    /// Last cleanup time
    last_cleanup: Arc<RwLock<Instant>>,
}

impl MemoryStore {
    /// Create a new MemoryStore with default settings
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_queue: Arc::new(RwLock::new(VecDeque::new())),
            max_entries: 1000,
            stats: Arc::new(RwLock::new(CacheStats::new())),
            cleanup_interval: 60, // Clean expired entries every 60 seconds
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }
    
    /// Create a new MemoryStore with specified maximum entries
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_queue: Arc::new(RwLock::new(VecDeque::new())),
            max_entries,
            stats: Arc::new(RwLock::new(CacheStats::new())),
            cleanup_interval: 60,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Set the cleanup interval
    pub fn with_cleanup_interval(mut self, seconds: u64) -> Self {
        self.cleanup_interval = seconds;
        self
    }
    
    /// Clean expired entries from the cache
    pub fn clean_expired(&self) -> Result<usize, CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for cache".to_string())
        })?;
        
        let mut lru_queue = self.lru_queue.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for LRU queue".to_string())
        })?;
        
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut expired_keys = Vec::new();
        
        // Find expired entries
        for (key, entry) in cache.iter() {
            if now > entry.response.created_at + entry.response.ttl {
                expired_keys.push(key.clone());
            }
        }
        
        let count = expired_keys.len();
        
        // Remove expired entries
        for key in &expired_keys {
            cache.remove(key);
            
            // Also remove from LRU queue
            if let Some(pos) = lru_queue.iter().position(|k| k == key) {
                lru_queue.remove(pos);
            }
        }
        
        // Update stats
        if let Ok(mut stats) = self.stats.write() {
            stats.expirations += count as u64;
        }
        
        Ok(count)
    }
    
    /// Update the LRU queue for a key
    fn update_lru(&self, key: &str) -> Result<(), CacheError> {
        let mut lru_queue = self.lru_queue.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for LRU queue".to_string())
        })?;
        
        // Remove the key from its current position
        if let Some(pos) = lru_queue.iter().position(|k| k == key) {
            lru_queue.remove(pos);
        }
        
        // Add the key to the front (most recently used)
        lru_queue.push_front(key.to_string());
        
        Ok(())
    }
    
    /// Evict least recently used entries if needed
    fn evict_if_needed(&self, new_key: &str) -> Result<(), CacheError> {
        let cache_len = {
            let cache = self.cache.read().map_err(|_| {
                CacheError::StoreError("Failed to acquire read lock".to_string())
            })?;
            cache.len()
        };
        
        // Check if we need to evict
        let key_exists = {
            let cache = self.cache.read().map_err(|_| {
                CacheError::StoreError("Failed to acquire read lock".to_string())
            })?;
            cache.contains_key(new_key)
        };
        
        if cache_len >= self.max_entries && !key_exists {
            let mut lru_queue = self.lru_queue.write().map_err(|_| {
                CacheError::StoreError("Failed to acquire write lock for LRU queue".to_string())
            })?;
            
            // Get the least recently used key
            if let Some(lru_key) = lru_queue.pop_back() {
                // Remove from cache
                let mut cache = self.cache.write().map_err(|_| {
                    CacheError::StoreError("Failed to acquire write lock for cache".to_string())
                })?;
                
                cache.remove(&lru_key);
                
                // Update stats
                if let Ok(mut stats) = self.stats.write() {
                    stats.record_eviction();
                }
            }
        }
        
        Ok(())
    }
    
    /// Get cache statistics
    pub fn get_stats(&self) -> Result<CacheStats, CacheError> {
        let stats = self.stats.read().map_err(|_| {
            CacheError::RetrieveError("Failed to acquire read lock for stats".to_string())
        })?;
        
        Ok(stats.clone())
    }
    
    /// Reset cache statistics
    pub fn reset_stats(&self) -> Result<(), CacheError> {
        let mut stats = self.stats.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for stats".to_string())
        })?;
        
        stats.reset();
        Ok(())
    }
    
    /// Get the current cache size
    pub fn size(&self) -> Result<usize, CacheError> {
        let cache = self.cache.read().map_err(|_| {
            CacheError::RetrieveError("Failed to acquire read lock".to_string())
        })?;
        
        Ok(cache.len())
    }
    
    /// Check if automatic cleanup is needed
    fn check_auto_cleanup(&self) -> Result<(), CacheError> {
        let mut last_cleanup = self.last_cleanup.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for last_cleanup".to_string())
        })?;
        
        let now = Instant::now();
        let elapsed = now.duration_since(*last_cleanup).as_secs();
        
        if elapsed >= self.cleanup_interval {
            *last_cleanup = now;
            drop(last_cleanup); // Release the lock before cleaning
            self.clean_expired()?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl CacheStore for MemoryStore {
    async fn get(&self, key: &str) -> Result<Option<CachedResponse>, CacheError> {
        // Check if cleanup is needed
        let _ = self.check_auto_cleanup();
        
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::RetrieveError("Failed to acquire write lock".to_string())
        })?;
        
        let result = if let Some(entry) = cache.get_mut(key) {
            // Update last accessed time
            entry.last_accessed = Instant::now();
            
            // Check if expired
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
                
            if now > entry.response.created_at + entry.response.ttl {
                // Update stats
                if let Ok(mut stats) = self.stats.write() {
                    stats.record_miss();
                    stats.record_expiration();
                }
                
                None
            } else {
                // Update stats
                if let Ok(mut stats) = self.stats.write() {
                    stats.record_hit();
                }
                
                Some(entry.response.clone())
            }
        } else {
            // Update stats
            if let Ok(mut stats) = self.stats.write() {
                stats.record_miss();
            }
            
            None
        };
        
        // Update LRU if item was found and not expired
        if result.is_some() {
            drop(cache); // Release the lock before updating LRU
            self.update_lru(key)?;
        }
        
        Ok(result)
    }
    
    async fn set(
        &self,
        key: &str,
        response: CachedResponse,
        _ttl: Duration,
    ) -> Result<(), CacheError> {
        // Evict if needed before acquiring write lock
        self.evict_if_needed(key)?;
        
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        // Create new entry
        let entry = LruCacheEntry {
            response,
            last_accessed: Instant::now(),
        };
        
        // Check if this is an update or new insertion
        let is_new = !cache.contains_key(key);
        
        // Insert into cache
        cache.insert(key.to_string(), entry);
        
        // Update stats
        if let Ok(mut stats) = self.stats.write() {
            if is_new {
                stats.record_insertion();
            }
        }
        
        // Update LRU
        drop(cache); // Release the lock before updating LRU
        self.update_lru(key)?;
        
        Ok(())
    }
    
    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        let existed = cache.remove(key).is_some();
        
        // Update stats if key existed
        if existed {
            if let Ok(mut stats) = self.stats.write() {
                stats.record_deletion();
            }
        }
        
        // Remove from LRU queue
        let mut lru_queue = self.lru_queue.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for LRU queue".to_string())
        })?;
        
        if let Some(pos) = lru_queue.iter().position(|k| k == key) {
            lru_queue.remove(pos);
        }
        
        Ok(())
    }
    
    async fn clear(&self) -> Result<(), CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        let count = cache.len();
        cache.clear();
        
        // Clear LRU queue
        let mut lru_queue = self.lru_queue.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock for LRU queue".to_string())
        })?;
        
        lru_queue.clear();
        
        // Update stats
        if let Ok(mut stats) = self.stats.write() {
            stats.deletions += count as u64;
        }
        
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let cache = self.cache.read().map_err(|_| {
            CacheError::RetrieveError("Failed to acquire read lock".to_string())
        })?;
        
        Ok(cache.contains_key(key))
    }
    
    async fn ttl(&self, key: &str) -> Result<Option<Duration>, CacheError> {
        let cache = self.cache.read().map_err(|_| {
            CacheError::RetrieveError("Failed to acquire read lock".to_string())
        })?;
        
        if let Some(entry) = cache.get(key) {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            if now > entry.response.created_at + entry.response.ttl {
                Ok(Some(Duration::from_secs(0)))
            } else {
                Ok(Some(Duration::from_secs(entry.response.created_at + entry.response.ttl - now)))
            }
        } else {
            Ok(None)
        }
    }
    
    async fn set_ttl(&self, key: &str, ttl: Duration) -> Result<(), CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        if let Some(entry) = cache.get_mut(key) {
            entry.response.ttl = ttl.as_secs();
            Ok(())
        } else {
            Err(CacheError::RetrieveError(format!("Key '{}' not found", key)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    
    #[tokio::test]
    async fn test_lru_eviction() {
        // Create a small cache
        let store = MemoryStore::with_max_entries(3);
        
        // Create test responses
        let create_response = |key: &str| {
            CachedResponse {
                status: 200,
                headers: vec![],
                body: vec![],
                created_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                ttl: 60,
                cache_key: key.to_string(),
                content_type: None,
            }
        };
        
        // Add 3 items
        store.set("key1", create_response("key1"), Duration::from_secs(60)).await.unwrap();
        store.set("key2", create_response("key2"), Duration::from_secs(60)).await.unwrap();
        store.set("key3", create_response("key3"), Duration::from_secs(60)).await.unwrap();
        
        // Access key1 to make it most recently used
        let _ = store.get("key1").await.unwrap();
        
        // Add a 4th item, should evict key2 (least recently used)
        store.set("key4", create_response("key4"), Duration::from_secs(60)).await.unwrap();
        
        // Check that key2 was evicted
        assert!(store.get("key1").await.unwrap().is_some());
        assert!(store.get("key2").await.unwrap().is_none());
        assert!(store.get("key3").await.unwrap().is_some());
        assert!(store.get("key4").await.unwrap().is_some());
    }
    
    #[tokio::test]
    async fn test_expiration() {
        // Create cache
        let store = MemoryStore::new();
        
        // Create a response with short TTL
        let response = CachedResponse {
            status: 200,
            headers: vec![],
            body: vec![],
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: 1, // 1 second TTL
            cache_key: "test_key".to_string(),
            content_type: None,
        };
        
        // Store it
        store.set("test_key", response, Duration::from_secs(1)).await.unwrap();
        
        // Verify it exists
        assert!(store.get("test_key").await.unwrap().is_some());
        
        // Wait for expiration
        sleep(Duration::from_secs(2));
        
        // Should be expired now
        assert!(store.get("test_key").await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_stats() {
        // Create cache
        let store = MemoryStore::new();
        
        // Create test response
        let response = CachedResponse {
            status: 200,
            headers: vec![],
            body: vec![],
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: 60,
            cache_key: "test_key".to_string(),
            content_type: None,
        };
        
        // Initial stats
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.insertions, 0);
        
        // Miss
        let _ = store.get("test_key").await.unwrap();
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.misses, 1);
        
        // Insert
        store.set("test_key", response.clone(), Duration::from_secs(60)).await.unwrap();
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.insertions, 1);
        
        // Hit
        let _ = store.get("test_key").await.unwrap();
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.hit_ratio, 0.5); // 1 hit, 1 miss
        
        // Delete
        store.delete("test_key").await.unwrap();
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.deletions, 1);
        
        // Reset stats
        store.reset_stats().unwrap();
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }
    
    #[tokio::test]
    async fn test_auto_cleanup() {
        // Create cache with short cleanup interval
        let store = MemoryStore::new().with_cleanup_interval(1);
        
        // Create responses with short TTL
        let create_response = |key: &str| {
            CachedResponse {
                status: 200,
                headers: vec![],
                body: vec![],
                created_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                ttl: 1, // 1 second TTL
                cache_key: key.to_string(),
                content_type: None,
            }
        };
        
        // Add items
        store.set("key1", create_response("key1"), Duration::from_secs(1)).await.unwrap();
        store.set("key2", create_response("key2"), Duration::from_secs(1)).await.unwrap();
        
        // Wait for expiration
        sleep(Duration::from_secs(2));
        
        // Access any key to trigger auto cleanup
        let _ = store.get("key3").await.unwrap();
        
        // Check that expired items were removed
        let size = store.size().unwrap();
        assert_eq!(size, 0);
        
        // Check stats
        let stats = store.get_stats().unwrap();
        assert!(stats.expirations >= 2);
    }
}