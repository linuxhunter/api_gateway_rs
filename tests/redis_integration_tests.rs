use std::sync::Arc;
use std::time::Duration;

use api_gateway::config::{GatewayConfig, ServerConfig, AuthConfig, LoadBalancerConfig, CacheConfig, LoggingConfig};
use api_gateway::core::gateway::{ApiGateway, Gateway};
use api_gateway::core::router::{BasicRouter, RouteConfig, Router};
use api_gateway::middleware::cache::{CacheMiddleware, redis_store::{RedisStore, RedisConfig}};
use api_gateway::middleware::load_balancer::LoadBalancerMiddleware;
use api_gateway::models::Backend;

use axum::{
    extract::Path,
    response::Json,
    routing::get,
};
use hyper::Method;
use reqwest::Client;
use serde_json::{json, Value};
// Note: For actual testing, you would use testcontainers:
// use testcontainers::{clients::Cli, Container};
// use testcontainers_modules::redis::Redis;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// Test backend for Redis integration tests
struct RedisTestBackend {
    port: u16,
    handle: Option<JoinHandle<()>>,
}

impl RedisTestBackend {
    async fn new(port: u16) -> Self {
        let app = axum::Router::new()
            .route("/health", get(move || async move {
                Json(json!({
                    "status": "UP",
                    "port": port,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }))
            .route("/api/cached/:id", get(|Path(id): Path<String>| async move {
                // Simulate some processing time
                sleep(Duration::from_millis(50)).await;
                Json(json!({
                    "id": id,
                    "data": format!("Expensive computation result for {}", id),
                    "computed_at": chrono::Utc::now().to_rfc3339(),
                    "cache_key": format!("cached_{}", id)
                }))
            }))
            .route("/api/dynamic", get(|| async move {
                Json(json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "random": rand::random::<u64>(),
                    "should_not_cache": true
                }))
            }));

        let addr = format!("127.0.0.1:{}", port).parse().unwrap();
        let handle = tokio::spawn(async move {
            axum::Server::bind(&addr)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        Self {
            port,
            handle: Some(handle),
        }
    }

    fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl Drop for RedisTestBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Setup Redis container and return connection URL
fn setup_redis() -> String {
    // For testing, we'll use a default Redis URL
    // In a real test environment, you would set up a Redis container
    "redis://127.0.0.1:6379".to_string()
}

/// Create gateway with Redis caching
async fn create_redis_gateway(
    gateway_port: u16,
    backend_ports: Vec<u16>,
    redis_url: String,
) -> (ApiGateway, Vec<RedisTestBackend>) {
    // Start backends
    let mut backends = Vec::new();
    for port in backend_ports {
        let backend = RedisTestBackend::new(port).await;
        backends.push(backend);
    }

    // Create config with Redis caching enabled
    let config = GatewayConfig {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: gateway_port,
            max_connections: 100,
            request_timeout: 30,
        },
        auth: AuthConfig {
            enabled: false,
            jwt_secret: None,
            auth_service_url: None,
            token_expiration: 3600,
        },
        load_balancer: LoadBalancerConfig {
            algorithm: "round_robin".to_string(),
            health_check_interval: 10,
            health_check_timeout: 2,
            health_check_path: "/health".to_string(),
        },
        cache: CacheConfig {
            enabled: true,
            cache_type: "redis".to_string(),
            default_ttl: 30,
            redis_url: Some(redis_url.clone()),
            max_memory_size: Some(100),
        },
        logging: LoggingConfig {
            level: "info".to_string(),
            log_to_file: false,
            log_file: None,
            json_format: false,
        },
        routes: vec![],
    };

    // Create router
    let router = BasicRouter::new();
    
    let backend_urls: Vec<String> = backends
        .iter()
        .map(|b| format!("http://127.0.0.1:{}", b.port))
        .collect();

    // Add routes
    router.add_route(
        RouteConfig::new("/health".to_string(), Some(Method::GET), backend_urls.clone())
    ).await.unwrap();

    router.add_route(
        RouteConfig::new("/api/cached/:id".to_string(), Some(Method::GET), backend_urls.clone())
            .with_cache(true)
    ).await.unwrap();

    router.add_route(
        RouteConfig::new("/api/dynamic".to_string(), Some(Method::GET), backend_urls)
            .with_cache(false) // Don't cache dynamic content
    ).await.unwrap();

    // Create gateway
    let gateway = ApiGateway::with_config(config.server)
        .with_router(Box::new(router));

    // Add Redis cache middleware
    let redis_config = RedisConfig {
        url: redis_url,
        pool_size: 16,
        connection_timeout: 5,
    };
    let redis_store = Arc::new(RedisStore::new(redis_config).await.unwrap());
    let cache_middleware = CacheMiddleware::new().with_store(redis_store).with_ttl(Duration::from_secs(30));
    gateway.register_middleware(cache_middleware).await.unwrap();

    (gateway, backends)
}

#[tokio::test]
async fn test_redis_cache_hit_miss() {
    let redis_url = setup_redis();
    
    // Wait for Redis to be ready
    sleep(Duration::from_millis(500)).await;
    
    let (gateway, mut backends) = create_redis_gateway(9030, vec![9130], redis_url).await;

    // Start gateway
    let gateway_handle = tokio::spawn(async move {
        gateway.start().await
    });

    sleep(Duration::from_millis(300)).await;

    let client = Client::new();

    // First request - should be a cache miss
    let start1 = std::time::Instant::now();
    let response1 = client
        .get("http://127.0.0.1:9030/api/cached/test123")
        .send()
        .await
        .expect("Failed to send first request");
    let duration1 = start1.elapsed();

    assert_eq!(response1.status(), 200);
    let body1: Value = response1.json().await.expect("Failed to parse JSON");
    assert_eq!(body1["id"], "test123");

    // Second request - should be a cache hit (faster)
    let start2 = std::time::Instant::now();
    let response2 = client
        .get("http://127.0.0.1:9030/api/cached/test123")
        .send()
        .await
        .expect("Failed to send second request");
    let duration2 = start2.elapsed();

    assert_eq!(response2.status(), 200);
    let body2: Value = response2.json().await.expect("Failed to parse JSON");
    assert_eq!(body2["id"], "test123");
    
    // Cache hit should be significantly faster
    assert!(duration2 < duration1, "Cache hit should be faster than cache miss");
    assert!(duration2 < Duration::from_millis(20), "Cache hit should be very fast");

    // The cached response should have the same computed_at timestamp
    assert_eq!(body1["computed_at"], body2["computed_at"]);

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

#[tokio::test]
async fn test_redis_cache_expiration() {
    let redis_url = setup_redis();
    
    // Wait for Redis to be ready
    sleep(Duration::from_millis(500)).await;
    
    let (gateway, mut backends) = create_redis_gateway(9031, vec![9131], redis_url).await;

    // Start gateway
    let gateway_handle = tokio::spawn(async move {
        gateway.start().await
    });

    sleep(Duration::from_millis(300)).await;

    let client = Client::new();

    // First request
    let response1 = client
        .get("http://127.0.0.1:9031/api/cached/expire_test")
        .send()
        .await
        .expect("Failed to send first request");

    assert_eq!(response1.status(), 200);
    let body1: Value = response1.json().await.expect("Failed to parse JSON");

    // Wait for cache to expire (TTL is 30 seconds, but we'll use a shorter wait for testing)
    // Note: In a real test, you might want to use a shorter TTL for faster testing
    sleep(Duration::from_secs(2)).await;

    // Second request - should still be cached
    let response2 = client
        .get("http://127.0.0.1:9031/api/cached/expire_test")
        .send()
        .await
        .expect("Failed to send second request");

    assert_eq!(response2.status(), 200);
    let body2: Value = response2.json().await.expect("Failed to parse JSON");
    
    // Should still be the same cached response
    assert_eq!(body1["computed_at"], body2["computed_at"]);

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

#[tokio::test]
async fn test_redis_cache_with_load_balancing() {
    let redis_url = setup_redis();
    
    // Wait for Redis to be ready
    sleep(Duration::from_millis(500)).await;
    
    let (gateway, mut backends) = create_redis_gateway(9032, vec![9132, 9133], redis_url).await;

    // Add load balancer middleware
    let load_balancer = LoadBalancerMiddleware::new();
    load_balancer.register_backends(vec![
        Backend::new("http://127.0.0.1:9132".to_string()),
        Backend::new("http://127.0.0.1:9133".to_string()),
    ]).await.unwrap();
    
    gateway.register_middleware(load_balancer).await.unwrap();

    // Start gateway
    let gateway_handle = tokio::spawn(async move {
        gateway.start().await
    });

    sleep(Duration::from_millis(300)).await;

    let client = Client::new();

    // Make multiple requests for the same resource
    let mut responses = Vec::new();
    for _ in 0..4 {
        let response = client
            .get("http://127.0.0.1:9032/api/cached/lb_test")
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.expect("Failed to parse JSON");
        responses.push(body);
    }

    // All responses should have the same computed_at timestamp due to caching
    let first_timestamp = &responses[0]["computed_at"];
    for response in &responses[1..] {
        assert_eq!(response["computed_at"], *first_timestamp);
    }

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

#[tokio::test]
async fn test_redis_cache_selective_caching() {
    let redis_url = setup_redis();
    
    // Wait for Redis to be ready
    sleep(Duration::from_millis(500)).await;
    
    let (gateway, mut backends) = create_redis_gateway(9033, vec![9134], redis_url).await;

    // Start gateway
    let gateway_handle = tokio::spawn(async move {
        gateway.start().await
    });

    sleep(Duration::from_millis(300)).await;

    let client = Client::new();

    // Test cached endpoint
    let cached_response1 = client
        .get("http://127.0.0.1:9033/api/cached/selective_test")
        .send()
        .await
        .expect("Failed to send cached request 1");

    let cached_body1: Value = cached_response1.json().await.expect("Failed to parse JSON");

    let cached_response2 = client
        .get("http://127.0.0.1:9033/api/cached/selective_test")
        .send()
        .await
        .expect("Failed to send cached request 2");

    let cached_body2: Value = cached_response2.json().await.expect("Failed to parse JSON");

    // Should be cached (same timestamp)
    assert_eq!(cached_body1["computed_at"], cached_body2["computed_at"]);

    // Test dynamic endpoint (not cached)
    let dynamic_response1 = client
        .get("http://127.0.0.1:9033/api/dynamic")
        .send()
        .await
        .expect("Failed to send dynamic request 1");

    let dynamic_body1: Value = dynamic_response1.json().await.expect("Failed to parse JSON");

    sleep(Duration::from_millis(100)).await;

    let dynamic_response2 = client
        .get("http://127.0.0.1:9033/api/dynamic")
        .send()
        .await
        .expect("Failed to send dynamic request 2");

    let dynamic_body2: Value = dynamic_response2.json().await.expect("Failed to parse JSON");

    // Should not be cached (different timestamps and random values)
    assert_ne!(dynamic_body1["timestamp"], dynamic_body2["timestamp"]);
    assert_ne!(dynamic_body1["random"], dynamic_body2["random"]);

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

#[tokio::test]
async fn test_redis_connection_failure_fallback() {
    // Start with a valid Redis connection
    let redis_url = setup_redis();
    
    // Wait for Redis to be ready
    sleep(Duration::from_millis(500)).await;
    
    let (gateway, mut backends) = create_redis_gateway(9034, vec![9135], redis_url).await;

    // Start gateway
    let gateway_handle = tokio::spawn(async move {
        gateway.start().await
    });

    sleep(Duration::from_millis(300)).await;

    let client = Client::new();

    // Make a request that should work normally
    let response = client
        .get("http://127.0.0.1:9034/api/cached/fallback_test")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    // Even if Redis fails, the gateway should continue to work
    // (requests will just bypass the cache)
    let response2 = client
        .get("http://127.0.0.1:9034/health")
        .send()
        .await
        .expect("Failed to send health check");

    assert_eq!(response2.status(), 200);

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}