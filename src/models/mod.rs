// Models for the API Gateway

/// Backend service definition
#[derive(Debug, Clone)]
pub struct Backend {
    /// Backend ID
    pub id: String,

    /// Backend URL
    pub url: String,

    /// Backend weight for load balancing
    pub weight: u32,

    /// Whether the backend is healthy
    pub healthy: bool,

    /// Health check path
    pub health_check_path: String,

    /// Backend timeout in seconds
    pub timeout_seconds: u64,

    /// Backend host
    pub host: String,

    /// Backend port
    pub port: u16,

    /// Backend tags for service discovery
    pub tags: Vec<String>,

    /// Backend metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Authentication token claims
#[derive(Debug, Clone)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,

    /// Issuer
    pub iss: Option<String>,

    /// Audience
    pub aud: Option<String>,

    /// Expiration time
    pub exp: u64,

    /// Issued at
    pub iat: u64,

    /// Roles or permissions
    pub roles: Vec<String>,
}

/// Token pair for authentication
#[derive(Debug, Clone)]
pub struct TokenPair {
    /// Access token
    pub access_token: String,

    /// Refresh token
    pub refresh_token: String,

    /// Access token expiration time
    pub expires_in: u64,
}
