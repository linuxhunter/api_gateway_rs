use thiserror::Error;

/// Gateway error types
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Backend service unavailable")]
    BackendUnavailable,
    
    #[error("Request timeout")]
    RequestTimeout,
    
    #[error("Cache error: {0}")]
    CacheError(#[from] CacheError),
    
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
    
    #[error("Load balancer error: {0}")]
    LoadBalancerError(#[from] LoadBalancerError),
    
    #[error("Authentication error: {0}")]
    AuthError(#[from] AuthError),
    
    #[error("Internal server error: {0}")]
    InternalError(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Route not found: {0}")]
    RouteNotFound(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// HTTP status code mapping for gateway errors
impl GatewayError {
    pub fn status_code(&self) -> u16 {
        match self {
            GatewayError::AuthenticationFailed(_) => 401,
            GatewayError::BackendUnavailable => 503,
            GatewayError::RequestTimeout => 504,
            GatewayError::CacheError(_) => 500,
            GatewayError::ConfigError(_) => 500,
            GatewayError::LoadBalancerError(_) => 503,
            GatewayError::AuthError(_) => 401,
            GatewayError::InternalError(_) => 500,
            GatewayError::InvalidRequest(_) => 400,
            GatewayError::RouteNotFound(_) => 404,
            GatewayError::IoError(_) => 500,
            GatewayError::SerializationError(_) => 500,
        }
    }
}

/// Cache specific errors
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Failed to connect to cache: {0}")]
    ConnectionError(String),
    
    #[error("Failed to store item in cache: {0}")]
    StoreError(String),
    
    #[error("Failed to retrieve item from cache: {0}")]
    RetrieveError(String),
    
    #[error("Cache item expired")]
    Expired,
}

/// Authentication specific errors
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    
    #[error("Authentication service unavailable: {0}")]
    ServiceUnavailable(String),
}

/// Load balancer specific errors
#[derive(Debug, Error)]
pub enum LoadBalancerError {
    #[error("No backend available")]
    NoBackendAvailable,
    
    #[error("Backend health check failed: {0}")]
    HealthCheckFailed(String),
    
    #[error("Backend connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Backend not found: {0}")]
    BackendNotFound(String),
    
    #[error("Backend already exists: {0}")]
    BackendAlreadyExists(String),
    
    #[error("Service discovery error: {0}")]
    ServiceDiscoveryError(String),
    
    #[error("Invalid load balancing algorithm: {0}")]
    InvalidAlgorithm(String),
}

/// Configuration specific errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(String),
    
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
    
    #[error("Failed to watch configuration file: {0}")]
    WatchError(String),
    
    #[error("Configuration change notification error: {0}")]
    NotificationError(String),
}