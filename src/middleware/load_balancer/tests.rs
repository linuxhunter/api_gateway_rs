#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    
    use hyper::{HeaderMap, Method, Uri};
    use bytes::Bytes;
    
    use crate::core::request::GatewayRequest;
    use crate::middleware::load_balancer::{BackendStats, LoadBalanceStrategy};
    use crate::middleware::load_balancer::strategies::{
        RoundRobinStrategy, WeightedRoundRobinStrategy, LeastConnectionsStrategy, IpHashStrategy
    };
    use crate::models::Backend;
    
    // Helper function to create test backends
    fn create_test_backends() -> Vec<Backend> {
        vec![
            Backend {
                id: "backend-1".to_string(),
                url: "http://localhost:8081".to_string(),
                weight: 1,
                healthy: true,
                health_check_path: "/health".to_string(),
                timeout_seconds: 30,
                host: "localhost".to_string(),
                port: 8081,
                tags: vec!["api".to_string()],
                metadata: HashMap::new(),
            },
            Backend {
                id: "backend-2".to_string(),
                url: "http://localhost:8082".to_string(),
                weight: 2,
                healthy: true,
                health_check_path: "/health".to_string(),
                timeout_seconds: 30,
                host: "localhost".to_string(),
                port: 8082,
                tags: vec!["api".to_string()],
                metadata: HashMap::new(),
            },
            Backend {
                id: "backend-3".to_string(),
                url: "http://localhost:8083".to_string(),
                weight: 1,
                healthy: true,
                health_check_path: "/health".to_string(),
                timeout_seconds: 30,
                host: "localhost".to_string(),
                port: 8083,
                tags: vec!["api".to_string()],
                metadata: HashMap::new(),
            },
        ]
    }
    
    // Helper function to create a test request
    fn create_test_request(ip: Option<IpAddr>) -> GatewayRequest {
        GatewayRequest::new(
            Method::GET,
            Uri::from_static("http://example.com/test"),
            HeaderMap::new(),
            Bytes::from("test"),
            ip,
        )
    }
    
    #[tokio::test]
    async fn test_round_robin_strategy() {
        let strategy = RoundRobinStrategy::new();
        let backends = create_test_backends();
        let request = create_test_request(None);
        
        // Test round robin selection
        let backend1 = strategy.select_backend(&backends, &request).await.unwrap();
        let backend2 = strategy.select_backend(&backends, &request).await.unwrap();
        let backend3 = strategy.select_backend(&backends, &request).await.unwrap();
        let backend4 = strategy.select_backend(&backends, &request).await.unwrap();
        
        assert_eq!(backend1.id, "backend-1");
        assert_eq!(backend2.id, "backend-2");
        assert_eq!(backend3.id, "backend-3");
        assert_eq!(backend4.id, "backend-1"); // Should wrap around
    }
    
    #[tokio::test]
    async fn test_weighted_round_robin_strategy() {
        let strategy = WeightedRoundRobinStrategy::new();
        let backends = create_test_backends();
        let request = create_test_request(None);
        
        // Test weighted round robin selection
        // backend-2 has weight 2, so it should appear twice in the rotation
        let backend1 = strategy.select_backend(&backends, &request).await.unwrap();
        let backend2 = strategy.select_backend(&backends, &request).await.unwrap();
        let backend3 = strategy.select_backend(&backends, &request).await.unwrap();
        let backend4 = strategy.select_backend(&backends, &request).await.unwrap();
        
        // The expected sequence is backend-1, backend-2, backend-2, backend-3, backend-1, ...
        // This is because backend-2 has weight 2, so it appears twice in the expanded list
        assert_eq!(backend1.id, "backend-1");
        assert_eq!(backend2.id, "backend-2");
        assert_eq!(backend3.id, "backend-2"); // Second occurrence due to weight 2
        assert_eq!(backend4.id, "backend-3");
    }
    
    #[tokio::test]
    async fn test_least_connections_strategy() {
        let strategy = LeastConnectionsStrategy::new();
        let backends = create_test_backends();
        let request = create_test_request(None);
        
        // Initially, all backends have 0 connections, so the first one should be selected
        let backend1 = strategy.select_backend(&backends, &request).await.unwrap();
        assert_eq!(backend1.id, "backend-1");
        
        // Update stats to simulate active connections
        let mut stats = HashMap::new();
        stats.insert("backend-1".to_string(), BackendStats {
            active_connections: 5,
            ..BackendStats::default()
        });
        stats.insert("backend-2".to_string(), BackendStats {
            active_connections: 2,
            ..BackendStats::default()
        });
        stats.insert("backend-3".to_string(), BackendStats {
            active_connections: 10,
            ..BackendStats::default()
        });
        
        strategy.update_stats(stats).await;
        
        // Now backend-2 has the least connections, so it should be selected
        let backend2 = strategy.select_backend(&backends, &request).await.unwrap();
        assert_eq!(backend2.id, "backend-2");
    }
    
    #[tokio::test]
    async fn test_ip_hash_strategy() {
        let strategy = IpHashStrategy::new();
        let backends = create_test_backends();
        
        // Create requests with different IPs
        let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));
        
        let request1 = create_test_request(Some(ip1));
        let request2 = create_test_request(Some(ip2));
        
        // Test that the same IP always gets routed to the same backend
        let backend1_first = strategy.select_backend(&backends, &request1).await.unwrap();
        let backend1_second = strategy.select_backend(&backends, &request1).await.unwrap();
        
        assert_eq!(backend1_first.id, backend1_second.id);
        
        // Test that different IPs may get routed to different backends
        // (Note: this is probabilistic, so there's a small chance they might get the same backend)
        let backend2 = strategy.select_backend(&backends, &request2).await.unwrap();
        
        // Test that a request without an IP falls back to the first backend
        let request_no_ip = create_test_request(None);
        let backend_no_ip = strategy.select_backend(&backends, &request_no_ip).await.unwrap();
        
        assert_eq!(backend_no_ip.id, "backend-1");
    }
}