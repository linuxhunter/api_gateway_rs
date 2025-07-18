mod config;
mod core;
mod error;
mod middleware;
mod models;

use std::str::FromStr;
use std::sync::Arc;

use hyper::Method;
use tokio::signal;

use crate::config::{BasicConfigManager, ConfigManager};
use crate::core::gateway::{ApiGateway, Gateway};
use crate::core::router::Router;
use crate::error::GatewayError;

#[tokio::main]
async fn main() -> Result<(), GatewayError> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config_manager = Arc::new(BasicConfigManager::new());
    let config = config_manager.get_config().await;

    // Create router
    let router = Arc::new(core::router::BasicRouter::new());

    // Register routes from configuration if any
    if !config.routes.is_empty() {
        let routes = config
            .routes
            .iter()
            .map(|r| {
                let method = r.method.as_ref().and_then(|m| Method::from_str(m).ok());
                core::router::RouteConfig::new(r.path.clone(), method, r.backends.clone())
                    .with_auth(r.auth_required)
                    .with_cache(r.cache_enabled)
                    .with_timeout(r.timeout_seconds)
            })
            .collect::<Vec<_>>();

        router
            .register_routes(routes)
            .await
            .expect("Failed to register routes");
    } else {
        // Add some test routes if no routes are configured
        tracing::info!("No routes configured, adding test routes");

        // Add a simple route for the root path
        router
            .add_route(
                core::router::RouteConfig::new(
                    "/".to_string(),
                    Some(Method::GET),
                    vec!["http://localhost:8081".to_string()],
                )
                .with_priority(10),
            )
            .await
            .expect("Failed to add root route");

        // Add a route with path parameters
        router
            .add_route(
                core::router::RouteConfig::new(
                    "/users/:id".to_string(),
                    Some(Method::GET),
                    vec!["http://localhost:8082/users".to_string()],
                )
                .with_auth(true),
            )
            .await
            .expect("Failed to add users route");

        // Add a wildcard route
        router
            .add_route(
                core::router::RouteConfig::new(
                    "/api/*".to_string(),
                    None,
                    vec!["http://localhost:8083/api".to_string()],
                )
                .with_cache(true),
            )
            .await
            .expect("Failed to add API wildcard route");

        // Add a route with multiple backends for load balancing
        router
            .add_route(
                core::router::RouteConfig::new(
                    "/services/:service_name/:endpoint".to_string(),
                    None,
                    vec![
                        "http://localhost:8084".to_string(),
                        "http://localhost:8085".to_string(),
                        "http://localhost:8086".to_string(),
                    ],
                )
                .with_timeout(5)
                .with_metadata("service_type", "microservice"),
            )
            .await
            .expect("Failed to add services route");

        tracing::info!("Test routes added successfully");
    }

    // Create API Gateway with configuration and router
    let router_clone = core::router::BasicRouter::new();

    // Copy routes from the configured router to the new one
    for route in router.get_routes().await {
        router_clone
            .add_route(route)
            .await
            .expect("Failed to add route to router");
    }

    let gateway = Arc::new(
        ApiGateway::with_config(config.server.clone()).with_router(Box::new(router_clone)),
    );

    // Start the gateway
    tracing::info!(
        "Starting API Gateway on {}:{}",
        config.server.host,
        config.server.port
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
