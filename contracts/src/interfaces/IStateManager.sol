// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title IStateManager
 * @author Arithmetic ZK System
 * @notice Interface for managing state transitions with zero-knowledge proof verification
 * @dev This interface defines the contract for state management operations including
 *      single and batch updates, proof storage, and verification results
 */
interface IStateManager {
    // ============ Events ============
    
    /**
     * @notice Emitted when a state is successfully updated
     * @param stateId The unique identifier of the state
     * @param oldState The previous state value
     * @param newState The new state value
     * @param proofId The unique identifier of the proof used for verification
     */
    event StateUpdated(bytes32 indexed stateId, bytes32 oldState, bytes32 newState, bytes32 indexed proofId);
    
    /**
     * @notice Emitted when multiple states are updated in a batch operation
     * @param stateIds Array of state identifiers that were updated
     * @param proofCount Number of proofs processed in the batch
     */
    event BatchStateUpdated(bytes32[] stateIds, uint256 proofCount);
    
    /**
     * @notice Emitted when a proof is stored in the contract
     * @param proofId The unique identifier of the stored proof
     * @param verified Whether the proof was successfully verified
     */
    event ProofStored(bytes32 indexed proofId, bool verified);

    // ============ Core State Functions ============
    
    /**
     * @notice Updates a single state with zero-knowledge proof verification
     * @dev Verifies the provided proof before updating the state. The proof must demonstrate
     *      that the state transition is valid according to the implemented logic.
     * @param stateId The unique identifier of the state to update
     * @param newState The new state value to set
     * @param proof The zero-knowledge proof bytes for verification
     * @param result The expected computation result that the proof validates
     * 
     * Requirements:
     * - `stateId` must be a valid state identifier
     * - `proof` must be a valid SP1 proof that verifies successfully
     * - `result` must match the expected computation output
     * 
     * Gas Cost: ~150,000 - 300,000 gas depending on proof complexity
     * 
     * @custom:example
     * ```solidity
     * bytes32 stateId = keccak256("user_balance_001");
     * bytes32 newState = keccak256(abi.encodePacked(newBalance));
     * stateManager.updateState(stateId, newState, zkProof, computationResult);
     * ```
     */
    function updateState(
        bytes32 stateId, 
        bytes32 newState, 
        bytes calldata proof, 
        bytes calldata result
    ) external;
    
    /**
     * @notice Retrieves the current state value for a given state identifier
     * @dev Returns the most recently updated state value. Returns bytes32(0) if state doesn't exist.
     * @param stateId The unique identifier of the state to query
     * @return The current state value as bytes32
     * 
     * Gas Cost: ~2,100 gas (single SLOAD operation)
     * 
     * @custom:example
     * ```solidity
     * bytes32 currentBalance = stateManager.getCurrentState(userStateId);
     * require(currentBalance != bytes32(0), "State not initialized");
     * ```
     */
    function getCurrentState(bytes32 stateId) external view returns (bytes32);
    
    /**
     * @notice Retrieves a stored proof by its unique identifier
     * @dev Returns the complete proof bytes that were stored during updateState call
     * @param proofId The unique identifier of the proof
     * @return The proof bytes as stored in the contract
     * 
     * Gas Cost: ~3,000 - 50,000 gas depending on proof size
     * 
     * @custom:example
     * ```solidity
     * bytes32 proofId = keccak256(abi.encodePacked(stateId, blockNumber));
     * bytes memory storedProof = stateManager.getStoredProof(proofId);
     * ```
     */
    function getStoredProof(bytes32 proofId) external view returns (bytes memory);
    
    /**
     * @notice Retrieves a stored computation result by proof identifier
     * @dev Returns the computation result that was validated by the proof
     * @param proofId The unique identifier of the proof
     * @return The computation result bytes as stored in the contract
     * 
     * Gas Cost: ~3,000 - 20,000 gas depending on result size
     * 
     * @custom:example
     * ```solidity
     * bytes memory result = stateManager.getStoredResult(proofId);
     * uint256 computedSum = abi.decode(result, (uint256));
     * ```
     */
    function getStoredResult(bytes32 proofId) external view returns (bytes memory);

    // ============ Batch Operations Interface ============
    
    /**
     * @notice Updates multiple states in a single transaction with batch proof verification
     * @dev Processes multiple state updates atomically. If any proof fails, entire batch reverts.
     *      Provides significant gas savings compared to individual updateState calls.
     * @param stateIds Array of unique state identifiers to update
     * @param newStates Array of new state values corresponding to stateIds
     * @param proofs Array of zero-knowledge proofs for each state transition
     * @param results Array of computation results that each proof validates
     * 
     * Requirements:
     * - All arrays must have the same length
     * - Each proof must verify successfully
     * - Maximum batch size is typically 50-100 items due to gas limits
     * 
     * Gas Cost: ~100,000 + (200,000 * number of states) gas
     * 
     * @custom:example
     * ```solidity
     * bytes32[] memory ids = new bytes32[](3);
     * bytes32[] memory states = new bytes32[](3);
     * bytes[] memory proofArray = new bytes[](3);
     * bytes[] memory resultArray = new bytes[](3);
     * // ... populate arrays ...
     * stateManager.batchUpdateStates(ids, states, proofArray, resultArray);
     * ```
     */
    function batchUpdateStates(
        bytes32[] calldata stateIds,
        bytes32[] calldata newStates,
        bytes[] calldata proofs,
        bytes[] calldata results
    ) external;
    
    /**
     * @notice Reads multiple state values in a single call
     * @dev Efficient batch reading of state values. Much more gas-efficient than multiple getCurrentState calls.
     * @param stateIds Array of state identifiers to read
     * @return Array of current state values corresponding to the input stateIds
     * 
     * Gas Cost: ~21,000 + (2,100 * number of states) gas
     * 
     * @custom:example
     * ```solidity
     * bytes32[] memory userStates = new bytes32[](10);
     * // ... populate with user state IDs ...
     * bytes32[] memory currentStates = stateManager.batchReadStates(userStates);
     * ```
     */
    function batchReadStates(bytes32[] calldata stateIds) external view returns (bytes32[] memory);

    // ============ Proof Management Interface ============
    
    /**
     * @notice Retrieves a proof by ID along with its existence status
     * @dev Returns both the proof data and whether it exists, avoiding separate existence checks
     * @param proofId The unique identifier of the proof
     * @return proof The proof bytes (empty if not found)
     * @return exists Whether the proof exists in storage
     * 
     * Gas Cost: ~5,000 gas
     * 
     * @custom:example
     * ```solidity
     * (bytes memory proof, bool exists) = stateManager.getProofById(proofId);
     * require(exists, "Proof not found");
     * ```
     */
    function getProofById(bytes32 proofId) external view returns (bytes memory proof, bool exists);
    
    /**
     * @notice Checks if a proof has been successfully verified
     * @dev Returns verification status without returning the actual proof data
     * @param proofId The unique identifier of the proof
     * @return Whether the proof exists and has been verified successfully
     * 
     * Gas Cost: ~2,100 gas
     * 
     * @custom:example
     * ```solidity
     * bool isValid = stateManager.isProofVerified(proofId);
     * require(isValid, "Invalid or unverified proof");
     * ```
     */
    function isProofVerified(bytes32 proofId) external view returns (bool);
    
    /**
     * @notice Gets comprehensive verification information for a proof
     * @dev Returns both verification status and the computation result in one call
     * @param proofId The unique identifier of the proof
     * @return verified Whether the proof was successfully verified
     * @return result The computation result that was validated by the proof
     * 
     * Gas Cost: ~5,000 - 25,000 gas depending on result size
     * 
     * @custom:example
     * ```solidity
     * (bool verified, bytes memory result) = stateManager.getVerificationResult(proofId);
     * require(verified, "Proof verification failed");
     * uint256 sum = abi.decode(result, (uint256));
     * ```
     */
    function getVerificationResult(bytes32 proofId) external view returns (bool verified, bytes memory result);
}