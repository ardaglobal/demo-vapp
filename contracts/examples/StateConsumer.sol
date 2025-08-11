// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IStateManager} from "../src/interfaces/IStateManager.sol";

/**
 * @title StateConsumer
 * @author Arithmetic ZK System
 * @notice Example contract demonstrating how to consume state data from the Arithmetic contract
 * @dev This contract shows best practices for reading states, handling errors, and batch operations
 * 
 * Integration patterns demonstrated:
 * - Safe state reading with existence checks
 * - Batch reading for gas optimization
 * - Error handling for non-existent states
 * - State validation and caching
 * - Event-based monitoring of state changes
 */
contract StateConsumer {
    // ============ State Variables ============
    
    /// @notice The state manager contract to read from
    IStateManager public immutable stateManager;
    
    /// @notice Contract owner for administrative functions
    address public owner;
    
    /// @notice Local cache of frequently accessed states (gas optimization)
    mapping(bytes32 => CachedState) public stateCache;
    
    /// @notice Whitelist of allowed state readers
    mapping(address => bool) public authorizedReaders;
    
    /// @notice Statistics for monitoring usage patterns
    struct StateStats {
        uint256 totalReads;
        uint256 batchReads;
        uint256 cacheHits;
        uint256 cacheMisses;
        uint256 errorCount;
    }
    
    StateStats public stats;
    
    /// @notice Cached state structure with metadata
    struct CachedState {
        bytes32 value;
        uint256 timestamp;
        bool exists;
        bool isStale;
    }
    
    /// @notice Configuration for state validation
    struct ValidationConfig {
        bool enableCaching;
        uint256 cacheExpiry; // seconds
        uint256 maxBatchSize;
        bool requireAuthorization;
    }
    
    ValidationConfig public config;
    
    // ============ Events ============
    
    /// @notice Emitted when states are successfully read
    event StatesRead(bytes32[] indexed stateIds, address indexed reader, uint256 timestamp);
    
    /// @notice Emitted when a state read fails
    event StateReadError(bytes32 indexed stateId, string reason, address indexed reader);
    
    /// @notice Emitted when cache is updated
    event CacheUpdated(bytes32 indexed stateId, bytes32 value, uint256 timestamp);
    
    /// @notice Emitted when authorization changes
    event AuthorizationChanged(address indexed account, bool authorized);
    
    // ============ Errors ============
    
    error UnauthorizedReader();
    error InvalidStateManager();
    error BatchSizeExceeded();
    error StateNotFound(bytes32 stateId);
    error InvalidConfiguration();
    
    // ============ Modifiers ============
    
    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }
    
    modifier onlyAuthorized() {
        if (config.requireAuthorization && !authorizedReaders[msg.sender] && msg.sender != owner) {
            revert UnauthorizedReader();
        }
        _;
    }
    
    // ============ Constructor ============
    
    /**
     * @notice Initialize the state consumer with a state manager contract
     * @param _stateManager Address of the state manager contract to read from
     * 
     * @custom:example
     * ```solidity
     * address arithmeticContract = 0x1234567890123456789012345678901234567890;
     * StateConsumer consumer = new StateConsumer(arithmeticContract);
     * ```
     */
    constructor(address _stateManager) {
        if (_stateManager == address(0)) revert InvalidStateManager();
        
        stateManager = IStateManager(_stateManager);
        owner = msg.sender;
        authorizedReaders[msg.sender] = true;
        
        // Default configuration
        config = ValidationConfig({
            enableCaching: true,
            cacheExpiry: 300, // 5 minutes
            maxBatchSize: 50,
            requireAuthorization: false
        });
        
        emit AuthorizationChanged(msg.sender, true);
    }
    
    // ============ Core Reading Functions ============
    
    /**
     * @notice Read a single state with comprehensive error handling
     * @param stateId The state identifier to read
     * @return value The current state value
     * @return exists Whether the state exists
     * @return fromCache Whether the result came from cache
     * 
     * Gas Cost: ~5,000 gas (cached) or ~25,000 gas (uncached)
     * 
     * @custom:example
     * ```solidity
     * bytes32 stateId = keccak256("user_balance_001");
     * (bytes32 value, bool exists, bool fromCache) = consumer.readState(stateId);
     * require(exists, "State not found");
     * uint256 balance = uint256(value);
     * ```
     */
    function readState(bytes32 stateId) 
        external 
        onlyAuthorized 
        returns (bytes32 value, bool exists, bool fromCache) 
    {
        // Check cache first if enabled
        if (config.enableCaching) {
            CachedState memory cached = stateCache[stateId];
            if (cached.exists && !_isCacheStale(cached)) {
                stats.totalReads++;
                stats.cacheHits++;
                emit StatesRead(_asSingleArray(stateId), msg.sender, block.timestamp);
                return (cached.value, true, true);
            }
        }
        
        // Read from state manager
        try stateManager.getCurrentState(stateId) returns (bytes32 stateValue) {
            exists = (stateValue != bytes32(0));
            value = stateValue;
            
            // Update cache if enabled
            if (config.enableCaching) {
                _updateCache(stateId, value, exists);
            }
            
            stats.totalReads++;
            stats.cacheMisses++;
            emit StatesRead(_asSingleArray(stateId), msg.sender, block.timestamp);
            
        } catch Error(string memory reason) {
            stats.errorCount++;
            emit StateReadError(stateId, reason, msg.sender);
            return (bytes32(0), false, false);
        }
    }
    
    /**
     * @notice Read multiple states in a single call for gas efficiency
     * @param stateIds Array of state identifiers to read
     * @return values Array of state values
     * @return existsArray Array indicating which states exist
     * @return cacheHitCount Number of cache hits in this batch
     * 
     * Gas Cost: ~15,000 + (3,000 * number of states) gas
     * 
     * @custom:example
     * ```solidity
     * bytes32[] memory userStates = new bytes32[](5);
     * // ... populate userStates ...
     * (bytes32[] memory values, bool[] memory exists, uint256 cacheHits) = 
     *     consumer.batchReadStates(userStates);
     * ```
     */
    function batchReadStates(bytes32[] calldata stateIds) 
        external 
        onlyAuthorized 
        returns (bytes32[] memory values, bool[] memory existsArray, uint256 cacheHitCount) 
    {
        uint256 length = stateIds.length;
        if (length > config.maxBatchSize) revert BatchSizeExceeded();
        
        values = new bytes32[](length);
        existsArray = new bool[](length);
        
        // Track which states need to be fetched from state manager
        bytes32[] memory uncachedIds = new bytes32[](length);
        uint256[] memory uncachedIndices = new uint256[](length);
        uint256 uncachedCount = 0;
        
        // First pass: check cache
        if (config.enableCaching) {
            for (uint256 i = 0; i < length; i++) {
                CachedState memory cached = stateCache[stateIds[i]];
                if (cached.exists && !_isCacheStale(cached)) {
                    values[i] = cached.value;
                    existsArray[i] = true;
                    cacheHitCount++;
                } else {
                    uncachedIds[uncachedCount] = stateIds[i];
                    uncachedIndices[uncachedCount] = i;
                    uncachedCount++;
                }
            }
        } else {
            // No caching - need to fetch all
            for (uint256 i = 0; i < length; i++) {
                uncachedIds[i] = stateIds[i];
                uncachedIndices[i] = i;
            }
            uncachedCount = length;
        }
        
        // Second pass: fetch uncached states
        if (uncachedCount > 0) {
            // Resize arrays to actual uncached count
            bytes32[] memory toFetch = new bytes32[](uncachedCount);
            for (uint256 i = 0; i < uncachedCount; i++) {
                toFetch[i] = uncachedIds[i];
            }
            
            try stateManager.batchReadStates(toFetch) returns (bytes32[] memory fetchedValues) {
                for (uint256 i = 0; i < uncachedCount; i++) {
                    uint256 originalIndex = uncachedIndices[i];
                    bytes32 value = fetchedValues[i];
                    bool exists = (value != bytes32(0));
                    
                    values[originalIndex] = value;
                    existsArray[originalIndex] = exists;
                    
                    // Update cache
                    if (config.enableCaching) {
                        _updateCache(toFetch[i], value, exists);
                    }
                }
            } catch Error(string memory reason) {
                stats.errorCount++;
                // Emit error for the entire batch
                for (uint256 i = 0; i < uncachedCount; i++) {
                    emit StateReadError(uncachedIds[i], reason, msg.sender);
                }
            }
        }
        
        stats.totalReads += length;
        stats.batchReads++;
        stats.cacheMisses += (length - cacheHitCount);
        
        emit StatesRead(stateIds, msg.sender, block.timestamp);
    }
    
    /**
     * @notice Read state with validation against expected value
     * @param stateId The state identifier to read
     * @param expectedValue The expected state value
     * @return matches Whether the actual value matches expected
     * @return actualValue The actual state value found
     * 
     * @custom:example
     * ```solidity
     * bytes32 expectedBalance = bytes32(uint256(1000));
     * (bool matches, bytes32 actual) = consumer.readAndValidate(stateId, expectedBalance);
     * require(matches, "Balance mismatch detected");
     * ```
     */
    function readAndValidate(bytes32 stateId, bytes32 expectedValue) 
        external 
        onlyAuthorized 
        returns (bool matches, bytes32 actualValue) 
    {
        (actualValue, bool exists,) = this.readState(stateId);
        
        if (!exists) {
            return (false, bytes32(0));
        }
        
        matches = (actualValue == expectedValue);
    }
    
    // ============ Cache Management ============
    
    /**
     * @notice Clear the entire state cache
     * @dev Only owner can clear cache to prevent DoS attacks
     */
    function clearCache() external onlyOwner {
        // Note: In production, you might want to iterate through known cache keys
        // or implement a more sophisticated cache clearing mechanism
        stats.cacheHits = 0;
        stats.cacheMisses = 0;
    }
    
    /**
     * @notice Update cache for a specific state
     * @param stateId The state identifier
     * @param value The state value
     * @param exists Whether the state exists
     */
    function _updateCache(bytes32 stateId, bytes32 value, bool exists) internal {
        stateCache[stateId] = CachedState({
            value: value,
            timestamp: block.timestamp,
            exists: exists,
            isStale: false
        });
        
        emit CacheUpdated(stateId, value, block.timestamp);
    }
    
    /**
     * @notice Check if a cached state is stale
     * @param cached The cached state to check
     * @return Whether the cache entry is stale
     */
    function _isCacheStale(CachedState memory cached) internal view returns (bool) {
        return cached.isStale || (block.timestamp - cached.timestamp > config.cacheExpiry);
    }
    
    // ============ Administrative Functions ============
    
    /**
     * @notice Update configuration settings
     * @param newConfig The new configuration
     */
    function updateConfiguration(ValidationConfig calldata newConfig) external onlyOwner {
        if (newConfig.maxBatchSize == 0 || newConfig.maxBatchSize > 1000) revert InvalidConfiguration();
        config = newConfig;
    }
    
    /**
     * @notice Grant or revoke read authorization
     * @param account The account to modify
     * @param authorized Whether to grant authorization
     */
    function setAuthorization(address account, bool authorized) external onlyOwner {
        authorizedReaders[account] = authorized;
        emit AuthorizationChanged(account, authorized);
    }
    
    /**
     * @notice Transfer ownership
     * @param newOwner The new owner address
     */
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid new owner");
        owner = newOwner;
        authorizedReaders[newOwner] = true;
        emit AuthorizationChanged(newOwner, true);
    }
    
    // ============ View Functions ============
    
    /**
     * @notice Get current statistics
     * @return Current usage statistics
     */
    function getStatistics() external view returns (StateStats memory) {
        return stats;
    }
    
    /**
     * @notice Get current configuration
     * @return Current validation configuration
     */
    function getConfiguration() external view returns (ValidationConfig memory) {
        return config;
    }
    
    /**
     * @notice Check if an account is authorized to read
     * @param account The account to check
     * @return Whether the account is authorized
     */
    function isAuthorizedReader(address account) external view returns (bool) {
        return authorizedReaders[account] || account == owner;
    }
    
    /**
     * @notice Get cache information for a state
     * @param stateId The state identifier
     * @return The cached state information
     */
    function getCacheInfo(bytes32 stateId) external view returns (CachedState memory) {
        return stateCache[stateId];
    }
    
    // ============ Utility Functions ============
    
    /**
     * @notice Convert a single bytes32 to an array (for events)
     * @param item The item to convert
     * @return Single-element array
     */
    function _asSingleArray(bytes32 item) internal pure returns (bytes32[] memory) {
        bytes32[] memory array = new bytes32[](1);
        array[0] = item;
        return array;
    }
}