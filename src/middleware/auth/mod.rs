use std::sync::Arc;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;
use crate::middleware::{Middleware, MiddlewareHandler};

/// Authentication middleware
pub struct AuthMiddleware {
    name: String,
}

impl AuthMiddleware {
    /// Create a new AuthMiddleware
    pub fn new() -> Self {
        Self {
            name: "auth".to_string(),
        }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError> {
        // Will be implemented in future tasks
        next.handle(request).await
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}