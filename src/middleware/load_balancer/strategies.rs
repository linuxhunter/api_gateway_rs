use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::core::request::GatewayRequest;
use crate::error::LoadBalancerError;
use crate::middleware::load_balancer::{BackendStats, LoadBalanceStrategy};
use crate::models::Backend;

/// Round Robin load balancing strategy
pub struct RoundRobinStrategy {
    /// Current index for round robin selection
    current: AtomicUsize,
    /// Strategy name
    name: &'static str,
}

impl RoundRobinStrategy {
    /// Create a new RoundRobinStrategy
    pub fn new() -> Self {
        Self {
            current: AtomicUsize::new(0),
            name: "round_robin",
        }
    }
}

#[async_trait]
impl LoadBalanceStrategy for RoundRobinStrategy {
    fn name(&self) -> &str {
        self.name
    }
    
    async fn select_backend(
        &self,
        backends: &[Backend],
        _request: &GatewayRequest,
    ) -> Result<Backend, LoadBalancerError> {
        if backends.is_empty() {
            return Err(LoadBalancerError::NoBackendAvailable);
        }
        
        // Get the next index in a thread-safe way
        let current = self.current.fetch_add(1, Ordering::SeqCst);
        let index = current % backends.len();
        
        // Return the selected backend
        Ok(backends[index].clone())
    }
    
    async fn update_stats(&self, _backend_stats: HashMap<String, BackendStats>) {
        // Round robin doesn't use stats
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// Weighted Round Robin load balancing strategy
pub struct WeightedRoundRobinStrategy {
    /// Current index for weighted round robin selection
    current: AtomicUsize,
    /// Strategy name
    name: &'static str,
}

impl WeightedRoundRobinStrategy {
    /// Create a new WeightedRoundRobinStrategy
    pub fn new() -> Self {
        Self {
            current: AtomicUsize::new(0),
            name: "weighted_round_robin",
        }
    }
    
    /// Expand backends based on their weights
    fn expand_backends(&self, backends: &[Backend]) -> Vec<Backend> {
        let mut expanded = Vec::new();
        
        for backend in backends {
            // Add the backend to the expanded list based on its weight
            for _ in 0..backend.weight {
                expanded.push(backend.clone());
            }
        }
        
        expanded
    }
}

#[async_trait]
impl LoadBalanceStrategy for WeightedRoundRobinStrategy {
    fn name(&self) -> &str {
        self.name
    }
    
    async fn select_backend(
        &self,
        backends: &[Backend],
        _request: &GatewayRequest,
    ) -> Result<Backend, LoadBalancerError> {
        if backends.is_empty() {
            return Err(LoadBalancerError::NoBackendAvailable);
        }
        
        // Expand backends based on weights
        let expanded = self.expand_backends(backends);
        
        if expanded.is_empty() {
            return Err(LoadBalancerError::NoBackendAvailable);
        }
        
        // Get the next index in a thread-safe way
        let current = self.current.fetch_add(1, Ordering::SeqCst);
        let index = current % expanded.len();
        
        // Return the selected backend
        Ok(expanded[index].clone())
    }
    
    async fn update_stats(&self, _backend_stats: HashMap<String, BackendStats>) {
        // Weighted round robin doesn't use stats
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// Least Connections load balancing strategy
pub struct LeastConnectionsStrategy {
    /// Backend statistics
    stats: Arc<RwLock<HashMap<String, BackendStats>>>,
    /// Strategy name
    name: &'static str,
}

impl LeastConnectionsStrategy {
    /// Create a new LeastConnectionsStrategy
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            name: "least_connections",
        }
    }
}

#[async_trait]
impl LoadBalanceStrategy for LeastConnectionsStrategy {
    fn name(&self) -> &str {
        self.name
    }
    
    async fn select_backend(
        &self,
        backends: &[Backend],
        _request: &GatewayRequest,
    ) -> Result<Backend, LoadBalancerError> {
        if backends.is_empty() {
            return Err(LoadBalancerError::NoBackendAvailable);
        }
        
        // Get current stats
        let stats = self.stats.read().await;
        
        // Find the backend with the least active connections
        let mut selected = &backends[0];
        let mut min_connections = usize::MAX;
        
        for backend in backends {
            let connections = stats
                .get(&backend.id)
                .map(|s| s.active_connections)
                .unwrap_or(0);
            
            if connections < min_connections {
                min_connections = connections;
                selected = backend;
            }
        }
        
        // Return the selected backend
        Ok(selected.clone())
    }
    
    async fn update_stats(&self, backend_stats: HashMap<String, BackendStats>) {
        // Update the stats
        let mut stats = self.stats.write().await;
        *stats = backend_stats;
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// IP Hash load balancing strategy
pub struct IpHashStrategy {
    /// Strategy name
    name: &'static str,
}

impl IpHashStrategy {
    /// Create a new IpHashStrategy
    pub fn new() -> Self {
        Self {
            name: "ip_hash",
        }
    }
    
    /// Hash an IP address
    fn hash_ip(&self, ip: &IpAddr) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        ip.hash(&mut hasher);
        hasher.finish()
    }
}

#[async_trait]
impl LoadBalanceStrategy for IpHashStrategy {
    fn name(&self) -> &str {
        self.name
    }
    
    async fn select_backend(
        &self,
        backends: &[Backend],
        request: &GatewayRequest,
    ) -> Result<Backend, LoadBalancerError> {
        if backends.is_empty() {
            return Err(LoadBalancerError::NoBackendAvailable);
        }
        
        // Get client IP
        let ip = match &request.client_ip {
            Some(ip) => ip,
            None => {
                // Fall back to round robin if no IP is available
                let index = 0 % backends.len();
                return Ok(backends[index].clone());
            }
        };
        
        // Hash the IP address
        let hash = self.hash_ip(ip);
        
        // Select backend based on hash
        let index = (hash as usize) % backends.len();
        
        // Return the selected backend
        Ok(backends[index].clone())
    }
    
    async fn update_stats(&self, _backend_stats: HashMap<String, BackendStats>) {
        // IP hash doesn't use stats
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// Factory for creating load balancing strategies
pub struct LoadBalanceStrategyFactory;

impl LoadBalanceStrategyFactory {
    /// Create a new load balancing strategy based on the algorithm name
    pub fn create(algorithm: &str) -> Result<Box<dyn LoadBalanceStrategy + Send + Sync>, LoadBalancerError> {
        match algorithm {
            "round_robin" => Ok(Box::new(RoundRobinStrategy::new())),
            "weighted_round_robin" => Ok(Box::new(WeightedRoundRobinStrategy::new())),
            "least_connections" => Ok(Box::new(LeastConnectionsStrategy::new())),
            "ip_hash" => Ok(Box::new(IpHashStrategy::new())),
            _ => Err(LoadBalancerError::InvalidAlgorithm(algorithm.to_string())),
        }
    }
}