use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tracing::{Level, Span};
use tracing::Instrument;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::ChronoUtc;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};
use uuid::Uuid;

use crate::core::middleware::{Middleware, MiddlewareHandler};
use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::GatewayError;

/// Middleware for logging requests and responses with structured logging
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

/// Initialize the tracing system with structured logging
pub fn init_tracing(log_level: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default to the specified log level if no RUST_LOG env var is set
        match log_level {
            "trace" => EnvFilter::new("trace"),
            "debug" => EnvFilter::new("debug"),
            "info" => EnvFilter::new("info"),
            "warn" => EnvFilter::new("warn"),
            "error" => EnvFilter::new("error"),
            _ => EnvFilter::new("info"), // Default to info
        }
    });

    // Create a formatting layer with timestamps, targets, and span events
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_timer(fmt::time::ChronoUtc::rfc_3339())
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .json();

    // Combine the layers and initialize the subscriber
    Registry::default().with(env_filter).with(fmt_layer).init();

    tracing::info!("Tracing system initialized with structured logging");
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
        // Create a span for this request with structured fields
        let request_span = tracing::span!(
            Level::INFO,
            "request",
            request_id = %request.request_id,
            method = %request.method,
            path = %request.uri.path(),
            query = ?request.uri.query(),
            client_ip = ?request.client_ip,
            user_agent = ?request.header("user-agent"),
        );

        // Enter the span for the duration of this function
        let _enter = request_span.enter();

        // Log the incoming request with structured data
        match self.log_level {
            LogLevel::Basic => {
                tracing::info!(
                    status = "received",
                    "Request received: {} {}",
                    request.method,
                    request.uri
                );
            }
            LogLevel::Detailed => {
                tracing::info!(
                    status = "received",
                    headers_count = request.headers.len(),
                    "Request received: {} {}",
                    request.method,
                    request.uri
                );

                // Log headers as structured data
                for (name, value) in request.headers.iter() {
                    if !name.as_str().to_lowercase().contains("authorization") {
                        // Don't log authorization headers in detail to avoid leaking credentials
                        tracing::debug!(header_name = %name, header_value = ?value, "Request header");
                    } else {
                        tracing::debug!(header_name = %name, "Authorization header present (value hidden)");
                    }
                }

                // Log client IP if available
                if let Some(ip) = request.client_ip {
                    tracing::debug!(client_ip = %ip, "Client IP");
                }
            }
            LogLevel::Full => {
                tracing::info!(
                    status = "received",
                    headers_count = request.headers.len(),
                    body_size = request.body.len(),
                    "Request received: {} {}",
                    request.method,
                    request.uri
                );

                // Log headers as structured data
                for (name, value) in request.headers.iter() {
                    if !name.as_str().to_lowercase().contains("authorization") {
                        // Don't log authorization headers in detail to avoid leaking credentials
                        tracing::debug!(header_name = %name, header_value = ?value, "Request header");
                    } else {
                        tracing::debug!(header_name = %name, "Authorization header present (value hidden)");
                    }
                }

                // Log client IP if available
                if let Some(ip) = request.client_ip {
                    tracing::debug!(client_ip = %ip, "Client IP");
                }

                // Log body if not empty
                if !request.body.is_empty() {
                    // Try to parse as UTF-8 string
                    match std::str::from_utf8(&request.body) {
                        Ok(body_str) => {
                            // Check if body is JSON for better formatting
                            if request
                                .header("content-type")
                                .map(|ct| ct.contains("json"))
                                .unwrap_or(false)
                            {
                                tracing::debug!(
                                    body_type = "json",
                                    body_size = request.body.len(),
                                    "Request body"
                                );
                                // Don't log the actual JSON to avoid leaking sensitive data
                            } else {
                                // For non-JSON bodies, log a limited preview
                                let preview = if body_str.len() > 100 {
                                    format!(
                                        "{}... [truncated {} bytes]",
                                        &body_str[..100],
                                        body_str.len() - 100
                                    )
                                } else {
                                    body_str.to_string()
                                };
                                tracing::debug!(body_preview = %preview, body_size = body_str.len(), "Request body");
                            }
                        }
                        Err(_) => {
                            tracing::debug!(
                                body_type = "binary",
                                body_size = request.body.len(),
                                "Binary request body"
                            );
                        }
                    }
                }
            }
        }

        // Start timing
        let start = Instant::now();

        // Create a child span for the backend processing
        let backend_span = tracing::span!(
            parent: &request_span,
            Level::INFO,
            "backend_processing",
            request_id = %request.request_id
        );

        // Process the request through the next middleware, instrumented with the backend span
        let result = next.handle(request.clone()).instrument(backend_span).await;

        // Calculate elapsed time
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        // Log the response with structured data
        match &result {
            Ok(response) => {
                let status_code = response.status.as_u16();
                let is_error = status_code >= 400;

                // Choose log level based on status code
                let log_level = if is_error { Level::WARN } else { Level::INFO };

                match self.log_level {
                    LogLevel::Basic => {
                        if is_error {
                            tracing::warn!(
                                status = "completed",
                                status_code = status_code,
                                elapsed_ms = elapsed_ms,
                                "Response: {} in {:?}",
                                status_code,
                                elapsed
                            );
                        } else {
                            tracing::info!(
                                status = "completed",
                                status_code = status_code,
                                elapsed_ms = elapsed_ms,
                                "Response: {} in {:?}",
                                status_code,
                                elapsed
                            );
                        }
                    }
                    LogLevel::Detailed => {
                        if is_error {
                            tracing::warn!(
                                status = "completed",
                                status_code = status_code,
                                elapsed_ms = elapsed_ms,
                                headers_count = response.headers.len(),
                                "Response: {} in {:?}",
                                status_code,
                                elapsed
                            );
                        } else {
                            tracing::info!(
                                status = "completed",
                                status_code = status_code,
                                elapsed_ms = elapsed_ms,
                                headers_count = response.headers.len(),
                                "Response: {} in {:?}",
                                status_code,
                                elapsed
                            );
                        }

                        // Log headers as structured data
                        for (name, value) in response.headers.iter() {
                            tracing::debug!(header_name = %name, header_value = ?value, "Response header");
                        }

                        // Log backend name if available
                        if let Some(backend) = &response.backend_name {
                            tracing::debug!(backend = %backend, "Backend used");
                        }

                        // Log cache info if available
                        if let Some(cache_info) = &response.cache_info {
                            tracing::debug!(
                                cache_hit = cache_info.cache_hit,
                                cache_ttl = ?cache_info.ttl_seconds,
                                "Cache info"
                            );
                        }
                    }
                    LogLevel::Full => {
                        if is_error {
                            tracing::warn!(
                                status = "completed",
                                status_code = status_code,
                                elapsed_ms = elapsed_ms,
                                headers_count = response.headers.len(),
                                body_size = response.body.len(),
                                "Response: {} in {:?}",
                                status_code,
                                elapsed
                            );
                        } else {
                            tracing::info!(
                                status = "completed",
                                status_code = status_code,
                                elapsed_ms = elapsed_ms,
                                headers_count = response.headers.len(),
                                body_size = response.body.len(),
                                "Response: {} in {:?}",
                                status_code,
                                elapsed
                            );
                        }

                        // Log headers as structured data
                        for (name, value) in response.headers.iter() {
                            tracing::debug!(header_name = %name, header_value = ?value, "Response header");
                        }

                        // Log backend name if available
                        if let Some(backend) = &response.backend_name {
                            tracing::debug!(backend = %backend, "Backend used");
                        }

                        // Log cache info if available
                        if let Some(cache_info) = &response.cache_info {
                            tracing::debug!(
                                cache_hit = cache_info.cache_hit,
                                cache_ttl = ?cache_info.ttl_seconds,
                                "Cache info"
                            );
                        }

                        // Log body if not empty
                        if !response.body.is_empty() {
                            // Try to parse as UTF-8 string
                            match std::str::from_utf8(&response.body) {
                                Ok(body_str) => {
                                    // Check if body is JSON for better formatting
                                    if response
                                        .headers
                                        .get("content-type")
                                        .and_then(|ct| ct.to_str().ok())
                                        .map(|ct| ct.contains("json"))
                                        .unwrap_or(false)
                                    {
                                        tracing::debug!(
                                            body_type = "json",
                                            body_size = response.body.len(),
                                            "Response body"
                                        );
                                        // Don't log the actual JSON to avoid leaking sensitive data
                                    } else {
                                        // For non-JSON bodies, log a limited preview
                                        let preview = if body_str.len() > 100 {
                                            format!(
                                                "{}... [truncated {} bytes]",
                                                &body_str[..100],
                                                body_str.len() - 100
                                            )
                                        } else {
                                            body_str.to_string()
                                        };
                                        tracing::debug!(body_preview = %preview, body_size = body_str.len(), "Response body");
                                    }
                                }
                                Err(_) => {
                                    tracing::debug!(
                                        body_type = "binary",
                                        body_size = response.body.len(),
                                        "Binary response body"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                // Log errors with structured data
                let error_type = std::any::type_name::<GatewayError>()
                    .split("::")
                    .last()
                    .unwrap_or("Unknown");

                tracing::error!(
                    status = "error",
                    error_type = error_type,
                    error_message = %error,
                    elapsed_ms = elapsed_ms,
                    "Error processing request: {} in {:?}",
                    error,
                    elapsed
                );
            }
        }

        result
    }

    fn name(&self) -> &str {
        "logging"
    }
}

/// Generate a unique trace ID for distributed tracing
pub fn generate_trace_id() -> String {
    Uuid::new_v4().to_string()
}

/// Create a span for a specific operation with common attributes
pub fn create_operation_span(operation: &str, request_id: &str) -> Span {
    tracing::span!(
        Level::INFO,
        "operation",
        operation = operation,
        request_id = request_id,
        start_time = tracing::field::Empty,
    )
}

/// Log an error with context information
pub fn log_error(error: &GatewayError, request_id: &str, context: &str) {
    let error_type = std::any::type_name::<GatewayError>()
        .split("::")
        .last()
        .unwrap_or("Unknown");

    tracing::error!(
        error_type = error_type,
        error_message = %error,
        request_id = request_id,
        context = context,
        "Error occurred: {}",
        error
    );
}

/// Log a security event
pub fn log_security_event(event_type: &str, request_id: &str, details: &str, severity: &str) {
    tracing::warn!(
        event_type = event_type,
        request_id = request_id,
        details = details,
        severity = severity,
        "Security event: {}",
        event_type
    );
}
