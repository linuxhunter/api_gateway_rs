mod config;
mod core;
mod error;
mod middleware;
mod models;

use std::sync::Arc;

use tokio::signal;

use crate::config::{BasicConfigManager, ConfigManager};
use crate::core::gateway::ApiGateway;
use crate::error::GatewayError;

#[tokio::main]
async fn main() -> Result<(), GatewayError> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config_manager = Arc::new(BasicConfigManager::new());
    let config = config_manager.get_config().await;

    // Create API Gateway
    let _gateway = Arc::new(ApiGateway::new());

    // Start the gateway
    println!(
        "Starting API Gateway on {}:{}",
        config.server.host, config.server.port
    );

    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("Shutting down API Gateway");

    Ok(())
}
