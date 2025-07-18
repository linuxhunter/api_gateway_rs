use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use reqwest::Client;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::LoadBalancerConfig;
use crate::error::LoadBalancerError;
use crate::middleware::load_balancer::{BackendRegistry, BackendStatus, HealthChecker};
use crate::models::Backend;

/// Advanced HTTP health checker implementation with fault detection and automatic recovery
pub struct AdvancedHealthChecker {
    /// HTTP client for health checks
    client: Client,

    /// Health check configuration
    pub config: LoadBalancerConfig,

    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,

    /// Health check statistics
    stats: Arc<RwLock<HashMap<String, HealthCheckStats>>>,

    /// Circuit breaker state
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreakerState>>>,
}

/// Health check statistics
#[derive(Debug, Clone)]
pub struct HealthCheckStats {
    /// Number of successful health checks
    pub success_count: u64,

    /// Number of failed health checks
    pub failure_count: u64,

    /// Last check time
    pub last_check_time: Option<Instant>,

    /// Last check status
    pub last_status: bool,

    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,

    /// Consecutive failures
    pub consecutive_failures: u32,

    /// Consecutive successes
    pub consecutive_successes: u32,

    /// Response time history (last 10 checks)
    pub response_time_history: Vec<u64>,

    /// Status code history (last 10 checks)
    pub status_code_history: Vec<u16>,
}

impl Default for HealthCheckStats {
    fn default() -> Self {
        Self {
            success_count: 0,
            failure_count: 0,
            last_check_time: None,
            last_status: false,
            avg_response_time_ms: 0.0,
            consecutive_failures: 0,
            consecutive_successes: 0,
            response_time_history: Vec::with_capacity(10),
            status_code_history: Vec::with_capacity(10),
        }
    }
}

/// Circuit breaker state for fault detection
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests are allowed
    Closed,

    /// Circuit is open, requests are blocked
    Open {
        /// When the circuit will transition to half-open
        retry_at: Instant,
    },

    /// Circuit is half-open, allowing a limited number of requests
    HalfOpen {
        /// Number of successful requests in half-open state
        success_count: u32,
    },
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self::Closed
    }
}

impl AdvancedHealthChecker {
    /// Create a new advanced HTTP health checker
    pub fn new(config: LoadBalancerConfig) -> Self {
        // Create a client with appropriate timeouts
        let client = Client::builder()
            .timeout(Duration::from_secs(config.health_check_timeout))
            .connect_timeout(Duration::from_secs(config.health_check_timeout / 2))
            .build()
            .unwrap_or_else(|_| {
                warn!("Failed to create HTTP client with custom settings, using default");
                Client::new()
            });

        Self {
            client,
            config,
            shutdown: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update health check statistics
    async fn update_stats(
        &self,
        backend_id: &str,
        success: bool,
        response_time_ms: u64,
        status_code: Option<u16>,
    ) {
        let mut stats = self.stats.write().await;

        let backend_stats = stats.entry(backend_id.to_string()).or_default();

        // Update basic stats
        if success {
            backend_stats.success_count += 1;
            backend_stats.consecutive_successes += 1;
            backend_stats.consecutive_failures = 0;
        } else {
            backend_stats.failure_count += 1;
            backend_stats.consecutive_failures += 1;
            backend_stats.consecutive_successes = 0;
        }

        // Update last check info
        backend_stats.last_check_time = Some(Instant::now());
        backend_stats.last_status = success;

        // Update response time history
        backend_stats.response_time_history.push(response_time_ms);
        if backend_stats.response_time_history.len() > 10 {
            backend_stats.response_time_history.remove(0);
        }

        // Update status code history if available
        if let Some(code) = status_code {
            backend_stats.status_code_history.push(code);
            if backend_stats.status_code_history.len() > 10 {
                backend_stats.status_code_history.remove(0);
            }
        }

        // Update average response time (only for successful checks)
        if success {
            let total_checks = backend_stats.success_count;
            if total_checks == 1 {
                backend_stats.avg_response_time_ms = response_time_ms as f64;
            } else {
                backend_stats.avg_response_time_ms = (backend_stats.avg_response_time_ms
                    * (total_checks - 1) as f64
                    + response_time_ms as f64)
                    / total_checks as f64;
            }
        }

        debug!(
            "Health check stats for {}: success={}, failures={}, consecutive_successes={}, consecutive_failures={}, avg_time={}ms",
            backend_id,
            backend_stats.success_count,
            backend_stats.failure_count,
            backend_stats.consecutive_successes,
            backend_stats.consecutive_failures,
            backend_stats.avg_response_time_ms
        );
    }

    /// Get health check statistics
    pub async fn get_stats(&self) -> HashMap<String, HealthCheckStats> {
        self.stats.read().await.clone()
    }

    /// Update circuit breaker state based on health check results
    async fn update_circuit_breaker(&self, backend_id: &str, success: bool) -> CircuitBreakerState {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        let stats = self.stats.read().await;

        let backend_stats = stats.get(backend_id);
        let current_state = circuit_breakers.entry(backend_id.to_string()).or_default();

        // Determine the new state based on current state and check result
        let new_state = match current_state {
            CircuitBreakerState::Closed => {
                // If we have too many consecutive failures, open the circuit
                if let Some(stats) = backend_stats {
                    if stats.consecutive_failures >= 3 {
                        warn!("Circuit breaker for backend {} transitioning from CLOSED to OPEN due to {} consecutive failures", 
                              backend_id, stats.consecutive_failures);

                        // Open the circuit for 30 seconds
                        CircuitBreakerState::Open {
                            retry_at: Instant::now() + Duration::from_secs(30),
                        }
                    } else {
                        // Stay closed
                        CircuitBreakerState::Closed
                    }
                } else {
                    // No stats yet, stay closed
                    CircuitBreakerState::Closed
                }
            }
            CircuitBreakerState::Open { retry_at } => {
                // Check if it's time to transition to half-open
                if Instant::now() >= *retry_at {
                    info!(
                        "Circuit breaker for backend {} transitioning from OPEN to HALF-OPEN",
                        backend_id
                    );
                    CircuitBreakerState::HalfOpen { success_count: 0 }
                } else {
                    // Stay open
                    CircuitBreakerState::Open {
                        retry_at: *retry_at,
                    }
                }
            }
            CircuitBreakerState::HalfOpen { success_count } => {
                if success {
                    // If we have enough consecutive successes, close the circuit
                    let new_success_count = *success_count + 1;
                    if new_success_count >= 2 {
                        info!("Circuit breaker for backend {} transitioning from HALF-OPEN to CLOSED after {} successful checks", 
                              backend_id, new_success_count);
                        CircuitBreakerState::Closed
                    } else {
                        // Stay half-open with updated success count
                        CircuitBreakerState::HalfOpen {
                            success_count: new_success_count,
                        }
                    }
                } else {
                    // If we have a failure in half-open state, go back to open
                    warn!("Circuit breaker for backend {} transitioning from HALF-OPEN back to OPEN due to failed check", 
                          backend_id);

                    // Open the circuit for 30 seconds
                    CircuitBreakerState::Open {
                        retry_at: Instant::now() + Duration::from_secs(30),
                    }
                }
            }
        };

        // Update the circuit breaker state
        *current_state = new_state.clone();

        new_state
    }

    /// Check if a backend should be marked as healthy based on its health check history and circuit breaker state
    async fn should_mark_healthy(&self, backend_id: &str, success: bool) -> bool {
        // Update circuit breaker first
        let circuit_state = self.update_circuit_breaker(backend_id, success).await;

        // If circuit is open, backend is unhealthy
        if matches!(circuit_state, CircuitBreakerState::Open { .. }) {
            return false;
        }

        // For closed or half-open circuits, check the stats
        let stats = self.stats.read().await;

        if let Some(backend_stats) = stats.get(backend_id) {
            // If we have consecutive successes above threshold, mark as healthy
            if backend_stats.consecutive_successes >= 2 {
                return true;
            }

            // If we have consecutive failures above threshold, mark as unhealthy
            if backend_stats.consecutive_failures >= 3 {
                return false;
            }

            // Otherwise, use the last status
            return backend_stats.last_status;
        }

        // Default to true if no stats available
        true
    }

    // Method moved to public implementation above

    /// Get circuit breaker state
    pub async fn get_circuit_breaker_state(&self, backend_id: &str) -> Option<CircuitBreakerState> {
        let circuit_breakers = self.circuit_breakers.read().await;
        circuit_breakers.get(backend_id).cloned()
    }
    
    /// Attempt to recover unhealthy backends (made public for testing)
    pub async fn attempt_recovery(&self, registry: &Arc<BackendRegistry>) {
        // Get all backends
        let backends = registry.get_all().await;

        for backend in backends {
            // Skip healthy backends
            if backend.healthy {
                continue;
            }

            // Get circuit breaker state
            let circuit_state = self.get_circuit_breaker_state(&backend.id).await;

            // Only attempt recovery for half-open or closed circuits
            if let Some(state) = circuit_state {
                match state {
                    CircuitBreakerState::Open { .. } => {
                        // Skip backends with open circuit breakers
                        continue;
                    }
                    _ => {
                        // Attempt recovery for half-open or closed circuits
                    }
                }
            }

            // Perform a recovery health check
            info!("Attempting recovery for unhealthy backend {}", backend.id);
            let is_healthy = self.check_health(&backend).await;

            if is_healthy {
                info!("Backend {} has recovered and is now healthy", backend.id);

                // Update backend health status
                if let Err(e) = registry.update_health(&backend.id, true).await {
                    error!("Failed to update backend health during recovery: {}", e);
                }
            } else {
                debug!(
                    "Recovery attempt for backend {} failed, still unhealthy",
                    backend.id
                );
            }
        }
    }
    
    /// Detect anomalies in response time (made public for testing)
    pub async fn detect_response_time_anomalies(&self, backend_id: &str) -> bool {
        let stats = self.stats.read().await;

        if let Some(backend_stats) = stats.get(backend_id) {
            // Need at least 5 data points for anomaly detection
            if backend_stats.response_time_history.len() < 5 {
                return false;
            }

            // Calculate mean and standard deviation
            let sum: u64 = backend_stats.response_time_history.iter().sum();
            let mean = sum as f64 / backend_stats.response_time_history.len() as f64;

            let variance_sum: f64 = backend_stats
                .response_time_history
                .iter()
                .map(|&x| {
                    let diff = x as f64 - mean;
                    diff * diff
                })
                .sum();

            let std_dev = (variance_sum / backend_stats.response_time_history.len() as f64).sqrt();

            // Get the latest response time
            if let Some(&latest) = backend_stats.response_time_history.last() {
                // Check if the latest response time is more than 3 standard deviations from the mean
                let z_score = (latest as f64 - mean).abs() / std_dev;

                if z_score > 3.0 {
                    warn!("Response time anomaly detected for backend {}: latest={}ms, mean={}ms, std_dev={}ms, z-score={}", 
                          backend_id, latest, mean, std_dev, z_score);
                    return true;
                }
            }
        }

        false
    }

    /// Reset circuit breaker (for manual intervention)
    pub async fn reset_circuit_breaker(&self, backend_id: &str) -> Result<(), LoadBalancerError> {
        let mut circuit_breakers = self.circuit_breakers.write().await;

        if circuit_breakers.contains_key(backend_id) {
            info!(
                "Manually resetting circuit breaker for backend {}",
                backend_id
            );
            circuit_breakers.insert(backend_id.to_string(), CircuitBreakerState::Closed);
            Ok(())
        } else {
            Err(LoadBalancerError::BackendNotFound(backend_id.to_string()))
        }
    }
}

#[async_trait]
impl HealthChecker for AdvancedHealthChecker {
    async fn check_health(&self, backend: &Backend) -> bool {
        let start_time = Instant::now();
        let health_check_url = format!("{}{}", backend.url, backend.health_check_path);

        debug!(
            "Performing health check for backend {} at {}",
            backend.id, health_check_url
        );

        // Check circuit breaker first
        let circuit_breakers = self.circuit_breakers.read().await;
        if let Some(CircuitBreakerState::Open { retry_at }) = circuit_breakers.get(&backend.id) {
            if Instant::now() < *retry_at {
                debug!(
                    "Circuit breaker for backend {} is OPEN, skipping health check",
                    backend.id
                );
                return false;
            }
        }
        drop(circuit_breakers);

        // Perform the health check
        let result = self
            .client
            .get(&health_check_url)
            .timeout(Duration::from_secs(self.config.health_check_timeout))
            .send()
            .await;

        // Calculate response time
        let response_time = start_time.elapsed();
        let response_time_ms = response_time.as_millis() as u64;

        // Process the result
        match result {
            Ok(response) => {
                let status = response.status();
                let status_code = status.as_u16();
                let success = status.is_success();

                if success {
                    debug!(
                        "Health check successful for backend {} (status: {}, time: {}ms)",
                        backend.id, status, response_time_ms
                    );
                } else {
                    warn!(
                        "Health check failed for backend {} (status: {}, time: {}ms)",
                        backend.id, status, response_time_ms
                    );
                }

                // Update statistics
                self.update_stats(&backend.id, success, response_time_ms, Some(status_code))
                    .await;

                // Check for response time anomalies
                let has_anomaly = self.detect_response_time_anomalies(&backend.id).await;

                // Determine if the backend should be marked as healthy
                let should_be_healthy = self.should_mark_healthy(&backend.id, success).await;

                // If there's a response time anomaly but the backend would otherwise be healthy,
                // log a warning but don't mark it as unhealthy yet
                if has_anomaly && should_be_healthy {
                    warn!(
                        "Backend {} has response time anomalies but is still considered healthy",
                        backend.id
                    );
                }

                should_be_healthy
            }
            Err(e) => {
                error!(
                    "Health check error for backend {}: {} (time: {}ms)",
                    backend.id, e, response_time_ms
                );

                // Update statistics
                self.update_stats(&backend.id, false, response_time_ms, None)
                    .await;

                // Determine if the backend should be marked as unhealthy
                self.should_mark_healthy(&backend.id, false).await
            }
        }
    }

    async fn start_health_checks(
        &self,
        registry: Arc<BackendRegistry>,
    ) -> Result<(), LoadBalancerError> {
        // Reset shutdown flag
        let mut shutdown = self.shutdown.write().await;
        *shutdown = false;
        drop(shutdown);

        // Clone necessary data for the health check task
        let checker = Arc::new(self.clone());
        let shutdown_flag = self.shutdown.clone();
        let interval = Duration::from_secs(self.config.health_check_interval);

        info!(
            "Starting health checker with interval of {} seconds",
            self.config.health_check_interval
        );

        // Spawn health check task
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);

            loop {
                // Check if shutdown was requested
                if *shutdown_flag.read().await {
                    info!("Health checker shutting down");
                    break;
                }

                // Wait for next interval
                timer.tick().await;

                // Get all backends
                let backends = registry.get_all().await;

                debug!("Running health checks for {} backends", backends.len());

                // Check health of each backend
                for backend in backends {
                    // Skip if backend is marked as removed
                    let stats = registry.get_stats(&backend.id).await.ok();
                    if let Some(stats) = stats {
                        if stats.last_health_status == BackendStatus::Removed {
                            continue;
                        }
                    }

                    // Perform health check
                    let is_healthy = checker.check_health(&backend).await;

                    // Update backend health status
                    if let Err(e) = registry.update_health(&backend.id, is_healthy).await {
                        error!("Failed to update backend health: {}", e);
                    } else {
                        if is_healthy {
                            debug!("Backend {} is healthy", backend.id);
                        } else {
                            warn!("Backend {} is unhealthy", backend.id);
                        }
                    }
                }

                // Attempt automatic recovery for unhealthy backends
                checker.attempt_recovery(&registry).await;
            }
        });

        Ok(())
    }

    async fn stop_health_checks(&self) -> Result<(), LoadBalancerError> {
        info!("Stopping health checker");

        // Set shutdown flag
        let mut shutdown = self.shutdown.write().await;
        *shutdown = true;

        Ok(())
    }
}

// Implementation moved to public methods above

// Implement Clone for AdvancedHealthChecker
impl Clone for AdvancedHealthChecker {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            config: self.config.clone(),
            shutdown: self.shutdown.clone(),
            stats: self.stats.clone(),
            circuit_breakers: self.circuit_breakers.clone(),
        }
    }
}

/// Health check factory
pub struct HealthCheckerFactory;

impl HealthCheckerFactory {
    /// Create a new health checker
    pub fn create(config: LoadBalancerConfig) -> Arc<dyn HealthChecker + Send + Sync> {
        Arc::new(AdvancedHealthChecker::new(config))
    }
}
