use crate::core::request::GatewayRequest;
use crate::error::GatewayError;
use crate::middleware::logging::{create_operation_span, log_error, log_security_event};
use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri};
use std::net::IpAddr;
use std::str::FromStr;
use tracing::Instrument;

/// Example of how to use the structured logging system
pub fn logging_examples() {
    // Create a sample request
    let request = create_sample_request();

    // Example 1: Using spans for operation tracking
    let span = create_operation_span("database_query", &request.request_id);
    let _enter = span.enter();

    // Record timing information
    span.record("start_time", tracing::field::display(chrono::Utc::now()));

    // Log information within the span
    tracing::info!(
        operation = "query_user",
        user_id = "12345",
        "Querying user information"
    );

    // Example 2: Logging errors with context
    let error = GatewayError::InternalError("Database connection failed".to_string());
    log_error(&error, &request.request_id, "While querying user database");

    // Example 3: Logging security events
    log_security_event(
        "unauthorized_access_attempt",
        &request.request_id,
        "Invalid JWT token provided",
        "medium",
    );

    // Example 4: Structured logging with additional context
    tracing::info!(
        request_id = %request.request_id,
        method = %request.method,
        path = %request.uri.path(),
        client_ip = ?request.client_ip,
        latency_ms = 42,
        status_code = 200,
        "Request processed successfully"
    );
}

/// Create a sample request for examples
fn create_sample_request() -> GatewayRequest {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", "Example/1.0".parse().unwrap());
    headers.insert("content-type", "application/json".parse().unwrap());

    GatewayRequest::new(
        Method::GET,
        Uri::from_str("https://example.com/api/users/123").unwrap(),
        headers,
        Bytes::from("{\"query\": \"example\"}"),
        Some(IpAddr::from_str("192.168.1.1").unwrap()),
    )
}

/// Example of how to use the tracing system in an async context
pub async fn async_logging_example() {
    // Create a span for the entire operation
    let operation_span = tracing::info_span!(
        "async_operation",
        operation_id = tracing::field::display(uuid::Uuid::new_v4())
    );
    
    // Enter the span for the duration of this function
    let _enter = operation_span.enter();

    // This code runs with the span active
    tracing::info!("Starting async operation");

    // Simulate some work
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Create a child span for a sub-operation
    let sub_operation_span = tracing::info_span!(
        "sub_operation",
        sub_id = "123"
    );

    // Use the .instrument() method for the sub-operation
    async {
        tracing::info!("Executing sub-operation");
        // Simulate more work
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tracing::info!("Sub-operation completed");
    }
    .instrument(sub_operation_span)
    .await;

    tracing::info!("Async operation completed");
}

