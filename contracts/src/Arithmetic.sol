// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {EventHelpers} from "./EventHelpers.sol";

struct PublicValuesStruct {
    int32 result;
}

/// @title Arithmetic.
/// @author Arda Global
/// @notice This contract implements a simple example of verifying the proof of a computing a
///         arithmetic operation.
contract Arithmetic is EventHelpers {
    /// @notice The address of the SP1 verifier contract.
    /// @dev This can either be a specific SP1Verifier for a specific version, or the
    ///      SP1VerifierGateway which can be used to verify proofs for any version of SP1.
    ///      For the list of supported verifiers on each chain, see:
    ///      https://github.com/succinctlabs/sp1-contracts/tree/main/contracts/deployments
    address public verifier;

    /// @notice The verification key for the arithmetic program.
    bytes32 public arithmeticProgramVKey;

    /// @notice Store state by state ID.
    mapping(bytes32 => bytes32) public currentState;

    /// @notice Store ZK proofs by proof ID.
    mapping(bytes32 => bytes) public storedProofs;

    /// @notice Store verification results by proof ID.
    mapping(bytes32 => bytes) public storedResults;

    /// @notice Track which proofs have been verified.
    mapping(bytes32 => bool) public verifiedProofs;

    /// @notice Store state history for each state ID.
    mapping(bytes32 => bytes32[]) public stateHistory;

    /// @notice Contract owner for access control.
    address public owner;

    /// @notice Authorized addresses that can post state updates.
    mapping(address => bool) public authorizedPosters;

    /// @notice Proof metadata structure
    struct ProofMetadata {
        bytes32 proofId;
        bytes32 stateId;
        address submitter;
        uint256 timestamp;
        bool verified;
        bool exists;
    }

    /// @notice Store proof metadata by proof ID.
    mapping(bytes32 => ProofMetadata) public proofMetadata;

    /// @notice Store proof IDs by state ID for quick lookup.
    mapping(bytes32 => bytes32[]) public stateToProofs;

    /// @notice Array of all proof IDs for enumeration.
    bytes32[] public allProofIds;

    /// @notice Mapping from proof ID to index in allProofIds array.
    mapping(bytes32 => uint256) public proofIdToIndex;

    /// @notice Store latest proof ID for each state ID.
    mapping(bytes32 => bytes32) public latestProofForState;

    /*//////////////////////////////////////////////////////////////
                        EVENT FILTERING HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Event type constants for filtering.
    string public constant EVENT_STATE_UPDATED = "StateUpdated";
    string public constant EVENT_BATCH_STATE_UPDATED = "BatchStateUpdated";
    string public constant EVENT_PROOF_STORED = "ProofStored";
    string public constant EVENT_PROOF_VERIFIED = "ProofVerified";
    string public constant EVENT_STATE_READ = "StateRead";
    string public constant EVENT_PROOF_READ = "ProofRead";

    /// @notice Pre-computed hashes for gas-efficient event type comparisons.
    bytes32 private constant STATE_UPDATE_HASH = keccak256(abi.encodePacked("state_update"));
    bytes32 private constant BATCH_UPDATE_HASH = keccak256(abi.encodePacked("batch_update"));
    bytes32 private constant PROOF_STORED_HASH = keccak256(abi.encodePacked("proof_stored"));


    /// @notice Global event statistics.
    EventStats public eventStats;

    /// @notice Track events by submitter for filtering.
    mapping(address => uint256) public eventCountBySubmitter;

    /// @notice Track events by state ID for filtering.
    mapping(bytes32 => uint256) public eventCountByStateId;

    /// @notice Track daily event counts for analytics.
    mapping(uint256 => uint256) public dailyEventCounts; // day => count

    /// @notice Enable/disable read event tracking (gas optimization).
    bool public readEventTrackingEnabled = true;

    /*//////////////////////////////////////////////////////////////
                        COMPREHENSIVE EVENT SYSTEM
    //////////////////////////////////////////////////////////////*/

    /// @notice Enhanced state update event with comprehensive data.
    event StateUpdated(
        bytes32 indexed stateId,
        bytes32 indexed newState,
        bytes32 indexed proofId,
        address updater,
        uint256 timestamp
    );

    /// @notice Batch state update event for multiple state changes.
    event BatchStateUpdated(
        bytes32[] stateIds,
        bytes32[] newStates,
        address indexed updater,
        uint256 indexed timestamp
    );

    /// @notice Event emitted when state is read for monitoring.
    event StateReadRequested(
        bytes32 indexed stateId,
        address indexed reader,
        uint256 indexed timestamp
    );

    /// @notice Enhanced proof storage event.
    event ProofStored(
        bytes32 indexed proofId,
        bytes32 indexed stateId,
        address indexed submitter,
        uint256 timestamp
    );

    /// @notice Enhanced proof verification event with result data.
    event ProofVerified(
        bytes32 indexed proofId,
        bool indexed success,
        bytes result,
        uint256 timestamp
    );

    /// @notice Event emitted when proof is read for monitoring.
    event ProofReadRequested(
        bytes32 indexed proofId,
        address indexed reader,
        uint256 indexed timestamp
    );

    /// @notice Legacy events for backwards compatibility.
    event AuthorizationChanged(address indexed account, bool authorized);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    /// @notice Event emitted for bulk operations.
    event BulkOperationExecuted(
        string indexed operationType,
        uint256 indexed itemCount,
        address indexed executor,
        uint256 timestamp
    );

    /// @notice Event emitted when contract state is queried.
    event ContractStateQueried(
        address indexed querier,
        string queryType,
        uint256 timestamp
    );

    /// @notice Custom errors for gas optimization.
    error UnauthorizedAccess();
    error InvalidArrayLength();
    error StateNotFound();
    error ProofNotFound();
    error InvalidLimit();
    error InvalidIndex();
    error ProofAlreadyExists();

    /// @notice Modifier to restrict access to owner only.
    modifier onlyOwner() {
        if (msg.sender != owner) revert UnauthorizedAccess();
        _;
    }

    /// @notice Modifier to restrict access to authorized posters only.
    modifier onlyAuthorized() {
        if (!authorizedPosters[msg.sender] && msg.sender != owner) revert UnauthorizedAccess();
        _;
    }

    constructor(address _verifier, bytes32 _arithmeticProgramVKey) {
        verifier = _verifier;
        arithmeticProgramVKey = _arithmeticProgramVKey;
        owner = msg.sender;
        authorizedPosters[msg.sender] = true;
        emit AuthorizationChanged(msg.sender, true);
    }

    /// @notice The entrypoint for verifying the proof of an arithmetic operation.
    /// @param _proofBytes The encoded proof.
    /// @param _publicValues The encoded public values.
    function verifyArithmeticProof(
        bytes calldata _publicValues,
        bytes calldata _proofBytes
    ) public view returns (int32) {
        ISP1Verifier(verifier).verifyProof(
            arithmeticProgramVKey,
            _publicValues,
            _proofBytes
        );
        PublicValuesStruct memory publicValues = abi.decode(
            _publicValues,
            (PublicValuesStruct)
        );
        return publicValues.result;
    }

    /// @notice Update state with ZK proof verification.
    /// @param stateId The state identifier.
    /// @param newStateRoot The new state root to store.
    /// @param proof The ZK proof to verify.
    /// @param publicValues The encoded public values for proof verification.
    function updateState(
        bytes32 stateId,
        bytes32 newStateRoot,
        bytes calldata proof,
        bytes calldata publicValues
    ) external onlyAuthorized {
        ISP1Verifier(verifier).verifyProof(
            arithmeticProgramVKey,
            publicValues,
            proof
        );
        
        bytes32 proofHash = keccak256(proof);
        
        currentState[stateId] = newStateRoot;
        storedProofs[proofHash] = proof;
        storedResults[proofHash] = publicValues;
        verifiedProofs[proofHash] = true;
        
        // Store proof metadata
        _storeProofMetadata(proofHash, stateId, msg.sender);
        
        emit StateUpdated(stateId, newStateRoot, proofHash, msg.sender, block.timestamp);
        
        // Update event statistics
        eventCountByStateId[stateId]++;
        _updateEventStats("state_update");
    }

    /// @notice Post new state update with proof verification and access control.
    /// @param stateId The state identifier.
    /// @param newState The new state root.
    /// @param proof The ZK proof to verify.
    /// @param result The verification result data.
    /// @return success True if the operation succeeded.
    function postStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external onlyAuthorized returns (bool success) {
        return _postStateUpdate(stateId, newState, proof, result);
    }

    /// @notice Internal implementation of state update with proof verification.
    /// @param stateId The state identifier.
    /// @param newState The new state root.
    /// @param proof The ZK proof to verify.
    /// @param result The verification result data.
    /// @return success True if the operation succeeded.
    function _postStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) internal returns (bool success) {
        try ISP1Verifier(verifier).verifyProof(
            arithmeticProgramVKey,
            result,
            proof
        ) {
            bytes32 proofHash = keccak256(proof);
            
            currentState[stateId] = newState;
            stateHistory[stateId].push(newState);
            storedProofs[proofHash] = proof;
            storedResults[proofHash] = result;
            verifiedProofs[proofHash] = true;
            
            // Store proof metadata
            _storeProofMetadata(proofHash, stateId, msg.sender);
            
            emit StateUpdated(stateId, newState, proofHash, msg.sender, block.timestamp);
            
            // Update event statistics
            eventCountByStateId[stateId]++;
            _updateEventStats("state_update");
            
            return true;
        } catch {
            return false;
        }
    }

    /// @notice Get the current state root for a given state ID.
    /// @param stateId The state identifier.
    /// @return The current state root.
    function getCurrentState(bytes32 stateId) external view returns (bytes32) {
        return currentState[stateId];
    }

    /// @notice Read current state with existence check.
    /// @param stateId The state identifier.
    /// @return state The current state root.
    /// @return exists True if the state exists.
    function readCurrentState(bytes32 stateId) external returns (bytes32 state, bool exists) {
        state = currentState[stateId];
        exists = (state != bytes32(0) || stateHistory[stateId].length > 0);
        
        // Track read event
        _trackReadEvent(stateId, bytes32(0));
    }

    /// @notice Read state history for a given state ID.
    /// @param stateId The state identifier.
    /// @param limit Maximum number of recent states to return (0 for all).
    /// @return states Array of recent state roots.
    function readStateHistory(bytes32 stateId, uint256 limit) external view returns (bytes32[] memory states) {
        bytes32[] storage history = stateHistory[stateId];
        uint256 length = history.length;
        
        if (length == 0) {
            return new bytes32[](0);
        }
        
        if (limit == 0 || limit > length) {
            limit = length;
        }
        
        states = new bytes32[](limit);
        uint256 startIndex = length - limit;
        
        for (uint256 i = 0; i < limit; i++) {
            states[i] = history[startIndex + i];
        }
    }

    /// @notice Get a stored proof by proof ID.
    /// @param proofId The proof identifier (hash).
    /// @return The stored proof bytes.
    function getStoredProof(bytes32 proofId) external view returns (bytes memory) {
        return storedProofs[proofId];
    }

    /// @notice Get stored verification result by proof ID.
    /// @param proofId The proof identifier (hash).
    /// @return The stored result bytes.
    function getStoredResult(bytes32 proofId) external view returns (bytes memory) {
        return storedResults[proofId];
    }

    /// @notice Read comprehensive proof details.
    /// @param proofId The proof identifier (hash).
    /// @return proof The stored proof bytes.
    /// @return result The stored result bytes.
    /// @return verified True if the proof has been verified.
    function readProofDetails(bytes32 proofId) external view returns (
        bytes memory proof,
        bytes memory result,
        bool verified
    ) {
        proof = storedProofs[proofId];
        result = storedResults[proofId];
        verified = verifiedProofs[proofId];
        
        if (proof.length == 0) revert ProofNotFound();
    }

    /// @notice Update multiple states in a single transaction.
    /// @param stateIds Array of state identifiers.
    /// @param newStates Array of new state roots.
    /// @param proofs Array of ZK proofs.
    /// @param results Array of verification results.
    /// @return successes Array indicating success/failure for each update.
    function batchUpdateStates(
        bytes32[] calldata stateIds,
        bytes32[] calldata newStates,
        bytes[] calldata proofs,
        bytes[] calldata results
    ) external onlyAuthorized returns (bool[] memory successes) {
        uint256 length = stateIds.length;
        
        if (length != newStates.length || 
            length != proofs.length || 
            length != results.length) {
            revert InvalidArrayLength();
        }
        
        successes = new bool[](length);
        
        // Cache frequently accessed values to reduce external calls
        address cachedVerifier = verifier;
        bytes32 cachedVKey = arithmeticProgramVKey;
        address cachedSender = msg.sender;
        uint256 cachedTimestamp = block.timestamp;
        
        // Pre-verify all proofs to fail fast if any are invalid
        for (uint256 i = 0; i < length;) {
            try ISP1Verifier(cachedVerifier).verifyProof(
                cachedVKey,
                results[i],
                proofs[i]
            ) {
                successes[i] = true;
            } catch {
                successes[i] = false;
            }
            
            unchecked {
                ++i;
            }
        }
        
        // Batch process all successful verifications
        for (uint256 i = 0; i < length;) {
            if (successes[i]) {
                bytes32 proofHash = keccak256(proofs[i]);
                
                // Batch storage updates
                currentState[stateIds[i]] = newStates[i];
                stateHistory[stateIds[i]].push(newStates[i]);
                storedProofs[proofHash] = proofs[i];
                storedResults[proofHash] = results[i];
                verifiedProofs[proofHash] = true;
                
                // Store proof metadata (optimized call)
                _storeProofMetadata(proofHash, stateIds[i], cachedSender);
                
                // Emit individual state updated events
                emit StateUpdated(stateIds[i], newStates[i], proofHash, cachedSender, cachedTimestamp);
                
                // Update event statistics
                eventCountByStateId[stateIds[i]]++;
            }
            
            unchecked {
                ++i;
            }
        }
        
        // Emit batch event and update stats once
        emit BatchStateUpdated(stateIds, newStates, cachedSender, cachedTimestamp);
        _updateEventStats("batch_update");
    }

    /// @notice Read multiple current states.
    /// @param stateIds Array of state identifiers.
    /// @return states Array of current state roots.
    function batchReadStates(bytes32[] calldata stateIds) external view returns (bytes32[] memory states) {
        uint256 length = stateIds.length;
        states = new bytes32[](length);
        
        for (uint256 i = 0; i < length;) {
            states[i] = currentState[stateIds[i]];
            
            unchecked {
                ++i;
            }
        }
    }

    /// @notice Grant or revoke authorization to post state updates.
    /// @param account The account to modify authorization for.
    /// @param authorized True to grant, false to revoke authorization.
    function setAuthorization(address account, bool authorized) external onlyOwner {
        authorizedPosters[account] = authorized;
        emit AuthorizationChanged(account, authorized);
    }

    /// @notice Transfer ownership of the contract.
    /// @param newOwner The new owner address.
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "new owner is zero address");
        address previousOwner = owner;
        owner = newOwner;
        
        authorizedPosters[previousOwner] = false;
        authorizedPosters[newOwner] = true;
        
        emit AuthorizationChanged(previousOwner, false);
        emit AuthorizationChanged(newOwner, true);
        emit OwnershipTransferred(previousOwner, newOwner);
    }

    /// @notice Check if an address is authorized to post state updates.
    /// @param account The account to check.
    /// @return True if authorized.
    function isAuthorized(address account) external view returns (bool) {
        return authorizedPosters[account] || account == owner;
    }


    /// @notice Get the total number of states in history for a state ID.
    /// @param stateId The state identifier.
    /// @return The number of historical states.
    function getStateHistoryLength(bytes32 stateId) external view returns (uint256) {
        return stateHistory[stateId].length;
    }

    /*//////////////////////////////////////////////////////////////
                        PROOF READING FUNCTIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Get proof by proof ID.
    /// @param proofId The proof identifier (hash).
    /// @return proof The proof bytes.
    /// @return exists True if the proof exists.
    function getProofById(bytes32 proofId) external returns (bytes memory proof, bool exists) {
        proof = storedProofs[proofId];
        exists = proofMetadata[proofId].exists;
        
        // Track read event
        _trackReadEvent(bytes32(0), proofId);
    }

    /// @notice Get proof by state ID (returns latest proof for that state).
    /// @param stateId The state identifier.
    /// @return proof The latest proof bytes for the state.
    /// @return proofId The proof identifier.
    function getProofByStateId(bytes32 stateId) external returns (bytes memory proof, bytes32 proofId) {
        proofId = latestProofForState[stateId];
        if (proofId == bytes32(0)) {
            return (new bytes(0), bytes32(0));
        }
        proof = storedProofs[proofId];
        
        // Track read events for both state and proof
        _trackReadEvent(stateId, proofId);
    }

    /// @notice Get latest proof for a state ID with timestamp.
    /// @param stateId The state identifier.
    /// @return proof The latest proof bytes.
    /// @return proofId The proof identifier.
    /// @return timestamp When the proof was submitted.
    function getLatestProof(bytes32 stateId) external view returns (
        bytes memory proof,
        bytes32 proofId,
        uint256 timestamp
    ) {
        proofId = latestProofForState[stateId];
        if (proofId == bytes32(0)) {
            return (new bytes(0), bytes32(0), 0);
        }
        
        proof = storedProofs[proofId];
        timestamp = proofMetadata[proofId].timestamp;
    }

    /*//////////////////////////////////////////////////////////////
                    PROOF VERIFICATION STATUS
    //////////////////////////////////////////////////////////////*/

    /// @notice Check if a proof is verified.
    /// @param proofId The proof identifier.
    /// @return True if the proof is verified.
    function isProofVerified(bytes32 proofId) external view returns (bool) {
        return proofMetadata[proofId].verified;
    }

    /// @notice Get verification result for a proof.
    /// @param proofId The proof identifier.
    /// @return verified True if the proof is verified.
    /// @return result The verification result bytes.
    function getVerificationResult(bytes32 proofId) external view returns (bool verified, bytes memory result) {
        verified = proofMetadata[proofId].verified;
        result = storedResults[proofId];
    }

    /// @notice Get timestamp when proof was submitted.
    /// @param proofId The proof identifier.
    /// @return The timestamp of proof submission.
    function getProofTimestamp(bytes32 proofId) external view returns (uint256) {
        return proofMetadata[proofId].timestamp;
    }

    /*//////////////////////////////////////////////////////////////
                        PROOF ENUMERATION
    //////////////////////////////////////////////////////////////*/

    /// @notice Get total number of stored proofs.
    /// @return The total count of proofs.
    function getProofCount() external view returns (uint256) {
        return allProofIds.length;
    }

    /// @notice Get proof by index in the enumeration.
    /// @param index The index in the proof array.
    /// @return proofId The proof identifier at the index.
    /// @return proof The proof bytes.
    function getProofByIndex(uint256 index) external view returns (bytes32 proofId, bytes memory proof) {
        if (index >= allProofIds.length) revert InvalidIndex();
        
        proofId = allProofIds[index];
        proof = storedProofs[proofId];
    }

    /// @notice Get recent proofs up to a specified limit.
    /// @param limit Maximum number of recent proofs to return.
    /// @return proofIds Array of recent proof identifiers.
    /// @return proofs Array of recent proof bytes.
    function getRecentProofs(uint256 limit) external view returns (
        bytes32[] memory proofIds,
        bytes[] memory proofs
    ) {
        uint256 totalProofs = allProofIds.length;
        if (totalProofs == 0) {
            return (new bytes32[](0), new bytes[](0));
        }
        
        if (limit == 0 || limit > totalProofs) {
            limit = totalProofs;
        }
        
        proofIds = new bytes32[](limit);
        proofs = new bytes[](limit);
        
        uint256 startIndex = totalProofs - limit;
        
        for (uint256 i = 0; i < limit;) {
            bytes32 proofId = allProofIds[startIndex + i];
            proofIds[i] = proofId;
            proofs[i] = storedProofs[proofId];
            
            unchecked {
                ++i;
            }
        }
    }

    /// @notice Get all proof IDs for a specific state ID.
    /// @param stateId The state identifier.
    /// @return proofIds Array of proof identifiers for the state.
    function getProofsByStateId(bytes32 stateId) external view returns (bytes32[] memory proofIds) {
        return stateToProofs[stateId];
    }

    /// @notice Get proof metadata by proof ID.
    /// @param proofId The proof identifier.
    /// @return metadata The complete proof metadata.
    function getProofMetadata(bytes32 proofId) external view returns (ProofMetadata memory metadata) {
        metadata = proofMetadata[proofId];
        if (!metadata.exists) revert ProofNotFound();
    }

    /// @notice Get submitter address for a proof.
    /// @param proofId The proof identifier.
    /// @return submitter The address that submitted the proof.
    function getProofSubmitter(bytes32 proofId) external view returns (address submitter) {
        if (!proofMetadata[proofId].exists) revert ProofNotFound();
        return proofMetadata[proofId].submitter;
    }

    /*//////////////////////////////////////////////////////////////
                        INTERNAL PROOF MANAGEMENT
    //////////////////////////////////////////////////////////////*/

    /// @notice Internal function to store proof metadata.
    /// @param proofId The proof identifier.
    /// @param stateId The associated state identifier.
    /// @param submitter The address submitting the proof.
    function _storeProofMetadata(bytes32 proofId, bytes32 stateId, address submitter) internal {
        // Revert if proof already exists to prevent duplicate submissions
        if (proofMetadata[proofId].exists) {
            revert ProofAlreadyExists();
        }
        
        proofMetadata[proofId] = ProofMetadata({
            proofId: proofId,
            stateId: stateId,
            submitter: submitter,
            timestamp: block.timestamp,
            verified: true, // Set to true since we verify before storing
            exists: true
        });
        
        // Add to enumeration
        proofIdToIndex[proofId] = allProofIds.length;
        allProofIds.push(proofId);
        
        // Add to state mapping
        stateToProofs[stateId].push(proofId);
        
        // Update latest proof for state
        latestProofForState[stateId] = proofId;
        
        emit ProofStored(proofId, stateId, submitter, block.timestamp);
        emit ProofVerified(proofId, true, new bytes(0), block.timestamp);
        
        // Update event statistics
        _updateEventStats("proof_stored");
    }

    /*//////////////////////////////////////////////////////////////
                        EVENT HELPER IMPLEMENTATIONS
    //////////////////////////////////////////////////////////////*/

    /// @notice Internal function to update event statistics.
    function _updateEventStats(string memory eventType) internal override {
        eventStats.lastEventTimestamp = block.timestamp;
        
        // Update daily counts
        uint256 today = block.timestamp / 86400;
        dailyEventCounts[today]++;
        
        // Update submitter counts
        eventCountBySubmitter[msg.sender]++;
        
        // Update specific event type counts
        bytes32 eventHash = keccak256(abi.encodePacked(eventType));
        
        if (eventHash == STATE_UPDATE_HASH) {
            eventStats.totalStateUpdates++;
        } else if (eventHash == BATCH_UPDATE_HASH) {
            eventStats.totalBatchUpdates++;
        } else if (eventHash == PROOF_STORED_HASH) {
            eventStats.totalProofStored++;
            eventStats.totalProofVerified++; // Proofs are verified when stored
        }
    }

    /// @notice Internal function to track read events.
    function _trackReadEvent(bytes32 stateId, bytes32 proofId) internal override {
        if (!readEventTrackingEnabled) return;
        
        eventStats.lastEventTimestamp = block.timestamp;
        
        // Update daily counts
        uint256 today = block.timestamp / 86400;
        dailyEventCounts[today]++;
        
        // Update submitter counts
        eventCountBySubmitter[msg.sender]++;
        
        if (stateId != bytes32(0)) {
            eventStats.totalStateReads++;
            eventCountByStateId[stateId]++;
            emit StateReadRequested(stateId, msg.sender, block.timestamp);
        }
        
        if (proofId != bytes32(0)) {
            eventStats.totalProofReads++;
            emit ProofReadRequested(proofId, msg.sender, block.timestamp);
        }
    }

    /// @notice Get event statistics for monitoring.
    function getEventStats() external view override returns (EventStats memory) {
        return eventStats;
    }

    /// @notice Get event count by submitter address.
    function getEventCountBySubmitter(address submitter) external view override returns (uint256) {
        return eventCountBySubmitter[submitter];
    }

    /// @notice Get event count by state ID.
    function getEventCountByStateId(bytes32 stateId) external view override returns (uint256) {
        return eventCountByStateId[stateId];
    }

    /// @notice Get daily event count for a specific day.
    function getDailyEventCount(uint256 day) external view override returns (uint256) {
        return dailyEventCounts[day];
    }

    /// @notice Toggle read event tracking (gas optimization).
    function setReadEventTracking(bool enabled) external override onlyOwner {
        readEventTrackingEnabled = enabled;
    }

    /// @notice Get aggregated event counts for a time range.
    function getEventCountInRange(uint256 startTime, uint256 endTime) external view override returns (
        uint256 totalEvents
    ) {
        if (startTime >= endTime) {
            return 0;
        }
        
        uint256 startDay = startTime / 86400;
        uint256 endDay = endTime / 86400;
        
        for (uint256 day = startDay; day <= endDay; day++) {
            totalEvents += dailyEventCounts[day];
        }
    }

}
