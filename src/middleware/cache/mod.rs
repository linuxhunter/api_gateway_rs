use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::{CacheError, GatewayError};
use crate::middleware::{Middleware, MiddlewareHandler};

/// Cache store trait
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Get a cached response
    async fn get(&self, key: &str) -> Result<Option<GatewayResponse>, CacheError>;
    
    /// Store a response in the cache
    async fn set(
        &self,
        key: &str,
        response: GatewayResponse,
        ttl: Duration,
    ) -> Result<(), CacheError>;
    
    /// Delete a cached response
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
    
    /// Clear the entire cache
    async fn clear(&self) -> Result<(), CacheError>;
}

/// Cache middleware
pub struct CacheMiddleware {
    name: String,
    cache_store: Option<Arc<dyn CacheStore>>,
    default_ttl: Duration,
}

impl CacheMiddleware {
    /// Create a new CacheMiddleware
    pub fn new() -> Self {
        Self {
            name: "cache".to_string(),
            cache_store: None,
            default_ttl: Duration::from_secs(60),
        }
    }
    
    /// Set the cache store
    pub fn with_store(mut self, store: Arc<dyn CacheStore>) -> Self {
        self.cache_store = Some(store);
        self
    }
    
    /// Set the default TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }
    
    /// Generate a cache key for a request
    fn generate_cache_key(&self, request: &GatewayRequest) -> String {
        format!("{}:{}", request.method, request.uri)
    }
}

#[async_trait]
impl Middleware for CacheMiddleware {
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError> {
        // Will be implemented in future tasks
        next.handle(request).await
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}