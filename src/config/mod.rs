use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use config::Config;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

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

impl ServerConfig {
    /// Validate the server configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate host
        if self.host.is_empty() {
            return Err(ConfigError::ValidationError(
                "Host cannot be empty".to_string(),
            ));
        }

        // Validate port (0 is a valid port but will be assigned by the OS)

        // Validate max connections
        if self.max_connections == 0 {
            return Err(ConfigError::ValidationError(
                "Maximum connections must be greater than 0".to_string(),
            ));
        }

        // Validate request timeout
        if self.request_timeout == 0 {
            return Err(ConfigError::ValidationError(
                "Request timeout must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
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

impl AuthConfig {
    /// Validate the authentication configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // If authentication is enabled, either JWT secret or auth service URL must be provided
        if self.enabled {
            if self.jwt_secret.is_none() && self.auth_service_url.is_none() {
                return Err(ConfigError::ValidationError(
                    "When authentication is enabled, either JWT secret or auth service URL must be provided".to_string(),
                ));
            }

            // If auth service URL is provided, it must be a valid URL
            if let Some(url) = &self.auth_service_url {
                if url.is_empty() {
                    return Err(ConfigError::ValidationError(
                        "Auth service URL cannot be empty".to_string(),
                    ));
                }

                // Basic URL validation (could be more sophisticated)
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(ConfigError::ValidationError(
                        "Auth service URL must start with http:// or https://".to_string(),
                    ));
                }
            }

            // Validate token expiration
            if self.token_expiration == 0 {
                return Err(ConfigError::ValidationError(
                    "Token expiration must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
    }
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

impl LoadBalancerConfig {
    /// Validate the load balancer configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate algorithm
        let valid_algorithms = [
            "round_robin",
            "weighted_round_robin",
            "least_connections",
            "ip_hash",
        ];
        if !valid_algorithms.contains(&self.algorithm.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid load balancing algorithm: {}. Valid options are: {}",
                self.algorithm,
                valid_algorithms.join(", ")
            )));
        }

        // Validate health check interval
        if self.health_check_interval == 0 {
            return Err(ConfigError::ValidationError(
                "Health check interval must be greater than 0".to_string(),
            ));
        }

        // Validate health check timeout
        if self.health_check_timeout == 0 {
            return Err(ConfigError::ValidationError(
                "Health check timeout must be greater than 0".to_string(),
            ));
        }

        // Health check timeout should be less than interval
        if self.health_check_timeout >= self.health_check_interval {
            return Err(ConfigError::ValidationError(
                "Health check timeout must be less than health check interval".to_string(),
            ));
        }

        // Validate health check path
        if self.health_check_path.is_empty() {
            return Err(ConfigError::ValidationError(
                "Health check path cannot be empty".to_string(),
            ));
        }

        if !self.health_check_path.starts_with('/') {
            return Err(ConfigError::ValidationError(
                "Health check path must start with /".to_string(),
            ));
        }

        Ok(())
    }
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

impl CacheConfig {
    /// Validate the cache configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // If caching is enabled, validate the configuration
        if self.enabled {
            // Validate cache type
            let valid_cache_types = ["memory", "redis"];
            if !valid_cache_types.contains(&self.cache_type.as_str()) {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid cache type: {}. Valid options are: {}",
                    self.cache_type,
                    valid_cache_types.join(", ")
                )));
            }

            // Validate TTL
            if self.default_ttl == 0 {
                return Err(ConfigError::ValidationError(
                    "Default TTL must be greater than 0".to_string(),
                ));
            }

            // If using Redis, validate Redis URL
            if self.cache_type == "redis" {
                if let Some(url) = &self.redis_url {
                    if url.is_empty() {
                        return Err(ConfigError::ValidationError(
                            "Redis URL cannot be empty when using Redis cache".to_string(),
                        ));
                    }

                    // Basic Redis URL validation
                    if !url.starts_with("redis://") {
                        return Err(ConfigError::ValidationError(
                            "Redis URL must start with redis://".to_string(),
                        ));
                    }
                } else {
                    return Err(ConfigError::ValidationError(
                        "Redis URL must be provided when using Redis cache".to_string(),
                    ));
                }
            }

            // If using memory cache, validate max memory size
            if self.cache_type == "memory" {
                if let Some(size) = self.max_memory_size {
                    if size == 0 {
                        return Err(ConfigError::ValidationError(
                            "Maximum memory cache size must be greater than 0".to_string(),
                        ));
                    }
                } else {
                    return Err(ConfigError::ValidationError(
                        "Maximum memory cache size must be provided when using memory cache"
                            .to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
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

impl LoggingConfig {
    /// Validate the logging configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.level.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "Invalid log level: {}. Valid options are: {}",
                self.level,
                valid_levels.join(", ")
            )));
        }

        // If logging to file, validate log file path
        if self.log_to_file {
            if let Some(path) = &self.log_file {
                if path.is_empty() {
                    return Err(ConfigError::ValidationError(
                        "Log file path cannot be empty when logging to file".to_string(),
                    ));
                }
            } else {
                return Err(ConfigError::ValidationError(
                    "Log file path must be provided when logging to file".to_string(),
                ));
            }
        }

        Ok(())
    }
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

impl GatewayConfig {
    /// Validate the entire gateway configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate server configuration
        self.server.validate()?;

        // Validate authentication configuration
        self.auth.validate()?;

        // Validate load balancer configuration
        self.load_balancer.validate()?;

        // Validate cache configuration
        self.cache.validate()?;

        // Validate logging configuration
        self.logging.validate()?;

        // Validate each route definition
        for (index, route) in self.routes.iter().enumerate() {
            route.validate().map_err(|e| {
                ConfigError::ValidationError(format!("Route at index {}: {}", index, e))
            })?;
        }

        // Check for duplicate routes
        let mut paths = Vec::new();
        for route in &self.routes {
            let route_key = (route.path.clone(), route.method.clone());
            if paths.contains(&route_key) {
                return Err(ConfigError::ValidationError(format!(
                    "Duplicate route found: {} {}",
                    route.method.as_ref().unwrap_or(&"ANY".to_string()),
                    route.path
                )));
            }
            paths.push(route_key);
        }

        Ok(())
    }

    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new configuration with custom server settings
    pub fn with_server(server: ServerConfig) -> Self {
        let mut config = Self::default();
        config.server = server;
        config
    }
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

impl RouteDefinition {
    /// Validate the route definition
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate path
        if self.path.is_empty() {
            return Err(ConfigError::ValidationError(
                "Route path cannot be empty".to_string(),
            ));
        }

        // Path should start with /
        if !self.path.starts_with('/') {
            return Err(ConfigError::ValidationError(
                "Route path must start with /".to_string(),
            ));
        }

        // Validate method if provided
        if let Some(method) = &self.method {
            let valid_methods = [
                "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "TRACE", "CONNECT",
            ];
            if !valid_methods.contains(&method.to_uppercase().as_str()) {
                return Err(ConfigError::ValidationError(format!(
                    "Invalid HTTP method: {}. Valid options are: {}",
                    method,
                    valid_methods.join(", ")
                )));
            }
        }

        // Validate backends
        if self.backends.is_empty() {
            return Err(ConfigError::ValidationError(
                "At least one backend service URL must be provided".to_string(),
            ));
        }

        // Validate each backend URL
        for backend in &self.backends {
            if backend.is_empty() {
                return Err(ConfigError::ValidationError(
                    "Backend service URL cannot be empty".to_string(),
                ));
            }

            // Basic URL validation
            if !backend.starts_with("http://") && !backend.starts_with("https://") {
                return Err(ConfigError::ValidationError(format!(
                    "Backend service URL must start with http:// or https://: {}",
                    backend
                )));
            }
        }

        // Validate timeout
        if self.timeout_seconds == 0 {
            return Err(ConfigError::ValidationError(
                "Request timeout must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// The new configuration
    pub config: GatewayConfig,
    
    /// The source of the change
    pub source: ConfigChangeSource,
    
    /// Timestamp of the change
    pub timestamp: SystemTime,
}

/// Source of a configuration change
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChangeSource {
    /// Configuration was loaded from a file
    FileLoad(PathBuf),
    
    /// Configuration was updated programmatically
    ProgrammaticUpdate,
    
    /// Configuration was reloaded due to file change
    FileReload(PathBuf),
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
    
    /// Subscribe to configuration changes
    async fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent>;
    
    /// Stop watching configuration file
    async fn stop_watching(&self) -> Result<(), ConfigError>;
}

/// Basic implementation of the ConfigManager
pub struct BasicConfigManager {
    config: Arc<RwLock<GatewayConfig>>,
    change_tx: broadcast::Sender<ConfigChangeEvent>,
    watcher_shutdown_tx: Arc<RwLock<Option<mpsc::Sender<()>>>>,
    watched_path: Arc<RwLock<Option<PathBuf>>>,
}

impl BasicConfigManager {
    /// Create a new BasicConfigManager with default configuration
    pub fn new() -> Self {
        // Create a broadcast channel for configuration changes
        let (change_tx, _) = broadcast::channel(16);
        
        Self {
            config: Arc::new(RwLock::new(GatewayConfig::default())),
            change_tx,
            watcher_shutdown_tx: Arc::new(RwLock::new(None)),
            watched_path: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new BasicConfigManager with a custom configuration
    pub fn with_config(config: GatewayConfig) -> Result<Self, ConfigError> {
        // Validate the configuration before creating the manager
        config.validate()?;
        
        // Create a broadcast channel for configuration changes
        let (change_tx, _) = broadcast::channel(16);

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            change_tx,
            watcher_shutdown_tx: Arc::new(RwLock::new(None)),
            watched_path: Arc::new(RwLock::new(None)),
        })
    }

    /// Parse configuration from a string
    fn parse_config(content: &str, format: &str) -> Result<GatewayConfig, ConfigError> {
        let config_builder = Config::builder();

        let config_result = match format {
            "json" => config_builder
                .add_source(config::File::from_str(content, config::FileFormat::Json)),
            "yaml" | "yml" => config_builder
                .add_source(config::File::from_str(content, config::FileFormat::Yaml)),
            "toml" => config_builder
                .add_source(config::File::from_str(content, config::FileFormat::Toml)),
            _ => {
                return Err(ConfigError::ValidationError(format!(
                    "Unsupported configuration format: {}",
                    format
                )))
            }
        };

        let config_built = config_result
            .build()
            .map_err(|e| ConfigError::LoadError(format!("Failed to build configuration: {}", e)))?;

        // Convert to our GatewayConfig
        let gateway_config: GatewayConfig = config_built
            .try_deserialize()
            .map_err(|e| ConfigError::ValidationError(format!("Invalid configuration: {}", e)))?;

        Ok(gateway_config)
    }

    /// Get the file format from the file extension
    fn get_file_format(path: &Path) -> Result<&str, ConfigError> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| {
                ConfigError::ValidationError(
                    "Configuration file must have an extension".to_string(),
                )
            })
    }
    
    /// Notify subscribers about configuration changes
    async fn notify_change(&self, source: ConfigChangeSource) {
        let config = self.config.read().await.clone();
        let event = ConfigChangeEvent {
            config,
            source,
            timestamp: SystemTime::now(),
        };
        
        // Send the event to all subscribers
        // It's OK if there are no subscribers or if sending fails
        let _ = self.change_tx.send(event);
    }
}

#[async_trait]
impl ConfigManager for BasicConfigManager {
    async fn get_config(&self) -> GatewayConfig {
        self.config.read().await.clone()
    }

    async fn load_from_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<(), ConfigError> {
        let path = path.as_ref();

        // Check if the file exists
        if !path.exists() {
            return Err(ConfigError::LoadError(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        // Get the file format
        let format = Self::get_file_format(path)?;

        // Read the file content
        let content = fs::read_to_string(path).map_err(|e| {
            ConfigError::LoadError(format!(
                "Failed to read configuration file {}: {}",
                path.display(),
                e
            ))
        })?;

        // Parse the configuration
        let gateway_config = Self::parse_config(&content, format)?;

        // Validate the configuration
        gateway_config.validate()?;

        // Update the configuration
        let mut current_config = self.config.write().await;
        *current_config = gateway_config;

        // Notify subscribers about the change
        drop(current_config); // Release the write lock before notifying
        self.notify_change(ConfigChangeSource::FileLoad(path.to_path_buf())).await;

        info!("Configuration loaded successfully from {}", path.display());
        Ok(())
    }

    async fn save_to_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<(), ConfigError> {
        let path = path.as_ref();

        // Get the file format
        let format = Self::get_file_format(path)?;

        // Get the current configuration
        let config = self.config.read().await.clone();

        // Serialize the configuration
        let content = match format {
            "json" => serde_json::to_string_pretty(&config).map_err(|e| {
                ConfigError::ValidationError(format!(
                    "Failed to serialize configuration to JSON: {}",
                    e
                ))
            })?,
            "yaml" | "yml" => serde_yaml::to_string(&config).map_err(|e| {
                ConfigError::ValidationError(format!(
                    "Failed to serialize configuration to YAML: {}",
                    e
                ))
            })?,
            "toml" => toml::to_string_pretty(&config).map_err(|e| {
                ConfigError::ValidationError(format!(
                    "Failed to serialize configuration to TOML: {}",
                    e
                ))
            })?,
            _ => {
                return Err(ConfigError::ValidationError(format!(
                    "Unsupported configuration format: {}",
                    format
                )))
            }
        };

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ConfigError::LoadError(format!(
                    "Failed to create parent directories for {}: {}",
                    path.display(),
                    e
                ))
            })?;
        }

        // Write the configuration to file
        fs::write(path, content).map_err(|e| {
            ConfigError::LoadError(format!(
                "Failed to write configuration to {}: {}",
                path.display(),
                e
            ))
        })?;

        info!("Configuration saved successfully to {}", path.display());
        Ok(())
    }

    async fn update_config(&self, config: GatewayConfig) -> Result<(), ConfigError> {
        // Validate the configuration before updating
        config.validate()?;

        // Update the configuration
        let mut current_config = self.config.write().await;
        *current_config = config;
        
        // Notify subscribers about the change
        drop(current_config); // Release the write lock before notifying
        self.notify_change(ConfigChangeSource::ProgrammaticUpdate).await;

        info!("Configuration updated successfully");
        Ok(())
    }

    async fn watch_config_file<P: AsRef<Path> + Send + 'static>(
        &self,
        path: P,
    ) -> Result<(), ConfigError> {
        let path = path.as_ref().to_owned();

        // Check if the file exists
        if !path.exists() {
            return Err(ConfigError::LoadError(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }
        
        // Check if we're already watching a file
        let mut watched_path = self.watched_path.write().await;
        if watched_path.is_some() {
            // Stop watching the current file first
            drop(watched_path); // Release the lock before calling stop_watching
            self.stop_watching().await?;
            watched_path = self.watched_path.write().await; // Re-acquire the lock
        }
        
        // Update the watched path
        *watched_path = Some(path.clone());
        drop(watched_path); // Release the lock

        // Create channels for the watcher
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let (event_tx, mut event_rx) = mpsc::channel::<()>(16);
        
        // Store the shutdown sender
        let mut watcher_shutdown = self.watcher_shutdown_tx.write().await;
        *watcher_shutdown = Some(shutdown_tx);
        drop(watcher_shutdown); // Release the lock

        // Clone the necessary data for the watcher task
        let config_clone = self.config.clone();
        let path_clone = path.clone();
        let change_tx = self.change_tx.clone();

        // Spawn a task to watch for file changes
        tokio::spawn(async move {
            // Create a debounce timer to avoid processing too many events
            let mut debounce_timer = tokio::time::interval(Duration::from_millis(500));
            let mut pending_reload = false;
            
            // Create a watcher
            let mut watcher = match RecommendedWatcher::new(
                move |res| {
                    match res {
                        Ok(_) => {
                            // File changed, send an event
                            let _ = event_tx.try_send(());
                        }
                        Err(e) => {
                            error!("Error watching configuration file: {}", e);
                        }
                    }
                },
                NotifyConfig::default(),
            ) {
                Ok(watcher) => watcher,
                Err(e) => {
                    error!("Failed to create file watcher: {}", e);
                    return;
                }
            };

            // Watch the file
            if let Err(e) = watcher.watch(&path_clone, RecursiveMode::NonRecursive) {
                error!(
                    "Failed to watch configuration file {}: {}",
                    path_clone.display(),
                    e
                );
                return;
            }

            info!("Watching configuration file: {}", path_clone.display());

            // Process events with debouncing
            loop {
                tokio::select! {
                    // Check for shutdown signal
                    _ = shutdown_rx.recv() => {
                        info!("Stopping configuration file watcher for {}", path_clone.display());
                        break;
                    }
                    
                    // Check for file change events
                    Some(_) = event_rx.recv() => {
                        debug!("Configuration file change detected, scheduling reload");
                        pending_reload = true;
                    }
                    
                    // Process debounced events
                    _ = debounce_timer.tick() => {
                        if pending_reload {
                            debug!("Processing configuration file change");
                            pending_reload = false;
                            
                            // Reload the configuration
                            if let Err(e) = reload_config(&path_clone, &config_clone, &change_tx).await {
                                error!("Failed to reload configuration: {}", e);
                            }
                        }
                    }
                }
            }
        });

        info!("Configuration file watcher started for {}", path.display());
        Ok(())
    }
    
    async fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_tx.subscribe()
    }
    
    async fn stop_watching(&self) -> Result<(), ConfigError> {
        // Check if we're watching a file
        let mut watched_path = self.watched_path.write().await;
        if watched_path.is_none() {
            // Not watching any file
            return Ok(());
        }
        
        // Get the path we're watching
        let path = watched_path.take().unwrap();
        
        // Send shutdown signal to the watcher task
        let mut watcher_shutdown = self.watcher_shutdown_tx.write().await;
        if let Some(tx) = watcher_shutdown.take() {
            // Send shutdown signal
            if let Err(e) = tx.send(()).await {
                warn!("Failed to send shutdown signal to watcher: {}", e);
            }
        }
        
        info!("Stopped watching configuration file: {}", path.display());
        Ok(())
    }
}

/// Helper function to reload configuration from a file
async fn reload_config(
    path: &Path,
    config: &Arc<RwLock<GatewayConfig>>,
    change_tx: &broadcast::Sender<ConfigChangeEvent>,
) -> Result<(), ConfigError> {
    // Read the file content
    let content = fs::read_to_string(path).map_err(|e| {
        ConfigError::LoadError(format!(
            "Failed to read configuration file {}: {}",
            path.display(),
            e
        ))
    })?;

    // Get the file format
    let format = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext,
        None => {
            return Err(ConfigError::ValidationError(
                "Configuration file must have an extension".to_string(),
            ));
        }
    };

    // Parse the configuration
    let gateway_config = BasicConfigManager::parse_config(&content, format)?;

    // Validate the configuration
    gateway_config.validate()?;

    // Update the configuration
    let mut current_config = config.write().await;
    *current_config = gateway_config.clone();
    drop(current_config); // Release the write lock

    // Notify subscribers about the change
    let event = ConfigChangeEvent {
        config: gateway_config,
        source: ConfigChangeSource::FileReload(path.to_path_buf()),
        timestamp: SystemTime::now(),
    };
    
    // Send the event to all subscribers
    // It's OK if there are no subscribers or if sending fails
    let _ = change_tx.send(event);

    info!("Configuration reloaded successfully from {}", path.display());
    Ok(())
}