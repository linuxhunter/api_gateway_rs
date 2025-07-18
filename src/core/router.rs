use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;

use async_trait::async_trait;
use hyper::Method;
use regex::Regex;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{self, Visitor};
use tokio::sync::RwLock;

use crate::core::request::GatewayRequest;
use crate::error::{ConfigError, GatewayError};

// Custom serialization for hyper::Method
fn serialize_method<S>(method: &Option<Method>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match method {
        Some(m) => serializer.serialize_some(&m.as_str()),
        None => serializer.serialize_none(),
    }
}

// Custom deserialization for hyper::Method
fn deserialize_method<'de, D>(deserializer: D) -> Result<Option<Method>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MethodVisitor;

    impl<'de> Visitor<'de> for MethodVisitor {
        type Value = Option<Method>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string representing an HTTP method or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s: String = Deserialize::deserialize(deserializer)?;
            Method::from_str(&s)
                .map(Some)
                .map_err(|_| de::Error::custom(format!("invalid HTTP method: {}", s)))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Method::from_str(value)
                .map(Some)
                .map_err(|_| de::Error::custom(format!("invalid HTTP method: {}", value)))
        }
    }

    deserializer.deserialize_option(MethodVisitor)
}

/// Route parameter extracted from path
#[derive(Debug, Clone)]
pub struct RouteParam {
    /// Parameter name
    pub name: String,
    
    /// Parameter value
    pub value: String,
}

/// Route match result
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// Matched route configuration
    pub route: RouteConfig,
    
    /// Extracted path parameters
    pub params: Vec<RouteParam>,
}

/// Route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Route path pattern
    pub path: String,
    
    /// HTTP method (None means any method)
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_method",
        deserialize_with = "deserialize_method"
    )]
    pub method: Option<Method>,
    
    /// Backend service URLs
    pub backends: Vec<String>,
    
    /// Whether authentication is required
    pub auth_required: bool,
    
    /// Whether caching is enabled
    pub cache_enabled: bool,
    
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    
    /// Route priority (higher values have higher priority)
    #[serde(default)]
    pub priority: i32,
    
    /// Route metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl RouteConfig {
    /// Create a new route configuration
    pub fn new(path: String, method: Option<Method>, backends: Vec<String>) -> Self {
        Self {
            path,
            method,
            backends,
            auth_required: false,
            cache_enabled: false,
            timeout_seconds: 30,
            priority: 0,
            metadata: HashMap::new(),
        }
    }
    
    /// Set authentication requirement
    pub fn with_auth(mut self, required: bool) -> Self {
        self.auth_required = required;
        self
    }
    
    /// Set caching
    pub fn with_cache(mut self, enabled: bool) -> Self {
        self.cache_enabled = enabled;
        self
    }
    
    /// Set timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
    
    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Router trait for matching requests to routes
#[async_trait]
pub trait Router: Send + Sync {
    /// Find a matching route for the given request
    async fn find_route(&self, request: &GatewayRequest) -> Result<RouteMatch, GatewayError>;
    
    /// Add a new route
    async fn add_route(&self, route: RouteConfig) -> Result<(), GatewayError>;
    
    /// Remove a route
    async fn remove_route(&self, path: &str, method: Option<Method>) -> Result<(), GatewayError>;
    
    /// Get all routes
    async fn get_routes(&self) -> Vec<RouteConfig>;
    
    /// Register routes from configuration
    async fn register_routes(&self, routes: Vec<RouteConfig>) -> Result<(), GatewayError>;
    
    /// Clear all routes
    async fn clear_routes(&self) -> Result<(), GatewayError>;
    
    /// Clone this router into a new boxed instance
    fn clone_box(&self) -> Box<dyn Router + Send + Sync>;
}

/// Path pattern for route matching
#[derive(Debug, Clone)]
struct PathPattern {
    /// Original path pattern
    pattern: String,
    
    /// Compiled regex for matching
    regex: Regex,
    
    /// Parameter names in order of appearance
    param_names: Vec<String>,
}

impl PathPattern {
    /// Create a new path pattern from a path string
    fn new(path: &str) -> Result<Self, GatewayError> {
        // Handle wildcard paths
        if path == "*" {
            return Ok(Self {
                pattern: path.to_string(),
                regex: Regex::new(".*").unwrap(),
                param_names: Vec::new(),
            });
        }
        
        let mut param_names = Vec::new();
        let mut regex_pattern = "^".to_string();
        
        // Parse the path and extract parameter names
        let path_parts = path.split('/').collect::<Vec<_>>();
        
        for (i, part) in path_parts.iter().enumerate() {
            if i > 0 {
                regex_pattern.push('/');
            }
            
            if part.is_empty() {
                continue;
            }
            
            // Handle path parameters
            if part.starts_with(':') {
                let param_name = &part[1..];
                param_names.push(param_name.to_string());
                regex_pattern.push_str(r"([^/]+)");
            } else if *part == "*" {
                // Handle wildcard segments
                regex_pattern.push_str(r"[^/]*");
            } else if *part == "**" {
                // Handle multi-segment wildcards
                regex_pattern.push_str(r".*");
            } else {
                // Regular path segment
                regex_pattern.push_str(&regex::escape(part));
            }
        }
        
        regex_pattern.push('$');
        
        // Compile the regex
        let regex = Regex::new(&regex_pattern).map_err(|e| {
            GatewayError::ConfigError(ConfigError::ValidationError(format!(
                "Invalid route pattern '{}': {}",
                path, e
            )))
        })?;
        
        Ok(Self {
            pattern: path.to_string(),
            regex,
            param_names,
        })
    }
    
    /// Check if this pattern matches the given path and extract parameters
    fn matches(&self, path: &str) -> Option<Vec<RouteParam>> {
        // Special case for wildcard routes
        if self.pattern == "*" {
            return Some(Vec::new());
        }
        
        // Try to match the regex
        let captures = self.regex.captures(path)?;
        
        // Extract parameters
        let mut params = Vec::new();
        for (i, name) in self.param_names.iter().enumerate() {
            if let Some(value) = captures.get(i + 1) {
                params.push(RouteParam {
                    name: name.clone(),
                    value: value.as_str().to_string(),
                });
            }
        }
        
        Some(params)
    }
}

/// Basic implementation of the Router
pub struct BasicRouter {
    routes: Arc<RwLock<Vec<(RouteConfig, PathPattern)>>>,
}

impl BasicRouter {
    /// Create a new BasicRouter
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Check if a route matches the given path and method
    fn is_match(
        route: &RouteConfig,
        pattern: &PathPattern,
        path: &str,
        method: &Method,
    ) -> Option<Vec<RouteParam>> {
        // Check if path matches
        let params = pattern.matches(path)?;
        
        // Check method if specified
        if let Some(route_method) = &route.method {
            if route_method != method {
                return None;
            }
        }
        
        Some(params)
    }
    
    /// Create a path pattern from a route
    fn create_path_pattern(route: &RouteConfig) -> Result<PathPattern, GatewayError> {
        PathPattern::new(&route.path)
    }
}

#[async_trait]
impl Router for BasicRouter {
    async fn find_route(&self, request: &GatewayRequest) -> Result<RouteMatch, GatewayError> {
        let routes = self.routes.read().await;
        
        // Collect all matching routes
        let mut matches = Vec::new();
        
        for (route, pattern) in routes.iter() {
            if let Some(params) = Self::is_match(route, pattern, request.uri.path(), &request.method) {
                matches.push((route.clone(), params, route.priority));
            }
        }
        
        // Sort by priority (higher first)
        matches.sort_by(|a, b| b.2.cmp(&a.2));
        
        // Return the highest priority match
        if let Some((route, params, _)) = matches.into_iter().next() {
            return Ok(RouteMatch { route, params });
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
        for (existing, _) in routes.iter() {
            if existing.path == route.path && existing.method == route.method {
                return Err(GatewayError::ConfigError(ConfigError::ValidationError(format!(
                    "Route already exists for {} {}",
                    route.method.as_ref().map_or("ANY".to_string(), |m| m.to_string()),
                    route.path
                ))));
            }
        }
        
        // Create path pattern
        let pattern = Self::create_path_pattern(&route)?;
        
        // Add the route
        routes.push((route, pattern));
        
        // Sort routes by priority
        routes.sort_by(|a, b| b.0.priority.cmp(&a.0.priority));
        
        Ok(())
    }
    
    async fn remove_route(&self, path: &str, method: Option<Method>) -> Result<(), GatewayError> {
        let mut routes = self.routes.write().await;
        
        let initial_len = routes.len();
        routes.retain(|(r, _)| r.path != path || r.method != method);
        
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
        let routes = self.routes.read().await;
        routes.iter().map(|(r, _)| r.clone()).collect()
    }
    
    async fn register_routes(&self, routes: Vec<RouteConfig>) -> Result<(), GatewayError> {
        for route in routes {
            self.add_route(route).await?;
        }
        Ok(())
    }
    
    async fn clear_routes(&self) -> Result<(), GatewayError> {
        let mut routes = self.routes.write().await;
        routes.clear();
        Ok(())
    }
    
    fn clone_box(&self) -> Box<dyn Router + Send + Sync> {
        Box::new(Self {
            routes: self.routes.clone(),
        })
    }
}