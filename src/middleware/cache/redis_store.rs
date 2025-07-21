use std::time::Duration;

use async_trait::async_trait;
use deadpool_redis::{Config, Pool, Runtime};
use redis::{AsyncCommands, RedisError};
use serde_json;

use crate::error::CacheError;
use crate::middleware::cache::models::CachedResponse;
use crate::middleware::cache::CacheStore;

/// Redis cache store configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis URL (redis://...)
    pub url: String,
    /// Connection pool size
    pub pool_size: usize,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 16,
            connection_timeout: 5,
        }
    }
}

/// Redis cache store implementation
pub struct RedisStore {
    /// Redis connection pool
    pool: Pool,
    /// Key prefix for all cache entries
    prefix: String,
}

impl RedisStore {
    /// Create a new RedisStore with the given configuration
    pub async fn new(config: RedisConfig) -> Result<Self, CacheError> {
        let cfg = Config::from_url(&config.url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1)).map_err(|e| {
            CacheError::ConnectionError(format!("Failed to create Redis pool: {}", e))
        })?;

        // Test connection
        let mut conn = pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to connect to Redis: {}", e))
        })?;

        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| CacheError::ConnectionError(format!("Redis ping failed: {}", e)))?;

        Ok(Self {
            pool,
            prefix: "api_gateway:cache:".to_string(),
        })
    }

    /// Set the key prefix
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_string();
        self
    }

    /// Get the full key with prefix
    fn get_prefixed_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    /// Handle Redis errors
    fn handle_redis_error(&self, err: RedisError) -> CacheError {
        match err.kind() {
            redis::ErrorKind::IoError => {
                CacheError::ConnectionError(format!("Redis IO error: {}", err))
            }
            redis::ErrorKind::ResponseError => {
                CacheError::RetrieveError(format!("Redis response error: {}", err))
            }
            _ => CacheError::StoreError(format!("Redis error: {}", err)),
        }
    }
}

#[async_trait]
impl CacheStore for RedisStore {
    async fn get(&self, key: &str) -> Result<Option<CachedResponse>, CacheError> {
        let prefixed_key = self.get_prefixed_key(key);

        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        // Check if key exists
        let exists: bool = conn
            .exists(&prefixed_key)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        if !exists {
            return Ok(None);
        }

        // Get the cached data
        let data: String = conn
            .get(&prefixed_key)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        // Deserialize the cached response
        let cached_response: CachedResponse = serde_json::from_str(&data).map_err(|e| {
            CacheError::RetrieveError(format!("Failed to deserialize cached response: {}", e))
        })?;

        // Check if expired
        if cached_response.is_expired() {
            // Delete expired entry
            let _: () = conn
                .del(&prefixed_key)
                .await
                .map_err(|e| self.handle_redis_error(e))?;

            return Ok(None);
        }

        Ok(Some(cached_response))
    }

    async fn set(
        &self,
        key: &str,
        response: CachedResponse,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let prefixed_key = self.get_prefixed_key(key);

        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        // Serialize the cached response
        let data = serde_json::to_string(&response).map_err(|e| {
            CacheError::StoreError(format!("Failed to serialize cached response: {}", e))
        })?;

        // Set with expiration
        let _: () = conn
            .set_ex(&prefixed_key, data, ttl.as_secs() as usize)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let prefixed_key = self.get_prefixed_key(key);

        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        let _: () = conn
            .del(&prefixed_key)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        Ok(())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        // Get all keys with our prefix
        let pattern = format!("{}*", self.prefix);
        let keys: Vec<String> = conn
            .keys(&pattern)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        if !keys.is_empty() {
            let _: () = conn
                .del(&keys)
                .await
                .map_err(|e| self.handle_redis_error(e))?;
        }

        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let prefixed_key = self.get_prefixed_key(key);

        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        let exists: bool = conn
            .exists(&prefixed_key)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        Ok(exists)
    }

    async fn ttl(&self, key: &str) -> Result<Option<Duration>, CacheError> {
        let prefixed_key = self.get_prefixed_key(key);

        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        let ttl: i64 = conn
            .ttl(&prefixed_key)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        match ttl {
            -2 => Ok(None),                         // Key does not exist
            -1 => Ok(Some(Duration::from_secs(0))), // Key exists but has no expiry
            ttl if ttl > 0 => Ok(Some(Duration::from_secs(ttl as u64))),
            _ => Ok(None),
        }
    }

    async fn set_ttl(&self, key: &str, ttl: Duration) -> Result<(), CacheError> {
        let prefixed_key = self.get_prefixed_key(key);

        let mut conn = self.pool.get().await.map_err(|e| {
            CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e))
        })?;

        // Check if key exists
        let exists: bool = conn
            .exists(&prefixed_key)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        if !exists {
            return Err(CacheError::RetrieveError(format!(
                "Key '{}' not found",
                key
            )));
        }

        let _: () = conn
            .expire(&prefixed_key, ttl.as_secs() as usize)
            .await
            .map_err(|e| self.handle_redis_error(e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use tokio;

    // These tests require a running Redis instance
    // They are marked as ignored by default

    #[tokio::test]
    #[ignore]
    async fn test_redis_connection() {
        let config = RedisConfig::default();
        let store = RedisStore::new(config).await;
        assert!(store.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_set_get() {
        let config = RedisConfig::default();
        let store = RedisStore::new(config).await.unwrap();

        // Create test response
        let response = CachedResponse {
            status: 200,
            headers: vec![],
            body: vec![1, 2, 3, 4],
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: 60,
            cache_key: "test_key".to_string(),
            content_type: Some("application/json".to_string()),
        };

        // Set in cache
        let result = store
            .set("test_key", response.clone(), Duration::from_secs(60))
            .await;
        assert!(result.is_ok());

        // Get from cache
        let result = store.get("test_key").await;
        assert!(result.is_ok());

        let cached = result.unwrap();
        assert!(cached.is_some());

        let cached = cached.unwrap();
        assert_eq!(cached.status, 200);
        assert_eq!(cached.body, vec![1, 2, 3, 4]);

        // Clean up
        let _ = store.delete("test_key").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_ttl() {
        let config = RedisConfig::default();
        let store = RedisStore::new(config).await.unwrap();

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
            cache_key: "ttl_test_key".to_string(),
            content_type: None,
        };

        // Set in cache with 10s TTL
        let result = store
            .set("ttl_test_key", response, Duration::from_secs(10))
            .await;
        assert!(result.is_ok());

        // Check TTL
        let result = store.ttl("ttl_test_key").await;
        assert!(result.is_ok());

        let ttl = result.unwrap();
        assert!(ttl.is_some());

        let ttl = ttl.unwrap();
        assert!(ttl.as_secs() <= 10);
        assert!(ttl.as_secs() > 0);

        // Update TTL
        let result = store.set_ttl("ttl_test_key", Duration::from_secs(30)).await;
        assert!(result.is_ok());

        // Check updated TTL
        let result = store.ttl("ttl_test_key").await;
        assert!(result.is_ok());

        let ttl = result.unwrap();
        assert!(ttl.is_some());

        let ttl = ttl.unwrap();
        assert!(ttl.as_secs() <= 30);
        assert!(ttl.as_secs() > 10);

        // Clean up
        let _ = store.delete("ttl_test_key").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_exists_delete() {
        let config = RedisConfig::default();
        let store = RedisStore::new(config).await.unwrap();

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
            cache_key: "exists_test_key".to_string(),
            content_type: None,
        };

        // Check before set
        let result = store.exists("exists_test_key").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Set in cache
        let result = store
            .set("exists_test_key", response, Duration::from_secs(60))
            .await;
        assert!(result.is_ok());

        // Check after set
        let result = store.exists("exists_test_key").await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Delete
        let result = store.delete("exists_test_key").await;
        assert!(result.is_ok());

        // Check after delete
        let result = store.exists("exists_test_key").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_clear() {
        let config = RedisConfig::default();
        let store = RedisStore::new(config).await.unwrap();

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
            cache_key: "clear_test_key1".to_string(),
            content_type: None,
        };

        // Set multiple keys
        let _ = store
            .set("clear_test_key1", response.clone(), Duration::from_secs(60))
            .await;
        let _ = store
            .set("clear_test_key2", response, Duration::from_secs(60))
            .await;

        // Verify they exist
        assert!(store.exists("clear_test_key1").await.unwrap());
        assert!(store.exists("clear_test_key2").await.unwrap());

        // Clear all
        let result = store.clear().await;
        assert!(result.is_ok());

        // Verify they're gone
        assert!(!store.exists("clear_test_key1").await.unwrap());
        assert!(!store.exists("clear_test_key2").await.unwrap());
    }
}
