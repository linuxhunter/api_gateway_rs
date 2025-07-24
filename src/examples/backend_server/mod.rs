pub mod server;
pub mod routes;
pub mod models;
pub mod simple;
pub mod multi_instance;

pub use server::run_server;
pub use simple::run_simple_server;
pub use multi_instance::{
    ServiceInstanceConfig, ServiceInstanceType, ServiceRegistry, ErrorType,
    run_service_instance,
};