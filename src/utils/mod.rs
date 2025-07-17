use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique ID
pub fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    
    format!("{:x}-{:x}", timestamp, counter)
}

/// Convert a duration to milliseconds
pub fn duration_to_millis(duration: std::time::Duration) -> u64 {
    duration.as_secs() * 1000 + u64::from(duration.subsec_millis())
}

/// Parse a header value to string
pub fn parse_header_to_string(value: &hyper::header::HeaderValue) -> Option<String> {
    value.to_str().ok().map(|s| s.to_string())
}

/// Check if a request is cacheable
pub fn is_cacheable(method: &hyper::Method) -> bool {
    matches!(method, &hyper::Method::GET | &hyper::Method::HEAD)
}