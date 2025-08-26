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

impl Backend {
    /// Create a new backend with the given URL
    pub fn new(url: String) -> Self {
        // Parse URL to extract host and port
        let parsed_url = url::Url::parse(&url).unwrap_or_else(|_| {
            url::Url::parse(&format!("http://{}", url)).unwrap()
        });
        
        let host = parsed_url.host_str().unwrap_or("localhost").to_string();
        let port = parsed_url.port().unwrap_or(80);
        let id = format!("{}:{}", host, port);

        Self {
            id,
            url,
            weight: 1,
            healthy: true,
            health_check_path: "/health".to_string(),
            timeout_seconds: 30,
            host,
            port,
            tags: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a new backend with custom settings
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    /// Set the health check path
    pub fn with_health_check_path(mut self, path: String) -> Self {
        self.health_check_path = path;
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
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
