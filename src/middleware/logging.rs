use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use crate::core::middleware::{Middleware, MiddlewareHandler};
use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;

/// Middleware for logging requests and responses
pub struct LoggingMiddleware {
    /// Log level for request logging
    log_level: LogLevel,
}

/// Log level for the logging middleware
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    /// Log basic information
    Basic,
    /// Log detailed information including headers
    Detailed,
    /// Log everything including request and response bodies
    Full,
}

impl LoggingMiddleware {
    /// Create a new logging middleware with the specified log level
    pub fn new(log_level: LogLevel) -> Self {
        Self { log_level }
    }
    
    /// Create a new logging middleware with basic log level
    pub fn basic() -> Self {
        Self::new(LogLevel::Basic)
    }
    
    /// Create a new logging middleware with detailed log level
    pub fn detailed() -> Self {
        Self::new(LogLevel::Detailed)
    }
    
    /// Create a new logging middleware with full log level
    pub fn full() -> Self {
        Self::new(LogLevel::Full)
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError> {
        // Log the incoming request
        match self.log_level {
            LogLevel::Basic => {
                tracing::info!(
                    "Request: {} {} (ID: {})",
                    request.method,
                    request.uri,
                    request.request_id
                );
            }
            LogLevel::Detailed => {
                tracing::info!(
                    "Request: {} {} (ID: {})",
                    request.method,
                    request.uri,
                    request.request_id
                );
                
                // Log headers
                for (name, value) in request.headers.iter() {
                    tracing::debug!("  Header: {}: {:?}", name, value);
                }
                
                // Log client IP if available
                if let Some(ip) = request.client_ip {
                    tracing::debug!("  Client IP: {}", ip);
                }
            }
            LogLevel::Full => {
                tracing::info!(
                    "Request: {} {} (ID: {})",
                    request.method,
                    request.uri,
                    request.request_id
                );
                
                // Log headers
                for (name, value) in request.headers.iter() {
                    tracing::debug!("  Header: {}: {:?}", name, value);
                }
                
                // Log client IP if available
                if let Some(ip) = request.client_ip {
                    tracing::debug!("  Client IP: {}", ip);
                }
                
                // Log body if not empty
                if !request.body.is_empty() {
                    // Try to parse as UTF-8 string
                    match std::str::from_utf8(&request.body) {
                        Ok(body_str) => {
                            tracing::debug!("  Body: {}", body_str);
                        }
                        Err(_) => {
                            tracing::debug!("  Body: [binary data, {} bytes]", request.body.len());
                        }
                    }
                }
            }
        }
        
        // Start timing
        let start = Instant::now();
        
        // Process the request through the next middleware
        let result = next.handle(request).await;
        
        // Calculate elapsed time
        let elapsed = start.elapsed();
        
        // Log the response
        match &result {
            Ok(response) => {
                match self.log_level {
                    LogLevel::Basic => {
                        tracing::info!(
                            "Response: {} ({:?})",
                            response.status.as_u16(),
                            elapsed
                        );
                    }
                    LogLevel::Detailed => {
                        tracing::info!(
                            "Response: {} ({:?})",
                            response.status.as_u16(),
                            elapsed
                        );
                        
                        // Log headers
                        for (name, value) in response.headers.iter() {
                            tracing::debug!("  Header: {}: {:?}", name, value);
                        }
                        
                        // Log backend name if available
                        if let Some(backend) = &response.backend_name {
                            tracing::debug!("  Backend: {}", backend);
                        }
                        
                        // Log cache info if available
                        if let Some(cache_info) = &response.cache_info {
                            tracing::debug!(
                                "  Cache: {} (TTL: {:?}s)",
                                if cache_info.cache_hit { "HIT" } else { "MISS" },
                                cache_info.ttl_seconds
                            );
                        }
                    }
                    LogLevel::Full => {
                        tracing::info!(
                            "Response: {} ({:?})",
                            response.status.as_u16(),
                            elapsed
                        );
                        
                        // Log headers
                        for (name, value) in response.headers.iter() {
                            tracing::debug!("  Header: {}: {:?}", name, value);
                        }
                        
                        // Log backend name if available
                        if let Some(backend) = &response.backend_name {
                            tracing::debug!("  Backend: {}", backend);
                        }
                        
                        // Log cache info if available
                        if let Some(cache_info) = &response.cache_info {
                            tracing::debug!(
                                "  Cache: {} (TTL: {:?}s)",
                                if cache_info.cache_hit { "HIT" } else { "MISS" },
                                cache_info.ttl_seconds
                            );
                        }
                        
                        // Log body if not empty
                        if !response.body.is_empty() {
                            // Try to parse as UTF-8 string
                            match std::str::from_utf8(&response.body) {
                                Ok(body_str) => {
                                    tracing::debug!("  Body: {}", body_str);
                                }
                                Err(_) => {
                                    tracing::debug!("  Body: [binary data, {} bytes]", response.body.len());
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                tracing::error!("Error: {} ({:?})", error, elapsed);
            }
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "logging"
    }
}