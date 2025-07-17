use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::ConfigError;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,
    
    /// Port to listen on
    pub port: u16,
    
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    
    /// Request timeout in seconds
    pub request_timeout: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: 1024,
            request_timeout: 30,
        }
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Whether authentication is enabled
    pub enabled: bool,
    
    /// JWT secret key (if using JWT)
    pub jwt_secret: Option<String>,
    
    /// Authentication service URL (if using external auth)
    pub auth_service_url: Option<String>,
    
    /// Token expiration time in seconds
    pub token_expiration: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            jwt_secret: None,
            auth_service_url: None,
            token_expiration: 3600,
        }
    }
}

/// Load balancer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    /// Load balancing algorithm
    pub algorithm: String,
    
    /// Health check interval in seconds
    pub health_check_interval: u64,
    
    /// Health check timeout in seconds
    pub health_check_timeout: u64,
    
    /// Health check path
    pub health_check_path: String,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            algorithm: "round_robin".to_string(),
            health_check_interval: 10,
            health_check_timeout: 2,
            health_check_path: "/health".to_string(),
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    
    /// Cache type (memory, redis)
    pub cache_type: String,
    
    /// Default TTL in seconds
    pub default_ttl: u64,
    
    /// Redis URL (if using Redis)
    pub redis_url: Option<String>,
    
    /// Maximum memory cache size in MB
    pub max_memory_size: Option<u64>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cache_type: "memory".to_string(),
            default_ttl: 60,
            redis_url: None,
            max_memory_size: Some(100),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    pub level: String,
    
    /// Whether to log to file
    pub log_to_file: bool,
    
    /// Log file path
    pub log_file: Option<String>,
    
    /// Whether to log in JSON format
    pub json_format: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            log_to_file: false,
            log_file: None,
            json_format: false,
        }
    }
}

/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Server configuration
    pub server: ServerConfig,
    
    /// Authentication configuration
    pub auth: AuthConfig,
    
    /// Load balancer configuration
    pub load_balancer: LoadBalancerConfig,
    
    /// Cache configuration
    pub cache: CacheConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Routes configuration
    pub routes: Vec<RouteDefinition>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            load_balancer: LoadBalancerConfig::default(),
            cache: CacheConfig::default(),
            logging: LoggingConfig::default(),
            routes: Vec::new(),
        }
    }
}

/// Route definition in configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDefinition {
    /// Route path pattern
    pub path: String,
    
    /// HTTP method (None means any method)
    pub method: Option<String>,
    
    /// Backend service URLs
    pub backends: Vec<String>,
    
    /// Whether authentication is required
    pub auth_required: bool,
    
    /// Whether caching is enabled
    pub cache_enabled: bool,
    
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

/// Configuration manager trait
#[async_trait]
pub trait ConfigManager: Send + Sync {
    /// Get the current configuration
    async fn get_config(&self) -> GatewayConfig;
    
    /// Load configuration from file
    async fn load_from_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<(), ConfigError>;
    
    /// Save configuration to file
    async fn save_to_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<(), ConfigError>;
    
    /// Update configuration
    async fn update_config(&self, config: GatewayConfig) -> Result<(), ConfigError>;
    
    /// Start watching configuration file for changes
    async fn watch_config_file<P: AsRef<Path> + Send + 'static>(
        &self,
        path: P,
    ) -> Result<(), ConfigError>;
}

/// Basic implementation of the ConfigManager
pub struct BasicConfigManager {
    config: Arc<RwLock<GatewayConfig>>,
}

impl BasicConfigManager {
    /// Create a new BasicConfigManager with default configuration
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(GatewayConfig::default())),
        }
    }
}

#[async_trait]
impl ConfigManager for BasicConfigManager {
    async fn get_config(&self) -> GatewayConfig {
        self.config.read().await.clone()
    }
    
    async fn load_from_file<P: AsRef<Path> + Send>(&self, _path: P) -> Result<(), ConfigError> {
        // Will be implemented in future tasks
        Err(ConfigError::LoadError("Not implemented yet".to_string()))
    }
    
    async fn save_to_file<P: AsRef<Path> + Send>(&self, _path: P) -> Result<(), ConfigError> {
        // Will be implemented in future tasks
        Err(ConfigError::LoadError("Not implemented yet".to_string()))
    }
    
    async fn update_config(&self, config: GatewayConfig) -> Result<(), ConfigError> {
        let mut current_config = self.config.write().await;
        *current_config = config;
        Ok(())
    }
    
    async fn watch_config_file<P: AsRef<Path> + Send + 'static>(
        &self,
        _path: P,
    ) -> Result<(), ConfigError> {
        // Will be implemented in future tasks
        Err(ConfigError::WatchError("Not implemented yet".to_string()))
    }
}