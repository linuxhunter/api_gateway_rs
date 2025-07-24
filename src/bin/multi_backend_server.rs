use std::collections::HashMap;
use std::sync::Arc;

use api_gateway::examples::backend_server::multi_instance::{
    ServiceInstanceConfig, ServiceInstanceType, ServiceRegistry, ErrorType, run_service_instance,
};
use api_gateway::examples::backend_server::models::ServerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    println!("Starting multiple backend service instances...");
    
    // Create service registry
    let registry = Arc::new(ServiceRegistry::new());
    
    // Create different service instance configurations
    let instances = create_service_instances();
    
    // Start all service instances concurrently
    let mut handles = Vec::new();
    
    for config in instances {
        let registry_clone = registry.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = run_service_instance(config, registry_clone).await {
                eprintln!("Service instance failed: {}", e);
            }
        });
        handles.push(handle);
    }
    
    // Wait for a short time to let services start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Print service registry information
    let _ = print_service_registry(&registry).await;
    
    // Wait for all services to complete (or Ctrl+C)
    println!("All services started. Press Ctrl+C to shutdown...");
    
    // Wait for all handles to complete
    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("Service handle error: {}", e);
        }
    }
    
    Ok(())
}

/// Create different service instance configurations
fn create_service_instances() -> Vec<ServiceInstanceConfig> {
    vec![
        // Fast service instance
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3001,
                name: "fast-service-1".to_string(),
                version: "1.0.0".to_string(),
                failure_rate: 0.01, // 1% failure rate
                min_delay_ms: 1,
                max_delay_ms: 5,
            },
            instance_type: ServiceInstanceType::FastService {
                max_response_time_ms: 10,
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("service_tier".to_string(), serde_json::Value::String("premium".to_string()));
                data.insert("max_concurrent_requests".to_string(), serde_json::Value::Number(1000.into()));
                data
            },
            capabilities: vec!["echo".to_string(), "fast_processing".to_string()],
            region: "us-east-1".to_string(),
            environment: "production".to_string(),
        },
        
        // Another fast service instance for load balancing
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3002,
                name: "fast-service-2".to_string(),
                version: "1.0.0".to_string(),
                failure_rate: 0.01,
                min_delay_ms: 1,
                max_delay_ms: 5,
            },
            instance_type: ServiceInstanceType::FastService {
                max_response_time_ms: 15,
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("service_tier".to_string(), serde_json::Value::String("standard".to_string()));
                data.insert("max_concurrent_requests".to_string(), serde_json::Value::Number(500.into()));
                data
            },
            capabilities: vec!["echo".to_string(), "fast_processing".to_string()],
            region: "us-west-2".to_string(),
            environment: "production".to_string(),
        },
        
        // Slow service instance
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3003,
                name: "slow-service-1".to_string(),
                version: "1.0.0".to_string(),
                failure_rate: 0.05, // 5% failure rate
                min_delay_ms: 100,
                max_delay_ms: 500,
            },
            instance_type: ServiceInstanceType::SlowService {
                min_processing_time_ms: 200,
                max_processing_time_ms: 1000,
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("processing_type".to_string(), serde_json::Value::String("heavy_computation".to_string()));
                data.insert("cpu_intensive".to_string(), serde_json::Value::Bool(true));
                data
            },
            capabilities: vec!["echo".to_string(), "heavy_processing".to_string(), "batch_processing".to_string()],
            region: "eu-west-1".to_string(),
            environment: "production".to_string(),
        },
        
        // Unreliable service instance
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3004,
                name: "unreliable-service-1".to_string(),
                version: "0.9.0".to_string(),
                failure_rate: 0.2, // 20% failure rate
                min_delay_ms: 10,
                max_delay_ms: 100,
            },
            instance_type: ServiceInstanceType::UnreliableService {
                failure_rate: 0.15, // 15% failure rate for this specific behavior
                error_types: vec![
                    ErrorType::InternalError,
                    ErrorType::ServiceUnavailable,
                    ErrorType::Timeout,
                    ErrorType::RateLimitExceeded,
                ],
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("stability".to_string(), serde_json::Value::String("experimental".to_string()));
                data.insert("beta_feature".to_string(), serde_json::Value::Bool(true));
                data
            },
            capabilities: vec!["echo".to_string(), "experimental_features".to_string()],
            region: "ap-southeast-1".to_string(),
            environment: "staging".to_string(),
        },
        
        // Database service instance
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3005,
                name: "database-service-1".to_string(),
                version: "2.1.0".to_string(),
                failure_rate: 0.02, // 2% failure rate
                min_delay_ms: 5,
                max_delay_ms: 50,
            },
            instance_type: ServiceInstanceType::DatabaseService {
                record_count: 1000000, // 1 million records
                query_time_per_record_us: 10, // 10 microseconds per record
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("database_type".to_string(), serde_json::Value::String("postgresql".to_string()));
                data.insert("connection_pool_size".to_string(), serde_json::Value::Number(50.into()));
                data.insert("read_replicas".to_string(), serde_json::Value::Number(3.into()));
                data
            },
            capabilities: vec!["query".to_string(), "transactions".to_string(), "indexing".to_string()],
            region: "us-east-1".to_string(),
            environment: "production".to_string(),
        },
        
        // Cache service instance
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3006,
                name: "cache-service-1".to_string(),
                version: "1.5.0".to_string(),
                failure_rate: 0.01, // 1% failure rate
                min_delay_ms: 1,
                max_delay_ms: 3,
            },
            instance_type: ServiceInstanceType::CacheService {
                hit_rate: 0.85, // 85% cache hit rate
                cache_response_time_ms: 2,
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("cache_type".to_string(), serde_json::Value::String("redis".to_string()));
                data.insert("memory_size_mb".to_string(), serde_json::Value::Number(2048.into()));
                data.insert("eviction_policy".to_string(), serde_json::Value::String("lru".to_string()));
                data
            },
            capabilities: vec!["cache".to_string(), "fast_lookup".to_string(), "ttl_support".to_string()],
            region: "us-west-2".to_string(),
            environment: "production".to_string(),
        },
        
        // Development environment service
        ServiceInstanceConfig {
            server: ServerConfig {
                port: 3007,
                name: "dev-service-1".to_string(),
                version: "1.0.0-dev".to_string(),
                failure_rate: 0.1, // 10% failure rate for testing
                min_delay_ms: 5,
                max_delay_ms: 25,
            },
            instance_type: ServiceInstanceType::FastService {
                max_response_time_ms: 20,
            },
            custom_data: {
                let mut data = HashMap::new();
                data.insert("debug_mode".to_string(), serde_json::Value::Bool(true));
                data.insert("log_level".to_string(), serde_json::Value::String("debug".to_string()));
                data.insert("feature_flags".to_string(), serde_json::Value::Array(vec![
                    serde_json::Value::String("new_algorithm".to_string()),
                    serde_json::Value::String("enhanced_logging".to_string()),
                ]));
                data
            },
            capabilities: vec!["echo".to_string(), "debug".to_string(), "testing".to_string()],
            region: "us-east-1".to_string(),
            environment: "development".to_string(),
        },
    ]
}

/// Print information about registered services
async fn print_service_registry(registry: &ServiceRegistry) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Service Registry Information ===");
    
    let instances = registry.list_instances().await;
    println!("Registered instances: {}", instances.len());
    
    for instance_id in instances {
        if let Some(config) = registry.get_instance(&instance_id).await {
            println!("\nInstance: {}", instance_id);
            println!("  Type: {}", config.instance_type.type_name());
            println!("  Region: {}", config.region);
            println!("  Environment: {}", config.environment);
            println!("  Capabilities: {}", config.capabilities.join(", "));
            println!("  Port: {}", config.server.port);
            println!("  Failure Rate: {:.1}%", config.server.failure_rate * 100.0);
            
            // Print instance-specific details
            match &config.instance_type {
                ServiceInstanceType::FastService { max_response_time_ms } => {
                    println!("  Max Response Time: {}ms", max_response_time_ms);
                }
                ServiceInstanceType::SlowService { min_processing_time_ms, max_processing_time_ms } => {
                    println!("  Processing Time: {}-{}ms", min_processing_time_ms, max_processing_time_ms);
                }
                ServiceInstanceType::UnreliableService { failure_rate, error_types } => {
                    println!("  Instance Failure Rate: {:.1}%", failure_rate * 100.0);
                    println!("  Error Types: {} different types", error_types.len());
                }
                ServiceInstanceType::DatabaseService { record_count, query_time_per_record_us } => {
                    println!("  Records: {}", record_count);
                    println!("  Query Time: {}μs per record", query_time_per_record_us);
                }
                ServiceInstanceType::CacheService { hit_rate, cache_response_time_ms } => {
                    println!("  Cache Hit Rate: {:.1}%", hit_rate * 100.0);
                    println!("  Cache Response Time: {}ms", cache_response_time_ms);
                }
            }
        }
    }
    
    // Print backend information for load balancer
    let backends = registry.get_backends().await;
    println!("\n=== Load Balancer Backend Information ===");
    println!("Available backends: {}", backends.len());
    
    for backend in backends {
        println!("\nBackend: {}", backend.id);
        println!("  URL: {}", backend.url);
        println!("  Weight: {}", backend.weight);
        println!("  Healthy: {}", backend.healthy);
        println!("  Tags: {}", backend.tags.join(", "));
        println!("  Health Check: {}", backend.health_check_path);
    }
    
    println!("\n=== Service Endpoints ===");
    println!("Each service provides the following endpoints:");
    println!("  GET  /health              - Health check");
    println!("  GET  /info               - Service information");
    println!("  POST /echo               - Echo request data");
    println!("  GET  /delay/:duration_ms - Delayed response");
    println!("  GET  /error/:status_code - Error response");
    println!("  POST /process/:data_size - Process data (varies by service type)");
    println!("  GET  /query/:record_id   - Query record (database services only)");
    println!("  GET  /cache/:key         - Cache lookup (cache services only)");
    
    println!("\n=== Example Usage ===");
    println!("Test fast service:      curl http://localhost:3001/health");
    println!("Test slow service:      curl http://localhost:3003/info");
    println!("Test database service:  curl http://localhost:3005/query/12345");
    println!("Test cache service:     curl http://localhost:3006/cache/user:123");
    println!("Test unreliable service: curl http://localhost:3004/echo -d '{{\"test\":\"data\"}}' -H 'Content-Type: application/json'");
    
    Ok(())
}