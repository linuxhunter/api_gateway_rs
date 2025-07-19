use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::{CacheInfo, GatewayResponse};
use crate::error::{CacheError, GatewayError};
use crate::middleware::{Middleware, MiddlewareHandler};

pub mod models;
pub mod memory_store;
use models::{CacheKeyOptions, CachePolicy, CachedResponse};

/// Cache store trait
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Get a cached response
    async fn get(&self, key: &str) -> Result<Option<CachedResponse>, CacheError>;

    /// Store a response in the cache
    async fn set(
        &self,
        key: &str,
        response: CachedResponse,
        ttl: Duration,
    ) -> Result<(), CacheError>;

    /// Delete a cached response
    async fn delete(&self, key: &str) -> Result<(), CacheError>;

    /// Clear the entire cache
    async fn clear(&self) -> Result<(), CacheError>;

    /// Check if a key exists in the cache
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;

    /// Get the TTL for a key
    async fn ttl(&self, key: &str) -> Result<Option<Duration>, CacheError>;

    /// Set the TTL for a key
    async fn set_ttl(&self, key: &str, ttl: Duration) -> Result<(), CacheError>;
}

/// Cache middleware
pub struct CacheMiddleware {
    name: String,
    cache_store: Option<Arc<dyn CacheStore>>,
    policy: CachePolicy,
}

impl CacheMiddleware {
    /// Create a new CacheMiddleware
    pub fn new() -> Self {
        Self {
            name: "cache".to_string(),
            cache_store: None,
            policy: CachePolicy::default(),
        }
    }

    /// Set the cache store
    pub fn with_store(mut self, store: Arc<dyn CacheStore>) -> Self {
        self.cache_store = Some(store);
        self
    }

    /// Set the cache policy
    pub fn with_policy(mut self, policy: CachePolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Set the default TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.policy.default_ttl = ttl;
        self
    }

    /// Generate a cache key for a request
    fn generate_cache_key(&self, request: &GatewayRequest) -> String {
        generate_cache_key(request, &self.policy.key_options)
    }
}

/// Generate a cache key for a request based on options
pub fn generate_cache_key(request: &GatewayRequest, options: &CacheKeyOptions) -> String {
    let mut key_parts = Vec::new();

    // Add prefix if specified
    if let Some(prefix) = &options.prefix {
        key_parts.push(prefix.clone());
    }

    // Add method if specified
    if options.include_method {
        key_parts.push(request.method.to_string());
    }

    // Add path (normalized if specified)
    let path = request.uri.path();
    let normalized_path = if options.normalize_path {
        normalize_path(path)
    } else {
        path.to_string()
    };
    key_parts.push(normalized_path);

    // Add query if specified
    if options.include_query {
        if let Some(query) = request.uri.query() {
            key_parts.push(query.to_string());
        }
    }

    // Add specified headers if present
    for header_name in &options.include_headers {
        if let Some(value) = request.header(header_name) {
            key_parts.push(format!("{}:{}", header_name, value));
        }
    }

    // Join all parts with a separator
    let key_string = key_parts.join(":");

    // Hash the key if it's too long
    if key_string.len() > 200 {
        let mut hasher = DefaultHasher::new();
        key_string.hash(&mut hasher);
        format!("hashed:{:x}", hasher.finish())
    } else {
        key_string
    }
}

/// Normalize a URL path by removing trailing slashes and duplicate slashes
fn normalize_path(path: &str) -> String {
    let mut result = String::new();
    let mut last_was_slash = false;

    for c in path.chars() {
        if c == '/' {
            if !last_was_slash || result.is_empty() {
                result.push(c);
            }
            last_was_slash = true;
        } else {
            result.push(c);
            last_was_slash = false;
        }
    }

    // Remove trailing slash if not root
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }

    result
}

#[async_trait]
impl Middleware for CacheMiddleware {
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError> {
        // Check if we have a cache store
        let cache_store = match &self.cache_store {
            Some(store) => store,
            None => return next.handle(request).await,
        };

        // Check if request is cacheable
        if !self.policy.is_request_cacheable(&request) {
            return next.handle(request).await;
        }

        // Generate cache key
        let cache_key = self.generate_cache_key(&request);

        // Try to get from cache
        match cache_store.get(&cache_key).await {
            Ok(Some(cached)) => {
                // Check if expired
                if cached.is_expired() {
                    // Delete expired entry
                    let _ = cache_store.delete(&cache_key).await;
                } else {
                    // Return cached response
                    return Ok(cached.to_gateway_response());
                }
            }
            Ok(None) => {
                // Cache miss, continue
            }
            Err(err) => {
                // Log error but continue
                eprintln!("Cache error: {}", err);
            }
        }

        // Forward request to next middleware
        let response = next.handle(request).await?;

        // Check if response is cacheable
        if !self.policy.is_response_cacheable(&response) {
            return Ok(response);
        }

        // Get TTL for this response
        let ttl = self.policy.get_ttl_for_response(&response);

        // Create cached response
        let cached_response =
            CachedResponse::from_gateway_response(&response, cache_key.clone(), ttl);

        // Store in cache
        if let Err(err) = cache_store.set(&cache_key, cached_response, ttl).await {
            eprintln!("Failed to cache response: {}", err);
        }

        // Add cache info to response
        let mut response_with_cache_info = response;
        let cache_info = CacheInfo {
            cache_hit: false,
            ttl_seconds: Some(ttl.as_secs()),
            cache_key,
        };
        response_with_cache_info.cache_info = Some(cache_info);

        Ok(response_with_cache_info)
    }

    fn name(&self) -> &str {
        &self.name
    }
}
