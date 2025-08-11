# State Management Integration Examples

This directory contains comprehensive examples demonstrating how to integrate with the Arithmetic contract's state management functionality.

## Overview

The state management system provides a complete solution for storing, reading, and validating zero-knowledge proof-verified state transitions. These examples show best practices for integration patterns that are secure, gas-efficient, and maintainable.

## Example Contracts

### 1. StateConsumer.sol - Reading State Data

The `StateConsumer` contract demonstrates how to safely read state data from the Arithmetic contract.

**Key Features:**
- Safe state reading with existence checks
- Local caching for gas optimization
- Batch reading operations
- Authorization controls
- Comprehensive error handling

**Usage Example:**
```solidity
// Deploy consumer pointing to arithmetic contract
StateConsumer consumer = new StateConsumer(arithmeticContractAddress);

// Read a single state
bytes32 stateId = keccak256("user_balance_001");
(bytes32 value, bool exists, bool fromCache) = consumer.readState(stateId);

// Batch read multiple states
bytes32[] memory stateIds = new bytes32[](5);
// ... populate stateIds ...
(bytes32[] memory values, bool[] memory existsArray, uint256 cacheHits) = 
    consumer.batchReadStates(stateIds);
```

### 2. StateUpdater.sol - Posting State Updates

The `StateUpdater` contract shows how to post state updates with proper validation and access control.

**Key Features:**
- Pre-validation of proofs before submission
- Queue-based batch processing for gas optimization
- Comprehensive access control
- Update statistics and monitoring
- Error recovery mechanisms

**Usage Example:**
```solidity
// Deploy updater pointing to arithmetic contract
StateUpdater updater = new StateUpdater(arithmeticContractAddress);

// Submit a single state update
bytes32 stateId = keccak256("user_balance_001");
bytes32 newState = keccak256(abi.encodePacked(newBalance));
(bytes32 updateId, bool success) = updater.submitStateUpdate(
    stateId, newState, zkProof, computationResult
);

// Submit batch updates
(bytes32[] memory updateIds, bool[] memory successes) = 
    updater.submitBatchUpdates(stateIds, newStates, proofs, results);
```

### 3. ProofReader.sol - Accessing Stored Proofs

The `ProofReader` contract demonstrates how to read and validate stored zero-knowledge proofs.

**Key Features:**
- Reading proofs with verification status
- Proof enumeration and filtering
- Metadata access and validation
- Batch proof operations
- Cache management for gas efficiency

**Usage Example:**
```solidity
// Deploy reader pointing to arithmetic contract
ProofReader reader = new ProofReader(arithmeticContractAddress);

// Read a single proof
bytes32 proofId = keccak256(someProofBytes);
(bytes memory proof, bool exists, bool verified, bool fromCache) = 
    reader.readProof(proofId);

// Read proof with its verification result
(bytes memory proof, bool verified, bytes memory result) = 
    reader.readProofWithResult(proofId);

// Check verification status only
bool isValid = reader.isProofVerified(proofId);
```

## Integration Best Practices

### 1. Error Handling Strategies

**Always Check Existence:**
```solidity
(bytes32 state, bool exists) = stateManager.readCurrentState(stateId);
require(exists, "State not found");
```

**Use Try-Catch for External Calls:**
```solidity
try stateManager.getCurrentState(stateId) returns (bytes32 state) {
    // Process state
} catch Error(string memory reason) {
    // Handle error gracefully
    emit StateReadError(stateId, reason);
}
```

**Validate Array Lengths:**
```solidity
require(
    stateIds.length == newStates.length && 
    stateIds.length == proofs.length,
    "Array length mismatch"
);
```

### 2. Gas Optimization Tips

**Use Batch Operations:**
```solidity
// Instead of multiple individual calls
for (uint i = 0; i < stateIds.length; i++) {
    getCurrentState(stateIds[i]); // Expensive!
}

// Use batch reading
bytes32[] memory states = batchReadStates(stateIds); // Much cheaper!
```

**Implement Caching:**
```solidity
mapping(bytes32 => CachedState) public stateCache;

struct CachedState {
    bytes32 value;
    uint256 timestamp;
    bool exists;
}
```

**Optimize Storage Patterns:**
```solidity
// Pack related data into structs
struct StateInfo {
    bytes32 value;    // 32 bytes
    bool exists;      // 1 byte
    uint32 timestamp; // 4 bytes
    // Total: 37 bytes - fits in 2 storage slots
}
```

### 3. Access Control Integration

**Multi-layered Authorization:**
```solidity
modifier onlyAuthorized() {
    require(
        authorizedUsers[msg.sender] || 
        stateManager.isAuthorized(msg.sender),
        "Unauthorized"
    );
    _;
}
```

**Role-based Permissions:**
```solidity
mapping(address => uint256) public userRoles;

uint256 constant READER_ROLE = 1;
uint256 constant UPDATER_ROLE = 2;
uint256 constant ADMIN_ROLE = 4;

function hasRole(address user, uint256 role) public view returns (bool) {
    return (userRoles[user] & role) != 0;
}
```

### 4. Monitoring and Analytics

**Track Usage Statistics:**
```solidity
struct UsageStats {
    uint256 totalReads;
    uint256 totalWrites;
    uint256 errorCount;
    uint256 gasUsed;
}
```

**Emit Detailed Events:**
```solidity
event StateOperation(
    string indexed operationType,
    bytes32 indexed stateId,
    address indexed user,
    uint256 gasUsed,
    bool success
);
```

## Common Pitfalls to Avoid

### 1. Insufficient Error Handling

❌ **Bad:**
```solidity
bytes32 state = stateManager.getCurrentState(stateId);
// Assumes state always exists - dangerous!
```

✅ **Good:**
```solidity
try stateManager.getCurrentState(stateId) returns (bytes32 state) {
    if (state == bytes32(0)) {
        revert StateNotFound(stateId);
    }
    // Process state safely
} catch {
    revert StateReadFailed(stateId);
}
```

### 2. Gas Inefficient Patterns

❌ **Bad:**
```solidity
// Reading states one by one in a loop
for (uint i = 0; i < 100; i++) {
    bytes32 state = stateManager.getCurrentState(stateIds[i]);
    // Process state
}
```

✅ **Good:**
```solidity
// Batch read all states at once
bytes32[] memory states = stateManager.batchReadStates(stateIds);
for (uint i = 0; i < states.length; i++) {
    // Process state
}
```

### 3. Missing Validation

❌ **Bad:**
```solidity
function updateState(bytes32 stateId, bytes32 newState, bytes calldata proof) external {
    stateManager.updateState(stateId, newState, proof, result);
    // No validation of inputs!
}
```

✅ **Good:**
```solidity
function updateState(bytes32 stateId, bytes32 newState, bytes calldata proof) external {
    require(stateId != bytes32(0), "Invalid state ID");
    require(newState != bytes32(0), "Invalid new state");
    require(proof.length > 0, "Empty proof");
    require(proof.length <= MAX_PROOF_SIZE, "Proof too large");
    
    stateManager.updateState(stateId, newState, proof, result);
}
```

### 4. Inadequate Access Control

❌ **Bad:**
```solidity
function updateState(bytes32 stateId, bytes32 newState) external {
    // Anyone can update any state!
    stateManager.updateState(stateId, newState, proof, result);
}
```

✅ **Good:**
```solidity
function updateState(bytes32 stateId, bytes32 newState) external onlyAuthorized {
    require(canUpdateState(msg.sender, stateId), "Unauthorized for this state");
    stateManager.updateState(stateId, newState, proof, result);
}
```

## Testing Integration

### Test Contract Deployment

```solidity
// In your test setup
function setUp() public {
    // Deploy arithmetic contract
    arithmetic = new Arithmetic(verifier, vkey);
    
    // Deploy integration contracts
    consumer = new StateConsumer(address(arithmetic));
    updater = new StateUpdater(address(arithmetic));
    reader = new ProofReader(address(arithmetic));
    
    // Setup authorizations
    arithmetic.setAuthorization(address(updater), true);
    consumer.setAuthorization(testUser, true);
}
```

### Integration Test Examples

```solidity
function test_Integration_CompleteWorkflow() public {
    // 1. Update state through updater
    vm.prank(authorizedUser);
    (bytes32 updateId, bool success) = updater.submitStateUpdate(
        stateId, newState, proof, result
    );
    assertTrue(success);
    
    // 2. Read state through consumer
    vm.prank(reader);
    (bytes32 readState, bool exists,) = consumer.readState(stateId);
    assertTrue(exists);
    assertEq(readState, newState);
    
    // 3. Verify proof through proof reader
    bytes32 proofId = keccak256(proof);
    assertTrue(proofReader.isProofVerified(proofId));
}
```

## Deployment Considerations

### 1. Gas Limits

- Single state update: ~200,000 - 400,000 gas
- Batch update (10 items): ~2,000,000 - 3,000,000 gas
- State reading: ~5,000 - 25,000 gas
- Batch read (10 items): ~50,000 - 150,000 gas

### 2. Contract Size Optimization

If approaching contract size limits, consider:
- Splitting functionality into multiple contracts
- Using proxy patterns for upgradeability
- Implementing function selectors for delegatecall patterns

### 3. Network Considerations

**Mainnet:**
- Optimize for gas costs
- Use batch operations heavily
- Implement comprehensive caching

**L2 Networks:**
- Can afford more frequent individual operations
- Still benefit from batching for UX
- Consider cross-chain state synchronization

## Security Considerations

### 1. Reentrancy Protection

```solidity
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract MyStateUpdater is ReentrancyGuard {
    function updateState(...) external nonReentrant {
        // Safe from reentrancy attacks
    }
}
```

### 2. Proof Validation

```solidity
function validateProof(bytes calldata proof) internal view {
    require(proof.length >= MIN_PROOF_SIZE, "Proof too small");
    require(proof.length <= MAX_PROOF_SIZE, "Proof too large");
    // Add more validation as needed
}
```

### 3. State Consistency

```solidity
function updateState(bytes32 stateId, bytes32 newState) external {
    bytes32 currentState = stateManager.getCurrentState(stateId);
    
    // Verify state transition is valid
    require(isValidTransition(currentState, newState), "Invalid transition");
    
    stateManager.updateState(stateId, newState, proof, result);
}
```

These examples provide a complete foundation for integrating with the state management system while following security best practices and optimizing for gas efficiency.