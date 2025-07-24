use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Extension, Path, Json},
    http::StatusCode,
    response::Json as JsonResponse,
};
use rand::Rng;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::examples::backend_server::models::{ServerConfig, HealthStatus, EchoRequest};

/// Health check endpoint
pub async fn health_check(
    Extension(config): Extension<Arc<RwLock<ServerConfig>>>,
) -> (StatusCode, JsonResponse<HealthStatus>) {
    let config = config.read().await;
    
    let health = HealthStatus {
        status: "UP".to_string(),
        name: config.name.clone(),
        version: config.version.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    
    (StatusCode::OK, JsonResponse(health))
}

/// Echo endpoint - returns the request body
pub async fn echo(
    Extension(config): Extension<Arc<RwLock<ServerConfig>>>,
    Json(payload): Json<EchoRequest>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    // Simulate random failures based on failure rate
    if should_fail(&config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR, 
            JsonResponse(serde_json::json!({
                "error": "Random failure occurred",
                "server": config.name,
            }))
        );
    }
    
    // Simulate random delay
    simulate_delay(&config).await;
    
    // Return the echo response
    let response = serde_json::json!({
        "message": "Echo response",
        "data": payload,
        "server": config.name,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    (StatusCode::OK, JsonResponse(response))
}

/// Delayed response endpoint - delays for the specified duration
pub async fn delayed_response(
    Extension(config): Extension<Arc<RwLock<ServerConfig>>>,
    Path(duration_ms): Path<u64>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    // Simulate explicit delay
    sleep(Duration::from_millis(duration_ms)).await;
    
    // Return response
    let response = serde_json::json!({
        "message": format!("Delayed response ({}ms)", duration_ms),
        "server": config.name,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    (StatusCode::OK, JsonResponse(response))
}

/// Error response endpoint - returns the specified status code
pub async fn error_response(
    Extension(config): Extension<Arc<RwLock<ServerConfig>>>,
    Path(status_code): Path<u16>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    let config = config.read().await;
    
    // Create a status code from the provided value, defaulting to 500 if invalid
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    
    // Return error response
    let response = serde_json::json!({
        "error": format!("Error response with status {}", status_code),
        "server": config.name,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    
    (status, JsonResponse(response))
}

/// Helper function to determine if a request should fail based on failure rate
fn should_fail(config: &ServerConfig) -> bool {
    if config.failure_rate <= 0.0 {
        return false;
    }
    
    let mut rng = rand::thread_rng();
    rng.gen::<f64>() < config.failure_rate
}

/// Helper function to simulate random delay
async fn simulate_delay(config: &ServerConfig) {
    if config.max_delay_ms <= 0 {
        return;
    }
    
    let mut rng = rand::thread_rng();
    let delay_ms = if config.min_delay_ms >= config.max_delay_ms {
        config.min_delay_ms
    } else {
        rng.gen_range(config.min_delay_ms..=config.max_delay_ms)
    };
    
    if delay_ms > 0 {
        sleep(Duration::from_millis(delay_ms as u64)).await;
    }
}