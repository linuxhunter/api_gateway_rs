use bytes::Bytes;
use hyper::{HeaderMap, StatusCode};

/// Cache information for a response
#[derive(Debug, Clone)]
pub struct CacheInfo {
    /// Whether the response was served from cache
    pub cache_hit: bool,
    
    /// Time-to-live for the cached response
    pub ttl_seconds: Option<u64>,
    
    /// Cache key used to store/retrieve the response
    pub cache_key: String,
}

/// Represents a response from the API Gateway
#[derive(Debug, Clone)]
pub struct GatewayResponse {
    /// HTTP status code
    pub status: StatusCode,
    
    /// HTTP headers
    pub headers: HeaderMap,
    
    /// Response body
    pub body: Bytes,
    
    /// Cache information (if applicable)
    pub cache_info: Option<CacheInfo>,
    
    /// Backend service that processed the request (if applicable)
    pub backend_name: Option<String>,
    
    /// Time taken to process the request in milliseconds
    pub processing_time_ms: u64,
}

impl GatewayResponse {
    /// Create a new GatewayResponse
    pub fn new(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body,
            cache_info: None,
            backend_name: None,
            processing_time_ms: 0,
        }
    }
    
    /// Create a new error response
    pub fn error(status: StatusCode, message: &str) -> Self {
        let body = format!("{{\"error\":\"{}\"}}",
            message.replace('\"', "\\\"")
        );
        
        let mut headers = HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        
        Self::new(status, headers, Bytes::from(body))
    }
    
    /// Set cache information for this response
    pub fn with_cache_info(mut self, cache_info: CacheInfo) -> Self {
        self.cache_info = Some(cache_info);
        self
    }
    
    /// Set backend name for this response
    pub fn with_backend_name(mut self, backend_name: String) -> Self {
        self.backend_name = Some(backend_name);
        self
    }
    
    /// Set processing time for this response
    pub fn with_processing_time(mut self, processing_time_ms: u64) -> Self {
        self.processing_time_ms = processing_time_ms;
        self
    }
}