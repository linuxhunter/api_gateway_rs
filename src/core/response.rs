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

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::{StatusCode, HeaderMap, header::HeaderValue};
    use bytes::Bytes;

    #[test]
    fn test_cache_info_creation() {
        let cache_info = CacheInfo {
            cache_hit: true,
            ttl_seconds: Some(300),
            cache_key: "test-key".to_string(),
        };

        assert!(cache_info.cache_hit);
        assert_eq!(cache_info.ttl_seconds, Some(300));
        assert_eq!(cache_info.cache_key, "test-key");
    }

    #[test]
    fn test_gateway_response_new() {
        let status = StatusCode::OK;
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        let body = Bytes::from(r#"{"message": "success"}"#);

        let response = GatewayResponse::new(status, headers.clone(), body.clone());

        assert_eq!(response.status, status);
        assert_eq!(response.headers, headers);
        assert_eq!(response.body, body);
        assert!(response.cache_info.is_none());
        assert!(response.backend_name.is_none());
        assert_eq!(response.processing_time_ms, 0);
    }

    #[test]
    fn test_gateway_response_error() {
        let response = GatewayResponse::error(StatusCode::BAD_REQUEST, "Invalid input");

        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers.get("content-type").unwrap(),
            "application/json"
        );
        
        let body_str = String::from_utf8(response.body.to_vec()).unwrap();
        assert!(body_str.contains("Invalid input"));
        assert!(body_str.contains("error"));
    }

    #[test]
    fn test_gateway_response_with_cache_info() {
        let cache_info = CacheInfo {
            cache_hit: true,
            ttl_seconds: Some(600),
            cache_key: "cache-key-123".to_string(),
        };

        let response = GatewayResponse::new(
            StatusCode::OK,
            HeaderMap::new(),
            Bytes::from("cached data"),
        ).with_cache_info(cache_info.clone());

        assert!(response.cache_info.is_some());
        let response_cache_info = response.cache_info.unwrap();
        assert_eq!(response_cache_info.cache_hit, cache_info.cache_hit);
        assert_eq!(response_cache_info.ttl_seconds, cache_info.ttl_seconds);
        assert_eq!(response_cache_info.cache_key, cache_info.cache_key);
    }

    #[test]
    fn test_gateway_response_with_backend_name() {
        let response = GatewayResponse::new(
            StatusCode::OK,
            HeaderMap::new(),
            Bytes::from("response data"),
        ).with_backend_name("backend-server-1".to_string());

        assert!(response.backend_name.is_some());
        assert_eq!(response.backend_name.unwrap(), "backend-server-1");
    }

    #[test]
    fn test_gateway_response_with_processing_time() {
        let response = GatewayResponse::new(
            StatusCode::OK,
            HeaderMap::new(),
            Bytes::from("response data"),
        ).with_processing_time(150);

        assert_eq!(response.processing_time_ms, 150);
    }
}