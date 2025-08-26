use std::sync::Arc;
use std::time::Duration;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::middleware::cache::memory_store::MemoryStore;
use crate::middleware::cache::models::{CacheKeyOptions, CachePolicy};
use crate::middleware::cache::redis_store::{RedisConfig, RedisStore};
use crate::middleware::cache::CacheMiddleware;
use crate::core::middleware::{Middleware, MiddlewareHandler};

/// Example middleware handler for testing
struct ExampleHandler;

#[async_trait::async_trait]
impl MiddlewareHandler for ExampleHandler {
    async fn handle(&self, _request: GatewayRequest) -> Result<GatewayResponse, crate::error::GatewayError> {
        // Create a simple response
        Ok(GatewayResponse::new(
            hyper::StatusCode::OK,
            hyper::HeaderMap::new(),
            bytes::Bytes::from("Hello from example handler"),
        ))
    }
}

/// Example of using memory cache
pub async fn memory_cache_example() {
    // Create a memory store
    let store = MemoryStore::new();
    
    // Create cache policy
    let policy = CachePolicy {
        default_ttl: Duration::from_secs(60),
        max_ttl: Duration::from_secs(3600),
        cacheable_methods: vec![hyper::Method::GET],
        cacheable_status_codes: vec![hyper::StatusCode::OK],
        respect_cache_control: true,
        key_options: CacheKeyOptions {
            include_query: true,
            include_headers: vec!["accept".to_string()],
            include_method: true,
            prefix: Some("example".to_string()),
            normalize_path: true,
        },
        cache_authenticated: false,
    };
    
    // Create cache middleware
    let cache_middleware = CacheMiddleware::new()
        .with_store(Arc::new(store))
        .with_policy(policy);
    
    // Create example request
    let request = GatewayRequest::new(
        hyper::Method::GET,
        "https://example.com/api/resource?param=value".parse().unwrap(),
        hyper::HeaderMap::new(),
        bytes::Bytes::new(),
        None,
    );
    
    // Create example handler
    let handler = Arc::new(ExampleHandler);
    
    // Process request through cache middleware
    let response = cache_middleware.process_request(request, handler).await;
    
    // Print response
    match response {
        Ok(resp) => {
            println!("Status: {}", resp.status);
            if let Some(cache_info) = &resp.cache_info {
                println!("Cache hit: {}", cache_info.cache_hit);
                println!("Cache key: {}", cache_info.cache_key);
                if let Some(ttl) = cache_info.ttl_seconds {
                    println!("TTL: {} seconds", ttl);
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

/// Example of using Redis cache
pub async fn redis_cache_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create Redis configuration
    let redis_config = RedisConfig {
        url: "redis://127.0.0.1:6379".to_string(),
        pool_size: 5,
        connection_timeout: 5,
    };
    
    // Create Redis store
    let store = RedisStore::new(redis_config).await?;
    
    // Create cache policy
    let policy = CachePolicy {
        default_ttl: Duration::from_secs(300), // 5 minutes
        max_ttl: Duration::from_secs(3600),    // 1 hour
        cacheable_methods: vec![hyper::Method::GET],
        cacheable_status_codes: vec![hyper::StatusCode::OK],
        respect_cache_control: true,
        key_options: CacheKeyOptions {
            include_query: true,
            include_headers: vec!["accept".to_string()],
            include_method: true,
            prefix: Some("redis_example".to_string()),
            normalize_path: true,
        },
        cache_authenticated: false,
    };
    
    // Create cache middleware with Redis store
    let cache_middleware = CacheMiddleware::new()
        .with_store(Arc::new(store))
        .with_policy(policy);
    
    // Create example request
    let request = GatewayRequest::new(
        hyper::Method::GET,
        "https://example.com/api/resource?param=value".parse().unwrap(),
        hyper::HeaderMap::new(),
        bytes::Bytes::new(),
        None,
    );
    
    // Create example handler
    let handler = Arc::new(ExampleHandler);
    
    // Process request through cache middleware
    let response = cache_middleware.process_request(request, handler).await;
    
    // Print response
    match response {
        Ok(resp) => {
            println!("Status: {}", resp.status);
            if let Some(cache_info) = &resp.cache_info {
                println!("Cache hit: {}", cache_info.cache_hit);
                println!("Cache key: {}", cache_info.cache_key);
                if let Some(ttl) = cache_info.ttl_seconds {
                    println!("TTL: {} seconds", ttl);
                }
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
    
    Ok(())
}

/// Example of configuring Redis cache with connection pool
pub async fn redis_pool_example() -> Result<(), Box<dyn std::error::Error>> {
    // Create Redis configuration with larger pool for high traffic
    let redis_config = RedisConfig {
        url: "redis://127.0.0.1:6379".to_string(),
        pool_size: 20, // Larger pool for high traffic
        connection_timeout: 2, // Shorter timeout
    };
    
    // Create Redis store with custom prefix
    let store = RedisStore::new(redis_config).await?
        .with_prefix("api_gateway:production:");
    
    // Create cache policy optimized for API responses
    let policy = CachePolicy {
        default_ttl: Duration::from_secs(60),
        max_ttl: Duration::from_secs(3600),
        cacheable_methods: vec![hyper::Method::GET, hyper::Method::HEAD],
        cacheable_status_codes: vec![
            hyper::StatusCode::OK,
            hyper::StatusCode::NOT_MODIFIED,
            hyper::StatusCode::PARTIAL_CONTENT,
        ],
        respect_cache_control: true,
        key_options: CacheKeyOptions {
            include_query: true,
            include_headers: vec![
                "accept".to_string(),
                "accept-language".to_string(),
                "accept-encoding".to_string(),
            ],
            include_method: true,
            prefix: Some("api".to_string()),
            normalize_path: true,
        },
        cache_authenticated: false,
    };
    
    // Create cache middleware with Redis store
    let _cache_middleware = CacheMiddleware::new()
        .with_store(Arc::new(store))
        .with_policy(policy);
    
    println!("Redis cache middleware configured with connection pool");
    
    Ok(())
}