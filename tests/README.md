# API Gateway Integration Tests

This directory contains comprehensive integration tests for the Rust API Gateway project. The tests are designed to validate the complete system functionality, performance characteristics, and reliability under various conditions.

## Test Structure

### 1. Integration Tests (`integration_tests.rs`)

**Purpose**: End-to-end testing of the complete API gateway system

**Test Coverage**:
- Basic request routing and response handling
- Load balancing with multiple backend servers
- Middleware chain execution and ordering
- Error handling and fallback mechanisms
- Timeout handling and circuit breaking
- Configuration hot-reload functionality

**Key Test Cases**:
- `test_end_to_end_basic_request`: Validates basic request flow through the gateway
- `test_load_balancing_round_robin`: Tests round-robin load balancing across multiple backends
- `test_middleware_chain_execution`: Verifies middleware execution order and functionality
- `test_error_handling_and_fallback`: Tests error scenarios and graceful degradation
- `test_timeout_handling`: Validates timeout behavior and recovery
- `test_configuration_hot_reload`: Tests dynamic configuration updates

### 2. Load Tests (`load_tests.rs`)

**Purpose**: Performance and scalability testing under various load conditions

**Test Coverage**:
- Concurrent user simulation
- Request throughput measurement
- Response time analysis
- Error rate monitoring
- Resource utilization tracking

**Key Test Cases**:
- `test_basic_load_health_check`: Basic load testing with health check endpoints
- `test_load_with_different_endpoints`: Mixed workload testing across different endpoints
- `test_load_with_slow_endpoints`: Performance testing with slow backend responses

**Load Test Configuration**:
```rust
LoadTestConfig {
    concurrent_users: 10,      // Number of concurrent users
    requests_per_user: 100,    // Requests per user
    test_duration: 30s,        // Maximum test duration
    ramp_up_time: 5s,         // Time to ramp up all users
}
```

### 3. Redis Integration Tests (`redis_integration_tests.rs`)

**Purpose**: Testing Redis-based caching functionality with real Redis instances

**Test Coverage**:
- Cache hit/miss scenarios
- Cache expiration and TTL handling
- Redis connection management
- Cache integration with load balancing
- Selective caching policies
- Redis connection failure fallback

**Key Test Cases**:
- `test_redis_cache_hit_miss`: Validates cache hit/miss behavior
- `test_redis_cache_expiration`: Tests cache TTL and expiration
- `test_redis_cache_with_load_balancing`: Integration of caching with load balancing
- `test_redis_cache_selective_caching`: Tests selective caching policies
- `test_redis_connection_failure_fallback`: Tests graceful degradation when Redis fails

**Requirements**: Docker must be running for Redis container tests

### 4. Backend Server Tests (`backend_server_test.rs`)

**Purpose**: Testing the example backend servers used in integration tests

**Test Coverage**:
- Health check endpoints
- Echo functionality
- Delayed responses
- Error response generation
- Server startup and shutdown

### 5. Performance Benchmarks (`benches/gateway_benchmarks.rs`)

**Purpose**: Detailed performance benchmarking using the Criterion framework

**Benchmark Categories**:
- Basic routing performance
- Load balancing overhead
- Caching performance (hit vs miss)
- Concurrent request handling
- Middleware chain overhead
- Memory usage patterns

## Running Tests

### Prerequisites

1. **Rust Environment**: Ensure you have Rust 1.70+ installed
2. **Docker**: Required for Redis integration tests
3. **Dependencies**: All test dependencies are automatically managed by Cargo

### Running Individual Test Suites

```bash
# Run all tests
cargo test

# Run specific test suites
cargo test --test integration_tests
cargo test --test load_tests
cargo test --test redis_integration_tests
cargo test --test backend_server_test

# Run benchmarks
cargo bench

# Run with logging
RUST_LOG=debug cargo test --test integration_tests
```

### Using the Test Runner Script

The project includes a comprehensive test runner script:

```bash
# Run all tests
./scripts/run_integration_tests.sh

# Run specific test types
./scripts/run_integration_tests.sh --unit
./scripts/run_integration_tests.sh --integration
./scripts/run_integration_tests.sh --load
./scripts/run_integration_tests.sh --redis
./scripts/run_integration_tests.sh --benchmarks

# Generate coverage report
./scripts/run_integration_tests.sh --coverage

# Show help
./scripts/run_integration_tests.sh --help
```

## Test Configuration

### Test Ports

The tests use specific port ranges to avoid conflicts:

- **Gateway Ports**: 9000-9099
- **Backend Ports**: 9100-9199
- **Redis Ports**: Dynamically assigned by testcontainers

### Environment Variables

- `RUST_LOG`: Set logging level (debug, info, warn, error)
- `CONFIG_PATH`: Path to test configuration file
- `DOCKER_HOST`: Docker daemon connection (for Redis tests)

### Test Configuration Files

- `tests/test_config.yaml`: Sample configuration for integration tests
- `config.yaml`: Default gateway configuration

## Performance Expectations

### Load Test Targets

- **Throughput**: > 1000 requests/second for simple endpoints
- **Response Time**: < 50ms average for cached responses
- **Error Rate**: < 1% under normal load conditions
- **Concurrent Users**: Support for 100+ concurrent users

### Benchmark Targets

- **Basic Routing**: < 1ms per request
- **Cache Hit**: < 0.1ms additional overhead
- **Load Balancing**: < 0.5ms additional overhead per backend

## Troubleshooting

### Common Issues

1. **Port Conflicts**: Tests use specific ports; ensure they're available
2. **Docker Issues**: Redis tests require Docker; check Docker daemon status
3. **Timing Issues**: Some tests are timing-sensitive; run on a quiet system
4. **Resource Limits**: Load tests may hit system limits; adjust ulimits if needed

### Debug Tips

```bash
# Run with verbose logging
RUST_LOG=trace cargo test --test integration_tests -- --nocapture

# Run single test with output
cargo test test_end_to_end_basic_request --test integration_tests -- --nocapture

# Check Docker containers
docker ps  # Should show Redis containers during Redis tests
```

### Performance Tuning

For better test performance:

1. **Increase file descriptor limits**: `ulimit -n 4096`
2. **Use SSD storage**: Faster I/O improves test execution
3. **Close unnecessary applications**: Reduce system load during testing
4. **Use release builds for benchmarks**: `cargo bench --release`

## Continuous Integration

The tests are designed to run in CI environments:

- **GitHub Actions**: Compatible with standard Rust CI workflows
- **Docker Support**: Uses testcontainers for isolated Redis testing
- **Parallel Execution**: Tests can run in parallel with proper port management
- **Timeout Handling**: All tests have reasonable timeouts for CI environments

## Contributing

When adding new integration tests:

1. **Follow naming conventions**: Use descriptive test names
2. **Use unique ports**: Avoid port conflicts with existing tests
3. **Clean up resources**: Ensure proper cleanup in test teardown
4. **Document test purpose**: Add clear comments explaining test objectives
5. **Consider CI impact**: Ensure tests run reliably in CI environments

## Test Results Interpretation

### Success Criteria

- **All tests pass**: No test failures or panics
- **Performance within bounds**: Response times and throughput meet targets
- **Error rates acceptable**: Error rates below defined thresholds
- **Resource usage reasonable**: Memory and CPU usage within limits

### Failure Analysis

When tests fail:

1. **Check logs**: Review test output and logs for error details
2. **Verify environment**: Ensure Docker, ports, and dependencies are available
3. **Run individually**: Isolate failing tests to identify root cause
4. **Check timing**: Some failures may be timing-related in slow environments

## Future Enhancements

Planned improvements to the test suite:

- **Chaos testing**: Introduce random failures to test resilience
- **Security testing**: Add tests for authentication and authorization
- **Monitoring integration**: Test metrics collection and alerting
- **Multi-region testing**: Test distributed deployment scenarios
- **Protocol testing**: Add tests for HTTP/2, WebSocket, and gRPC support