use std::sync::Arc;
use std::time::Duration;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::middleware::cache::memory_store::MemoryStore;
use crate::middleware::cache::models::{CachePolicy, CachedResponse};
use crate::middleware::cache::{CacheMiddleware, CacheStore};
use crate::middleware::{Middleware, MiddlewareHandler};

/// Example middleware handler for testing
struct ExampleHandler;

#[async_trait::async_trait]
impl MiddlewareHandler for ExampleHandler {
    async fn handle(&self, _request: GatewayRequest) -> Result<GatewayResponse, crate::error::GatewayError> {
        // Create a simple response
        let response = GatewayResponse::new(
            hyper::StatusCode::OK,
            hyper::HeaderMap::new(),
            bytes::Bytes::from("Hello, World!"),
        );
        
        Ok(response)
    }
}

/// Example of using the memory cache with LRU eviction
pub async fn memory_cache_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Memory Cache Example");
    
    // Create a memory store with LRU eviction
    let store = Arc::new(MemoryStore::with_max_entries(100));
    
    // Create a cache policy
    let policy = CachePolicy {
        default_ttl: Duration::from_secs(60),
        ..Default::default()
    };
    
    // Create a cache middleware
    let cache_middleware = CacheMiddleware::new()
        .with_store(store.clone())
        .with_policy(policy);
    
    // Create a request
    let request = GatewayRequest::new(
        hyper::Method::GET,
        "https://example.com/api/resource".parse().unwrap(),
        hyper::HeaderMap::new(),
        bytes::Bytes::new(),
        None,
    );
    
    // Create a handler
    let handler = Arc::new(ExampleHandler);
    
    // Process the request through the cache middleware
    let response = cache_middleware.process_request(request.clone(), handler.clone()).await?;
    println!("First request: {:?}", response.status);
    
    // Process the same request again (should be cached)
    let cached_response = cache_middleware.process_request(request.clone(), handler.clone()).await?;
    println!("Second request (cached): {:?}", cached_response.status);
    
    // Get cache statistics
    let stats = store.get_stats()?;
    println!("Cache Statistics:");
    println!("  Hits: {}", stats.hits);
    println!("  Misses: {}", stats.misses);
    println!("  Hit Ratio: {:.2}", stats.hit_ratio);
    println!("  Insertions: {}", stats.insertions);
    
    // Demonstrate LRU eviction
    println!("\nDemonstrating LRU eviction:");
    
    // Create a small cache for demonstration
    let small_store = Arc::new(MemoryStore::with_max_entries(3));
    
    // Add some items
    for i in 1..=3 {
        let key = format!("key{}", i);
        let response = CachedResponse {
            status: 200,
            headers: vec![],
            body: vec![],
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl: 60,
            cache_key: key.clone(),
            content_type: None,
        };
        
        small_store.set(&key, response, Duration::from_secs(60)).await?;
        println!("Added item: {}", key);
    }
    
    // Access key1 to make it most recently used
    let _ = small_store.get("key1").await?;
    println!("Accessed key1");
    
    // Add a 4th item, should evict key2 (least recently used)
    let response = CachedResponse {
        status: 200,
        headers: vec![],
        body: vec![],
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        ttl: 60,
        cache_key: "key4".to_string(),
        content_type: None,
    };
    
    small_store.set("key4", response, Duration::from_secs(60)).await?;
    println!("Added key4");
    
    // Check what's in the cache
    println!("Items in cache:");
    println!("  key1: {}", small_store.exists("key1").await?);
    println!("  key2: {}", small_store.exists("key2").await?);
    println!("  key3: {}", small_store.exists("key3").await?);
    println!("  key4: {}", small_store.exists("key4").await?);
    
    Ok(())
}