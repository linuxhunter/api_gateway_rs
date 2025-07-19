use std::time::{Duration, SystemTime};

use bytes::Bytes;
use hyper::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use crate::core::response::GatewayResponse;

/// Represents a cached response with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// The HTTP status code
    pub status: u16,

    /// The HTTP headers (serialized as Vec of tuples)
    pub headers: Vec<(String, String)>,

    /// The response body
    pub body: Vec<u8>,

    /// When the response was cached
    pub created_at: u64, // Unix timestamp in seconds

    /// Time-to-live in seconds
    pub ttl: u64,

    /// Cache key used to store this response
    pub cache_key: String,

    /// Content type of the response
    pub content_type: Option<String>,
}

impl CachedResponse {
    /// Create a new CachedResponse from a GatewayResponse
    pub fn from_gateway_response(
        response: &GatewayResponse,
        cache_key: String,
        ttl: Duration,
    ) -> Self {
        // Convert headers to a serializable format
        let headers = response
            .headers
            .iter()
            .filter_map(|(name, value)| {
                let name_str = name.as_str().to_string();
                let value_str = value.to_str().ok()?.to_string();
                Some((name_str, value_str))
            })
            .collect();

        // Extract content type
        let content_type = response
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            status: response.status.as_u16(),
            headers,
            body: response.body.to_vec(),
            created_at: now,
            ttl: ttl.as_secs(),
            cache_key,
            content_type,
        }
    }

    /// Convert back to a GatewayResponse
    pub fn to_gateway_response(&self) -> GatewayResponse {
        // Convert headers back to HeaderMap
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            if let Ok(header_name) = name.parse::<hyper::header::HeaderName>() {
                if let Ok(header_value) = value.parse::<hyper::header::HeaderValue>() {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // Create response
        let mut response = GatewayResponse::new(
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK),
            headers,
            Bytes::from(self.body.clone()),
        );

        // Add cache info
        let cache_info = crate::core::response::CacheInfo {
            cache_hit: true,
            ttl_seconds: Some(self.remaining_ttl()),
            cache_key: self.cache_key.clone(),
        };

        response.cache_info = Some(cache_info);
        response
    }

    /// Check if the cached response has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now > self.created_at + self.ttl
    }

    /// Get the remaining TTL in seconds
    pub fn remaining_ttl(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now > self.created_at + self.ttl {
            0
        } else {
            self.created_at + self.ttl - now
        }
    }
}

/// Cache key generation options
#[derive(Debug, Clone)]
pub struct CacheKeyOptions {
    /// Include query parameters in the cache key
    pub include_query: bool,

    /// Include specific headers in the cache key
    pub include_headers: Vec<String>,

    /// Include the HTTP method in the cache key
    pub include_method: bool,

    /// Custom prefix for the cache key
    pub prefix: Option<String>,

    /// Normalize the URL path (remove trailing slashes, etc.)
    pub normalize_path: bool,
}

impl Default for CacheKeyOptions {
    fn default() -> Self {
        Self {
            include_query: true,
            include_headers: vec![],
            include_method: true,
            prefix: None,
            normalize_path: true,
        }
    }
}

/// Cache policy for determining what and how to cache
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Default TTL for cached responses
    pub default_ttl: Duration,

    /// Maximum TTL for cached responses
    pub max_ttl: Duration,

    /// HTTP methods that can be cached
    pub cacheable_methods: Vec<hyper::Method>,

    /// HTTP status codes that can be cached
    pub cacheable_status_codes: Vec<StatusCode>,

    /// Headers that control caching behavior
    pub respect_cache_control: bool,

    /// Cache key generation options
    pub key_options: CacheKeyOptions,

    /// Whether to cache responses with authorization headers
    pub cache_authenticated: bool,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(60),
            max_ttl: Duration::from_secs(3600),
            cacheable_methods: vec![hyper::Method::GET, hyper::Method::HEAD],
            cacheable_status_codes: vec![
                StatusCode::OK,
                StatusCode::CREATED,
                StatusCode::NOT_FOUND,
                StatusCode::MOVED_PERMANENTLY,
                StatusCode::PERMANENT_REDIRECT,
            ],
            respect_cache_control: true,
            key_options: CacheKeyOptions::default(),
            cache_authenticated: false,
        }
    }
}

impl CachePolicy {
    /// Check if a request is cacheable based on this policy
    pub fn is_request_cacheable(&self, request: &crate::core::request::GatewayRequest) -> bool {
        // Check if method is cacheable
        if !self.cacheable_methods.contains(&request.method) {
            return false;
        }

        // Don't cache authenticated requests if policy disallows it
        if !self.cache_authenticated {
            if request.headers.contains_key(hyper::header::AUTHORIZATION) {
                return false;
            }
        }

        true
    }

    /// Check if a response is cacheable based on this policy
    pub fn is_response_cacheable(&self, response: &GatewayResponse) -> bool {
        // Check if status code is cacheable
        if !self.cacheable_status_codes.contains(&response.status) {
            return false;
        }

        // Check cache control headers if enabled
        if self.respect_cache_control {
            if let Some(cache_control) = response.headers.get(hyper::header::CACHE_CONTROL) {
                if let Ok(value) = cache_control.to_str() {
                    if value.contains("no-store") || value.contains("private") {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get TTL for a response based on this policy and response headers
    pub fn get_ttl_for_response(&self, response: &GatewayResponse) -> Duration {
        // Start with default TTL
        let mut ttl = self.default_ttl;

        // Check for Cache-Control max-age directive
        if self.respect_cache_control {
            if let Some(cache_control) = response.headers.get(hyper::header::CACHE_CONTROL) {
                if let Ok(value) = cache_control.to_str() {
                    // Parse max-age directive
                    if let Some(max_age_str) = value
                        .split(',')
                        .map(str::trim)
                        .find(|s| s.starts_with("max-age="))
                    {
                        if let Some(max_age) = max_age_str
                            .strip_prefix("max-age=")
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            ttl = Duration::from_secs(max_age);
                        }
                    }
                }
            }
        }

        // Check for Expires header
        if self.respect_cache_control {
            if let Some(expires) = response.headers.get(hyper::header::EXPIRES) {
                if let Ok(expires_str) = expires.to_str() {
                    // Try to parse the expires date
                    if let Ok(expires_time) = httpdate::parse_http_date(expires_str) {
                        let now = SystemTime::now();
                        if expires_time > now {
                            if let Ok(duration) = expires_time.duration_since(now) {
                                ttl = duration;
                            }
                        } else {
                            // Expired already, use minimal TTL
                            ttl = Duration::from_secs(1);
                        }
                    }
                }
            }
        }

        // Ensure TTL doesn't exceed max_ttl
        if ttl > self.max_ttl {
            ttl = self.max_ttl;
        }

        ttl
    }
}
