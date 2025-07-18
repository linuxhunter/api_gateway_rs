use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use rand::{thread_rng, Rng};
use sha2::Sha256;
use tokio::sync::RwLock;

use crate::error::AuthError;
use crate::middleware::auth::models::{AuthConfig, Claims, TokenPair};
use crate::middleware::auth::AuthService;

/// JWT-based authentication service
pub struct JwtAuthService {
    config: AuthConfig,
    refresh_tokens: Arc<RwLock<Vec<RefreshToken>>>,
}

/// Refresh token with metadata
struct RefreshToken {
    token: String,
    user_id: String,
    expires_at: SystemTime,
}

impl JwtAuthService {
    /// Create a new JWT authentication service with the specified configuration
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            refresh_tokens: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Generate a random refresh token
    fn generate_refresh_token(&self, user_id: &str) -> RefreshToken {
        let token: String = thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        
        let expires_at = SystemTime::now() + Duration::from_secs(self.config.refresh_token_expiration_seconds);
        
        RefreshToken {
            token,
            user_id: user_id.to_string(),
            expires_at,
        }
    }
    
    /// Clean expired refresh tokens
    async fn clean_expired_tokens(&self) {
        let mut tokens = self.refresh_tokens.write().await;
        let now = SystemTime::now();
        tokens.retain(|token| token.expires_at > now);
    }
}

#[async_trait]
impl AuthService for JwtAuthService {
    async fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        // Get the secret key from config
        let secret_key = self.config.secret_key.as_ref()
            .ok_or_else(|| AuthError::ServiceUnavailable("No secret key configured".to_string()))?;
        
        // Create HMAC-SHA256 key for verification
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret_key.as_bytes())
            .map_err(|_| AuthError::ServiceUnavailable("Failed to create key".to_string()))?;
        
        // Verify and decode the token
        let claims: Claims = token.verify_with_key(&key)
            .map_err(|_| AuthError::InvalidToken)?;
        
        // Check if token is expired
        if claims.is_expired() {
            return Err(AuthError::TokenExpired);
        }
        
        // Check if token is not yet valid
        if claims.is_not_valid_yet() {
            return Err(AuthError::InvalidToken);
        }
        
        // Check issuer if configured
        if let Some(iss) = &claims.iss {
            if iss != &self.config.issuer {
                return Err(AuthError::InvalidToken);
            }
        }
        
        // Check audience if configured
        if let (Some(aud), Some(expected_aud)) = (&claims.aud, &self.config.audience) {
            if aud != expected_aud {
                return Err(AuthError::InvalidToken);
            }
        }
        
        Ok(claims)
    }
    
    async fn generate_tokens(&self, mut claims: Claims) -> Result<TokenPair, AuthError> {
        // Get the secret key from config
        let secret_key = self.config.secret_key.as_ref()
            .ok_or_else(|| AuthError::ServiceUnavailable("No secret key configured".to_string()))?;
        
        // Create HMAC-SHA256 key for signing
        let key: Hmac<Sha256> = Hmac::new_from_slice(secret_key.as_bytes())
            .map_err(|_| AuthError::ServiceUnavailable("Failed to create key".to_string()))?;
        
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
        
        // Sign the claims to create a JWT
        let access_token = claims.sign_with_key(&key)
            .map_err(|_| AuthError::ServiceUnavailable("Failed to sign token".to_string()))?;
        
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
        let user_id = {
            let tokens = self.refresh_tokens.read().await;
            let token = tokens.iter().find(|t| t.token == refresh_token);
            
            match token {
                Some(token) => token.user_id.clone(),
                None => return Err(AuthError::InvalidToken),
            }
        };
        
        // Remove the old refresh token
        {
            let mut tokens = self.refresh_tokens.write().await;
            tokens.retain(|t| t.token != refresh_token);
        }
        
        // Generate new tokens
        let claims = Claims::new(user_id);
        self.generate_tokens(claims).await
    }
    
    async fn revoke_token(&self, token: &str) -> Result<(), AuthError> {
        let mut tokens = self.refresh_tokens.write().await;
        let initial_len = tokens.len();
        tokens.retain(|t| t.token != token);
        
        if tokens.len() < initial_len {
            Ok(())
        } else {
            Err(AuthError::InvalidToken)
        }
    }
    
    fn get_config(&self) -> &AuthConfig {
        &self.config
    }
}