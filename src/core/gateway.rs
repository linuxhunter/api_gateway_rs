use std::sync::Arc;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::core::router::Router;
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

/// Handler for processing requests
struct RequestHandler {
    router: Box<dyn Router + Send + Sync>,
}

impl RequestHandler {
    /// Create a new request handler
    fn new(router: Box<dyn Router + Send + Sync>) -> Self {
        Self { router }
    }

    /// Process a request
    async fn process_request(
        &self,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        // Find a matching route
        let route_match = self.router.find_route(&request).await?;

        // Log the matched route
        tracing::info!(
            "Route matched: {} {} -> {}",
            request.method,
            request.uri.path(),
            route_match.route.path
        );

        // Extract route parameters if any
        if !route_match.params.is_empty() {
            let params = route_match
                .params
                .iter()
                .map(|p| format!("{}={}", p.name, p.value))
                .collect::<Vec<_>>()
                .join(", ");

            tracing::debug!("Route parameters: {}", params);
        }

        // For now, just return a simple response
        // In future tasks, we'll implement backend forwarding
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );

        let body = format!(
            r#"{{
                "message": "Route matched successfully",
                "path": "{}",
                "method": "{}",
                "backends": {:?},
                "params": {:?}
            }}"#,
            route_match.route.path,
            request.method,
            route_match.route.backends,
            route_match
                .params
                .iter()
                .map(|p| format!("{}={}", p.name, p.value))
                .collect::<Vec<_>>()
        );

        Ok(GatewayResponse::new(
            hyper::StatusCode::OK,
            headers,
            bytes::Bytes::from(body),
        ))
    }
}

/// Basic implementation of the API Gateway
pub struct ApiGateway {
    /// Router for request routing
    router: Option<Box<dyn Router + Send + Sync>>,
    /// Server configuration
    config: crate::config::ServerConfig,
    /// Server state (handle and shutdown sender)
    server_state: Arc<tokio::sync::Mutex<ServerState>>,
    /// Middleware registry
    middleware_registry: Arc<tokio::sync::RwLock<crate::core::middleware::MiddlewareRegistry>>,
}

// Manual implementation of Clone for ApiGateway
impl Clone for ApiGateway {
    fn clone(&self) -> Self {
        // We can't clone the router directly, but we can use clone_box if needed
        let router = self.router.as_ref().map(|r| r.clone_box());
        
        Self {
            router,
            config: self.config.clone(),
            server_state: self.server_state.clone(),
            middleware_registry: self.middleware_registry.clone(),
        }
    }
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
            server_state: Arc::new(tokio::sync::Mutex::new(ServerState {
                server_handle: None,
                shutdown_tx: None,
            })),
            middleware_registry: Arc::new(tokio::sync::RwLock::new(
                crate::core::middleware::MiddlewareRegistry::new()
            )),
        }
    }

    /// Create a new API Gateway with custom configuration
    pub fn with_config(config: crate::config::ServerConfig) -> Self {
        Self {
            router: None,
            config,
            server_state: Arc::new(tokio::sync::Mutex::new(ServerState {
                server_handle: None,
                shutdown_tx: None,
            })),
            middleware_registry: Arc::new(tokio::sync::RwLock::new(
                crate::core::middleware::MiddlewareRegistry::new()
            )),
        }
    }

    /// Set the router for this gateway
    pub fn with_router(mut self, router: Box<dyn Router + Send + Sync>) -> Self {
        self.router = Some(router);
        self
    }
    
    /// Register a middleware with this gateway
    pub async fn register_middleware<M>(&self, middleware: M) -> Result<(), GatewayError>
    where
        M: crate::core::middleware::Middleware + 'static,
    {
        let mut registry = self.middleware_registry.write().await;
        registry.register(middleware);
        tracing::info!("Registered middleware: {}", registry.get_all().last().unwrap().name());
        Ok(())
    }
    
    /// Get all registered middlewares
    pub async fn get_middlewares(&self) -> Vec<Arc<dyn crate::core::middleware::Middleware>> {
        let registry = self.middleware_registry.read().await;
        registry.get_all()
    }
    
    /// Create a middleware chain with the registered middlewares
    async fn create_middleware_chain(
        &self,
        final_handler: impl Fn(GatewayRequest) -> futures::future::BoxFuture<'static, Result<GatewayResponse, GatewayError>>
            + Send
            + Sync
            + 'static,
    ) -> Arc<dyn crate::core::middleware::MiddlewareHandler> {
        let registry = self.middleware_registry.read().await;
        let chain = registry.create_chain();
        
        // Create the final handler that will process the request after all middleware
        let final_handler = Arc::new(crate::core::middleware::FinalHandler::new(final_handler));
        
        // Build the middleware chain
        chain.build(final_handler)
    }
}

#[async_trait]
impl Gateway for ApiGateway {
    async fn process_request(
        &self,
        request: GatewayRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        // Check if router is configured
        let _router = match &self.router {
            Some(router) => router,
            None => {
                return Err(GatewayError::InternalError(
                    "Router not configured".to_string(),
                ));
            }
        };

        // Clone the router for the final handler
        let router_clone = _router.clone_box();
        
        // Create a middleware chain with the final handler
        let middleware_chain = self.create_middleware_chain(move |req| {
            let handler = RequestHandler::new(router_clone.clone_box());
            Box::pin(async move {
                handler.process_request(req).await
            })
        }).await;
        
        // Process the request through the middleware chain
        middleware_chain.handle(request).await
    }

    async fn start(&self) -> Result<(), GatewayError> {
        // Check if server is already running
        let mut server_state = self.server_state.lock().await;
        if server_state.server_handle.is_some() {
            return Err(GatewayError::InternalError(
                "Server is already running".to_string(),
            ));
        }

        // Check if router is configured
        let router = match &self.router {
            Some(router) => router.clone_box(),
            None => {
                return Err(GatewayError::InternalError(
                    "Router not configured".to_string(),
                ));
            }
        };

        // Create a self reference for the fallback closure
        let gateway_ref = Arc::new(self.clone());

        // Create a new Axum router
        let app = axum::Router::new()
            // Add a basic health check endpoint
            .route("/health", axum::routing::get(|| async { "OK" }))
            // Add a catch-all route for API Gateway
            .fallback(move |req: axum::http::Request<axum::body::Body>| {
                let gateway = gateway_ref.clone();
                async move {
                    // Convert Axum request to GatewayRequest
                    let (parts, body) = req.into_parts();
                    let body_bytes = match hyper::body::to_bytes(body).await {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            tracing::error!("Failed to read request body: {}", e);
                            return axum::http::Response::builder()
                                .status(500)
                                .body(axum::body::Body::from("Failed to read request body"))
                                .unwrap();
                        }
                    };

                    let client_ip = parts
                        .headers
                        .get("x-forwarded-for")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.split(',').next())
                        .and_then(|s| s.trim().parse().ok());

                    let gateway_request = GatewayRequest::new(
                        parts.method,
                        parts.uri,
                        parts.headers,
                        body_bytes,
                        client_ip,
                    );

                    // Process the request through the gateway (which uses middleware chain)
                    match gateway.process_request(gateway_request).await {
                        Ok(response) => {
                            // Convert GatewayResponse to Axum response
                            let mut builder =
                                axum::http::Response::builder().status(response.status);

                            // Add headers
                            for (name, value) in response.headers.iter() {
                                builder = builder.header(name, value);
                            }

                            builder.body(axum::body::Body::from(response.body)).unwrap()
                        }
                        Err(e) => {
                            // Handle error
                            let status = e.status_code();
                            let body = format!(
                                "{{\"error\":\"{}\"}}",
                                e.to_string().replace('\"', "\\\"")
                            );

                            axum::http::Response::builder()
                                .status(status)
                                .header("content-type", "application/json")
                                .body(axum::body::Body::from(body))
                                .unwrap()
                        }
                    }
                }
            })
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