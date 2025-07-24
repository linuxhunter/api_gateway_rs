use serde::{Deserialize, Serialize};

/// Configuration for the backend server
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Port to listen on
    pub port: u16,
    
    /// Server name
    pub name: String,
    
    /// Server version
    pub version: String,
    
    /// Probability of request failure (0.0 - 1.0)
    pub failure_rate: f64,
    
    /// Minimum delay in milliseconds
    pub min_delay_ms: u32,
    
    /// Maximum delay in milliseconds
    pub max_delay_ms: u32,
}

/// Health status response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthStatus {
    /// Status string (UP, DOWN, etc.)
    pub status: String,
    
    /// Server name
    pub name: String,
    
    /// Server version
    pub version: String,
    
    /// Current timestamp
    pub timestamp: String,
}

/// Echo request payload
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EchoRequest {
    /// Message to echo
    pub message: Option<String>,
    
    /// Additional data
    #[serde(default)]
    pub data: serde_json::Value,
}