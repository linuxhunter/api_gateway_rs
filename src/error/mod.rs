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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_gateway_error_status_codes() {
        assert_eq!(GatewayError::AuthenticationFailed("test".to_string()).status_code(), 401);
        assert_eq!(GatewayError::BackendUnavailable.status_code(), 503);
        assert_eq!(GatewayError::RequestTimeout.status_code(), 504);
        assert_eq!(GatewayError::InternalError("test".to_string()).status_code(), 500);
        assert_eq!(GatewayError::InvalidRequest("test".to_string()).status_code(), 400);
        assert_eq!(GatewayError::RouteNotFound("test".to_string()).status_code(), 404);
        assert_eq!(GatewayError::SerializationError("test".to_string()).status_code(), 500);
    }

    #[test]
    fn test_gateway_error_from_cache_error() {
        let cache_error = CacheError::ConnectionError("Redis down".to_string());
        let gateway_error: GatewayError = cache_error.into();

        if let GatewayError::CacheError(CacheError::ConnectionError(msg)) = gateway_error {
            assert_eq!(msg, "Redis down");
        } else {
            panic!("Expected CacheError conversion");
        }
    }

    #[test]
    fn test_gateway_error_from_config_error() {
        let config_error = ConfigError::ValidationError("Invalid config".to_string());
        let gateway_error: GatewayError = config_error.into();

        if let GatewayError::ConfigError(ConfigError::ValidationError(msg)) = gateway_error {
            assert_eq!(msg, "Invalid config");
        } else {
            panic!("Expected ConfigError conversion");
        }
    }

    #[test]
    fn test_gateway_error_from_load_balancer_error() {
        let lb_error = LoadBalancerError::NoBackendAvailable;
        let gateway_error: GatewayError = lb_error.into();

        if let GatewayError::LoadBalancerError(LoadBalancerError::NoBackendAvailable) = gateway_error {
            // Expected
        } else {
            panic!("Expected LoadBalancerError conversion");
        }
    }

    #[test]
    fn test_gateway_error_from_auth_error() {
        let auth_error = AuthError::InvalidToken;
        let gateway_error: GatewayError = auth_error.into();

        if let GatewayError::AuthError(AuthError::InvalidToken) = gateway_error {
            // Expected
        } else {
            panic!("Expected AuthError conversion");
        }
    }

    #[test]
    fn test_gateway_error_from_io_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let gateway_error: GatewayError = io_error.into();

        if let GatewayError::IoError(_) = gateway_error {
            // Expected
        } else {
            panic!("Expected IoError conversion");
        }
    }

    #[test]
    fn test_error_display() {
        let error = GatewayError::AuthenticationFailed("test".to_string());
        let display_str = format!("{}", error);
        assert!(display_str.contains("Authentication failed"));
        assert!(display_str.contains("test"));
    }

    #[test]
    fn test_all_error_variants_have_status_codes() {
        let test_errors = vec![
            GatewayError::AuthenticationFailed("test".to_string()),
            GatewayError::BackendUnavailable,
            GatewayError::RequestTimeout,
            GatewayError::CacheError(CacheError::ConnectionError("test".to_string())),
            GatewayError::ConfigError(ConfigError::LoadError("test".to_string())),
            GatewayError::LoadBalancerError(LoadBalancerError::NoBackendAvailable),
            GatewayError::AuthError(AuthError::InvalidToken),
            GatewayError::InternalError("test".to_string()),
            GatewayError::InvalidRequest("test".to_string()),
            GatewayError::RouteNotFound("test".to_string()),
            GatewayError::IoError(io::Error::new(io::ErrorKind::Other, "test")),
            GatewayError::SerializationError("test".to_string()),
        ];

        for error in test_errors {
            let status_code = error.status_code();
            assert!(status_code >= 400 && status_code < 600, 
                "Status code {} should be a valid HTTP error code", status_code);
        }
    }
}