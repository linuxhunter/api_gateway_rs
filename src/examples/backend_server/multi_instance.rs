use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Extension, Path, Json},
    http::StatusCode,
    response::Json as JsonResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::sleep;
use rand::Rng;

use crate::examples::backend_server::models::ServerConfig;
use crate::models::Backend;

/// Service instance configuration with differentiated behavior
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceInstanceConfig {
    /// Base server configuration
    pub server: ServerConfig,
    
    /// Instance-specific behavior
    pub instance_type: ServiceInstanceType,
    
    /// Custom response data
    pub custom_data: HashMap<String, serde_json::Value>,
    
    /// Service capabilities
    pub capabilities: Vec<String>,
    
    /// Geographic region
    pub region: String,
    
    /// Environment (dev, staging, prod)
    pub environment: String,
}

/// Different types of service instances with unique behaviors
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ServiceInstanceType {
    /// Fast response service with minimal processing
    FastService {
        /// Maximum response time in milliseconds
        max_response_time_ms: u32,
    },
    
    /// Slow service that simulates heavy processing
    SlowService {
        /// Minimum processing time in milliseconds
        min_processing_time_ms: u32,
        /// Maximum processing time in milliseconds
        max_processing_time_ms: u32,
    },
    
    /// Unreliable service with high failure rate
    UnreliableService {
        /// Failure rate (0.0 - 1.0)
        failure_rate: f64,
        /// Types of errors to simulate
        error_types: Vec<ErrorType>,
    },
    
    /// Database service that simulates database operations
    DatabaseService {
        /// Simulated database size
        record_count: u64,
        /// Query processing time per record in microseconds
        query_time_per_record_us: u32,
    },
    
    /// Cache service that provides cached responses
    CacheService {
        /// Cache hit rate (0.0 - 1.0)
        hit_rate: f64,
        /// Cache response time in milliseconds
        cache_response_time_ms: u32,
    },
}

/// Types of errors that can be simulated
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ErrorType {
    /// Internal server error (500)
    InternalError,
    /// Service unavailable (503)
    ServiceUnavailable,
    /// Timeout error (504)
    Timeout,
    /// Bad gateway (502)
    BadGateway,
    /// Rate limit exceeded (429)
    RateLimitExceeded,
}

/// Service registry for managing multiple backend instances
pub struct ServiceRegistry {
    /// Registered service instances
    instances: RwLock<HashMap<String, ServiceInstanceConfig>>,
    
    /// Backend models for load balancer integration
    backends: RwLock<HashMap<String, Backend>>,
}

impl ServiceRegistry {
    /// Create a new service registry
    pub fn new() -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
            backends: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register a service instance
    pub async fn register_instance(&self, config: ServiceInstanceConfig) -> Result<(), String> {
        let instance_id = format!("{}:{}", config.server.name, config.server.port);
        
        // Create backend model for load balancer
        let backend = Backend {
            id: instance_id.clone(),
            url: format!("http://127.0.0.1:{}", config.server.port),
            weight: match &config.instance_type {
                ServiceInstanceType::FastService { .. } => 100,
                ServiceInstanceType::SlowService { .. } => 50,
                ServiceInstanceType::UnreliableService { .. } => 25,
                ServiceInstanceType::DatabaseService { .. } => 75,
                ServiceInstanceType::CacheService { .. } => 150,
            },
            healthy: true,
            health_check_path: "/health".to_string(),
            host: "127.0.0.1".to_string(),
            port: config.server.port,
            timeout_seconds: 30,
            tags: vec![
                config.instance_type.type_name().to_string(),
                config.region.clone(),
                config.environment.clone(),
            ],
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("instance_type".to_string(), config.instance_type.type_name().to_string());
                metadata.insert("region".to_string(), config.region.clone());
                metadata.insert("environment".to_string(), config.environment.clone());
                metadata.insert("capabilities".to_string(), config.capabilities.join(","));
                metadata
            },
        };
        
        // Store both configurations
        let mut instances = self.instances.write().await;
        let mut backends = self.backends.write().await;
        
        instances.insert(instance_id.clone(), config);
        backends.insert(instance_id, backend);
        
        Ok(())
    }
    
    /// Deregister a service instance
    pub async fn deregister_instance(&self, instance_id: &str) -> Result<(), String> {
        let mut instances = self.instances.write().await;
        let mut backends = self.backends.write().await;
        
        instances.remove(instance_id);
        backends.remove(instance_id);
        
        Ok(())
    }
    
    /// Get all registered backends for load balancer
    pub async fn get_backends(&self) -> Vec<Backend> {
        let backends = self.backends.read().await;
        backends.values().cloned().collect()
    }
    
    /// Get service instance configuration
    pub async fn get_instance(&self, instance_id: &str) -> Option<ServiceInstanceConfig> {
        let instances = self.instances.read().await;
        instances.get(instance_id).cloned()
    }
    
    /// List all registered instances
    pub async fn list_instances(&self) -> Vec<String> {
        let instances = self.instances.read().await;
        instances.keys().cloned().collect()
    }
}

impl ServiceInstanceType {
    /// Get the type name as a string
    pub fn type_name(&self) -> &'static str {
        match self {
            ServiceInstanceType::FastService { .. } => "fast",
            ServiceInstanceType::SlowService { .. } => "slow",
            ServiceInstanceType::UnreliableService { .. } => "unreliable",
            ServiceInstanceType::DatabaseService { .. } => "database",
            ServiceInstanceType::CacheService { .. } => "cache",
        }
    }
}

/// Run a service instance with differentiated behavior
pub async fn run_service_instance(
    config: ServiceInstanceConfig,
    registry: Arc<ServiceRegistry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], config.server.port));
    let instance_id = format!("{}:{}", config.server.name, config.server.port);
    
    // Register this instance
    registry.register_instance(config.clone()).await?;
    
    println!("Starting {} service instance '{}' on port {}", 
             config.instance_type.type_name(), 
             config.server.name, 
             config.server.port);
    
    // Create shared state
    let shared_config = Arc::new(RwLock::new(config));
    let shared_registry = registry.clone();
    
    // Build application with differentiated routes
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/echo", post(echo))
        .route("/info", get(service_info))
        .route("/delay/:duration_ms", get(delayed_response))
        .route("/error/:status_code", get(error_response))
        .route("/process/:data_size", post(process_data))
        .route("/query/:record_id", get(query_record))
        .route("/cache/:key", get(cache_lookup))
        .layer(Extension(shared_config))
        .layer(Extension(shared_registry));
    
    println!("Service instance listening on {}", addr);
    
    // Start the server
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service());
    
    // Handle graceful shutdown
    let graceful = server.with_graceful_shutdown(async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        
        println!("Shutting down service instance {}", instance_id);
        
        // Deregister from registry
        if let Err(e) = registry.deregister_instance(&instance_id).await {
            eprintln!("Failed to deregister instance: {}", e);
        }
    });
    
    graceful.await?;
    
    Ok(())
}

/// Health check endpoint with instance-specific information
async fn health_check(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    // Simulate health check behavior based on instance type
    let (status, health_status) = match &config.instance_type {
        ServiceInstanceType::UnreliableService { failure_rate, .. } => {
            if rand::thread_rng().gen::<f64>() < *failure_rate {
                (StatusCode::SERVICE_UNAVAILABLE, "DOWN")
            } else {
                (StatusCode::OK, "UP")
            }
        }
        _ => (StatusCode::OK, "UP"),
    };
    
    let response = serde_json::json!({
        "status": health_status,
        "name": config.server.name,
        "version": config.server.version,
        "instance_type": config.instance_type.type_name(),
        "region": config.region,
        "environment": config.environment,
        "capabilities": config.capabilities,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    (status, JsonResponse(response))
}

/// Service information endpoint
async fn service_info(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
) -> JsonResponse<serde_json::Value> {
    let config = config.read().await;
    
    let mut info = serde_json::json!({
        "name": config.server.name,
        "version": config.server.version,
        "instance_type": config.instance_type.type_name(),
        "region": config.region,
        "environment": config.environment,
        "capabilities": config.capabilities,
        "custom_data": config.custom_data,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    // Add instance-specific information
    match &config.instance_type {
        ServiceInstanceType::FastService { max_response_time_ms } => {
            info["max_response_time_ms"] = serde_json::Value::Number((*max_response_time_ms).into());
        }
        ServiceInstanceType::SlowService { min_processing_time_ms, max_processing_time_ms } => {
            info["min_processing_time_ms"] = serde_json::Value::Number((*min_processing_time_ms).into());
            info["max_processing_time_ms"] = serde_json::Value::Number((*max_processing_time_ms).into());
        }
        ServiceInstanceType::UnreliableService { failure_rate, error_types } => {
            info["failure_rate"] = serde_json::Value::Number(
                serde_json::Number::from_f64(*failure_rate).unwrap_or_else(|| serde_json::Number::from(0))
            );
            info["error_types"] = serde_json::to_value(error_types).unwrap_or(serde_json::Value::Null);
        }
        ServiceInstanceType::DatabaseService { record_count, query_time_per_record_us } => {
            info["record_count"] = serde_json::Value::Number((*record_count).into());
            info["query_time_per_record_us"] = serde_json::Value::Number((*query_time_per_record_us).into());
        }
        ServiceInstanceType::CacheService { hit_rate, cache_response_time_ms } => {
            info["hit_rate"] = serde_json::Value::Number(
                serde_json::Number::from_f64(*hit_rate).unwrap_or_else(|| serde_json::Number::from(0))
            );
            info["cache_response_time_ms"] = serde_json::Value::Number((*cache_response_time_ms).into());
        }
    }
    
    JsonResponse(info)
}

/// Echo endpoint with instance-specific behavior
async fn echo(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    // Apply instance-specific behavior
    match &config.instance_type {
        ServiceInstanceType::FastService { max_response_time_ms } => {
            // Fast service - minimal delay
            let delay = rand::thread_rng().gen_range(1..=*max_response_time_ms);
            sleep(Duration::from_millis(delay as u64)).await;
        }
        ServiceInstanceType::SlowService { min_processing_time_ms, max_processing_time_ms } => {
            // Slow service - significant processing time
            let delay = rand::thread_rng().gen_range(*min_processing_time_ms..=*max_processing_time_ms);
            sleep(Duration::from_millis(delay as u64)).await;
        }
        ServiceInstanceType::UnreliableService { failure_rate, error_types } => {
            // Unreliable service - random failures
            if rand::thread_rng().gen::<f64>() < *failure_rate {
                let error_type = &error_types[rand::thread_rng().gen_range(0..error_types.len())];
                let (status, message) = match error_type {
                    ErrorType::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
                    ErrorType::ServiceUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "Service unavailable"),
                    ErrorType::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Request timeout"),
                    ErrorType::BadGateway => (StatusCode::BAD_GATEWAY, "Bad gateway"),
                    ErrorType::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded"),
                };
                
                return (status, JsonResponse(serde_json::json!({
                    "error": message,
                    "server": config.server.name,
                    "instance_type": config.instance_type.type_name(),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })));
            }
        }
        ServiceInstanceType::DatabaseService { .. } => {
            // Database service - simulate query processing
            sleep(Duration::from_millis(50)).await;
        }
        ServiceInstanceType::CacheService { hit_rate, cache_response_time_ms } => {
            // Cache service - fast response for cache hits
            if rand::thread_rng().gen::<f64>() < *hit_rate {
                sleep(Duration::from_millis(*cache_response_time_ms as u64)).await;
            } else {
                // Cache miss - slower response
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
    
    // Return successful response with instance-specific data
    let mut response = serde_json::json!({
        "message": "Echo response",
        "data": payload,
        "server": config.server.name,
        "instance_type": config.instance_type.type_name(),
        "region": config.region,
        "environment": config.environment,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    // Add custom data if available
    if !config.custom_data.is_empty() {
        response["custom_data"] = serde_json::to_value(&config.custom_data).unwrap_or(serde_json::Value::Null);
    }
    
    (StatusCode::OK, JsonResponse(response))
}

/// Delayed response endpoint
async fn delayed_response(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
    Path(duration_ms): Path<u64>,
) -> JsonResponse<serde_json::Value> {
    let config = config.read().await;
    
    sleep(Duration::from_millis(duration_ms)).await;
    
    JsonResponse(serde_json::json!({
        "message": format!("Delayed response ({}ms)", duration_ms),
        "server": config.server.name,
        "instance_type": config.instance_type.type_name(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Error response endpoint
async fn error_response(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
    Path(status_code): Path<u16>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    
    (status, JsonResponse(serde_json::json!({
        "error": format!("Error response with status {}", status_code),
        "server": config.server.name,
        "instance_type": config.instance_type.type_name(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}

/// Process data endpoint - simulates data processing
async fn process_data(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
    Path(data_size): Path<u64>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    // Simulate processing time based on data size and instance type
    let processing_time_ms = match &config.instance_type {
        ServiceInstanceType::FastService { .. } => data_size / 1000, // 1ms per KB
        ServiceInstanceType::SlowService { .. } => data_size / 100,  // 10ms per KB
        ServiceInstanceType::DatabaseService { query_time_per_record_us, .. } => {
            (data_size * (*query_time_per_record_us as u64)) / 1000 // Convert microseconds to milliseconds
        }
        _ => data_size / 500, // Default: 2ms per KB
    };
    
    sleep(Duration::from_millis(processing_time_ms)).await;
    
    (StatusCode::OK, JsonResponse(serde_json::json!({
        "message": "Data processed successfully",
        "data_size": data_size,
        "processing_time_ms": processing_time_ms,
        "processed_data": payload,
        "server": config.server.name,
        "instance_type": config.instance_type.type_name(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}

/// Query record endpoint - simulates database queries
async fn query_record(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
    Path(record_id): Path<u64>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    match &config.instance_type {
        ServiceInstanceType::DatabaseService { record_count, query_time_per_record_us } => {
            // Simulate database query time
            let query_time_ms = (*query_time_per_record_us as u64) / 1000;
            sleep(Duration::from_millis(query_time_ms)).await;
            
            // Check if record exists
            if record_id > *record_count {
                return (StatusCode::NOT_FOUND, JsonResponse(serde_json::json!({
                    "error": "Record not found",
                    "record_id": record_id,
                    "max_records": record_count,
                    "server": config.server.name,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })));
            }
            
            // Return simulated record data
            (StatusCode::OK, JsonResponse(serde_json::json!({
                "record_id": record_id,
                "data": {
                    "id": record_id,
                    "name": format!("Record {}", record_id),
                    "value": rand::thread_rng().gen::<u32>(),
                    "created_at": chrono::Utc::now().to_rfc3339(),
                },
                "query_time_ms": query_time_ms,
                "server": config.server.name,
                "instance_type": "database",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })))
        }
        _ => {
            // Non-database services return error
            (StatusCode::NOT_IMPLEMENTED, JsonResponse(serde_json::json!({
                "error": "Query operation not supported by this service type",
                "instance_type": config.instance_type.type_name(),
                "server": config.server.name,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })))
        }
    }
}

/// Cache lookup endpoint - simulates cache operations
async fn cache_lookup(
    Extension(config): Extension<Arc<RwLock<ServiceInstanceConfig>>>,
    Path(key): Path<String>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    match &config.instance_type {
        ServiceInstanceType::CacheService { hit_rate, cache_response_time_ms } => {
            // Simulate cache lookup time
            sleep(Duration::from_millis(*cache_response_time_ms as u64)).await;
            
            // Simulate cache hit/miss
            let is_hit = rand::thread_rng().gen::<f64>() < *hit_rate;
            
            if is_hit {
                (StatusCode::OK, JsonResponse(serde_json::json!({
                    "key": key,
                    "value": format!("cached_value_for_{}", key),
                    "cache_hit": true,
                    "response_time_ms": cache_response_time_ms,
                    "server": config.server.name,
                    "instance_type": "cache",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })))
            } else {
                (StatusCode::NOT_FOUND, JsonResponse(serde_json::json!({
                    "key": key,
                    "cache_hit": false,
                    "message": "Cache miss",
                    "server": config.server.name,
                    "instance_type": "cache",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })))
            }
        }
        _ => {
            // Non-cache services return error
            (StatusCode::NOT_IMPLEMENTED, JsonResponse(serde_json::json!({
                "error": "Cache operation not supported by this service type",
                "instance_type": config.instance_type.type_name(),
                "server": config.server.name,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })))
        }
    }
}