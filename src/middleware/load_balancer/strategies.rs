use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use crate::core::request::GatewayRequest;
use crate::error::LoadBalancerError;
use crate::middleware::load_balancer::{BackendStats, LoadBalanceStrategy};
use crate::models::Backend;

/// Round Robin load balancing strategy
/// 
/// This strategy distributes requests evenly across all backends in a circular order.
/// Each backend receives requests in turn, regardless of the current load or response time.
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
        
        debug!("Round Robin selected backend {} (index {})", backends[index].id, index);
        
        // Return the selected backend
        Ok(backends[index].clone())
    }
    
    async fn update_stats(&self, _backend_stats: HashMap<String, BackendStats>) {
        // Round robin doesn't use stats
        trace!("Round Robin strategy ignoring stats update (not used by this algorithm)");
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// Weighted Round Robin load balancing strategy
/// 
/// This strategy distributes requests across backends based on their assigned weights.
/// Backends with higher weights receive proportionally more requests than those with lower weights.
/// For example, a backend with weight 2 will receive twice as many requests as a backend with weight 1.
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
    /// 
    /// This method creates a virtual list where each backend appears multiple times
    /// according to its weight. This allows us to use a simple round-robin selection
    /// on the expanded list to achieve weighted distribution.
    fn expand_backends(&self, backends: &[Backend]) -> Vec<Backend> {
        let mut expanded = Vec::new();
        
        for backend in backends {
            // Add the backend to the expanded list based on its weight
            for _ in 0..backend.weight {
                expanded.push(backend.clone());
            }
        }
        
        trace!("Expanded {} backends into {} weighted entries", backends.len(), expanded.len());
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
        
        let selected = &expanded[index];
        debug!("Weighted Round Robin selected backend {} (weight {})", selected.id, selected.weight);
        
        // Return the selected backend
        Ok(expanded[index].clone())
    }
    
    async fn update_stats(&self, _backend_stats: HashMap<String, BackendStats>) {
        // Weighted round robin doesn't use stats
        trace!("Weighted Round Robin strategy ignoring stats update (not used by this algorithm)");
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// Least Connections load balancing strategy
/// 
/// This strategy routes requests to the backend with the fewest active connections.
/// It's useful for distributing load more evenly when request processing times vary significantly.
/// The strategy maintains a count of active connections for each backend and selects the one with the lowest count.
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
            
            trace!("Backend {} has {} active connections", backend.id, connections);
            
            if connections < min_connections {
                min_connections = connections;
                selected = backend;
            }
        }
        
        debug!("Least Connections selected backend {} with {} connections", 
               selected.id, 
               stats.get(&selected.id).map(|s| s.active_connections).unwrap_or(0));
        
        // Return the selected backend
        Ok(selected.clone())
    }
    
    async fn update_stats(&self, backend_stats: HashMap<String, BackendStats>) {
        // Update the stats
        let mut stats = self.stats.write().await;
        
        debug!("Updating Least Connections stats for {} backends", backend_stats.len());
        
        // Log connection counts for debugging
        for (id, stat) in &backend_stats {
            trace!("Backend {} has {} active connections", id, stat.active_connections);
        }
        
        *stats = backend_stats;
    }
    
    fn clone_box(&self) -> Box<dyn LoadBalanceStrategy + Send + Sync> {
        Box::new(Self::new())
    }
}

/// IP Hash load balancing strategy
/// 
/// This strategy uses a hash of the client's IP address to determine which backend to route to.
/// It ensures that requests from the same client IP are consistently routed to the same backend,
/// which is useful for maintaining session affinity without requiring sticky sessions.
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
    /// 
    /// This method creates a deterministic hash value from an IP address,
    /// which is used to select a backend consistently for the same IP.
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
                debug!("No client IP available, falling back to first backend");
                // Fall back to first backend if no IP is available
                return Ok(backends[0].clone());
            }
        };
        
        // Hash the IP address
        let hash = self.hash_ip(ip);
        
        // Select backend based on hash
        let index = (hash as usize) % backends.len();
        
        debug!("IP Hash selected backend {} for IP {} (hash: {})", 
               backends[index].id, ip, hash);
        
        // Return the selected backend
        Ok(backends[index].clone())
    }
    
    async fn update_stats(&self, _backend_stats: HashMap<String, BackendStats>) {
        // IP hash doesn't use stats
        trace!("IP Hash strategy ignoring stats update (not used by this algorithm)");
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
        debug!("Creating load balancing strategy: {}", algorithm);
        
        match algorithm {
            "round_robin" => Ok(Box::new(RoundRobinStrategy::new())),
            "weighted_round_robin" => Ok(Box::new(WeightedRoundRobinStrategy::new())),
            "least_connections" => Ok(Box::new(LeastConnectionsStrategy::new())),
            "ip_hash" => Ok(Box::new(IpHashStrategy::new())),
            _ => {
                let error = LoadBalancerError::InvalidAlgorithm(algorithm.to_string());
                debug!("Invalid load balancing algorithm: {}", algorithm);
                Err(error)
            }
        }
    }
}