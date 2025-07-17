use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;

/// Core API Gateway trait that defines the main functionality
#[async_trait]
pub trait Gateway: Send + Sync {
    /// Process an incoming request and return a response
    async fn process_request(
        &self,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError>;

    /// Start the gateway server
    async fn start(&self) -> Result<(), GatewayError>;

    /// Stop the gateway server
    async fn stop(&self) -> Result<(), GatewayError>;

    /// Check if the gateway is healthy
    async fn health_check(&self) -> bool;
}

/// Basic implementation of the API Gateway
pub struct ApiGateway {
    /// Router for request routing
    router: Option<Box<dyn crate::core::router::Router>>,
    /// Server configuration
    config: crate::config::ServerConfig,
    /// Server state (handle and shutdown sender)
    server_state: tokio::sync::Mutex<ServerState>,
}

/// Server state that can be mutated
struct ServerState {
    /// Server handle for graceful shutdown
    server_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ApiGateway {
    /// Create a new API Gateway instance
    pub fn new() -> Self {
        Self {
            router: None,
            config: crate::config::ServerConfig::default(),
            server_state: tokio::sync::Mutex::new(ServerState {
                server_handle: None,
                shutdown_tx: None,
            }),
        }
    }

    /// Create a new API Gateway with custom configuration
    pub fn with_config(config: crate::config::ServerConfig) -> Self {
        Self {
            router: None,
            config,
            server_state: tokio::sync::Mutex::new(ServerState {
                server_handle: None,
                shutdown_tx: None,
            }),
        }
    }

    /// Set the router for this gateway
    pub fn with_router(mut self, router: Box<dyn crate::core::router::Router>) -> Self {
        self.router = Some(router);
        self
    }
}

#[async_trait]
impl Gateway for ApiGateway {
    async fn process_request(
        &self,
        _request: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        // Will be implemented in future tasks
        Err(GatewayError::InternalError(
            "Not implemented yet".to_string(),
        ))
    }

    async fn start(&self) -> Result<(), GatewayError> {
        // Check if server is already running
        let mut server_state = self.server_state.lock().await;
        if server_state.server_handle.is_some() {
            return Err(GatewayError::InternalError(
                "Server is already running".to_string(),
            ));
        }

        // Create a new Axum router
        let app = axum::Router::new()
            // Add a basic health check endpoint
            .route("/health", axum::routing::get(|| async { "OK" }))
            // Add middleware for request tracing
            .layer(tower_http::trace::TraceLayer::new_for_http());

        // Build the server with the configured address
        let addr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| GatewayError::InternalError(format!("Invalid address: {}", e)))?;

        tracing::info!("Starting API Gateway server on {}", addr);

        // Create a shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Clone config for the server task
        let config = self.config.clone();

        // Start the server in a separate task
        let server_handle = tokio::spawn(async move {
            let server = axum::Server::bind(&addr).serve(app.into_make_service());

            // Enable graceful shutdown
            let graceful = server.with_graceful_shutdown(async {
                shutdown_rx.await.ok();
                tracing::info!("Shutdown signal received, starting graceful shutdown");
            });

            // Start the server
            if let Err(e) = graceful.await {
                tracing::error!("Server error: {}", e);
            }

            tracing::info!(
                "Server on {}:{} has been shut down",
                config.host,
                config.port
            );
        });

        // Store the server handle and shutdown sender
        server_state.server_handle = Some(server_handle);
        server_state.shutdown_tx = Some(shutdown_tx);

        tracing::info!("API Gateway server started successfully");
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        // Get the server state mutex
        let mut server_state = self.server_state.lock().await;

        // Check if server is running
        if server_state.server_handle.is_none() {
            return Err(GatewayError::InternalError(
                "Server is not running".to_string(),
            ));
        }

        // Send shutdown signal
        if let Some(tx) = server_state.shutdown_tx.take() {
            // We don't care if the receiver is dropped
            let _ = tx.send(());
            tracing::info!("Shutdown signal sent to server");
        }

        // Get the server handle
        if let Some(handle) = server_state.server_handle.take() {
            // Wait for the server to shut down
            match handle.await {
                Ok(_) => {
                    tracing::info!("Server has been shut down gracefully");
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Error while shutting down server: {}", e);
                    Err(GatewayError::InternalError(format!(
                        "Error while shutting down server: {}",
                        e
                    )))
                }
            }
        } else {
            Err(GatewayError::InternalError(
                "Server handle not found".to_string(),
            ))
        }
    }

    async fn health_check(&self) -> bool {
        true
    }
}
