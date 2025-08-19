# SP1 Arithmetic Contracts

Smart contracts for zero-knowledge arithmetic proof verification with comprehensive state management built on [SP1](https://github.com/succinctlabs/sp1) and [SP1VerifierGateway](https://github.com/succinctlabs/sp1-contracts/blob/main/contracts/src/SP1VerifierGateway.sol).

## Contract Architecture

- **Arithmetic.sol**: Main contract implementing SP1 proof verification with state management
- **interfaces/IStateManager.sol**: Standardized interface for state operations and proof management
- **examples/**: Production-ready integration contracts
  - **StateConsumer.sol**: Safe state reading with caching and batch operations
  - **StateUpdater.sol**: State updates with validation and queue-based processing
  - **ProofReader.sol**: Proof access with verification and enumeration
- **test/StateManagement.t.sol**: Comprehensive test suite for state management functionality

## Requirements

- [Foundry](https://book.getfoundry.sh/getting-started/installation)

## Test

```sh
# Run all tests
forge test -v

# Test state management functionality specifically
forge test --match-contract StateManagementTest -vv

# Test integration examples
forge test --match-contract StateConsumerTest
forge test --match-contract StateUpdaterTest
forge test --match-contract ProofReaderTest
```

## Usage

The Arithmetic contract provides complete state management with ZK proof verification:

```solidity
// Update state with ZK proof verification
arithmetic.postStateUpdate(stateId, newState, proof, publicValues);

// Read current state
bytes32 currentState = arithmetic.getCurrentState(stateId);

// Batch operations (gas efficient)
arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
bytes32[] memory states = arithmetic.batchReadStates(stateIds);

// Proof verification
bool verified = arithmetic.isProofVerified(proofId);
bytes memory storedProof = arithmetic.getStoredProof(proofId);
```

See `examples/README.md` for detailed integration patterns and best practices.

## Deployment

Included is an example `.env.example` file that you can use to deploy the contract to Sepolia.

#### Step 1: Set the `VERIFIER` environment variable

Find the address of the `verifier` to use from the [deployments](https://docs.succinct.xyz/docs/sp1/verification/contract-addresses) list for the chain you are deploying to. Set it to the `VERIFIER` environment variable, for example:

```sh
VERIFIER=0x397A5f7f3dBd538f23DE225B51f532c34448dA9B
```

Note: you can use either the [SP1VerifierGateway](https://github.com/succinctlabs/sp1-contracts/blob/main/contracts/src/SP1VerifierGateway.sol) or a specific version, but it is highly recommended to use the gateway as this will allow you to use different versions of SP1.

#### Step 2: Set the `PROGRAM_VKEY` environment variable

This step is required for the contract to be able to verify proofs.

Find your program verification key by going into the `../script` directory and running `RUST_LOG=info cargo run --package arithmetic-script --release -- --vkey`, which will print an output like:

> Program Verification Key: 0x00620892344c310c32a74bf0807a5c043964264e4f37c96a10ad12b5c9214e0e

Then set the `PROGRAM_VKEY` environment variable to the output of that command, for example:

```sh
PROGRAM_VKEY=0x00620892344c310c32a74bf0807a5c043964264e4f37c96a10ad12b5c9214e0e
```

#### Step 3: Deploy the contract

Fill out the rest of the details needed for deployment:

```sh
RPC_URL=...
```

```sh
PRIVATE_KEY=...
```

Then deploy the contract to the chain:

```sh
forge create src/Arithmetic.sol:Arithmetic --broadcast --rpc-url $RPC_URL --private-key $PRIVATE_KEY --constructor-args $VERIFIER $PROGRAM_VKEY
```

It can also be a good idea to verify the contract when you deploy, in which case you would also need to set `ETHERSCAN_API_KEY`:

```sh
forge create src/Arithmetic.sol:Arithmetic --broadcast --rpc-url $SEPOLIA_RPC_URL --private-key $METAMASK_PRIVATE_KEY --constructor-args $SEPOLIA_GROTH16_VERIFIER $PROGRAM_VKEY --verify --verifier etherscan --etherscan-api-key $ETHERSCAN_API_KEY
```

## GitHub Actions Deployment

The repository includes a GitHub Actions workflow that can deploy contracts automatically or manually:

### Automatic Deployment
The workflow automatically triggers when:
- Changes are pushed to the `main` branch in the `contracts/` directory
- Pull requests are opened against `main` with changes in the `contracts/` directory

Automatic deployments will:
- Deploy to Sepolia testnet by default
- Verify the contract on Etherscan
- Run tests before deployment

### Manual Deployment
You can also trigger deployments manually and choose whether to verify the contract.

### Required GitHub Secrets

The following secrets need to be configured in your GitHub repository:

#### Core Secrets
- `PRIVATE_KEY`: The private key of the deployer account
- `PROGRAM_VKEY`: The program verification key (get this by running `cargo run --package arithmetic-script --release -- --vkey` in the `../script` directory)

#### Sepolia Network Secrets
- `SEPOLIA_RPC_URL`: RPC URL for Sepolia testnet
- `SEPOLIA_VERIFIER`: SP1 verifier contract address on Sepolia
- `ETHERSCAN_API_KEY`: API key for Etherscan (used for contract verification)

### Usage

1. Go to your repository's Actions tab
2. Select "Deploy Smart Contracts" workflow
3. Click "Run workflow"
4. Choose whether to verify the contract (defaults to true)
5. Click "Run workflow" to start the deployment

The workflow will:
- Build and test the contracts
- Deploy the `Arithmetic` contract to Sepolia testnet
- Optionally verify the contract on Etherscan
