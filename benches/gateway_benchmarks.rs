use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::time::Duration;

use api_gateway::config::{
    AuthConfig, CacheConfig, GatewayConfig, LoadBalancerConfig, LoggingConfig, ServerConfig,
};
use api_gateway::core::gateway::{ApiGateway, Gateway};
use api_gateway::core::router::{BasicRouter, RouteConfig, Router};
use api_gateway::middleware::cache::{memory_store::MemoryStore, CacheMiddleware, CacheStore};
use api_gateway::middleware::load_balancer::LoadBalancerMiddleware;
use api_gateway::models::Backend;

use axum::{response::Json, routing::get};
use hyper::Method;
use serde_json::json;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// Benchmark backend server
struct BenchmarkBackend {
    port: u16,
    handle: Option<JoinHandle<()>>,
}

impl BenchmarkBackend {
    async fn new(port: u16) -> Self {
        let app = axum::Router::new()
            .route(
                "/health",
                get(|| async {
                    Json(json!({
                        "status": "UP",
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }))
                }),
            )
            .route(
                "/api/fast",
                get(|| async { Json(json!({"message": "fast response"})) }),
            )
            .route(
                "/api/data/:id",
                get(
                    |axum::extract::Path(id): axum::extract::Path<String>| async move {
                        Json(json!({
                            "id": id,
                            "data": format!("Data for {}", id),
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }))
                    },
                ),
            );

        let addr = format!("127.0.0.1:{}", port).parse().unwrap();
        let handle = tokio::spawn(async move {
            axum::Server::bind(&addr)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });

        sleep(Duration::from_millis(50)).await;

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

impl Drop for BenchmarkBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Create a benchmark gateway setup
async fn create_benchmark_gateway(
    port: u16,
    backend_ports: Vec<u16>,
) -> (ApiGateway, Vec<BenchmarkBackend>) {
    // Start backends
    let mut backends = Vec::new();
    for backend_port in backend_ports {
        let backend = BenchmarkBackend::new(backend_port).await;
        backends.push(backend);
    }

    // Create minimal config for benchmarking
    let config = GatewayConfig {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port,
            max_connections: 1000,
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
            health_check_interval: 30, // Longer interval for benchmarks
            health_check_timeout: 5,
            health_check_path: "/health".to_string(),
        },
        cache: CacheConfig {
            enabled: false,
            cache_type: "memory".to_string(),
            default_ttl: 60,
            redis_url: None,
            max_memory_size: Some(100),
        },
        logging: LoggingConfig {
            level: "error".to_string(), // Minimal logging for benchmarks
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

    // Add benchmark routes
    router
        .add_route(RouteConfig::new(
            "/health".to_string(),
            Some(Method::GET),
            backend_urls.clone(),
        ))
        .await
        .unwrap();

    router
        .add_route(RouteConfig::new(
            "/api/fast".to_string(),
            Some(Method::GET),
            backend_urls.clone(),
        ))
        .await
        .unwrap();

    router
        .add_route(RouteConfig::new(
            "/api/data/:id".to_string(),
            Some(Method::GET),
            backend_urls,
        ))
        .await
        .unwrap();

    // Create gateway
    let gateway = ApiGateway::with_config(config.server).with_router(Box::new(router));

    (gateway, backends)
}

/// Benchmark basic request routing
fn bench_basic_routing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (gateway, mut backends) =
        rt.block_on(async { create_benchmark_gateway(9020, vec![9120]).await });

    // Start gateway
    let gateway_handle = rt.spawn(async move { gateway.start().await });

    rt.block_on(sleep(Duration::from_millis(200)));

    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("basic_routing");
    group.throughput(Throughput::Elements(1));

    group.bench_function("health_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let response = client
                    .get("http://127.0.0.1:9020/health")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            })
        });
    });

    group.bench_function("fast_endpoint", |b| {
        b.iter(|| {
            rt.block_on(async {
                let response = client
                    .get("http://127.0.0.1:9020/api/fast")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            })
        });
    });

    group.finish();

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

/// Benchmark load balancing performance
fn bench_load_balancing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (gateway, mut backends) =
        rt.block_on(async { create_benchmark_gateway(9021, vec![9121, 9122, 9123]).await });

    // Add load balancer middleware
    rt.block_on(async {
        let load_balancer = LoadBalancerMiddleware::new();
        load_balancer
            .register_backends(vec![
                Backend::new("http://127.0.0.1:9121".to_string()),
                Backend::new("http://127.0.0.1:9122".to_string()),
                Backend::new("http://127.0.0.1:9123".to_string()),
            ])
            .await
            .unwrap();

        gateway.register_middleware(load_balancer).await.unwrap();
    });

    // Start gateway
    let gateway_handle = rt.spawn(async move { gateway.start().await });

    rt.block_on(sleep(Duration::from_millis(200)));

    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("load_balancing");
    group.throughput(Throughput::Elements(1));

    for backend_count in [1, 2, 3].iter() {
        group.bench_with_input(
            BenchmarkId::new("round_robin", backend_count),
            backend_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let response = client
                            .get("http://127.0.0.1:9021/health")
                            .send()
                            .await
                            .unwrap();
                        black_box(response.status());
                    })
                });
            },
        );
    }

    group.finish();

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

/// Benchmark caching performance
fn bench_caching(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (gateway, mut backends) =
        rt.block_on(async { create_benchmark_gateway(9022, vec![9124]).await });

    // Add cache middleware
    rt.block_on(async {
        let cache_store = Arc::new(MemoryStore::new());
        let cache_middleware = CacheMiddleware::new()
            .with_store(cache_store)
            .with_ttl(Duration::from_secs(60));
        gateway.register_middleware(cache_middleware).await.unwrap();
    });

    // Start gateway
    let gateway_handle = rt.spawn(async move { gateway.start().await });

    rt.block_on(sleep(Duration::from_millis(200)));

    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("caching");
    group.throughput(Throughput::Elements(1));

    // Warm up cache
    rt.block_on(async {
        for i in 0..10 {
            let _ = client
                .get(&format!("http://127.0.0.1:9022/api/data/{}", i))
                .send()
                .await;
        }
    });

    group.bench_function("cache_hit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let response = client
                    .get("http://127.0.0.1:9022/api/data/1")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            })
        });
    });

    group.bench_function("cache_miss", |b| {
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            rt.block_on(async {
                let response = client
                    .get(&format!("http://127.0.0.1:9022/api/data/new_{}", counter))
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            })
        });
    });

    group.finish();

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

/// Benchmark concurrent request handling
fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (gateway, mut backends) =
        rt.block_on(async { create_benchmark_gateway(9023, vec![9125, 9126]).await });

    // Start gateway
    let gateway_handle = rt.spawn(async move { gateway.start().await });

    rt.block_on(sleep(Duration::from_millis(200)));

    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("concurrent_requests");

    for concurrency in [1, 5, 10, 20].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));

        group.bench_with_input(
            BenchmarkId::new("concurrent", concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();

                        for _ in 0..concurrency {
                            let client = client.clone();
                            let handle = tokio::spawn(async move {
                                client
                                    .get("http://127.0.0.1:9023/api/fast")
                                    .send()
                                    .await
                                    .unwrap()
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            let response = handle.await.unwrap();
                            black_box(response.status());
                        }
                    })
                });
            },
        );
    }

    group.finish();

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

/// Benchmark middleware chain performance
fn bench_middleware_chain(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (gateway, mut backends) =
        rt.block_on(async { create_benchmark_gateway(9024, vec![9127]).await });

    // Add multiple middlewares
    rt.block_on(async {
        use api_gateway::middleware::logging::LoggingMiddleware;
        use api_gateway::middleware::timing::TimingMiddleware;

        gateway
            .register_middleware(LoggingMiddleware::basic())
            .await
            .unwrap();
        gateway
            .register_middleware(TimingMiddleware::default())
            .await
            .unwrap();

        let cache_store = Arc::new(MemoryStore::new());
        let cache_middleware = CacheMiddleware::new()
            .with_store(cache_store)
            .with_ttl(Duration::from_secs(60));
        gateway.register_middleware(cache_middleware).await.unwrap();
    });

    // Start gateway
    let gateway_handle = rt.spawn(async move { gateway.start().await });

    rt.block_on(sleep(Duration::from_millis(200)));

    let client = reqwest::Client::new();

    let mut group = c.benchmark_group("middleware_chain");
    group.throughput(Throughput::Elements(1));

    group.bench_function("full_middleware_chain", |b| {
        b.iter(|| {
            rt.block_on(async {
                let response = client
                    .get("http://127.0.0.1:9024/api/fast")
                    .send()
                    .await
                    .unwrap();
                black_box(response.status());
            })
        });
    });

    group.finish();

    // Cleanup
    gateway_handle.abort();
    for backend in &mut backends {
        backend.stop();
    }
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_usage");

    group.bench_function("router_creation", |b| {
        b.iter(|| {
            let router = BasicRouter::new();
            black_box(router);
        });
    });

    group.bench_function("route_registration", |b| {
        b.iter(|| {
            rt.block_on(async {
                let router = BasicRouter::new();
                for i in 0..100 {
                    let route = RouteConfig::new(
                        format!("/api/endpoint/{}", i),
                        Some(Method::GET),
                        vec![format!("http://backend-{}.example.com", i)],
                    );
                    router.add_route(route).await.unwrap();
                }
                black_box(router);
            })
        });
    });

    group.bench_function("cache_operations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let cache = MemoryStore::new();

                // Fill cache
                for i in 0..100 {
                    let key = format!("key_{}", i);
                    let value = api_gateway::middleware::cache::models::CachedResponse {
                        status: 200,
                        headers: vec![],
                        body: format!("value_{}", i).into_bytes(),
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        ttl: 60,
                        cache_key: key.clone(),
                        content_type: Some("text/plain".to_string()),
                    };
                    cache
                        .set(&key, value, Duration::from_secs(60))
                        .await
                        .unwrap();
                }

                // Read from cache
                for i in 0..100 {
                    let key = format!("key_{}", i);
                    let _ = cache.get(&key).await;
                }

                black_box(cache);
            })
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_basic_routing,
    bench_load_balancing,
    bench_caching,
    bench_concurrent_requests,
    bench_middleware_chain,
    bench_memory_usage
);

criterion_main!(benches);
