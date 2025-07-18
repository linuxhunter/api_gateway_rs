use std::collections::HashMap;
use std::sync::Arc;

use crate::config::LoadBalancerConfig;
use crate::core::request::GatewayRequest;
use crate::middleware::load_balancer::{
    BackendRegistry, HttpHealthChecker, LoadBalancerMiddleware, ServiceDiscovery,
};
use crate::middleware::load_balancer::strategies::LoadBalanceStrategyFactory;
use crate::models::Backend;

/// Example of how to use the load balancer middleware
pub async fn load_balancer_example() {
    // Create a load balancer configuration
    let config = LoadBalancerConfig {
        algorithm: "round_robin".to_string(),
        health_check_interval: 10,
        health_check_timeout: 2,
        health_check_path: "/health".to_string(),
    };
    
    // Create a health checker
    let health_checker = Arc::new(HttpHealthChecker::new(config.clone()));
    
    // Create a load balancing strategy
    let strategy = LoadBalanceStrategyFactory::create(&config.algorithm)
        .expect("Failed to create load balancing strategy");
    
    // Create a load balancer middleware
    let load_balancer = LoadBalancerMiddleware::new()
        .with_strategy(strategy.into())
        .with_health_checker(health_checker);
    
    // Create some backend services
    let backends = vec![
        Backend {
            id: "backend-1".to_string(),
            url: "http://localhost:8081".to_string(),
            weight: 1,
            healthy: true,
            health_check_path: "/health".to_string(),
            timeout_seconds: 30,
            host: "localhost".to_string(),
            port: 8081,
            tags: vec!["api".to_string()],
            metadata: HashMap::new(),
        },
        Backend {
            id: "backend-2".to_string(),
            url: "http://localhost:8082".to_string(),
            weight: 2,
            healthy: true,
            health_check_path: "/health".to_string(),
            timeout_seconds: 30,
            host: "localhost".to_string(),
            port: 8082,
            tags: vec!["api".to_string()],
            metadata: HashMap::new(),
        },
        Backend {
            id: "backend-3".to_string(),
            url: "http://localhost:8083".to_string(),
            weight: 1,
            healthy: true,
            health_check_path: "/health".to_string(),
            timeout_seconds: 30,
            host: "localhost".to_string(),
            port: 8083,
            tags: vec!["api".to_string()],
            metadata: HashMap::new(),
        },
    ];
    
    // Register backends with the load balancer
    load_balancer.register_backends(backends).await.expect("Failed to register backends");
    
    // Start the health checker
    load_balancer.start_health_checker().await.expect("Failed to start health checker");
    
    // Now the load balancer is ready to process requests
    // In a real application, this would be part of the middleware chain
}

/// Example implementation of a service discovery provider
pub struct FileBasedServiceDiscovery {
    config_path: String,
    registry: Arc<BackendRegistry>,
}

impl FileBasedServiceDiscovery {
    /// Create a new file-based service discovery provider
    pub fn new(config_path: &str) -> Self {
        Self {
            config_path: config_path.to_string(),
            registry: Arc::new(BackendRegistry::new()),
        }
    }
    
    /// Load backends from a configuration file
    async fn load_backends_from_file(&self) -> Result<Vec<Backend>, crate::error::LoadBalancerError> {
        // In a real implementation, this would read from a file
        // For this example, we'll just return some hardcoded backends
        Ok(vec![
            Backend {
                id: "backend-1".to_string(),
                url: "http://localhost:8081".to_string(),
                weight: 1,
                healthy: true,
                health_check_path: "/health".to_string(),
                timeout_seconds: 30,
                host: "localhost".to_string(),
                port: 8081,
                tags: vec!["api".to_string()],
                metadata: HashMap::new(),
            },
            Backend {
                id: "backend-2".to_string(),
                url: "http://localhost:8082".to_string(),
                weight: 2,
                healthy: true,
                health_check_path: "/health".to_string(),
                timeout_seconds: 30,
                host: "localhost".to_string(),
                port: 8082,
                tags: vec!["api".to_string()],
                metadata: HashMap::new(),
            },
        ])
    }
}

#[async_trait::async_trait]
impl ServiceDiscovery for FileBasedServiceDiscovery {
    async fn discover_services(&self) -> Result<Vec<Backend>, crate::error::LoadBalancerError> {
        self.load_backends_from_file().await
    }
    
    async fn register_service(&self, backend: Backend) -> Result<(), crate::error::LoadBalancerError> {
        self.registry.register(backend).await
    }
    
    async fn deregister_service(&self, backend_id: &str) -> Result<(), crate::error::LoadBalancerError> {
        self.registry.deregister(backend_id).await
    }
    
    async fn get_service(&self, backend_id: &str) -> Result<Backend, crate::error::LoadBalancerError> {
        self.registry.get(backend_id).await
    }
    
    async fn start(&self) -> Result<(), crate::error::LoadBalancerError> {
        // Load initial backends
        let backends = self.load_backends_from_file().await?;
        
        // Register backends
        for backend in backends {
            self.registry.register(backend).await?;
        }
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<(), crate::error::LoadBalancerError> {
        // Nothing to do for this simple implementation
        Ok(())
    }
}