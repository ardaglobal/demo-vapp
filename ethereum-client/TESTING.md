# Testing Guide for Ethereum Client

This guide provides comprehensive testing strategies for the Ethereum client, from basic unit tests to full integration testing with live networks.

## Quick Start

### 1. Install and Setup

```bash
# From project root
cd ethereum-client

# Install development dependencies
make install-deps

# Set up environment (copy and edit .env file)
make setup-env
```

### 2. Run Basic Tests

```bash
# Run all tests
make test

# Or use the test script
./test_commands.sh all
```

## Test Categories

### 🔬 Unit Tests (`make test-unit`)

**What they test:**
- Configuration validation
- Type serialization/deserialization  
- Error handling
- URL building logic
- Merkle proof verification logic

**No external dependencies required**

```bash
# Run unit tests
cargo test

# Or with make
make test-unit

# Run specific test
cargo test test_config_validation
```

### 🎭 Mock Integration Tests (`make test-mock`)

**What they test:**
- Configuration setup
- Type conversions
- Error scenarios  
- Serialization roundtrips

**No network or API keys required**

```bash
# Run mock integration
cargo run --example mock_integration

# Or with make
make test-mock
```

### 🔧 Basic Usage Test (`make test-basic`)

**What they test:**
- Network connectivity via Alchemy
- Contract interaction (read operations)
- Configuration loading from environment

**Requirements:**
- Valid `ALCHEMY_API_KEY`
- Valid `ARITHMETIC_CONTRACT_ADDRESS`
- Valid `VERIFIER_CONTRACT_ADDRESS`

```bash
# Set up environment first
cp .env.example .env
# Edit .env with your values

# Run basic usage test
cargo run --example basic_usage

# Or with make
make test-basic
```

### 🖥️ CLI Tests (`make test-cli`)

**What they test:**
- CLI argument parsing
- Command execution
- Read-only operations

```bash
# Test CLI help
cargo run --bin ethereum_service -- --help

# Test network stats
cargo run --bin ethereum_service network-stats

# Test with script
./test_commands.sh cli
```

## Environment Setup

### Required Environment Variables

Create `.env` file from template:

```bash
cp .env.example .env
```

**Minimal setup for testing:**
```env
ALCHEMY_API_KEY=your_actual_alchemy_api_key
ARITHMETIC_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890  
VERIFIER_CONTRACT_ADDRESS=0x0987654321098765432109876543210987654321
ETHEREUM_NETWORK=sepolia  # or mainnet, base, etc.
```

**Full setup for write operations:**
```env
# Add signer for state publishing tests
PRIVATE_KEY=your_private_key_here
SIGNER_ADDRESS=0xYourSignerAddressHere

# Optional database for caching tests
DATABASE_URL=postgresql://postgres:password@localhost:5432/ethereum_cache
```

### Network Configurations

**Sepolia (Recommended for testing):**
```env
ETHEREUM_NETWORK=sepolia
CHAIN_ID=11155111
```

**Base Sepolia:**
```env
ETHEREUM_NETWORK=base-sepolia
CHAIN_ID=84532
```

**Mainnet (Production):**
```env
ETHEREUM_NETWORK=mainnet
CHAIN_ID=1
```

## Test Scenarios

### 1. Configuration Testing

```bash
# Test config validation
cargo test test_config_validation

# Test environment loading
cargo test test_config_from_env
```

### 2. Network Connectivity

```bash
# Test basic connectivity
cargo run --example basic_usage

# Test specific network stats
./ethereum-client/test_commands.sh cli
```

### 3. State Operations

**Read Operations (safe):**
```bash
# Test state reading
cargo run --bin ethereum_service get-state \
  --state-id 0x0000000000000000000000000000000000000000000000000000000000000001

# Test historical states  
cargo run --bin ethereum_service get-history \
  --state-id 0x0000000000000000000000000000000000000000000000000000000000000001 \
  --limit 10
```

**Write Operations (requires signer & gas):**
```bash
# ⚠️ This will attempt to send a transaction
cargo run --bin ethereum_service publish-state \
  --state-id 0x1234567890123456789012345678901234567890123456789012345678901234 \
  --state-root 0x5678901234567890123456789012345678901234567890123456789012345678 \
  --proof 0xabcdef... \
  --public-values 0x123456...
```

### 4. Proof Verification

```bash
# Test proof verification (will fail with mock data)
cargo run --bin ethereum_service verify-proof \
  --proof 0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 \
  --public-values 0x0000000000000000000000000000000000000000000000000000000000000000

# Test inclusion proof
cargo run --bin ethereum_service check-inclusion \
  --leaf-hash 0x1234567890123456789012345678901234567890123456789012345678901234 \
  --leaf-index 0 \
  --siblings 0x5678901234567890123456789012345678901234567890123456789012345678,0x9abc... \
  --root 0xabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789
```

## Troubleshooting

### Common Issues

**1. "ALCHEMY_API_KEY is required"**
```bash
# Solution: Set up environment
cp .env.example .env
# Edit .env with your Alchemy API key
```

**2. "Invalid address format"**
```bash
# Ensure addresses are 42 characters starting with 0x
ARITHMETIC_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
```

**3. "Network connection failed"**
```bash
# Check your internet connection and API key
curl -X POST https://eth-sepolia.g.alchemy.com/v2/YOUR_API_KEY \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

**4. "Transaction failed" (for write operations)**
- Ensure you have testnet ETH in your signer wallet
- Check gas price and limits
- Verify contract addresses are correct
- Ensure your proof data is valid

### Debug Mode

Enable debug logging:
```bash
RUST_LOG=debug cargo run --example basic_usage
```

Trace level logging:
```bash  
RUST_LOG=trace cargo run --bin ethereum_service network-stats
```

### Test Database Setup

For tests requiring database caching:

```bash
# Start test database
make setup-db

# Set environment variable
export DATABASE_URL=postgresql://postgres:password@localhost:5433/ethereum_cache

# Run tests with database features
cargo test --features database

# Cleanup when done
make cleanup-db
```

## Continuous Integration

### Local CI Simulation

```bash
# Run full CI pipeline locally
make ci

# This runs:
# - Format check: make fmt-check  
# - Linting: make lint
# - Build: make build
# - Tests: make test
```

### GitHub Actions Setup

Example `.github/workflows/ethereum-client.yml`:

```yaml
name: Ethereum Client Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run unit tests
        working-directory: ethereum-client
        run: cargo test
      
      - name: Run mock integration tests  
        working-directory: ethereum-client
        run: cargo run --example mock_integration
      
      - name: Check formatting
        working-directory: ethereum-client
        run: cargo fmt -- --check
      
      - name: Run clippy
        working-directory: ethereum-client  
        run: cargo clippy -- -D warnings
```

## Performance Testing

### Load Testing

```bash
# Test with multiple concurrent requests
for i in {1..10}; do
  cargo run --bin ethereum_service network-stats &
done
wait
```

### Memory Usage

```bash
# Monitor memory usage during tests
/usr/bin/time -v cargo test
```

## Integration with Main Project

### Running from Project Root

```bash
# From demo-vapp root directory
cd ethereum-client && make test

# Or use the test script
./ethereum-client/test_commands.sh all

# Add to main project tests
cargo test -p ethereum-client
```

### Integration with Database Module

```bash
# Test with existing database
export DATABASE_URL=postgresql://postgres:password@127.0.0.1:5432/arithmetic_db

# Run ethereum client with existing DB
cd ethereum-client && cargo run --features database --example basic_usage
```

## Best Practices

### 1. Test Isolation
- Each test should be independent
- Use random addresses/IDs to avoid conflicts
- Clean up any state changes

### 2. Environment Separation  
- Use separate API keys for testing
- Test on testnets only
- Never use mainnet for automated tests

### 3. Error Handling
- Test both success and failure cases
- Verify error messages are meaningful  
- Test network failure scenarios

### 4. Resource Management
- Limit API calls in tests
- Respect rate limits  
- Use mocks for unit tests

### 5. Documentation
- Document any external dependencies
- Include setup instructions
- Provide troubleshooting steps

## Test Coverage

Generate coverage report:

```bash
# Install cargo-tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --out Html

# Open coverage report
open tarpaulin-report.html
```

Target coverage goals:
- **Unit tests:** 90%+ coverage
- **Integration tests:** Cover all major workflows  
- **Error paths:** Test all error conditions