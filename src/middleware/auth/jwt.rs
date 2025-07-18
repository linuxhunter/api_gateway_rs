use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use jwt::{AlgorithmType, Header, SignWithKey, Token, VerifyWithKey};
use rand::{thread_rng, Rng};
use rsa::{pkcs1::DecodeRsaPrivateKey, pkcs8::DecodePublicKey, RsaPrivateKey, RsaPublicKey};
use sha2::Sha256;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::error::AuthError;
use crate::middleware::auth::models::{AuthConfig, Claims, TokenPair};
use crate::middleware::auth::AuthService;

/// JWT-based authentication service
pub struct JwtAuthService {
    config: AuthConfig,
    refresh_tokens: Arc<RwLock<Vec<RefreshToken>>>,
}

/// Refresh token with metadata
#[derive(Debug, Clone)]
struct RefreshToken {
    token: String,
    user_id: String,
    expires_at: SystemTime,
    token_family: String,
}

/// Supported signing algorithms
#[derive(Debug, Clone)]
enum SigningAlgorithm {
    /// HMAC with SHA-256
    HS256(Hmac<Sha256>),
    /// RSA with SHA-256
    RS256 {
        private_key: Option<RsaPrivateKey>,
        public_key: Option<RsaPublicKey>,
    },
}

impl JwtAuthService {
    /// Create a new JWT authentication service with the specified configuration
    pub fn new(config: AuthConfig) -> Result<Self, AuthError> {
        // Validate configuration
        if config.secret_key.is_none() && (config.private_key.is_none() || config.public_key.is_none()) {
            return Err(AuthError::ServiceUnavailable(
                "Either secret key or private/public key pair must be configured".to_string(),
            ));
        }

        info!("Initializing JWT authentication service with issuer: {}", config.issuer);
        
        Ok(Self {
            config,
            refresh_tokens: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    /// Initialize the signing algorithm based on configuration
    fn init_algorithm(&self) -> Result<SigningAlgorithm, AuthError> {
        if let Some(secret_key) = &self.config.secret_key {
            debug!("Using HMAC-SHA256 signing algorithm");
            // Create HMAC-SHA256 key for signing/verification
            let key = Hmac::new_from_slice(secret_key.as_bytes())
                .map_err(|e| {
                    error!("Failed to create HMAC key: {}", e);
                    AuthError::ServiceUnavailable("Failed to create HMAC key".to_string())
                })?;
            
            Ok(SigningAlgorithm::HS256(key))
        } else if let (Some(private_key_pem), Some(public_key_pem)) = 
            (&self.config.private_key, &self.config.public_key) {
            debug!("Using RSA-SHA256 signing algorithm");
            
            // Parse private key if available
            let private_key = if !private_key_pem.is_empty() {
                Some(RsaPrivateKey::from_pkcs1_pem(private_key_pem).map_err(|e| {
                    error!("Failed to parse RSA private key: {}", e);
                    AuthError::ServiceUnavailable("Failed to parse RSA private key".to_string())
                })?)
            } else {
                None
            };
            
            // Parse public key
            let public_key = RsaPublicKey::from_public_key_pem(public_key_pem).map_err(|e| {
                error!("Failed to parse RSA public key: {}", e);
                AuthError::ServiceUnavailable("Failed to parse RSA public key".to_string())
            })?;
            
            Ok(SigningAlgorithm::RS256 {
                private_key,
                public_key: Some(public_key),
            })
        } else {
            error!("No valid signing algorithm configuration found");
            Err(AuthError::ServiceUnavailable("No valid signing algorithm configuration".to_string()))
        }
    }
    
    /// Generate a random refresh token
    fn generate_refresh_token(&self, user_id: &str) -> RefreshToken {
        // Generate a random token
        let token: String = thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        
        // Generate a random token family ID
        let token_family: String = thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        
        let expires_at = SystemTime::now() + Duration::from_secs(self.config.refresh_token_expiration_seconds);
        
        debug!("Generated refresh token for user {} with expiration in {} seconds", 
               user_id, self.config.refresh_token_expiration_seconds);
        
        RefreshToken {
            token,
            user_id: user_id.to_string(),
            expires_at,
            token_family,
        }
    }
    
    /// Clean expired refresh tokens
    async fn clean_expired_tokens(&self) {
        let mut tokens = self.refresh_tokens.write().await;
        let now = SystemTime::now();
        let initial_count = tokens.len();
        
        tokens.retain(|token| token.expires_at > now);
        
        let removed = initial_count - tokens.len();
        if removed > 0 {
            debug!("Cleaned {} expired refresh tokens", removed);
        }
    }
    
    /// Sign claims with the configured algorithm
    fn sign_claims(&self, claims: &Claims) -> Result<String, AuthError> {
        match self.init_algorithm()? {
            SigningAlgorithm::HS256(key) => {
                claims.sign_with_key(&key)
                    .map_err(|e| {
                        error!("Failed to sign token with HMAC: {}", e);
                        AuthError::ServiceUnavailable("Failed to sign token".to_string())
                    })
            },
            SigningAlgorithm::RS256 { private_key, .. } => {
                if let Some(private_key) = private_key {
                    // For RSA, we need to manually create the JWT
                    let header = Header {
                        algorithm: AlgorithmType::Rs256,
                        ..Default::default()
                    };
                    
                    // Create the JWT token
                    let token = Token::new(header, claims.clone());
                    
                    // Sign the token
                    // Note: This is a simplified example. In a real implementation,
                    // you would use a proper RSA signing library like jsonwebtoken
                    // or jwt-simple that supports RSA signing.
                    
                    // For now, we'll return an error since the jwt crate doesn't support RSA directly
                    error!("RSA signing not fully implemented");
                    Err(AuthError::ServiceUnavailable("RSA signing not fully implemented".to_string()))
                } else {
                    error!("Private key required for signing but not provided");
                    Err(AuthError::ServiceUnavailable("Private key required for signing".to_string()))
                }
            }
        }
    }
    
    /// Verify token signature and decode claims
    fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        match self.init_algorithm()? {
            SigningAlgorithm::HS256(key) => {
                token.verify_with_key(&key)
                    .map_err(|e| {
                        warn!("Failed to verify token with HMAC: {}", e);
                        AuthError::InvalidToken
                    })
            },
            SigningAlgorithm::RS256 { public_key, .. } => {
                if let Some(public_key) = public_key {
                    // For RSA, we need to manually verify the JWT
                    // Note: This is a simplified example. In a real implementation,
                    // you would use a proper RSA verification library like jsonwebtoken
                    // or jwt-simple that supports RSA verification.
                    
                    // For now, we'll return an error since the jwt crate doesn't support RSA directly
                    error!("RSA verification not fully implemented");
                    Err(AuthError::ServiceUnavailable("RSA verification not fully implemented".to_string()))
                } else {
                    error!("Public key required for verification but not provided");
                    Err(AuthError::ServiceUnavailable("Public key required for verification".to_string()))
                }
            }
        }
    }
}

#[async_trait]
impl AuthService for JwtAuthService {
    async fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        // Verify and decode the token
        let claims = self.verify_token(token)?;
        
        // Check if token is expired
        if claims.is_expired() {
            warn!("Token expired for user: {}", claims.sub);
            return Err(AuthError::TokenExpired);
        }
        
        // Check if token is not yet valid
        if claims.is_not_valid_yet() {
            warn!("Token not yet valid for user: {}", claims.sub);
            return Err(AuthError::InvalidToken);
        }
        
        // Check issuer if configured
        if let Some(iss) = &claims.iss {
            if iss != &self.config.issuer {
                warn!("Token issuer mismatch: expected {}, got {}", self.config.issuer, iss);
                return Err(AuthError::InvalidToken);
            }
        }
        
        // Check audience if configured
        if let (Some(aud), Some(expected_aud)) = (&claims.aud, &self.config.audience) {
            if aud != expected_aud {
                warn!("Token audience mismatch: expected {}, got {}", expected_aud, aud);
                return Err(AuthError::InvalidToken);
            }
        }
        
        debug!("Token validated successfully for user: {}", claims.sub);
        Ok(claims)
    }
    
    async fn generate_tokens(&self, mut claims: Claims) -> Result<TokenPair, AuthError> {
        // Set issuer if not already set
        if claims.iss.is_none() {
            claims.iss = Some(self.config.issuer.clone());
        }
        
        // Set audience if configured and not already set
        if claims.aud.is_none() && self.config.audience.is_some() {
            claims.aud = self.config.audience.clone();
        }
        
        // Set expiration time if not already set
        if claims.exp.is_none() {
            let exp = SystemTime::now()
                .checked_add(Duration::from_secs(self.config.token_expiration_seconds))
                .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());
            
            claims.exp = exp;
        }
        
        // Set issued at time if not already set
        if claims.iat.is_none() {
            let iat = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs());
            
            claims.iat = iat;
        }
        
        // Sign the claims to create a JWT
        let access_token = self.sign_claims(&claims)?;
        
        // Generate a refresh token
        let refresh_token_data = self.generate_refresh_token(&claims.sub);
        let refresh_token = refresh_token_data.token.clone();
        
        // Store the refresh token
        {
            // Clean expired tokens first
            self.clean_expired_tokens().await;
            
            // Add the new token
            let mut tokens = self.refresh_tokens.write().await;
            tokens.push(refresh_token_data);
        }
        
        info!("Generated new token pair for user: {}", claims.sub);
        
        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.config.token_expiration_seconds,
            token_type: "Bearer".to_string(),
        })
    }
    
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        // Clean expired tokens first
        self.clean_expired_tokens().await;
        
        // Find the refresh token
        let (user_id, token_family) = {
            let tokens = self.refresh_tokens.read().await;
            let token = tokens.iter().find(|t| t.token == refresh_token);
            
            match token {
                Some(token) => (token.user_id.clone(), token.token_family.clone()),
                None => {
                    warn!("Refresh token not found or expired");
                    return Err(AuthError::InvalidToken);
                }
            }
        };
        
        // Remove the old refresh token
        {
            let mut tokens = self.refresh_tokens.write().await;
            tokens.retain(|t| t.token != refresh_token);
        }
        
        // Generate new tokens
        let mut claims = Claims::new(user_id);
        
        // Add token family to custom claims for token rotation tracking
        claims = claims.with_claim("tfm", token_family);
        
        debug!("Refreshing tokens for user with new token pair");
        self.generate_tokens(claims).await
    }
    
    async fn revoke_token(&self, token: &str) -> Result<(), AuthError> {
        let mut tokens = self.refresh_tokens.write().await;
        let initial_len = tokens.len();
        
        // Find the token family if it exists
        let token_family = tokens.iter()
            .find(|t| t.token == token)
            .map(|t| t.token_family.clone());
        
        // If we found a token family, revoke all tokens in that family (for security)
        if let Some(family) = token_family {
            tokens.retain(|t| t.token_family != family);
            info!("Revoked all tokens in family for security");
        } else {
            // Otherwise just remove the specific token
            tokens.retain(|t| t.token != token);
        }
        
        if tokens.len() < initial_len {
            debug!("Successfully revoked token");
            Ok(())
        } else {
            warn!("Token not found for revocation");
            Err(AuthError::InvalidToken)
        }
    }
    
    fn get_config(&self) -> &AuthConfig {
        &self.config
    }
}