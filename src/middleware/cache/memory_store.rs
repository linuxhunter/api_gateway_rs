use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;

use crate::error::CacheError;
use crate::middleware::cache::models::CachedResponse;
use crate::middleware::cache::CacheStore;

/// Simple in-memory cache store implementation
pub struct MemoryStore {
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
    max_entries: usize,
}

impl MemoryStore {
    /// Create a new MemoryStore with default settings
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries: 1000,
        }
    }
    
    /// Create a new MemoryStore with specified maximum entries
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
        }
    }
    
    /// Clean expired entries from the cache
    pub fn clean_expired(&self) -> Result<usize, CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, v)| now > v.created_at + v.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        
        let count = expired_keys.len();
        
        for key in expired_keys {
            cache.remove(&key);
        }
        
        Ok(count)
    }
}

#[async_trait]
impl CacheStore for MemoryStore {
    async fn get(&self, key: &str) -> Result<Option<CachedResponse>, CacheError> {
        let cache = self.cache.read().map_err(|_| {
            CacheError::RetrieveError("Failed to acquire read lock".to_string())
        })?;
        
        Ok(cache.get(key).cloned())
    }
    
    async fn set(
        &self,
        key: &str,
        response: CachedResponse,
        _ttl: Duration,
    ) -> Result<(), CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        // Check if we need to clean up
        if cache.len() >= self.max_entries && !cache.contains_key(key) {
            // Simple eviction strategy: remove oldest entry
            if let Some((oldest_key, _)) = cache
                .iter()
                .min_by_key(|(_, v)| v.created_at)
                .map(|(k, _)| (k.clone(), ()))
            {
                cache.remove(&oldest_key);
            }
        }
        
        cache.insert(key.to_string(), response);
        Ok(())
    }
    
    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        cache.remove(key);
        Ok(())
    }
    
    async fn clear(&self) -> Result<(), CacheError> {
        let mut cache = self.cache.write().map_err(|_| {
            CacheError::StoreError("Failed to acquire write lock".to_string())
        })?;
        
        cache.clear();
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
            
            if now > entry.created_at + entry.ttl {
                Ok(Some(Duration::from_secs(0)))
            } else {
                Ok(Some(Duration::from_secs(entry.created_at + entry.ttl - now)))
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
            entry.ttl = ttl.as_secs();
            Ok(())
        } else {
            Err(CacheError::RetrieveError(format!("Key '{}' not found", key)))
        }
    }
}