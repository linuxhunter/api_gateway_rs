use std::collections::HashMap;
use std::time::{Duration, SystemTime};

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
            jti: None,
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
    
    /// Add a custom claim
    pub fn with_claim<T: Serialize>(mut self, name: impl Into<String>, value: T) -> Self {
        if let Ok(value) = serde_json::to_value(value) {
            self.custom.insert(name.into(), value);
        }
        self
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

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT issuer
    pub issuer: String,
    
    /// JWT audience
    pub audience: Option<String>,
    
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
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            issuer: "api-gateway".to_string(),
            audience: None,
            secret_key: None,
            public_key: None,
            private_key: None,
            token_expiration_seconds: 3600, // 1 hour
            refresh_token_expiration_seconds: 86400 * 7, // 7 days
        }
    }
}