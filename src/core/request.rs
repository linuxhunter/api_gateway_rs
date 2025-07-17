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
}

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
}