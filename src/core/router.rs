use std::sync::Arc;

use async_trait::async_trait;
use hyper::Method;
use tokio::sync::RwLock;

use crate::core::request::GatewayRequest;
use crate::error::GatewayError;

/// Route configuration
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// Route path pattern
    pub path: String,
    
    /// HTTP method (None means any method)
    pub method: Option<Method>,
    
    /// Backend service URLs
    pub backends: Vec<String>,
    
    /// Whether authentication is required
    pub auth_required: bool,
    
    /// Whether caching is enabled
    pub cache_enabled: bool,
    
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

/// Router trait for matching requests to routes
#[async_trait]
pub trait Router: Send + Sync {
    /// Find a matching route for the given request
    async fn find_route(&self, request: &GatewayRequest) -> Result<RouteConfig, GatewayError>;
    
    /// Add a new route
    async fn add_route(&self, route: RouteConfig) -> Result<(), GatewayError>;
    
    /// Remove a route
    async fn remove_route(&self, path: &str, method: Option<Method>) -> Result<(), GatewayError>;
    
    /// Get all routes
    async fn get_routes(&self) -> Vec<RouteConfig>;
}

/// Basic implementation of the Router
pub struct BasicRouter {
    routes: Arc<RwLock<Vec<RouteConfig>>>,
}

impl BasicRouter {
    /// Create a new BasicRouter
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Check if a route matches the given path and method
    fn is_match(route: &RouteConfig, path: &str, method: &Method) -> bool {
        // Simple path matching for now, will be enhanced in future tasks
        if route.path != path && route.path != "*" {
            return false;
        }
        
        // Check method if specified
        if let Some(route_method) = &route.method {
            if route_method != method {
                return false;
            }
        }
        
        true
    }
}

#[async_trait]
impl Router for BasicRouter {
    async fn find_route(&self, request: &GatewayRequest) -> Result<RouteConfig, GatewayError> {
        let routes = self.routes.read().await;
        
        for route in routes.iter() {
            if Self::is_match(route, request.uri.path(), &request.method) {
                return Ok(route.clone());
            }
        }
        
        Err(GatewayError::RouteNotFound(format!(
            "No route found for {} {}",
            request.method,
            request.uri.path()
        )))
    }
    
    async fn add_route(&self, route: RouteConfig) -> Result<(), GatewayError> {
        let mut routes = self.routes.write().await;
        
        // Check for duplicate routes
        for existing in routes.iter() {
            if existing.path == route.path && existing.method == route.method {
                return Err(GatewayError::ConfigError(crate::error::ConfigError::ValidationError(format!(
                    "Route already exists for {} {}",
                    route.method.as_ref().map_or("ANY".to_string(), |m| m.to_string()),
                    route.path
                ))));
            }
        }
        
        routes.push(route);
        Ok(())
    }
    
    async fn remove_route(&self, path: &str, method: Option<Method>) -> Result<(), GatewayError> {
        let mut routes = self.routes.write().await;
        
        let initial_len = routes.len();
        routes.retain(|r| r.path != path || r.method != method);
        
        if routes.len() == initial_len {
            return Err(GatewayError::RouteNotFound(format!(
                "No route found for {} {}",
                method.as_ref().map_or("ANY".to_string(), |m| m.to_string()),
                path
            )));
        }
        
        Ok(())
    }
    
    async fn get_routes(&self) -> Vec<RouteConfig> {
        self.routes.read().await.clone()
    }
}