use std::net::IpAddr;
use std::time::SystemTime;

use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri};

/// Represents a request to the API Gateway
#[derive(Debug, Clone)]
pub struct GatewayRequest {
    /// HTTP method
    pub method: Method,
    
    /// Request URI
    pub uri: Uri,
    
    /// HTTP headers
    pub headers: HeaderMap,
    
    /// Request body
    pub body: Bytes,
    
    /// Client IP address
    pub client_ip: Option<IpAddr>,
    
    /// Request timestamp
    pub timestamp: SystemTime,
    
    /// Request ID for tracing
    pub request_id: String,
}

impl GatewayRequest {
    /// Create a new GatewayRequest
    pub fn new(
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: Bytes,
        client_ip: Option<IpAddr>,
    ) -> Self {
        Self {
            method,
            uri,
            headers,
            body,
            client_ip,
            timestamp: SystemTime::now(),
            request_id: generate_request_id(),
        }
    }
    
    /// Get a header value as a string
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }
    
    /// Set the backend for this request
    pub fn set_backend(&mut self, backend: &crate::models::Backend) {
        // Add backend information to headers for downstream middleware
        if let Ok(value) = hyper::header::HeaderValue::from_str(&backend.id) {
            self.headers.insert("X-Backend-ID", value);
        }
        
        if let Ok(value) = hyper::header::HeaderValue::from_str(&backend.host) {
            self.headers.insert("X-Backend-Host", value);
        }
        
        if let Ok(value) = hyper::header::HeaderValue::from_str(&backend.port.to_string()) {
            self.headers.insert("X-Backend-Port", value);
        }
    }
}

// GatewayRequest already derives Clone

/// Generate a unique request ID
fn generate_request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    
    format!("{:x}-{:x}", timestamp, counter)
}#[cfg
(test)]
mod tests {
    use super::*;
    use hyper::{Method, Uri, HeaderMap, header::HeaderValue};
    use bytes::Bytes;
    use std::net::{IpAddr, Ipv4Addr};
    use std::collections::HashMap;
    use crate::models::Backend;

    #[test]
    fn test_gateway_request_new() {
        let method = Method::GET;
        let uri = Uri::from_static("http://example.com/test");
        let headers = HeaderMap::new();
        let body = Bytes::from("test body");
        let client_ip = Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));

        let request = GatewayRequest::new(method.clone(), uri.clone(), headers.clone(), body.clone(), client_ip);

        assert_eq!(request.method, method);
        assert_eq!(request.uri, uri);
        assert_eq!(request.headers, headers);
        assert_eq!(request.body, body);
        assert_eq!(request.client_ip, client_ip);
        assert!(!request.request_id.is_empty());
    }

    #[test]
    fn test_gateway_request_header() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert("authorization", HeaderValue::from_static("Bearer token123"));

        let request = GatewayRequest::new(
            Method::POST,
            Uri::from_static("http://example.com/api"),
            headers,
            Bytes::from("{}"),
            None,
        );

        assert_eq!(request.header("content-type"), Some("application/json".to_string()));
        assert_eq!(request.header("authorization"), Some("Bearer token123".to_string()));
        assert_eq!(request.header("non-existent"), None);
    }

    #[test]
    fn test_gateway_request_set_backend() {
        let mut request = GatewayRequest::new(
            Method::GET,
            Uri::from_static("http://example.com/test"),
            HeaderMap::new(),
            Bytes::from("test"),
            None,
        );

        let backend = Backend {
            id: "backend-1".to_string(),
            url: "http://localhost:8080".to_string(),
            weight: 1,
            healthy: true,
            health_check_path: "/health".to_string(),
            timeout_seconds: 30,
            host: "localhost".to_string(),
            port: 8080,
            tags: vec!["api".to_string()],
            metadata: HashMap::new(),
        };

        request.set_backend(&backend);

        assert_eq!(request.header("X-Backend-ID"), Some("backend-1".to_string()));
        assert_eq!(request.header("X-Backend-Host"), Some("localhost".to_string()));
        assert_eq!(request.header("X-Backend-Port"), Some("8080".to_string()));
    }

    #[test]
    fn test_gateway_request_clone() {
        let request = GatewayRequest::new(
            Method::POST,
            Uri::from_static("http://example.com/api"),
            HeaderMap::new(),
            Bytes::from("test data"),
            Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
        );

        let cloned_request = request.clone();

        assert_eq!(request.method, cloned_request.method);
        assert_eq!(request.uri, cloned_request.uri);
        assert_eq!(request.headers, cloned_request.headers);
        assert_eq!(request.body, cloned_request.body);
        assert_eq!(request.client_ip, cloned_request.client_ip);
        assert_eq!(request.request_id, cloned_request.request_id);
    }
}