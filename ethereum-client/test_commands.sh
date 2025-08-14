#!/bin/bash

# Ethereum Client Test Commands
# This script provides easy commands to test the Ethereum client

set -e  # Exit on any error

echo "🧪 Ethereum Client Test Suite"
echo "============================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Function to check if environment variables are set
check_env() {
    local var_name=$1
    local var_value=$(eval echo \$${var_name})
    
    if [ -z "$var_value" ]; then
        print_status $RED "✗ Environment variable $var_name is not set"
        return 1
    else
        print_status $GREEN "✓ Environment variable $var_name is set"
        return 0
    fi
}

# Function to run a test command
run_test() {
    local test_name=$1
    local command=$2
    
    print_status $BLUE "🔍 Running: $test_name"
    echo "Command: $command"
    echo ""
    
    if eval $command; then
        print_status $GREEN "✅ $test_name - PASSED"
    else
        print_status $RED "❌ $test_name - FAILED"
        return 1
    fi
    echo ""
}

# Main test function
run_tests() {
    local test_type=$1
    
    case $test_type in
        "unit")
            print_status $YELLOW "🧪 Running Unit Tests"
            run_test "Cargo Unit Tests" "cd ethereum-client && cargo test"
            ;;
        
        "mock")
            print_status $YELLOW "🎭 Running Mock Integration Tests"
            run_test "Mock Integration Example" "cd ethereum-client && cargo run --example mock_integration"
            ;;
        
        "basic")
            print_status $YELLOW "🔧 Running Basic Usage Test (requires valid config)"
            
            # Check required environment variables
            local env_ok=true
            check_env "ALCHEMY_API_KEY" || env_ok=false
            check_env "ARITHMETIC_CONTRACT_ADDRESS" || env_ok=false
            check_env "VERIFIER_CONTRACT_ADDRESS" || env_ok=false
            
            if [ "$env_ok" = true ]; then
                run_test "Basic Usage Example" "cd ethereum-client && cargo run --example basic_usage"
            else
                print_status $RED "❌ Basic usage test skipped - missing required environment variables"
                print_status $YELLOW "💡 Set up your .env file first: cp .env.example .env"
            fi
            ;;
        
        "cli")
            print_status $YELLOW "🖥️ Testing CLI Commands"
            
            # Test CLI help
            run_test "CLI Help Command" "cd ethereum-client && cargo run --bin ethereum_service -- --help"
            
            # Test network stats (read-only, should work with any valid API key)
            if check_env "ALCHEMY_API_KEY" && check_env "ARITHMETIC_CONTRACT_ADDRESS" && check_env "VERIFIER_CONTRACT_ADDRESS"; then
                run_test "CLI Network Stats" "cd ethereum-client && cargo run --bin ethereum_service network-stats"
                
                # Test state reading (will likely fail but tests the flow)
                print_status $BLUE "🔍 Testing state reading (expected to fail gracefully)"
                run_test "CLI Get State (expected failure)" "cd ethereum-client && cargo run --bin ethereum_service get-state --state-id 0x0000000000000000000000000000000000000000000000000000000000000001 || true"
            else
                print_status $YELLOW "⚠️  CLI tests require environment variables - skipping"
            fi
            ;;
        
        "build")
            print_status $YELLOW "🔨 Testing Build Process"
            run_test "Cargo Check" "cd ethereum-client && cargo check"
            run_test "Cargo Build" "cd ethereum-client && cargo build"
            run_test "Cargo Build Release" "cd ethereum-client && cargo build --release"
            ;;
        
        "lint")
            print_status $YELLOW "📏 Running Lints and Formatting"
            run_test "Cargo Clippy" "cd ethereum-client && cargo clippy -- -D warnings"
            run_test "Cargo Format Check" "cd ethereum-client && cargo fmt -- --check"
            ;;
        
        "all")
            print_status $YELLOW "🚀 Running All Tests"
            run_tests "build"
            run_tests "lint" 
            run_tests "unit"
            run_tests "mock"
            run_tests "cli"
            run_tests "basic"
            ;;
        
        *)
            echo "Usage: $0 {unit|mock|basic|cli|build|lint|all}"
            echo ""
            echo "Test Types:"
            echo "  unit   - Run unit tests (no network required)"
            echo "  mock   - Run mock integration tests (no network required)"
            echo "  basic  - Run basic usage example (requires valid Alchemy config)"
            echo "  cli    - Test CLI commands (requires valid Alchemy config)"
            echo "  build  - Test build process"
            echo "  lint   - Run lints and formatting checks"
            echo "  all    - Run all tests"
            echo ""
            echo "Setup:"
            echo "  1. Copy .env.example to .env: cp ethereum-client/.env.example ethereum-client/.env"
            echo "  2. Fill in your Alchemy API key and contract addresses"
            echo "  3. Run tests: $0 all"
            exit 1
            ;;
    esac
}

# Check if we're in the right directory
if [ ! -d "ethereum-client" ]; then
    print_status $RED "❌ Error: ethereum-client directory not found"
    print_status $YELLOW "💡 Run this script from the project root directory"
    exit 1
fi

# Run the specified tests
run_tests $1

print_status $GREEN "🎉 Test suite completed!"