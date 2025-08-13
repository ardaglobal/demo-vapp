# Bring Your Own Proving Key (BYO-PK) with Sindri

This document explains the implementation of Bring Your Own Proving Key (BYO-PK) functionality in the Arda Global demo-vapp project, enabling you to maintain full control over your proving keys while leveraging Sindri's remote proving infrastructure.

## 🔑 What is BYO-PK?

Bring Your Own Proving Key (BYO-PK) allows you to:

1. **Generate your own proving and verification keys locally**
2. **Upload your proving key to Sindri with your circuit**
3. **Prove both locally and remotely using the same key**
4. **Maintain cryptographic control over your proof generation**

This ensures that proofs generated locally and on Sindri are cryptographically equivalent and verifiable with the same verification key.

## 📁 Repository Structure

```
demo-vapp/
├── circuits/
│   └── counter/                     # Example SP1 circuit
│       ├── src/main.rs             # SP1 guest program
│       ├── Cargo.toml              # Circuit dependencies
│       ├── sindri.json             # Sindri manifest with BYO-PK config
│       ├── keys/                   # Generated proving/verification keys
│       │   ├── proving.key         # Your proving key (BYO-PK)
│       │   └── verifying.key       # Corresponding verification key
│       ├── inputs/                 # Example input files
│       │   └── example.json        # Sample inputs for testing
│       ├── build.sh               # Build ELF and generate keys
│       ├── prove.sh               # Generate proof locally
│       ├── verify_local.sh        # Verify proof locally
│       ├── deploy_sindri.sh       # Upload circuit+keys to Sindri
│       ├── prove_sindri.sh        # Request proof from Sindri
│       └── verify_compare.sh      # Compare local vs Sindri results
├── contracts/
│   ├── src/
│   │   ├── SP1VerifierGateway.sol  # Enhanced verifier with BYO-PK support
│   │   ├── VAppStateManager.sol    # vApp state management
│   │   └── Arithmetic.sol          # Original arithmetic contract
│   └── ...
├── scripts/
│   └── ci_bootstrap.sh            # CI helper functions
├── .github/workflows/
│   └── ci-zk.yml                  # GitHub Actions CI pipeline
├── sindri.json                    # Root Sindri configuration
└── BYO_PK_README.md              # This file
```

## 🚀 Quick Start

### 1. Build Circuit and Generate Keys

```bash
cd circuits/counter
./build.sh
```

This will:
- Compile the SP1 guest program to RISC-V ELF
- Generate your proving key and verification key
- Prepare files for Sindri upload

### 2. Test Local Proving

```bash
./prove.sh inputs/example.json
./verify_local.sh
```

### 3. Deploy to Sindri with BYO-PK

```bash
export SINDRI_API_KEY="your-sindri-api-key"
./deploy_sindri.sh
```

### 4. Generate Sindri Proof

```bash
./prove_sindri.sh inputs/example.json
```

### 5. Compare Results

```bash
./verify_compare.sh
```

This verifies both proofs with your local verification key and confirms public outputs match exactly.

## 🔧 Configuration

### Sindri Manifest (`sindri.json`)

The key configuration for BYO-PK is in the `artifactPaths` section:

```json
{
  "$schema": "https://sindri.app/api/v1/sindri-manifest-schema.json",
  "name": "arda-counter-v1",
  "circuitType": "sp1",
  "artifactPaths": {
    "elf": "target/riscv32im-succinct-zkvm-elf/release/counter",
    "provingKey": "keys/proving.key",        // BYO-PK: your proving key
    "verificationKey": "keys/verifying.key"  // Your verification key
  },
  "publicOutputs": [
    "prev_state_root",
    "next_state_root",
    "batch_commitment",
    "operation_result"
  ]
}
```

### Circuit Code (`src/main.rs`)

The SP1 guest program implements vApp state transitions:

```rust
pub fn main() {
    // Read inputs
    let a = sp1_zkvm::io::read::<i32>();
    let b = sp1_zkvm::io::read::<i32>();
    let prev_state_root = sp1_zkvm::io::read::<[u8; 32]>();
    let batch_data = sp1_zkvm::io::read::<Vec<u8>>();
    
    // Compute result and state transition
    let result = addition(a, b);
    let next_state_root = compute_next_state(prev_state_root, result);
    let batch_commitment = compute_commitment(batch_data, result);
    
    // Commit public outputs
    sp1_zkvm::io::commit(&result);
    sp1_zkvm::io::commit_slice(&prev_state_root);
    sp1_zkvm::io::commit_slice(&next_state_root);
    sp1_zkvm::io::commit_slice(&batch_commitment);
}
```

## 🏗️ Build Process

### Local Build (`build.sh`)

1. **Compile Guest Program**:
   ```bash
   cargo build --target riscv32im-succinct-zkvm-elf --release
   ```

2. **Generate Keys**:
   ```bash
   sp1 setup \
     --elf "target/riscv32im-succinct-zkvm-elf/release/counter" \
     --proving-key "keys/proving.key" \
     --verification-key "keys/verifying.key"
   ```

### Sindri Upload (`deploy_sindri.sh`)

1. **Create Bundle**:
   - Source code (`src/`, `Cargo.toml`)
   - Compiled ELF binary
   - **Your proving key** (`keys/proving.key`)
   - Verification key (`keys/verifying.key`)
   - Sindri manifest (`sindri.json`)

2. **Upload to Sindri**:
   ```bash
   sindri circuits create \
     --api-key "$SINDRI_API_KEY" \
     --file "counter-circuit.tgz"
   ```

## ✅ Verification Process

### Local Verification

```bash
sp1 verify \
  --verification-key "keys/verifying.key" \
  --proof ".out/local/proof.bin"
```

### Sindri Verification

The same verification key works for Sindri proofs:

```bash
sp1 verify \
  --verification-key "keys/verifying.key" \
  --proof ".out/sindri/proof.bin"
```

### Comparison (`verify_compare.sh`)

1. **Verify both proofs** with the same verification key
2. **Compare public outputs** byte-for-byte
3. **Confirm state transitions** are identical
4. **Validate batch commitments** match exactly

## 🔐 Security Model

### What You Control

- ✅ **Proving Key Generation**: Generated locally with your randomness
- ✅ **Verification Key**: Derived from your proving key
- ✅ **Circuit Logic**: Your SP1 guest program
- ✅ **Input Data**: What gets proven

### What Sindri Provides

- 🏗️ **Infrastructure**: High-performance proving environment
- ⚡ **Speed**: Faster proof generation than local hardware
- 🔄 **Reliability**: Managed proving service
- 📊 **Monitoring**: Proof generation analytics

### Security Guarantees

1. **Same Cryptographic Security**: Proofs from local and Sindri are equivalent
2. **No Key Exposure**: Sindri cannot generate proofs without your key
3. **Verifiable Results**: All proofs verify with your verification key
4. **Audit Trail**: Complete record of all proof generation

## 🚢 On-Chain Integration

### Verification Key Registration

```solidity
// Register your verification key on-chain
SP1VerifierGateway gateway = SP1VerifierGateway(GATEWAY_ADDRESS);
bytes32 vkeyHash = keccak256(abi.encode(verificationKey));
gateway.registerVerificationKey(verificationKey, "arda-counter-v1");
```

### vApp State Updates

```solidity
// Update vApp state with verified proof
VAppStateManager stateManager = VAppStateManager(STATE_MANAGER_ADDRESS);
stateManager.updateVAppState(
    vappId,
    prevStateRoot,
    nextStateRoot,
    batchCommitment,
    operationResult,
    publicValues,
    proofBytes  // Works with both local and Sindri proofs!
);
```

## 🔄 CI/CD Pipeline

### GitHub Actions Workflow (`.github/workflows/ci-zk.yml`)

The CI pipeline tests the complete BYO-PK flow:

1. **Build**: Compile circuit and generate keys
2. **Local Prove**: Generate proof locally
3. **Local Verify**: Verify local proof
4. **Sindri Deploy**: Upload circuit with BYO-PK
5. **Sindri Prove**: Generate proof on Sindri
6. **Compare**: Verify both proofs match exactly

### Environment Variables

```bash
# Required for Sindri integration
SINDRI_API_KEY="your-sindri-api-key"

# Optional: Git commit for traceability
GIT_SHA="$(git rev-parse HEAD)"
```

## 📊 Monitoring and Analytics

### Verification Statistics

Track proof generation and verification:

```solidity
// Get verification stats for your circuit
(uint256 count, uint256 lastTime) = gateway.getVerificationStats(vkeyHash);
```

### Performance Metrics

- **Local Proving Time**: Measured in scripts
- **Sindri Proving Time**: From Sindri API response
- **Verification Time**: Consistent for both proof types
- **Proof Size**: Should be identical for local vs Sindri

## 🛠️ Troubleshooting

### Common Issues

1. **ELF Not Found**
   ```bash
   # Ensure you're building with the correct target
   rustup target add riscv32im-succinct-zkvm-elf
   ```

2. **Key Generation Fails**
   ```bash
   # Check SP1 CLI installation
   sp1 --version
   ```

3. **Sindri Upload Fails**
   ```bash
   # Verify API key is set
   echo $SINDRI_API_KEY
   ```

4. **Verification Mismatch**
   ```bash
   # Check that the same ELF was used for both proofs
   sha256sum target/riscv32im-succinct-zkvm-elf/release/counter
   ```

### Debug Mode

Enable verbose logging in scripts:

```bash
export DEBUG=1
./build.sh
```

## 🚀 Production Deployment

### Key Management

1. **Secure Storage**: Store proving keys in secure key management systems
2. **Version Control**: Track key versions with circuit updates
3. **Backup Strategy**: Maintain secure backups of proving keys
4. **Access Control**: Limit who can access proving keys

### Monitoring

1. **Proof Generation**: Monitor success rates and timing
2. **Verification**: Track on-chain verification events
3. **State Updates**: Monitor vApp state transitions
4. **Cost Analysis**: Compare local vs Sindri proving costs

### Scaling

1. **Multiple Circuits**: Each circuit has its own BYO-PK
2. **Key Rotation**: Process for updating keys when needed
3. **Load Balancing**: Mix of local and Sindri proving based on demand
4. **Caching**: Cache verification keys on-chain for gas efficiency

## 📚 References

- [SP1 Documentation](https://docs.succinct.xyz/)
- [Sindri API Documentation](https://sindri.app/docs/)
- [SP1 Contracts](https://github.com/succinctlabs/sp1-contracts)
- [Arda Global vApp Paper](https://arda.global/vapp-paper)

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on contributing to this BYO-PK implementation.

---

**🎉 Ready to prove with your own keys!**

This BYO-PK implementation gives you the best of both worlds: full cryptographic control over your proving keys while leveraging Sindri's powerful proving infrastructure. Your keys, your proofs, your control.
