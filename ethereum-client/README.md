# Ethereum Client

A comprehensive Rust client for interacting with Ethereum networks via Alchemy, specifically designed for SP1 zero-knowledge proof vApp state management.

## Features

- **State Root Publishing**: Publish state roots to smart contracts with ZK proof verification
- **Proof Verification**: Verify ZK proofs both on-chain and off-chain
- **State Reading**: Read current and historical state data
- **Inclusion Proofs**: Generate and verify Merkle inclusion proofs
- **Event Monitoring**: Real-time monitoring of contract events
- **Multi-Network Support**: Ethereum mainnet, Sepolia, Base, Arbitrum, Optimism and their testnets
- **Alchemy Integration**: Optimized for Alchemy's enhanced APIs
- **Database Caching**: Optional PostgreSQL integration for performance
- **Batch Operations**: Gas-efficient batch state updates and reads

## Quick Start

### 1. Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ethereum-client = { path = "../ethereum-client" }
```

### 2. Configuration

Copy the environment template:

```bash
cp .env.example .env
```

Configure your environment variables:

```env
# Required
ALCHEMY_API_KEY=your_alchemy_api_key_here
ARITHMETIC_CONTRACT_ADDRESS=0x1234567890123456789012345678901234567890
VERIFIER_CONTRACT_ADDRESS=0x0987654321098765432109876543210987654321

# Optional
ETHEREUM_NETWORK=sepolia
PRIVATE_KEY=your_private_key_for_write_operations
DATABASE_URL=postgresql://user:pass@localhost:5432/ethereum_cache
```

### 3. Usage

#### As a Library

```rust
use ethereum_client::{Config, EthereumClient, StateId, StateRoot};
use alloy_primitives::{Bytes, FixedBytes};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::from_env()?;
    
    // Create client
    let client = EthereumClient::new(config).await?;
    
    // Publish state root
    let state_id = StateId::random();
    let state_root = StateRoot::random();
    let proof = Bytes::from(vec![1, 2, 3, 4]);
    let public_values = Bytes::from(vec![5, 6, 7, 8]);
    
    let result = client.publish_state_root(
        state_id, 
        state_root, 
        proof, 
        public_values
    ).await?;
    
    println!("Published state with TX: {:?}", result.transaction_hash);
    
    // Read current state
    let current_state = client.get_current_state(state_id).await?;
    println!("Current state root: {:?}", current_state.state_root);
    
    Ok(())
}
```

#### As a CLI Tool

```bash
# Monitor contract events
cargo run --bin ethereum_service monitor

# Publish state root
cargo run --bin ethereum_service publish-state \
  --state-id 0x1234... \
  --state-root 0x5678... \
  --proof 0xabcd... \
  --public-values 0xef01...

# Get current state
cargo run --bin ethereum_service get-state \
  --state-id 0x1234...

# Get network statistics  
cargo run --bin ethereum_service network-stats

# Verify ZK proof
cargo run --bin ethereum_service verify-proof \
  --proof 0xabcd... \
  --public-values 0xef01...
```

## API Reference

### Core Methods

#### State Management

```rust
// Publish single state root
async fn publish_state_root(
    &self,
    state_id: StateId,
    new_state_root: StateRoot,
    proof: Bytes,
    public_values: Bytes,
) -> Result<StateUpdate>

// Batch publish state roots (gas efficient)
async fn batch_publish_state_roots(
    &self,
    updates: Vec<(StateId, StateRoot, Bytes, Bytes)>,
) -> Result<BatchStateUpdate>

// Get current state
async fn get_current_state(&self, state_id: StateId) -> Result<StateResponse>

// Get historical states
async fn get_historical_states(
    &self, 
    state_id: StateId, 
    limit: Option<u64>
) -> Result<HistoricalState>
```

#### Proof Operations

```rust
// Verify ZK proof
async fn verify_zk_proof(
    &self,
    proof: Bytes,
    public_values: Bytes,
) -> Result<ProofVerificationResult>

// Check Merkle inclusion proof
async fn check_inclusion_proof(
    &self,
    leaf_hash: FixedBytes<32>,
    leaf_index: u64,
    siblings: Vec<FixedBytes<32>>,
    root: StateRoot,
) -> Result<InclusionProof>
```

#### Monitoring

```rust
// Monitor contract events (runs indefinitely)
async fn monitor_events(&self) -> Result<()>

// Get network statistics
async fn get_network_stats(&self) -> Result<NetworkStats>
```

## Configuration

### Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `ALCHEMY_API_KEY` | Alchemy API key | ✓ | - |
| `ETHEREUM_NETWORK` | Network name | - | `sepolia` |
| `CHAIN_ID` | Chain ID | - | Auto-detected |
| `ARITHMETIC_CONTRACT_ADDRESS` | Main contract address | ✓ | - |
| `VERIFIER_CONTRACT_ADDRESS` | SP1 verifier address | ✓ | - |
| `PRIVATE_KEY` | Private key for signing | - | - |
| `SIGNER_ADDRESS` | Signer address | - | - |
| `DEPLOYMENT_BLOCK` | Contract deployment block | - | Current - 10000 |
| `ENABLE_EVENT_MONITORING` | Enable event monitoring | - | `true` |
| `POLLING_INTERVAL_SECONDS` | Event polling interval | - | `30` |
| `MAX_BLOCK_RANGE` | Max blocks per query | - | `1000` |
| `RATE_LIMIT_PER_SECOND` | API rate limit | - | `100` |
| `DATABASE_URL` | PostgreSQL connection URL | - | - |

### Supported Networks

- **Mainnet**: Ethereum, Base, Arbitrum, Optimism
- **Testnets**: Sepolia, Base Sepolia, Arbitrum Sepolia, Optimism Sepolia

Network-specific Alchemy URLs are automatically configured based on the `ETHEREUM_NETWORK` setting.

## Database Integration

Optional PostgreSQL integration provides:

- **State Caching**: Improved query performance
- **Event History**: Long-term event storage
- **Analytics**: Network usage statistics
- **Reliability**: Backup for critical state data

### Setup

```bash
# Start PostgreSQL (Docker)
docker run -d \
  --name ethereum-cache \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=ethereum_cache \
  -p 5432:5432 \
  postgres:15

# Set DATABASE_URL
export DATABASE_URL=postgresql://postgres:password@localhost:5432/ethereum_cache
```

The database schema is automatically initialized on first run.

## Error Handling

The client uses comprehensive error types:

```rust
pub enum EthereumError {
    Provider(TransportError),
    Contract(String),
    Signer(String),
    Network(reqwest::Error),
    Config(String),
    ProofVerificationFailed(String),
    StateNotFound(String),
    AlchemyApi { status_code: u16, message: String },
    Database(sqlx::Error),
}
```

All methods return `Result<T, EthereumError>` for proper error handling.

## Gas Optimization

- **Batch Operations**: Use `batch_publish_state_roots()` for multiple updates
- **Event Filtering**: Efficient event queries with block ranges
- **Caching**: Database integration reduces redundant RPC calls
- **Retry Logic**: Automatic retry with exponential backoff

## Security Considerations

- **Private Key Management**: Keys are loaded from environment variables only
- **RPC Security**: All connections use HTTPS/WSS
- **Input Validation**: Comprehensive validation of all inputs
- **Rate Limiting**: Respects Alchemy API rate limits
- **Access Control**: Integration with contract authorization system

## Monitoring & Observability

- **Structured Logging**: JSON-formatted logs with tracing
- **Metrics**: Gas usage, transaction success rates, API latency
- **Health Checks**: Network connectivity and sync status
- **Event Tracking**: Real-time contract event monitoring

## Examples

See the `examples/` directory for complete examples:

- `basic_usage.rs` - Simple state publishing
- `batch_operations.rs` - Gas-efficient batch updates  
- `event_monitoring.rs` - Real-time event processing
- `proof_verification.rs` - ZK proof verification
- `historical_data.rs` - Querying historical states

## Testing

```bash
# Unit tests
cargo test

# Integration tests (requires running testnet)
cargo test --features integration-tests

# With database tests
cargo test --features database
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

MIT License - see LICENSE file for details