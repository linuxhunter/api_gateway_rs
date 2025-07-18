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
use crate::middleware::logging::LoggingMiddleware;
use crate::middleware::timing::TimingMiddleware;

#[tokio::main]
async fn main() -> Result<(), GatewayError> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config_manager = Arc::new(BasicConfigManager::new());

    // Try to load configuration from file
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());
    if std::path::Path::new(&config_path).exists() {
        match config_manager.load_from_file(&config_path).await {
            Ok(_) => {
                tracing::info!("Configuration loaded from {}", config_path);

                // Create a static path for watching
                let config_path_owned = config_path.clone();

                // Start watching the configuration file for changes
                if let Err(e) = config_manager.watch_config_file(config_path_owned).await {
                    tracing::warn!("Failed to watch configuration file: {}", e);
                } else {
                    tracing::info!("Watching configuration file for changes: {}", config_path);

                    // Subscribe to configuration changes
                    let mut config_changes = config_manager.subscribe_to_changes().await;

                    // Spawn a task to handle configuration changes
                    tokio::spawn(async move {
                        while let Ok(event) = config_changes.recv().await {
                            match event.source {
                                config::ConfigChangeSource::FileReload(path) => {
                                    tracing::info!(
                                        "Configuration reloaded from {}",
                                        path.display()
                                    );
                                    // Here you would handle the configuration change
                                    // For example, update routes, middlewares, etc.
                                }
                                config::ConfigChangeSource::FileLoad(path) => {
                                    tracing::info!("Configuration loaded from {}", path.display());
                                }
                                config::ConfigChangeSource::ProgrammaticUpdate => {
                                    tracing::info!("Configuration updated programmatically");
                                }
                            }

                            // You could implement dynamic reconfiguration here
                            // For example, update routes, middlewares, etc.
                        }
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load configuration from {}: {}", config_path, e);
                tracing::info!("Using default configuration");
            }
        }
    } else {
        tracing::info!(
            "Configuration file {} not found, using default configuration",
            config_path
        );

        // Save default configuration to file for reference
        if let Err(e) = config_manager.save_to_file(&config_path).await {
            tracing::warn!(
                "Failed to save default configuration to {}: {}",
                config_path,
                e
            );
        } else {
            tracing::info!("Default configuration saved to {}", config_path);
        }
    }

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

    // Create the API Gateway
    let gateway = Arc::new(
        ApiGateway::with_config(config.server.clone()).with_router(Box::new(router_clone)),
    );

    // Register middlewares
    tracing::info!("Registering middlewares");

    // Register logging middleware
    gateway
        .register_middleware(LoggingMiddleware::detailed())
        .await
        .expect("Failed to register logging middleware");

    // Register timing middleware
    gateway
        .register_middleware(TimingMiddleware::default())
        .await
        .expect("Failed to register timing middleware");

    tracing::info!("Middlewares registered successfully");

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
