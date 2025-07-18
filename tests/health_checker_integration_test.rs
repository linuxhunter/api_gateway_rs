// Integration tests are moved to unit tests in the health_checker_tests.rs file
// This is because the integration tests require access to the crate's internal types
// which is not possible in the integration test environment
#[cfg(test)]
mod health_checker_integration_tests {
    // Tests moved to src/middleware/load_balancer/health_checker_tests.rs
}