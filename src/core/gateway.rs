use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;

/// Core API Gateway trait that defines the main functionality
#[async_trait]
pub trait Gateway: Send + Sync {
    /// Process an incoming request and return a response
    async fn process_request(&self, request: GatewayRequest) -> Result<GatewayResponse, GatewayError>;
    
    /// Start the gateway server
    async fn start(&self) -> Result<(), GatewayError>;
    
    /// Stop the gateway server
    async fn stop(&self) -> Result<(), GatewayError>;
    
    /// Check if the gateway is healthy
    async fn health_check(&self) -> bool;
}

/// Basic implementation of the API Gateway
pub struct ApiGateway {
    // Will be expanded in future tasks
}

impl ApiGateway {
    /// Create a new API Gateway instance
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Gateway for ApiGateway {
    async fn process_request(&self, _request: GatewayRequest) -> Result<GatewayResponse, GatewayError> {
        // Will be implemented in future tasks
        Err(GatewayError::InternalError("Not implemented yet".to_string()))
    }
    
    async fn start(&self) -> Result<(), GatewayError> {
        // Will be implemented in future tasks
        Err(GatewayError::InternalError("Not implemented yet".to_string()))
    }
    
    async fn stop(&self) -> Result<(), GatewayError> {
        // Will be implemented in future tasks
        Ok(())
    }
    
    async fn health_check(&self) -> bool {
        true
    }
}