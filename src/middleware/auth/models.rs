use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

/// Claims contained in a JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    
    /// Issuer
    pub iss: Option<String>,
    
    /// Audience
    pub aud: Option<String>,
    
    /// Expiration time (as Unix timestamp)
    pub exp: Option<u64>,
    
    /// Issued at (as Unix timestamp)
    pub iat: Option<u64>,
    
    /// Not before (as Unix timestamp)
    pub nbf: Option<u64>,
    
    /// JWT ID
    pub jti: Option<String>,
    
    /// Custom claims
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

impl Claims {
    /// Create new claims for a subject (user ID)
    pub fn new(subject: impl Into<String>) -> Self {
        // Generate a random JWT ID
        let jti: String = thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
            
        Self {
            sub: subject.into(),
            iss: None,
            aud: None,
            exp: None,
            iat: Some(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            nbf: None,
            jti: Some(jti),
            custom: HashMap::new(),
        }
    }
    
    /// Set expiration time
    pub fn with_expiration(mut self, duration: Duration) -> Self {
        let exp = SystemTime::now()
            .checked_add(duration)
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());
        
        self.exp = exp;
        self
    }
    
    /// Set issuer
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.iss = Some(issuer.into());
        self
    }
    
    /// Set audience
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.aud = Some(audience.into());
        self
    }
    
    /// Set not before time
    pub fn with_not_before(mut self, duration: Duration) -> Self {
        let nbf = SystemTime::now()
            .checked_add(duration)
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs());
        
        self.nbf = nbf;
        self
    }
    
    /// Add a custom claim
    pub fn with_claim<T: Serialize>(mut self, name: impl Into<String>, value: T) -> Self {
        if let Ok(value) = serde_json::to_value(value) {
            self.custom.insert(name.into(), value);
        }
        self
    }
    
    /// Get a custom claim value
    pub fn get_claim<T: for<'de> Deserialize<'de>>(&self, name: &str) -> Option<T> {
        self.custom.get(name)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }
    
    /// Check if the claims are expired
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.exp {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            exp < now
        } else {
            false
        }
    }
    
    /// Check if the claims are not yet valid
    pub fn is_not_valid_yet(&self) -> bool {
        if let Some(nbf) = self.nbf {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            nbf > now
        } else {
            false
        }
    }
    
    /// Get remaining validity time in seconds
    pub fn remaining_validity(&self) -> Option<u64> {
        self.exp.and_then(|exp| {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            if exp > now {
                Some(exp - now)
            } else {
                None
            }
        })
    }
}

/// Token pair containing access and refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Access token (JWT)
    pub access_token: String,
    
    /// Refresh token
    pub refresh_token: String,
    
    /// Access token expiration time in seconds
    pub expires_in: u64,
    
    /// Token type (usually "Bearer")
    pub token_type: String,
}

/// Supported JWT algorithms
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum JwtAlgorithm {
    /// HMAC with SHA-256
    HS256,
    /// RSA with SHA-256
    RS256,
}

impl Default for JwtAlgorithm {
    fn default() -> Self {
        Self::HS256
    }
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT issuer
    pub issuer: String,
    
    /// JWT audience
    pub audience: Option<String>,
    
    /// JWT algorithm to use
    #[serde(default)]
    pub algorithm: JwtAlgorithm,
    
    /// JWT signing key (for HMAC algorithms)
    pub secret_key: Option<String>,
    
    /// Public key for RSA/ECDSA verification (PEM format)
    pub public_key: Option<String>,
    
    /// Private key for RSA/ECDSA signing (PEM format)
    pub private_key: Option<String>,
    
    /// Token expiration time in seconds
    pub token_expiration_seconds: u64,
    
    /// Refresh token expiration time in seconds
    pub refresh_token_expiration_seconds: u64,
    
    /// Whether to use token rotation for refresh tokens
    #[serde(default = "default_token_rotation")]
    pub use_token_rotation: bool,
    
    /// Maximum number of refresh tokens per user
    #[serde(default = "default_max_refresh_tokens")]
    pub max_refresh_tokens_per_user: usize,
    
    /// Whether to include token ID (jti) in tokens
    #[serde(default = "default_true")]
    pub include_jti: bool,
}

fn default_token_rotation() -> bool {
    true
}

fn default_max_refresh_tokens() -> usize {
    5
}

fn default_true() -> bool {
    true
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            issuer: "api-gateway".to_string(),
            audience: None,
            algorithm: JwtAlgorithm::HS256,
            secret_key: None,
            public_key: None,
            private_key: None,
            token_expiration_seconds: 3600, // 1 hour
            refresh_token_expiration_seconds: 86400 * 7, // 7 days
            use_token_rotation: true,
            max_refresh_tokens_per_user: 5,
            include_jti: true,
        }
    }
}