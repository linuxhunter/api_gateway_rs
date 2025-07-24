// API Gateway Library

pub mod config;
pub mod core;
pub mod error;
pub mod middleware;
pub mod models;
pub mod utils;
pub mod examples;

// Re-export commonly used types
pub use error::{GatewayError, AuthError, CacheError, LoadBalancerError, ConfigError};
pub use models::{Backend, Claims, TokenPair};
pub use core::{
    gateway::{ApiGateway, Gateway},
    request::GatewayRequest,
    response::GatewayResponse,
    router::Router,
    middleware::{Middleware, MiddlewareHandler},
};