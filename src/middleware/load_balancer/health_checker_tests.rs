#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::RwLock;

    use crate::config::LoadBalancerConfig;
    use crate::middleware::load_balancer::health_checker::{
        AdvancedHealthChecker, CircuitBreakerState,
    };
    use crate::middleware::load_balancer::{BackendRegistry, BackendStatus, HealthChecker};
    use crate::models::Backend;

    // Helper function to create a test backend
    fn create_test_backend(id: &str, healthy: bool) -> Backend {
        Backend {
            id: id.to_string(),
            url: format!("http://localhost:808{}", id.chars().last().unwrap_or('0')),
            weight: 1,
            healthy,
            health_check_path: "/health".to_string(),
            timeout_seconds: 1,
            host: "localhost".to_string(),
            port: 8080 + id.chars().last().unwrap_or('0') as u16 - '0' as u16,
            tags: vec!["test".to_string()],
            metadata: HashMap::new(),
        }
    }

    // Helper function to create a test config
    fn create_test_config() -> LoadBalancerConfig {
        LoadBalancerConfig {
            algorithm: "round_robin".to_string(),
            health_check_interval: 1,
            health_check_timeout: 1,
            health_check_path: "/health".to_string(),
        }
    }

    #[tokio::test]
    async fn test_health_checker_initialization() {
        let config = create_test_config();
        let health_checker = AdvancedHealthChecker::new(config.clone());

        // Just verify the health checker was created successfully
        // We can't access private fields directly
        assert!(
            health_checker
                .check_health(&create_test_backend("1", true))
                .await
                == false
        );
    }

    #[tokio::test]
    async fn test_health_checker_mock_check() {
        let config = create_test_config();
        let health_checker = AdvancedHealthChecker::new(config);

        // Create a test backend
        let backend = create_test_backend("1", true);

        // The actual HTTP request will fail since there's no server running,
        // but we can still test the health checker's behavior
        let result = health_checker.check_health(&backend).await;

        // First check should fail because there's no server
        assert!(!result);

        // Check that stats were updated
        let stats = health_checker.get_stats().await;
        assert!(stats.contains_key(&backend.id));

        let backend_stats = stats.get(&backend.id).unwrap();
        assert_eq!(backend_stats.failure_count, 1);
        assert_eq!(backend_stats.success_count, 0);
        assert_eq!(backend_stats.consecutive_failures, 1);
        assert_eq!(backend_stats.consecutive_successes, 0);
        assert!(!backend_stats.last_status);

        // Check that response time history was updated
        assert_eq!(backend_stats.response_time_history.len(), 1);
    }

    #[tokio::test]
    async fn test_health_checker_with_registry() {
        let config = create_test_config();
        let health_checker = Arc::new(AdvancedHealthChecker::new(config));

        // Create a backend registry
        let registry = Arc::new(BackendRegistry::new());

        // Register some backends
        let backend1 = create_test_backend("1", true);
        let backend2 = create_test_backend("2", false);

        registry.register(backend1.clone()).await.unwrap();
        registry.register(backend2.clone()).await.unwrap();

        // Start health checks
        health_checker
            .start_health_checks(registry.clone())
            .await
            .unwrap();

        // Wait for health checks to run
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Stop health checks
        health_checker.stop_health_checks().await.unwrap();

        // Check that backend health was updated
        let backend1_health = registry.get(&backend1.id).await.unwrap().healthy;
        let backend2_health = registry.get(&backend2.id).await.unwrap().healthy;

        // Both should be marked as unhealthy since there's no actual server
        assert!(!backend1_health);
        assert!(!backend2_health);

        // Check backend stats
        let backend1_stats = registry.get_stats(&backend1.id).await.unwrap();
        let backend2_stats = registry.get_stats(&backend2.id).await.unwrap();

        assert_eq!(backend1_stats.last_health_status, BackendStatus::Unhealthy);
        assert_eq!(backend2_stats.last_health_status, BackendStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_health_checker_recovery() {
        let config = create_test_config();
        let health_checker = Arc::new(AdvancedHealthChecker::new(config));

        // Create a backend registry
        let registry = Arc::new(BackendRegistry::new());

        // Register a backend
        let backend = create_test_backend("1", false);
        registry.register(backend.clone()).await.unwrap();

        // Manually update health to simulate recovery
        registry.update_health(&backend.id, true).await.unwrap();

        // Verify the backend is now healthy
        let updated_backend = registry.get(&backend.id).await.unwrap();
        assert!(updated_backend.healthy);

        // Check backend stats
        let stats = registry.get_stats(&backend.id).await.unwrap();
        assert_eq!(stats.last_health_status, BackendStatus::Healthy);
    }

    #[tokio::test]
    async fn test_circuit_breaker_transitions() {
        let config = create_test_config();
        let health_checker = AdvancedHealthChecker::new(config);

        // Create a test backend
        let backend = create_test_backend("1", true);

        // Simulate multiple failed health checks to trigger circuit breaker
        for _ in 0..3 {
            health_checker.check_health(&backend).await;
        }

        // Check circuit breaker state - should be open after 3 failures
        let circuit_state = health_checker.get_circuit_breaker_state(&backend.id).await;
        assert!(matches!(
            circuit_state,
            Some(CircuitBreakerState::Open { .. })
        ));

        // Reset the circuit breaker
        health_checker
            .reset_circuit_breaker(&backend.id)
            .await
            .unwrap();

        // Check circuit breaker state - should be closed after reset
        let circuit_state = health_checker.get_circuit_breaker_state(&backend.id).await;
        assert_eq!(circuit_state, Some(CircuitBreakerState::Closed));
    }

    // We can't test automatic recovery directly since it uses private methods
    // Instead, we'll test the health check functionality which is public
    #[tokio::test]
    async fn test_health_check_with_unhealthy_backend() {
        let config = create_test_config();
        let health_checker = Arc::new(AdvancedHealthChecker::new(config));

        // Create a backend registry
        let registry = Arc::new(BackendRegistry::new());

        // Register a backend as unhealthy
        let backend = create_test_backend("1", false);
        registry.register(backend.clone()).await.unwrap();

        // Perform a health check
        let is_healthy = health_checker.check_health(&backend).await;

        // The check should fail since there's no actual server
        assert!(!is_healthy);

        // Update backend health status
        registry
            .update_health(&backend.id, is_healthy)
            .await
            .unwrap();

        // Backend should still be unhealthy
        let backend_health = registry.get(&backend.id).await.unwrap().healthy;
        assert!(!backend_health);
    }

    // We can't test response time anomaly detection directly since it uses private methods
    // Instead, we'll test the health check functionality which indirectly uses anomaly detection
    #[tokio::test]
    async fn test_health_check_functionality() {
        let config = create_test_config();
        let health_checker = AdvancedHealthChecker::new(config);

        // Create a test backend
        let backend = create_test_backend("1", true);

        // Perform a health check
        let result = health_checker.check_health(&backend).await;

        // The check should fail since there's no actual server
        assert!(!result);

        // Get stats and verify they were updated
        let stats = health_checker.get_stats().await;
        assert!(stats.contains_key(&backend.id));
    }
}
