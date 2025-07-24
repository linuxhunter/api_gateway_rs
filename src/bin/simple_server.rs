use std::net::SocketAddr;


use axum::{
    extract::{Json, Path},
    http::StatusCode,
    response::Json as JsonResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

// Echo request payload
#[derive(Debug, Clone, Deserialize, Serialize)]
struct EchoRequest {
    message: Option<String>,
    #[serde(default)]
    data: serde_json::Value,
}

// Health check endpoint
async fn health_check() -> JsonResponse<serde_json::Value> {
    JsonResponse(serde_json::json!({
        "status": "UP",
        "name": "simple-backend",
        "version": "1.0.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// Echo endpoint - returns the request body
async fn echo(Json(payload): Json<EchoRequest>) -> JsonResponse<serde_json::Value> {
    JsonResponse(serde_json::json!({
        "message": "Echo response",
        "data": payload,
        "server": "simple-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// Delayed response endpoint - delays for the specified duration
async fn delayed_response(Path(duration_ms): Path<u64>) -> JsonResponse<serde_json::Value> {
    // Simulate explicit delay
    sleep(Duration::from_millis(duration_ms)).await;

    JsonResponse(serde_json::json!({
        "message": format!("Delayed response ({}ms)", duration_ms),
        "server": "simple-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// Error response endpoint - returns the specified status code
async fn error_response(
    Path(status_code): Path<u16>,
) -> (StatusCode, JsonResponse<serde_json::Value>) {
    // Create a status code from the provided value, defaulting to 500 if invalid
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    (
        status,
        JsonResponse(serde_json::json!({
            "error": format!("Error response with status {}", status_code),
            "server": "simple-backend",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

#[tokio::main]
async fn main() {
    // Define the address to bind to
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    // Build our application with routes
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/echo", post(echo))
        .route("/delay/:duration_ms", get(delayed_response))
        .route("/error/:status_code", get(error_response));

    println!("Backend server listening on {}", addr);

    // Start the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
