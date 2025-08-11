// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IStateManager} from "../src/interfaces/IStateManager.sol";

/**
 * @title ProofReader
 * @author Arithmetic ZK System
 * @notice Example contract demonstrating how to read and validate stored proofs
 * @dev Shows best practices for proof verification, enumeration, and metadata access
 * 
 * Integration patterns demonstrated:
 * - Reading and re-validating stored proofs
 * - Proof enumeration and filtering
 * - Metadata access and validation
 * - Batch proof operations
 * - External proof verification patterns
 * - Gas-efficient proof queries
 */
contract ProofReader {
    // ============ State Variables ============
    
    /// @notice The state manager contract to read proofs from
    IStateManager public immutable stateManager;
    
    /// @notice Contract owner for administrative functions
    address public owner;
    
    /// @notice Authorized proof readers
    mapping(address => bool) public authorizedReaders;
    
    /// @notice Local cache of proof metadata for gas optimization
    mapping(bytes32 => CachedProofMetadata) public proofCache;
    
    /// @notice Reader statistics for monitoring
    struct ReaderStats {
        uint256 totalReads;
        uint256 batchReads;
        uint256 verificationChecks;
        uint256 cacheHits;
        uint256 cacheMisses;
        uint256 errorCount;
    }
    
    ReaderStats public stats;
    
    /// @notice Cached proof metadata structure
    struct CachedProofMetadata {
        bytes32 proofId;
        bytes32 stateId;
        bool verified;
        bool exists;
        uint256 timestamp;
        uint256 cacheTime;
        bool isStale;
    }
    
    /// @notice Configuration for proof reading operations
    struct ReaderConfig {
        bool enableCaching;
        bool requireAuthorization;
        bool enableReVerification;
        uint256 cacheExpiry; // seconds
        uint256 maxBatchSize;
        uint256 maxProofSize;
    }
    
    ReaderConfig public config;
    
    /// @notice Proof search filters
    struct ProofFilter {
        bytes32 stateId; // Filter by state ID (bytes32(0) for any)
        address submitter; // Filter by submitter (address(0) for any)
        uint256 minTimestamp; // Minimum timestamp
        uint256 maxTimestamp; // Maximum timestamp
        bool verifiedOnly; // Only verified proofs
        uint256 limit; // Maximum results (0 for no limit)
    }
    
    // ============ Events ============
    
    /// @notice Emitted when proofs are successfully read
    event ProofsRead(
        bytes32[] indexed proofIds,
        address indexed reader,
        uint256 timestamp,
        bool fromCache
    );
    
    /// @notice Emitted when proof verification is checked
    event ProofVerificationChecked(
        bytes32 indexed proofId,
        bool verified,
        address indexed reader
    );
    
    /// @notice Emitted when proof enumeration is performed
    event ProofEnumeration(
        uint256 totalProofs,
        uint256 filteredCount,
        address indexed reader
    );
    
    /// @notice Emitted when cache is updated
    event CacheUpdated(bytes32 indexed proofId, uint256 timestamp);
    
    /// @notice Emitted when authorization changes
    event AuthorizationChanged(address indexed account, bool authorized);
    
    // ============ Errors ============
    
    error UnauthorizedReader();
    error InvalidStateManager();
    error ProofNotFound(bytes32 proofId);
    error BatchSizeExceeded();
    error InvalidFilter();
    error InvalidConfiguration();
    error ProofTooLarge();
    
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
     * @notice Initialize the proof reader with a state manager contract
     * @param _stateManager Address of the state manager contract to read from
     * 
     * @custom:example
     * ```solidity
     * address arithmeticContract = 0x1234567890123456789012345678901234567890;
     * ProofReader reader = new ProofReader(arithmeticContract);
     * ```
     */
    constructor(address _stateManager) {
        if (_stateManager == address(0)) revert InvalidStateManager();
        
        stateManager = IStateManager(_stateManager);
        owner = msg.sender;
        authorizedReaders[msg.sender] = true;
        
        // Default configuration
        config = ReaderConfig({
            enableCaching: true,
            requireAuthorization: false,
            enableReVerification: false,
            cacheExpiry: 300, // 5 minutes
            maxBatchSize: 100,
            maxProofSize: 50000 // 50KB
        });
        
        emit AuthorizationChanged(msg.sender, true);
    }
    
    // ============ Core Proof Reading Functions ============
    
    /**
     * @notice Read a single proof with comprehensive metadata
     * @param proofId The proof identifier to read
     * @return proof The proof bytes
     * @return exists Whether the proof exists
     * @return verified Whether the proof is verified
     * @return fromCache Whether the result came from cache
     * 
     * Gas Cost: ~10,000 gas (cached) or ~50,000 gas (uncached)
     * 
     * @custom:example
     * ```solidity
     * bytes32 proofId = keccak256(someProofBytes);
     * (bytes memory proof, bool exists, bool verified, bool fromCache) = 
     *     reader.readProof(proofId);
     * require(exists && verified, "Invalid or unverified proof");
     * ```
     */
    function readProof(bytes32 proofId) 
        external 
        onlyAuthorized 
        returns (bytes memory proof, bool exists, bool verified, bool fromCache) 
    {
        // Check cache first if enabled
        if (config.enableCaching) {
            CachedProofMetadata memory cached = proofCache[proofId];
            if (cached.exists && !_isCacheStale(cached)) {
                try stateManager.getStoredProof(proofId) returns (bytes memory cachedProof) {
                    stats.totalReads++;
                    stats.cacheHits++;
                    emit ProofsRead(_asSingleArray(proofId), msg.sender, block.timestamp, true);
                    return (cachedProof, cached.exists, cached.verified, true);
                } catch {
                    // Cache inconsistent, fall through to full read
                }
            }
        }
        
        // Read from state manager
        try stateManager.getProofById(proofId) returns (bytes memory proofBytes, bool proofExists) {
            exists = proofExists;
            proof = proofBytes;
            
            if (exists) {
                // Check if proof size is reasonable
                if (proof.length > config.maxProofSize) revert ProofTooLarge();
                
                // Check verification status
                verified = stateManager.isProofVerified(proofId);
                
                // Update cache if enabled
                if (config.enableCaching) {
                    _updateProofCache(proofId, bytes32(0), verified, exists);
                }
                
                stats.totalReads++;
                stats.cacheMisses++;
                stats.verificationChecks++;
                
                emit ProofsRead(_asSingleArray(proofId), msg.sender, block.timestamp, false);
                emit ProofVerificationChecked(proofId, verified, msg.sender);
            } else {
                stats.errorCount++;
            }
            
        } catch Error(string memory reason) {
            stats.errorCount++;
            revert ProofNotFound(proofId);
        }
    }
    
    /**
     * @notice Read multiple proofs in a single call for gas efficiency
     * @param proofIds Array of proof identifiers to read
     * @return proofs Array of proof bytes
     * @return existsArray Array indicating which proofs exist
     * @return verifiedArray Array indicating which proofs are verified
     * @return cacheHitCount Number of cache hits in this batch
     * 
     * Gas Cost: ~30,000 + (15,000 * number of proofs) gas
     * 
     * @custom:example
     * ```solidity
     * bytes32[] memory proofIds = new bytes32[](5);
     * // ... populate proofIds ...
     * (bytes[] memory proofs, bool[] memory exists, bool[] memory verified, uint256 cacheHits) =
     *     reader.batchReadProofs(proofIds);
     * ```
     */
    function batchReadProofs(bytes32[] calldata proofIds)
        external
        onlyAuthorized
        returns (
            bytes[] memory proofs,
            bool[] memory existsArray,
            bool[] memory verifiedArray,
            uint256 cacheHitCount
        )
    {
        uint256 length = proofIds.length;
        if (length > config.maxBatchSize) revert BatchSizeExceeded();
        
        proofs = new bytes[](length);
        existsArray = new bool[](length);
        verifiedArray = new bool[](length);
        
        for (uint256 i = 0; i < length; i++) {
            try this.readProof(proofIds[i]) returns (
                bytes memory proof,
                bool exists,
                bool verified,
                bool fromCache
            ) {
                proofs[i] = proof;
                existsArray[i] = exists;
                verifiedArray[i] = verified;
                
                if (fromCache) {
                    cacheHitCount++;
                }
            } catch {
                proofs[i] = new bytes(0);
                existsArray[i] = false;
                verifiedArray[i] = false;
            }
        }
        
        stats.batchReads++;
        
        emit ProofsRead(proofIds, msg.sender, block.timestamp, cacheHitCount > 0);
    }
    
    /**
     * @notice Read proof with its verification result
     * @param proofId The proof identifier
     * @return proof The proof bytes
     * @return verified Whether the proof is verified
     * @return result The verification result bytes
     * 
     * @custom:example
     * ```solidity
     * (bytes memory proof, bool verified, bytes memory result) = 
     *     reader.readProofWithResult(proofId);
     * if (verified) {
     *     uint256 computedSum = abi.decode(result, (uint256));
     * }
     * ```
     */
    function readProofWithResult(bytes32 proofId)
        external
        onlyAuthorized
        returns (bytes memory proof, bool verified, bytes memory result)
    {
        // Read proof first
        (proof, bool exists,,) = this.readProof(proofId);
        
        if (!exists) revert ProofNotFound(proofId);
        
        // Get verification result
        (verified, result) = stateManager.getVerificationResult(proofId);
        
        stats.verificationChecks++;
        emit ProofVerificationChecked(proofId, verified, msg.sender);
    }
    
    // ============ Proof Enumeration Functions ============
    
    /**
     * @notice Get proofs filtered by criteria
     * @param filter The filter criteria to apply
     * @return proofIds Array of matching proof identifiers
     * @return totalCount Total number of proofs that match filter
     * 
     * Note: This is a simplified implementation. In production, you might want
     * to implement pagination and more sophisticated filtering.
     * 
     * @custom:example
     * ```solidity
     * ProofFilter memory filter = ProofFilter({
     *     stateId: keccak256("user_balance_001"),
     *     submitter: address(0), // any submitter
     *     minTimestamp: block.timestamp - 86400, // last 24 hours
     *     maxTimestamp: 0, // no max
     *     verifiedOnly: true,
     *     limit: 10
     * });
     * (bytes32[] memory ids, uint256 total) = reader.getFilteredProofs(filter);
     * ```
     */
    function getFilteredProofs(ProofFilter calldata filter)
        external
        onlyAuthorized
        returns (bytes32[] memory proofIds, uint256 totalCount)
    {
        // This is a simplified implementation
        // In a production system, you'd want to implement proper indexing and pagination
        
        // For this example, we'll simulate by reading a limited set of proofs
        // In reality, you'd need the state manager to support filtered queries
        
        // Get recent proofs (this would be replaced with proper filtering logic)
        uint256 limit = filter.limit == 0 ? 100 : filter.limit;
        if (limit > config.maxBatchSize) limit = config.maxBatchSize;
        
        proofIds = new bytes32[](limit);
        totalCount = 0;
        
        // Simulate filtering logic (in production, implement proper indexing)
        for (uint256 i = 0; i < limit; i++) {
            // This would be replaced with actual proof enumeration from state manager
            bytes32 mockProofId = keccak256(abi.encodePacked("proof", i, block.timestamp));
            proofIds[totalCount] = mockProofId;
            totalCount++;
        }
        
        // Resize array to actual count
        assembly {
            mstore(proofIds, totalCount)
        }
        
        emit ProofEnumeration(limit, totalCount, msg.sender);
    }
    
    /**
     * @notice Get proofs associated with a specific state ID
     * @param stateId The state identifier
     * @param limit Maximum number of proofs to return (0 for all)
     * @return proofIds Array of proof identifiers for the state
     * @return proofs Array of proof bytes
     * 
     * @custom:example
     * ```solidity
     * bytes32 stateId = keccak256("user_balance_001");
     * (bytes32[] memory ids, bytes[] memory proofs) = 
     *     reader.getProofsByState(stateId, 5);
     * ```
     */
    function getProofsByState(bytes32 stateId, uint256 limit)
        external
        onlyAuthorized
        returns (bytes32[] memory proofIds, bytes[] memory proofs)
    {
        // This would typically call the state manager's enumeration functions
        // For now, we'll implement a basic version
        
        if (limit == 0 || limit > config.maxBatchSize) {
            limit = config.maxBatchSize;
        }
        
        // In production, you'd use: stateManager.getProofsByStateId(stateId)
        proofIds = new bytes32[](0); // Placeholder
        proofs = new bytes[](0); // Placeholder
        
        emit ProofEnumeration(0, 0, msg.sender);
    }
    
    // ============ Verification Functions ============
    
    /**
     * @notice Check if a proof is verified without reading the full proof
     * @param proofId The proof identifier
     * @return verified Whether the proof is verified
     * 
     * Gas Cost: ~5,000 gas
     * 
     * @custom:example
     * ```solidity
     * bool isValid = reader.isProofVerified(proofId);
     * require(isValid, "Proof not verified");
     * ```
     */
    function isProofVerified(bytes32 proofId) external view returns (bool verified) {
        return stateManager.isProofVerified(proofId);
    }
    
    /**
     * @notice Batch check verification status of multiple proofs
     * @param proofIds Array of proof identifiers
     * @return verifiedArray Array indicating verification status
     * 
     * @custom:example
     * ```solidity
     * bool[] memory verified = reader.batchCheckVerification(proofIds);
     * ```
     */
    function batchCheckVerification(bytes32[] calldata proofIds)
        external
        view
        returns (bool[] memory verifiedArray)
    {
        uint256 length = proofIds.length;
        verifiedArray = new bool[](length);
        
        for (uint256 i = 0; i < length; i++) {
            verifiedArray[i] = stateManager.isProofVerified(proofIds[i]);
        }
    }
    
    // ============ Cache Management ============
    
    /**
     * @notice Update proof cache entry
     * @param proofId The proof identifier
     * @param stateId The associated state identifier
     * @param verified Whether the proof is verified
     * @param exists Whether the proof exists
     */
    function _updateProofCache(
        bytes32 proofId,
        bytes32 stateId,
        bool verified,
        bool exists
    ) internal {
        proofCache[proofId] = CachedProofMetadata({
            proofId: proofId,
            stateId: stateId,
            verified: verified,
            exists: exists,
            timestamp: block.timestamp,
            cacheTime: block.timestamp,
            isStale: false
        });
        
        emit CacheUpdated(proofId, block.timestamp);
    }
    
    /**
     * @notice Check if a cached entry is stale
     * @param cached The cached metadata to check
     * @return Whether the cache entry is stale
     */
    function _isCacheStale(CachedProofMetadata memory cached) internal view returns (bool) {
        return cached.isStale || (block.timestamp - cached.cacheTime > config.cacheExpiry);
    }
    
    /**
     * @notice Clear the proof cache
     */
    function clearCache() external onlyOwner {
        // Note: In production, implement proper cache clearing mechanism
        stats.cacheHits = 0;
        stats.cacheMisses = 0;
    }
    
    // ============ Administrative Functions ============
    
    /**
     * @notice Update configuration settings
     * @param newConfig The new configuration
     */
    function updateConfiguration(ReaderConfig calldata newConfig) external onlyOwner {
        if (newConfig.maxBatchSize == 0 || newConfig.maxBatchSize > 1000) revert InvalidConfiguration();
        if (newConfig.maxProofSize == 0) revert InvalidConfiguration();
        
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
     * @return Current reader statistics
     */
    function getStatistics() external view returns (ReaderStats memory) {
        return stats;
    }
    
    /**
     * @notice Get current configuration
     * @return Current reader configuration
     */
    function getConfiguration() external view returns (ReaderConfig memory) {
        return config;
    }
    
    /**
     * @notice Get cached proof metadata
     * @param proofId The proof identifier
     * @return The cached metadata
     */
    function getCachedMetadata(bytes32 proofId) external view returns (CachedProofMetadata memory) {
        return proofCache[proofId];
    }
    
    /**
     * @notice Check if an account is authorized to read
     * @param account The account to check
     * @return Whether the account is authorized
     */
    function isAuthorizedReader(address account) external view returns (bool) {
        return authorizedReaders[account] || account == owner;
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