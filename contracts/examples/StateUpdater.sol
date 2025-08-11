// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IStateManager} from "../src/interfaces/IStateManager.sol";

/**
 * @title StateUpdater
 * @author Arithmetic ZK System
 * @notice Example contract demonstrating how to post state updates with proper validation
 * @dev Shows best practices for proof validation, access control, and batch operations
 * 
 * Integration patterns demonstrated:
 * - Pre-validation of proofs before submission
 * - Access control integration with state manager
 * - Batch update optimizations
 * - Error handling and recovery
 * - State transition validation
 * - Gas optimization strategies
 */
contract StateUpdater {
    // ============ State Variables ============
    
    /// @notice The state manager contract to update
    IStateManager public immutable stateManager;
    
    /// @notice Contract owner for administrative functions
    address public owner;
    
    /// @notice Authorized state updaters
    mapping(address => bool) public authorizedUpdaters;
    
    /// @notice Pending state updates waiting for execution
    mapping(bytes32 => PendingUpdate) public pendingUpdates;
    
    /// @notice Queue of pending update IDs for batch processing
    bytes32[] public updateQueue;
    
    /// @notice Mapping from update ID to queue index
    mapping(bytes32 => uint256) public updateQueueIndex;
    
    /// @notice Update statistics for monitoring
    struct UpdateStats {
        uint256 totalUpdates;
        uint256 successfulUpdates;
        uint256 failedUpdates;
        uint256 batchUpdates;
        uint256 queuedUpdates;
        uint256 processedUpdates;
    }
    
    UpdateStats public stats;
    
    /// @notice Pending update structure
    struct PendingUpdate {
        bytes32 stateId;
        bytes32 newState;
        bytes proof;
        bytes result;
        address submitter;
        uint256 timestamp;
        uint256 priority;
        bool exists;
        bool processed;
    }
    
    /// @notice Configuration for update validation
    struct UpdateConfig {
        bool enablePreValidation;
        bool enableQueueing;
        uint256 maxQueueSize;
        uint256 maxBatchSize;
        uint256 minProofSize;
        uint256 maxProofSize;
        bool requireAuthorization;
    }
    
    UpdateConfig public config;
    
    // ============ Events ============
    
    /// @notice Emitted when a state update is submitted
    event StateUpdateSubmitted(
        bytes32 indexed updateId,
        bytes32 indexed stateId,
        address indexed submitter,
        uint256 timestamp
    );
    
    /// @notice Emitted when a state update is successfully processed
    event StateUpdateProcessed(
        bytes32 indexed updateId,
        bytes32 indexed stateId,
        bool success,
        uint256 gasUsed
    );
    
    /// @notice Emitted when updates are processed in batch
    event BatchUpdateProcessed(
        uint256 indexed batchSize,
        uint256 successCount,
        uint256 failureCount,
        address indexed processor
    );
    
    /// @notice Emitted when an update is queued for later processing
    event UpdateQueued(bytes32 indexed updateId, uint256 queuePosition);
    
    /// @notice Emitted when authorization changes
    event AuthorizationChanged(address indexed account, bool authorized);
    
    /// @notice Emitted when configuration is updated
    event ConfigurationUpdated(UpdateConfig newConfig);
    
    // ============ Errors ============
    
    error UnauthorizedUpdater();
    error InvalidStateManager();
    error InvalidProofSize();
    error UpdateNotFound();
    error UpdateAlreadyProcessed();
    error QueueFull();
    error BatchSizeExceeded();
    error InvalidConfiguration();
    error PreValidationFailed(string reason);
    
    // ============ Modifiers ============
    
    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }
    
    modifier onlyAuthorized() {
        if (config.requireAuthorization && !authorizedUpdaters[msg.sender] && msg.sender != owner) {
            revert UnauthorizedUpdater();
        }
        _;
    }
    
    // ============ Constructor ============
    
    /**
     * @notice Initialize the state updater with a state manager contract
     * @param _stateManager Address of the state manager contract to update
     * 
     * @custom:example
     * ```solidity
     * address arithmeticContract = 0x1234567890123456789012345678901234567890;
     * StateUpdater updater = new StateUpdater(arithmeticContract);
     * ```
     */
    constructor(address _stateManager) {
        if (_stateManager == address(0)) revert InvalidStateManager();
        
        stateManager = IStateManager(_stateManager);
        owner = msg.sender;
        authorizedUpdaters[msg.sender] = true;
        
        // Default configuration
        config = UpdateConfig({
            enablePreValidation: true,
            enableQueueing: false,
            maxQueueSize: 100,
            maxBatchSize: 50,
            minProofSize: 32,
            maxProofSize: 10000,
            requireAuthorization: true
        });
        
        emit AuthorizationChanged(msg.sender, true);
    }
    
    // ============ Core Update Functions ============
    
    /**
     * @notice Submit a single state update with comprehensive validation
     * @param stateId The state identifier to update
     * @param newState The new state value
     * @param proof The zero-knowledge proof
     * @param result The computation result
     * @return updateId Unique identifier for tracking this update
     * @return success Whether the update was immediately processed
     * 
     * Gas Cost: ~200,000 - 400,000 gas depending on proof complexity
     * 
     * @custom:example
     * ```solidity
     * bytes32 stateId = keccak256("user_balance_001");
     * bytes32 newState = keccak256(abi.encodePacked(newBalance));
     * (bytes32 updateId, bool success) = updater.submitStateUpdate(
     *     stateId, newState, zkProof, computationResult
     * );
     * ```
     */
    function submitStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external onlyAuthorized returns (bytes32 updateId, bool success) {
        // Generate unique update ID
        updateId = keccak256(abi.encodePacked(
            stateId,
            newState,
            block.timestamp,
            msg.sender,
            stats.totalUpdates
        ));
        
        // Validate proof size
        if (proof.length < config.minProofSize || proof.length > config.maxProofSize) {
            revert InvalidProofSize();
        }
        
        // Pre-validate if enabled
        if (config.enablePreValidation) {
            _preValidateUpdate(stateId, newState, proof, result);
        }
        
        stats.totalUpdates++;
        
        emit StateUpdateSubmitted(updateId, stateId, msg.sender, block.timestamp);
        
        // Try immediate processing if queuing is disabled
        if (!config.enableQueueing) {
            success = _processUpdate(updateId, stateId, newState, proof, result);
        } else {
            // Queue for batch processing
            _queueUpdate(updateId, stateId, newState, proof, result);
            success = false; // Will be processed later
        }
    }
    
    /**
     * @notice Submit multiple state updates in a single transaction
     * @param stateIds Array of state identifiers
     * @param newStates Array of new state values
     * @param proofs Array of zero-knowledge proofs
     * @param results Array of computation results
     * @return updateIds Array of update identifiers
     * @return successes Array indicating success/failure for each update
     * 
     * Gas Cost: ~150,000 + (300,000 * number of updates) gas
     * 
     * @custom:example
     * ```solidity
     * bytes32[] memory states = new bytes32[](3);
     * bytes32[] memory values = new bytes32[](3);
     * bytes[] memory proofArray = new bytes[](3);
     * bytes[] memory resultArray = new bytes[](3);
     * // ... populate arrays ...
     * (bytes32[] memory ids, bool[] memory results) = 
     *     updater.submitBatchUpdates(states, values, proofArray, resultArray);
     * ```
     */
    function submitBatchUpdates(
        bytes32[] calldata stateIds,
        bytes32[] calldata newStates,
        bytes[] calldata proofs,
        bytes[] calldata results
    ) external onlyAuthorized returns (bytes32[] memory updateIds, bool[] memory successes) {
        uint256 length = stateIds.length;
        
        // Validate array lengths
        if (length != newStates.length || length != proofs.length || length != results.length) {
            revert("Array length mismatch");
        }
        
        if (length > config.maxBatchSize) revert BatchSizeExceeded();
        
        updateIds = new bytes32[](length);
        successes = new bool[](length);
        
        uint256 successCount = 0;
        uint256 failureCount = 0;
        
        for (uint256 i = 0; i < length; i++) {
            try this.submitStateUpdate(stateIds[i], newStates[i], proofs[i], results[i]) 
                returns (bytes32 updateId, bool success) {
                updateIds[i] = updateId;
                successes[i] = success;
                if (success) successCount++;
                else failureCount++;
            } catch {
                updateIds[i] = bytes32(0);
                successes[i] = false;
                failureCount++;
            }
        }
        
        stats.batchUpdates++;
        
        emit BatchUpdateProcessed(length, successCount, failureCount, msg.sender);
    }
    
    /**
     * @notice Process queued updates in batch for gas efficiency
     * @param maxUpdates Maximum number of updates to process (0 for all)
     * @return processedCount Number of updates successfully processed
     * @return failedCount Number of updates that failed processing
     * 
     * Gas Cost: ~100,000 + (250,000 * number of updates) gas
     * 
     * @custom:example
     * ```solidity
     * // Process up to 10 queued updates
     * (uint256 processed, uint256 failed) = updater.processQueuedUpdates(10);
     * ```
     */
    function processQueuedUpdates(uint256 maxUpdates) 
        external 
        onlyAuthorized 
        returns (uint256 processedCount, uint256 failedCount) 
    {
        if (!config.enableQueueing) return (0, 0);
        
        uint256 queueLength = updateQueue.length;
        if (queueLength == 0) return (0, 0);
        
        uint256 toProcess = (maxUpdates == 0 || maxUpdates > queueLength) ? queueLength : maxUpdates;
        
        // Prepare batch arrays
        bytes32[] memory stateIds = new bytes32[](toProcess);
        bytes32[] memory newStates = new bytes32[](toProcess);
        bytes[] memory proofs = new bytes[](toProcess);
        bytes[] memory results = new bytes[](toProcess);
        bytes32[] memory updateIds = new bytes32[](toProcess);
        
        // Collect updates from queue
        for (uint256 i = 0; i < toProcess; i++) {
            bytes32 updateId = updateQueue[i];
            PendingUpdate storage update = pendingUpdates[updateId];
            
            updateIds[i] = updateId;
            stateIds[i] = update.stateId;
            newStates[i] = update.newState;
            proofs[i] = update.proof;
            results[i] = update.result;
        }
        
        // Submit batch to state manager
        try stateManager.batchUpdateStates(stateIds, newStates, proofs, results) 
            returns (bool[] memory successes) {
            
            // Process results and update queue
            for (uint256 i = 0; i < toProcess; i++) {
                bytes32 updateId = updateIds[i];
                PendingUpdate storage update = pendingUpdates[updateId];
                
                update.processed = true;
                
                if (successes[i]) {
                    processedCount++;
                    stats.successfulUpdates++;
                } else {
                    failedCount++;
                    stats.failedUpdates++;
                }
                
                emit StateUpdateProcessed(updateId, update.stateId, successes[i], 0);
            }
        } catch {
            failedCount = toProcess;
            stats.failedUpdates += toProcess;
        }
        
        // Remove processed items from queue
        _removeFromQueue(toProcess);
        
        stats.processedUpdates += processedCount + failedCount;
        
        emit BatchUpdateProcessed(toProcess, processedCount, failedCount, msg.sender);
    }
    
    // ============ Validation Functions ============
    
    /**
     * @notice Pre-validate an update before submission
     * @param stateId The state identifier
     * @param newState The new state value
     * @param proof The zero-knowledge proof
     * @param result The computation result
     */
    function _preValidateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) internal view {
        // Basic validations
        if (stateId == bytes32(0)) {
            revert PreValidationFailed("Invalid state ID");
        }
        
        if (newState == bytes32(0)) {
            revert PreValidationFailed("Invalid new state");
        }
        
        if (proof.length == 0) {
            revert PreValidationFailed("Empty proof");
        }
        
        if (result.length == 0) {
            revert PreValidationFailed("Empty result");
        }
        
        // Check if state already exists with same value
        try stateManager.getCurrentState(stateId) returns (bytes32 currentState) {
            if (currentState == newState) {
                revert PreValidationFailed("State unchanged");
            }
        } catch {
            // State doesn't exist yet, which is fine
        }
    }
    
    /**
     * @notice Validate update parameters externally
     * @param stateId The state identifier
     * @param newState The new state value
     * @param proof The zero-knowledge proof
     * @param result The computation result
     * @return isValid Whether the parameters are valid
     * @return errorMessage Error message if invalid
     */
    function validateUpdateParameters(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external view returns (bool isValid, string memory errorMessage) {
        try this._preValidateUpdate(stateId, newState, proof, result) {
            return (true, "");
        } catch Error(string memory reason) {
            return (false, reason);
        } catch {
            return (false, "Unknown validation error");
        }
    }
    
    // ============ Internal Functions ============
    
    /**
     * @notice Process a single update immediately
     * @param updateId The update identifier
     * @param stateId The state identifier
     * @param newState The new state value
     * @param proof The zero-knowledge proof
     * @param result The computation result
     * @return success Whether the update succeeded
     */
    function _processUpdate(
        bytes32 updateId,
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) internal returns (bool success) {
        uint256 gasBefore = gasleft();
        
        try stateManager.updateState(stateId, newState, proof, result) {
            success = true;
            stats.successfulUpdates++;
        } catch {
            success = false;
            stats.failedUpdates++;
        }
        
        uint256 gasUsed = gasBefore - gasleft();
        
        emit StateUpdateProcessed(updateId, stateId, success, gasUsed);
        
        return success;
    }
    
    /**
     * @notice Queue an update for batch processing
     * @param updateId The update identifier
     * @param stateId The state identifier
     * @param newState The new state value
     * @param proof The zero-knowledge proof
     * @param result The computation result
     */
    function _queueUpdate(
        bytes32 updateId,
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) internal {
        if (updateQueue.length >= config.maxQueueSize) revert QueueFull();
        
        pendingUpdates[updateId] = PendingUpdate({
            stateId: stateId,
            newState: newState,
            proof: proof,
            result: result,
            submitter: msg.sender,
            timestamp: block.timestamp,
            priority: 0, // Could implement priority system
            exists: true,
            processed: false
        });
        
        updateQueueIndex[updateId] = updateQueue.length;
        updateQueue.push(updateId);
        stats.queuedUpdates++;
        
        emit UpdateQueued(updateId, updateQueue.length - 1);
    }
    
    /**
     * @notice Remove processed items from the front of the queue
     * @param count Number of items to remove
     */
    function _removeFromQueue(uint256 count) internal {
        if (count == 0 || count > updateQueue.length) return;
        
        // Shift remaining items to the front
        for (uint256 i = count; i < updateQueue.length; i++) {
            updateQueue[i - count] = updateQueue[i];
            updateQueueIndex[updateQueue[i]] = i - count;
        }
        
        // Remove items from the end
        for (uint256 i = 0; i < count; i++) {
            updateQueue.pop();
        }
    }
    
    // ============ Administrative Functions ============
    
    /**
     * @notice Update configuration settings
     * @param newConfig The new configuration
     */
    function updateConfiguration(UpdateConfig calldata newConfig) external onlyOwner {
        if (newConfig.maxBatchSize == 0 || newConfig.maxBatchSize > 1000) revert InvalidConfiguration();
        if (newConfig.maxQueueSize == 0) revert InvalidConfiguration();
        if (newConfig.minProofSize == 0 || newConfig.maxProofSize == 0) revert InvalidConfiguration();
        if (newConfig.minProofSize > newConfig.maxProofSize) revert InvalidConfiguration();
        
        config = newConfig;
        emit ConfigurationUpdated(newConfig);
    }
    
    /**
     * @notice Grant or revoke update authorization
     * @param account The account to modify
     * @param authorized Whether to grant authorization
     */
    function setAuthorization(address account, bool authorized) external onlyOwner {
        authorizedUpdaters[account] = authorized;
        emit AuthorizationChanged(account, authorized);
    }
    
    /**
     * @notice Transfer ownership
     * @param newOwner The new owner address
     */
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid new owner");
        owner = newOwner;
        authorizedUpdaters[newOwner] = true;
        emit AuthorizationChanged(newOwner, true);
    }
    
    // ============ View Functions ============
    
    /**
     * @notice Get current statistics
     * @return Current update statistics
     */
    function getStatistics() external view returns (UpdateStats memory) {
        return stats;
    }
    
    /**
     * @notice Get current configuration
     * @return Current update configuration
     */
    function getConfiguration() external view returns (UpdateConfig memory) {
        return config;
    }
    
    /**
     * @notice Get pending update information
     * @param updateId The update identifier
     * @return The pending update details
     */
    function getPendingUpdate(bytes32 updateId) external view returns (PendingUpdate memory) {
        return pendingUpdates[updateId];
    }
    
    /**
     * @notice Get queue status
     * @return queueLength Current queue length
     * @return queueCapacity Maximum queue capacity
     */
    function getQueueStatus() external view returns (uint256 queueLength, uint256 queueCapacity) {
        return (updateQueue.length, config.maxQueueSize);
    }
    
    /**
     * @notice Check if an account is authorized to update
     * @param account The account to check
     * @return Whether the account is authorized
     */
    function isAuthorizedUpdater(address account) external view returns (bool) {
        return authorizedUpdaters[account] || account == owner;
    }
}