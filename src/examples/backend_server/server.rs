use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::Extension,
    routing::{get, post},
    Router,
};
use tokio::sync::RwLock;

use crate::examples::backend_server::models::ServerConfig;
use crate::examples::backend_server::routes::{
    delayed_response, echo, error_response, health_check,
};

/// Run a simple HTTP server for testing the API gateway
pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    // Create a shared state for the server
    let shared_config = Arc::new(RwLock::new(config));

    // Build our application with routes
    let app = Router::new()
        .route("/health", get(health_check))
        //.route("/echo", post(echo))
        .route("/delay/:duration_ms", get(delayed_response))
        .route("/error/:status_code", get(error_response))
        .layer(Extension(shared_config));

    println!("Backend server listening on {}", addr);

    // Start the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

/// Run a backend server with default configuration
pub async fn run_default_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig {
        port: 3001,
        name: "default-backend".to_string(),
        version: "1.0.0".to_string(),
        failure_rate: 0.0,
        min_delay_ms: 0,
        max_delay_ms: 0,
    };

    run_server(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_config() {
        let config = ServerConfig {
            port: 3002,
            name: "test-backend".to_string(),
            version: "1.0.0".to_string(),
            failure_rate: 0.1,
            min_delay_ms: 10,
            max_delay_ms: 50,
        };

        assert_eq!(config.port, 3002);
        assert_eq!(config.name, "test-backend");
        assert!(config.failure_rate > 0.0);
    }
}
