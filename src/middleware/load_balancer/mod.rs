use std::sync::Arc;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::{GatewayError, LoadBalancerError};
use crate::middleware::{Middleware, MiddlewareHandler};
use crate::models::Backend;

/// Load balancing strategy trait
#[async_trait]
pub trait LoadBalanceStrategy: Send + Sync {
    /// Select a backend for the given request
    async fn select_backend(
        &self,
        backends: &[Backend],
        request: &GatewayRequest,
    ) -> Result<Backend, LoadBalancerError>;
}

/// Health checker for backend services
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Check if a backend is healthy
    async fn check_health(&self, backend: &Backend) -> bool;
    
    /// Start periodic health checks
    async fn start_health_checks(&self, backends: Arc<Vec<Backend>>) -> Result<(), LoadBalancerError>;
    
    /// Stop health checks
    async fn stop_health_checks(&self) -> Result<(), LoadBalancerError>;
}

/// Load balancer middleware
pub struct LoadBalancerMiddleware {
    name: String,
    strategy: Option<Arc<dyn LoadBalanceStrategy>>,
    health_checker: Option<Arc<dyn HealthChecker>>,
    backends: Arc<Vec<Backend>>,
}

impl LoadBalancerMiddleware {
    /// Create a new LoadBalancerMiddleware
    pub fn new() -> Self {
        Self {
            name: "load_balancer".to_string(),
            strategy: None,
            health_checker: None,
            backends: Arc::new(Vec::new()),
        }
    }
    
    /// Set the load balancing strategy
    pub fn with_strategy(mut self, strategy: Arc<dyn LoadBalanceStrategy>) -> Self {
        self.strategy = Some(strategy);
        self
    }
    
    /// Set the health checker
    pub fn with_health_checker(mut self, health_checker: Arc<dyn HealthChecker>) -> Self {
        self.health_checker = Some(health_checker);
        self
    }
    
    /// Set the backends
    pub fn with_backends(mut self, backends: Vec<Backend>) -> Self {
        self.backends = Arc::new(backends);
        self
    }
}

#[async_trait]
impl Middleware for LoadBalancerMiddleware {
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