#!/bin/bash

# Integration Test Runner Script for API Gateway
# This script runs all integration tests, load tests, and benchmarks

set -e

echo "🚀 Starting API Gateway Integration Test Suite"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Docker is running (needed for Redis tests)
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        print_warning "Docker is not running. Redis integration tests will be skipped."
        return 1
    fi
    return 0
}

# Run unit tests
run_unit_tests() {
    print_status "Running unit tests..."
    if cargo test --lib; then
        print_success "Unit tests passed"
    else
        print_error "Unit tests failed"
        return 1
    fi
}

# Run integration tests
run_integration_tests() {
    print_status "Running integration tests..."
    if cargo test --test integration_tests; then
        print_success "Integration tests passed"
    else
        print_error "Integration tests failed"
        return 1
    fi
}

# Run load tests
run_load_tests() {
    print_status "Running load tests..."
    if cargo test --test load_tests; then
        print_success "Load tests passed"
    else
        print_error "Load tests failed"
        return 1
    fi
}

# Run Redis integration tests (requires Docker)
run_redis_tests() {
    if check_docker; then
        print_status "Running Redis integration tests..."
        if cargo test --test redis_integration_tests; then
            print_success "Redis integration tests passed"
        else
            print_error "Redis integration tests failed"
            return 1
        fi
    else
        print_warning "Skipping Redis integration tests (Docker not available)"
    fi
}

# Run backend server tests
run_backend_tests() {
    print_status "Running backend server tests..."
    if cargo test --test backend_server_test; then
        print_success "Backend server tests passed"
    else
        print_error "Backend server tests failed"
        return 1
    fi
}

# Run benchmarks
run_benchmarks() {
    print_status "Running performance benchmarks..."
    if cargo bench --bench gateway_benchmarks; then
        print_success "Benchmarks completed"
    else
        print_error "Benchmarks failed"
        return 1
    fi
}

# Run all tests with coverage (if cargo-tarpaulin is installed)
run_coverage() {
    if command -v cargo-tarpaulin &> /dev/null; then
        print_status "Running tests with coverage..."
        cargo tarpaulin --out Html --output-dir coverage
        print_success "Coverage report generated in coverage/"
    else
        print_warning "cargo-tarpaulin not installed. Skipping coverage report."
        print_warning "Install with: cargo install cargo-tarpaulin"
    fi
}

# Main execution
main() {
    local run_benchmarks=false
    local run_coverage=false
    local run_all=true

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --benchmarks)
                run_benchmarks=true
                run_all=false
                shift
                ;;
            --coverage)
                run_coverage=true
                shift
                ;;
            --unit)
                run_unit_tests
                exit $?
                ;;
            --integration)
                run_integration_tests
                exit $?
                ;;
            --load)
                run_load_tests
                exit $?
                ;;
            --redis)
                run_redis_tests
                exit $?
                ;;
            --backend)
                run_backend_tests
                exit $?
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo "Options:"
                echo "  --unit          Run only unit tests"
                echo "  --integration   Run only integration tests"
                echo "  --load          Run only load tests"
                echo "  --redis         Run only Redis integration tests"
                echo "  --backend       Run only backend server tests"
                echo "  --benchmarks    Include benchmark tests"
                echo "  --coverage      Generate coverage report"
                echo "  --help          Show this help message"
                echo ""
                echo "Without options, runs all tests except benchmarks"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    # Build the project first
    print_status "Building project..."
    if cargo build; then
        print_success "Build successful"
    else
        print_error "Build failed"
        exit 1
    fi

    local failed_tests=()

    if $run_all; then
        # Run all test suites
        print_status "Running complete test suite..."
        
        run_unit_tests || failed_tests+=("unit")
        run_backend_tests || failed_tests+=("backend")
        run_integration_tests || failed_tests+=("integration")
        run_load_tests || failed_tests+=("load")
        run_redis_tests || failed_tests+=("redis")
    fi

    if $run_benchmarks; then
        run_benchmarks || failed_tests+=("benchmarks")
    fi

    if $run_coverage; then
        run_coverage || failed_tests+=("coverage")
    fi

    # Summary
    echo ""
    echo "=============================================="
    echo "🏁 Test Suite Summary"
    echo "=============================================="

    if [ ${#failed_tests[@]} -eq 0 ]; then
        print_success "All tests passed! 🎉"
        
        # Display some useful information
        echo ""
        print_status "Next steps:"
        echo "  • View benchmark results: open target/criterion/report/index.html"
        if $run_coverage; then
            echo "  • View coverage report: open coverage/tarpaulin-report.html"
        fi
        echo "  • Run specific test: cargo test <test_name>"
        echo "  • Run with logs: RUST_LOG=debug cargo test <test_name>"
        
        exit 0
    else
        print_error "Some tests failed:"
        for test in "${failed_tests[@]}"; do
            echo "  ❌ $test"
        done
        exit 1
    fi
}

# Run main function with all arguments
main "$@"