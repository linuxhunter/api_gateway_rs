use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;
use hyper::Uri;

// Include the strategies and examples modules
pub mod strategies;
pub mod examples;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::{GatewayError, LoadBalancerError};
use crate::middleware::{Middleware, MiddlewareHandler};
use crate::models::Backend;
use crate::config::LoadBalancerConfig;

/// Service discovery provider trait
#[async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Discover backend services
    async fn discover_services(&self) -> Result<Vec<Backend>, LoadBalancerError>;
    
    /// Register a backend service
    async fn register_service(&self, backend: Backend) -> Result<(), LoadBalancerError>;
    
    /// Deregister a backend service
    async fn deregister_service(&self, backend_id: &str) -> Result<(), LoadBalancerError>;
    
    /// Get a specific backend service by ID
    async fn get_service(&self, backend_id: &str) -> Result<Backend, LoadBalancerError>;
    
    /// Start the service discovery process
    async fn start(&self) -> Result<(), LoadBalancerError>;
    
    /// Stop the service discovery process
    async fn stop(&self) -> Result<(), LoadBalancerError>;
}

/// Backend service status
#[derive(Debug, Clone, PartialEq)]
pub enum BackendStatus {
    /// Backend is healthy and available
    Healthy,
    
    /// Backend is unhealthy but still registered
    Unhealthy,
    
    /// Backend is temporarily disabled
    Disabled,
    
    /// Backend is permanently removed
    Removed,
}

/// Backend service statistics
#[derive(Debug, Clone)]
pub struct BackendStats {
    /// Number of active connections
    pub active_connections: usize,
    
    /// Total number of requests processed
    pub total_requests: u64,
    
    /// Number of successful requests
    pub successful_requests: u64,
    
    /// Number of failed requests
    pub failed_requests: u64,
    
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    
    /// Last health check timestamp
    pub last_health_check: Option<Instant>,
    
    /// Last health check status
    pub last_health_status: BackendStatus,
}

impl Default for BackendStats {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_response_time_ms: 0.0,
            last_health_check: None,
            last_health_status: BackendStatus::Healthy,
        }
    }
}

/// Backend service registry
pub struct BackendRegistry {
    /// Registered backends
    backends: RwLock<HashMap<String, Backend>>,
    
    /// Backend statistics
    stats: RwLock<HashMap<String, BackendStats>>,
}

impl BackendRegistry {
    /// Create a new backend registry
    pub fn new() -> Self {
        Self {
            backends: RwLock::new(HashMap::new()),
            stats: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register a backend
    pub async fn register(&self, backend: Backend) -> Result<(), LoadBalancerError> {
        let mut backends = self.backends.write().await;
        let mut stats = self.stats.write().await;
        
        // Check if backend already exists
        if backends.contains_key(&backend.id) {
            return Err(LoadBalancerError::BackendAlreadyExists(backend.id.clone()));
        }
        
        // Store the ID before moving the backend
        let backend_id = backend.id.clone();
        
        // Add backend and initialize stats
        backends.insert(backend_id.clone(), backend);
        stats.insert(backend_id, BackendStats::default());
        
        Ok(())
    }
    
    /// Deregister a backend
    pub async fn deregister(&self, backend_id: &str) -> Result<(), LoadBalancerError> {
        let mut backends = self.backends.write().await;
        
        // Check if backend exists
        if !backends.contains_key(backend_id) {
            return Err(LoadBalancerError::BackendNotFound(backend_id.to_string()));
        }
        
        // Remove backend
        backends.remove(backend_id);
        
        Ok(())
    }
    
    /// Get a backend by ID
    pub async fn get(&self, backend_id: &str) -> Result<Backend, LoadBalancerError> {
        let backends = self.backends.read().await;
        
        backends
            .get(backend_id)
            .cloned()
            .ok_or_else(|| LoadBalancerError::BackendNotFound(backend_id.to_string()))
    }
    
    /// Get all backends
    pub async fn get_all(&self) -> Vec<Backend> {
        let backends = self.backends.read().await;
        backends.values().cloned().collect()
    }
    
    /// Get healthy backends
    pub async fn get_healthy(&self) -> Vec<Backend> {
        let backends = self.backends.read().await;
        let stats = self.stats.read().await;
        
        backends
            .values()
            .filter(|b| {
                if let Some(backend_stats) = stats.get(&b.id) {
                    backend_stats.last_health_status == BackendStatus::Healthy
                } else {
                    b.healthy
                }
            })
            .cloned()
            .collect()
    }
    
    /// Update backend health status
    pub async fn update_health(&self, backend_id: &str, healthy: bool) -> Result<(), LoadBalancerError> {
        let mut backends = self.backends.write().await;
        let mut stats = self.stats.write().await;
        
        // Check if backend exists
        if let Some(backend) = backends.get_mut(backend_id) {
            backend.healthy = healthy;
            
            // Update stats
            if let Some(backend_stats) = stats.get_mut(backend_id) {
                backend_stats.last_health_check = Some(Instant::now());
                backend_stats.last_health_status = if healthy {
                    BackendStatus::Healthy
                } else {
                    BackendStatus::Unhealthy
                };
            }
            
            Ok(())
        } else {
            Err(LoadBalancerError::BackendNotFound(backend_id.to_string()))
        }
    }
    
    /// Update backend statistics after a request
    pub async fn update_stats(
        &self,
        backend_id: &str,
        success: bool,
        response_time_ms: u64,
    ) -> Result<(), LoadBalancerError> {
        let mut stats = self.stats.write().await;
        
        if let Some(backend_stats) = stats.get_mut(backend_id) {
            // Update request counts
            backend_stats.total_requests += 1;
            if success {
                backend_stats.successful_requests += 1;
            } else {
                backend_stats.failed_requests += 1;
            }
            
            // Update average response time
            let total_requests = backend_stats.successful_requests + backend_stats.failed_requests;
            backend_stats.avg_response_time_ms = (backend_stats.avg_response_time_ms * (total_requests - 1) as f64
                + response_time_ms as f64) / total_requests as f64;
            
            Ok(())
        } else {
            Err(LoadBalancerError::BackendNotFound(backend_id.to_string()))
        }
    }
    
    /// Get statistics for a backend
    pub async fn get_stats(&self, backend_id: &str) -> Result<BackendStats, LoadBalancerError> {
        let stats = self.stats.read().await;
        
        stats
            .get(backend_id)
            .cloned()
            .ok_or_else(|| LoadBalancerError::BackendNotFound(backend_id.to_string()))
    }
    
    /// Get statistics for all backends
    pub async fn get_all_stats(&self) -> HashMap<String, BackendStats> {
        let stats = self.stats.read().await;
        stats.clone()
    }
}

/// Load balancing strategy trait
#[async_trait]
pub trait LoadBalanceStrategy: Send + Sync {
    /// Get the name of this strategy
    fn name(&self) -> &str;
    
    /// Select a backend for the given request
    async fn select_backend(
        &self,
        backends: &[Backend],
        request: &GatewayRequest,
    ) -> Result<Backend, LoadBalancerError>;
    
    /// Update the strategy with backend statistics
    async fn update_stats(&self, backend_stats: HashMap<String, BackendStats>);
    
    /// Clone this strategy into a new boxed instance
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync>;
}

/// Health checker for backend services
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Check if a backend is healthy
    async fn check_health(&self, backend: &Backend) -> bool;
    
    /// Start periodic health checks
    async fn start_health_checks(&self, registry: Arc<BackendRegistry>) -> Result<(), LoadBalancerError>;
    
    /// Stop health checks
    async fn stop_health_checks(&self) -> Result<(), LoadBalancerError>;
}

/// Basic HTTP health checker implementation
pub struct HttpHealthChecker {
    /// Health check configuration
    config: LoadBalancerConfig,
    
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl HttpHealthChecker {
    /// Create a new HTTP health checker
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            config,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl HealthChecker for HttpHealthChecker {
    async fn check_health(&self, backend: &Backend) -> bool {
        // Simplified implementation without using hyper client directly
        // In a real implementation, we would use reqwest or another HTTP client
        // that doesn't require additional feature flags
        
        // For now, just simulate a health check
        tracing::debug!("Simulating health check for backend {}", backend.id);
        
        // Assume backend is healthy if it's marked as healthy in the model
        backend.healthy
    }
    
    async fn start_health_checks(&self, registry: Arc<BackendRegistry>) -> Result<(), LoadBalancerError> {
        // Reset shutdown flag
        let mut shutdown = self.shutdown.write().await;
        *shutdown = false;
        drop(shutdown);
        
        // Clone necessary data for the health check task
        let checker = Arc::new(self.clone());
        let shutdown_flag = self.shutdown.clone();
        let interval = Duration::from_secs(self.config.health_check_interval);
        
        // Spawn health check task
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            
            loop {
                // Check if shutdown was requested
                if *shutdown_flag.read().await {
                    tracing::info!("Health checker shutting down");
                    break;
                }
                
                // Wait for next interval
                timer.tick().await;
                
                // Get all backends
                let backends = registry.get_all().await;
                
                // Check health of each backend
                for backend in backends {
                    let is_healthy = checker.check_health(&backend).await;
                    
                    // Update backend health status
                    if let Err(e) = registry.update_health(&backend.id, is_healthy).await {
                        tracing::error!("Failed to update backend health: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    async fn stop_health_checks(&self) -> Result<(), LoadBalancerError> {
        // Set shutdown flag
        let mut shutdown = self.shutdown.write().await;
        *shutdown = true;
        
        Ok(())
    }
}

// Implement Clone for HttpHealthChecker
impl Clone for HttpHealthChecker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

/// Load balancer middleware
pub struct LoadBalancerMiddleware {
    name: String,
    strategy: Option<Arc<dyn LoadBalanceStrategy + Send + Sync>>,
    health_checker: Option<Arc<dyn HealthChecker + Send + Sync>>,
    registry: Arc<BackendRegistry>,
}

impl LoadBalancerMiddleware {
    /// Create a new LoadBalancerMiddleware
    pub fn new() -> Self {
        Self {
            name: "load_balancer".to_string(),
            strategy: None,
            health_checker: None,
            registry: Arc::new(BackendRegistry::new()),
        }
    }
    
    /// Set the load balancing strategy
    pub fn with_strategy(mut self, strategy: Arc<dyn LoadBalanceStrategy + Send + Sync>) -> Self {
        self.strategy = Some(strategy);
        self
    }
    
    /// Set the health checker
    pub fn with_health_checker(mut self, health_checker: Arc<dyn HealthChecker + Send + Sync>) -> Self {
        self.health_checker = Some(health_checker);
        self
    }
    
    /// Register backends
    pub async fn register_backends(&self, backends: Vec<Backend>) -> Result<(), LoadBalancerError> {
        for backend in backends {
            self.registry.register(backend).await?;
        }
        
        Ok(())
    }
    
    /// Start the health checker
    pub async fn start_health_checker(&self) -> Result<(), LoadBalancerError> {
        if let Some(health_checker) = &self.health_checker {
            health_checker.start_health_checks(self.registry.clone()).await?;
        }
        
        Ok(())
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