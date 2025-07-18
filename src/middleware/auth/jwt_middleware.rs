use std::sync::Arc;

use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{post, Router},
    Extension,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::error::AuthError;
use crate::middleware::auth::{AuthService, models::TokenPair};

/// Request body for token refresh
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    /// Refresh token
    pub refresh_token: String,
}

/// Request body for token revocation
#[derive(Debug, Deserialize)]
pub struct RevokeTokenRequest {
    /// Token to revoke
    pub token: String,
    
    /// Token type (access or refresh)
    #[serde(default)]
    pub token_type_hint: Option<String>,
}

/// Response for error cases
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    
    /// Error description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

/// Create JWT authentication routes
pub fn jwt_routes<T: AuthService + 'static>(auth_service: Arc<T>) -> Router {
    Router::new()
        .route("/auth/refresh", post(refresh_token))
        .route("/auth/revoke", post(revoke_token))
        .layer(Extension(auth_service))
}

/// Handle token refresh requests
async fn refresh_token(
    Extension(auth_service): Extension<Arc<dyn AuthService>>,
    Json(request): Json<RefreshTokenRequest>,
) -> Response {
    debug!("Processing token refresh request");
    
    match auth_service.refresh_token(&request.refresh_token).await {
        Ok(token_pair) => {
            info!("Token refreshed successfully");
            (StatusCode::OK, Json(token_pair)).into_response()
        }
        Err(err) => {
            let status = match err {
                AuthError::InvalidToken => {
                    warn!("Invalid refresh token");
                    StatusCode::UNAUTHORIZED
                }
                AuthError::TokenExpired => {
                    warn!("Expired refresh token");
                    StatusCode::UNAUTHORIZED
                }
                _ => {
                    error!("Token refresh error: {}", err);
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            
            let error_response = ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: Some(err.to_string()),
            };
            
            (status, Json(error_response)).into_response()
        }
    }
}

/// Handle token revocation requests
async fn revoke_token(
    Extension(auth_service): Extension<Arc<dyn AuthService>>,
    Json(request): Json<RevokeTokenRequest>,
) -> Response {
    debug!("Processing token revocation request");
    
    // For security, we always return 200 OK even if the token doesn't exist
    // This prevents token enumeration attacks
    match auth_service.revoke_token(&request.token).await {
        Ok(_) => {
            info!("Token revoked successfully");
        }
        Err(err) => {
            // Log the error but don't expose it to the client
            debug!("Token revocation error (not exposed to client): {}", err);
        }
    }
    
    StatusCode::OK.into_response()
}