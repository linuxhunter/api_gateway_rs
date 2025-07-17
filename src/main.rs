mod config;
mod core;
mod error;
mod middleware;
mod models;

use std::sync::Arc;

use tokio::signal;

use crate::config::{BasicConfigManager, ConfigManager};
use crate::core::gateway::{ApiGateway, Gateway};
use crate::error::GatewayError;

#[tokio::main]
async fn main() -> Result<(), GatewayError> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config_manager = Arc::new(BasicConfigManager::new());
    let config = config_manager.get_config().await;

    // Create API Gateway with configuration
    let gateway = Arc::new(ApiGateway::with_config(config.server.clone()));

    // Start the gateway
    tracing::info!(
        "Starting API Gateway on {}:{}",
        config.server.host, config.server.port
    );
    
    // Start the server
    gateway.start().await?;
    tracing::info!("API Gateway started successfully");

    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    tracing::info!("Shutdown signal received, stopping API Gateway");

    // Stop the gateway
    gateway.stop().await?;
    tracing::info!("API Gateway stopped successfully");

    Ok(())
}
