use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;

use crate::core::middleware::{Middleware, MiddlewareHandler};
use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;

/// Middleware for measuring request processing time
pub struct TimingMiddleware {
    /// Threshold in milliseconds for slow request logging
    slow_threshold_ms: u64,
}

impl TimingMiddleware {
    /// Create a new timing middleware with the specified threshold
    pub fn new(slow_threshold_ms: u64) -> Self {
        Self { slow_threshold_ms }
    }
    
    /// Create a new timing middleware with default threshold (500ms)
    pub fn default() -> Self {
        Self::new(500)
    }
}

#[async_trait]
impl Middleware for TimingMiddleware {
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError> {
        // Start timing
        let start = Instant::now();
        
        // Process the request through the next middleware
        let result = next.handle(request.clone()).await;
        
        // Calculate elapsed time
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        
        // Log timing information
        if elapsed_ms > self.slow_threshold_ms {
            tracing::warn!(
                "Slow request: {} {} took {}ms (threshold: {}ms)",
                request.method,
                request.uri,
                elapsed_ms,
                self.slow_threshold_ms
            );
        } else {
            tracing::debug!(
                "Request timing: {} {} took {}ms",
                request.method,
                request.uri,
                elapsed_ms
            );
        }
        
        // Add timing information to the response if successful
        if let Ok(response) = result.as_ref() {
            // Clone the response and add timing information
            let response = response.clone().with_processing_time(elapsed_ms);
            return Ok(response);
        }
        
        result
    }
    
    fn name(&self) -> &str {
        "timing"
    }
}