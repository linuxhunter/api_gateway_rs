pub mod models;
pub mod jwt;
pub mod jwt_middleware;

use std::sync::Arc;

use async_trait::async_trait;

use crate::core::request::GatewayRequest;
use crate::core::response::GatewayResponse;
use crate::error::{AuthError, GatewayError};
use crate::middleware::{Middleware, MiddlewareHandler};
use crate::middleware::auth::models::{AuthConfig, Claims, TokenPair};

/// Authentication service interface
#[async_trait]
pub trait AuthService: Send + Sync {
    /// Validate a token and return the claims if valid
    async fn validate_token(&self, token: &str) -> Result<Claims, AuthError>;
    
    /// Generate a new token pair from claims
    async fn generate_tokens(&self, claims: Claims) -> Result<TokenPair, AuthError>;
    
    /// Refresh a token using a refresh token
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair, AuthError>;
    
    /// Revoke a token
    async fn revoke_token(&self, token: &str) -> Result<(), AuthError>;
    
    /// Get the authentication configuration
    fn get_config(&self) -> &AuthConfig;
}

/// Authentication middleware
pub struct AuthMiddleware {
    name: String,
    auth_service: Arc<dyn AuthService>,
    require_auth: bool,
}

impl AuthMiddleware {
    /// Create a new AuthMiddleware with the specified authentication service
    pub fn new(auth_service: Arc<dyn AuthService>) -> Self {
        Self {
            name: "auth".to_string(),
            auth_service,
            require_auth: true,
        }
    }
    
    /// Create a new AuthMiddleware that makes authentication optional
    pub fn optional(auth_service: Arc<dyn AuthService>) -> Self {
        Self {
            name: "auth-optional".to_string(),
            auth_service,
            require_auth: false,
        }
    }
    
    /// Extract the token from the Authorization header
    fn extract_token(&self, request: &GatewayRequest) -> Option<String> {
        request
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|auth_header| {
                if auth_header.starts_with("Bearer ") {
                    Some(auth_header[7..].to_string())
                } else {
                    None
                }
            })
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn process_request(
        &self,
        request: GatewayRequest,
        next: Arc<dyn MiddlewareHandler>,
    ) -> Result<GatewayResponse, GatewayError> {
        // Extract token from request
        let token = self.extract_token(&request);
        
        match token {
            Some(token) => {
                // Validate the token
                match self.auth_service.validate_token(&token).await {
                    Ok(claims) => {
                        // Token is valid, proceed with the request
                        // In a future implementation, we might want to add the claims to the request context
                        tracing::debug!("Authentication successful for user: {}", claims.sub);
                        next.handle(request).await
                    }
                    Err(err) => {
                        // Token validation failed
                        tracing::warn!("Authentication failed: {}", err);
                        Err(GatewayError::AuthError(err))
                    }
                }
            }
            None if !self.require_auth => {
                // Authentication is optional and no token was provided
                tracing::debug!("No authentication token provided, but authentication is optional");
                next.handle(request).await
            }
            None => {
                // Authentication is required but no token was provided
                tracing::warn!("Authentication required but no token provided");
                Err(GatewayError::AuthError(AuthError::InvalidToken))
            }
        }
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}