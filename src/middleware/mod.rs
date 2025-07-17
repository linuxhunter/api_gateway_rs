pub mod auth;
pub mod cache;
pub mod load_balancer;

use std::sync::Arc;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;

/// Middleware trait for processing requests and responses
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Process a request before it reaches the backend
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError>;
    
    /// Get the name of this middleware
    fn name(&self) -> &str;
}

/// Handler for the next middleware in the chain
#[async_trait]
pub trait MiddlewareHandler: Send + Sync {
    /// Handle the request by passing it to the next middleware or backend
    async fn handle(&self, request: GatewayRequest) -> Result<GatewayResponse, GatewayError>;
}

/// Chain of middleware handlers
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    /// Create a new middleware chain
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }
    
    /// Add a middleware to the chain
    pub fn add<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Arc::new(middleware));
    }
    
    /// Build the middleware chain
    pub fn build(self, final_handler: Arc<dyn MiddlewareHandler>) -> Arc<dyn MiddlewareHandler> {
        let mut handler = final_handler;
        
        // Build the chain in reverse order
        for middleware in self.middlewares.into_iter().rev() {
            handler = Arc::new(MiddlewareLink {
                middleware,
                next: handler,
            });
        }
        
        handler
    }
}

/// Link in the middleware chain
struct MiddlewareLink {
    middleware: Arc<dyn Middleware>,
    next: Arc<dyn MiddlewareHandler>,
}

#[async_trait]
impl MiddlewareHandler for MiddlewareLink {
    async fn handle(&self, request: GatewayRequest) -> Result<GatewayResponse, GatewayError> {
        self.middleware.process_request(request, self.next.clone()).await
    }
}