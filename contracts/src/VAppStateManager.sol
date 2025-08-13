// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {SP1VerifierGateway} from "./SP1VerifierGateway.sol";
import {EventHelpers} from "./EventHelpers.sol";

/// @title VAppStateManager
/// @author Arda Global
/// @notice State manager for vApp with BYO proving key integration
/// @dev This contract manages state transitions for vApps using SP1 proofs
///      with Bring Your Own Proving Key (BYO-PK) model via Sindri.
contract VAppStateManager is EventHelpers {
    
    /*//////////////////////////////////////////////////////////////
                                STRUCTS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice State transition data
    struct StateTransition {
        bytes32 prevStateRoot;
        bytes32 nextStateRoot;
        bytes32 batchCommitment;
        int32 operationResult;
        uint256 timestamp;
        address submitter;
    }
    
    /// @notice vApp configuration
    struct VAppConfig {
        bytes32 vkeyHash;          // Verification key hash for this vApp
        string name;               // vApp name
        address owner;             // vApp owner
        bool active;               // Whether vApp is active
        uint256 createdAt;         // Creation timestamp
    }
    
    /*//////////////////////////////////////////////////////////////
                                EVENTS
    //////////////////////////////////////////////////////////////*/
    
    event VAppRegistered(
        bytes32 indexed vappId,
        string name,
        bytes32 indexed vkeyHash,
        address indexed owner
    );
    
    event StateTransitionVerified(
        bytes32 indexed vappId,
        bytes32 indexed prevStateRoot,
        bytes32 indexed nextStateRoot,
        bytes32 batchCommitment,
        address submitter
    );
    
    event VAppStateUpdated(
        bytes32 indexed vappId,
        bytes32 indexed newStateRoot,
        uint256 timestamp
    );
    
    /*//////////////////////////////////////////////////////////////
                                ERRORS
    //////////////////////////////////////////////////////////////*/
    
    error UnauthorizedAccess();
    error VAppNotFound();
    error VAppInactive();
    error InvalidStateTransition();
    error VerificationFailed();
    error VAppAlreadyExists();
    
    /*//////////////////////////////////////////////////////////////
                            STATE VARIABLES
    //////////////////////////////////////////////////////////////*/
    
    /// @notice The SP1 verifier gateway
    SP1VerifierGateway public immutable verifierGateway;
    
    /// @notice Contract owner
    address public owner;
    
    /// @notice vApp configurations by ID
    mapping(bytes32 => VAppConfig) public vappConfigs;
    
    /// @notice Current state roots by vApp ID
    mapping(bytes32 => bytes32) public currentStateRoots;
    
    /// @notice State transition history by vApp ID
    mapping(bytes32 => StateTransition[]) public stateHistory;
    
    /// @notice Latest state transition by vApp ID
    mapping(bytes32 => StateTransition) public latestTransitions;
    
    /// @notice Authorized operators per vApp
    mapping(bytes32 => mapping(address => bool)) public authorizedOperators;
    
    /// @notice All registered vApp IDs
    bytes32[] public allVAppIds;
    
    /// @notice Event statistics (inherited from EventHelpers)
    EventStats public eventStats;
    
    /*//////////////////////////////////////////////////////////////
                                MODIFIERS
    //////////////////////////////////////////////////////////////*/
    
    modifier onlyOwner() {
        if (msg.sender != owner) revert UnauthorizedAccess();
        _;
    }
    
    modifier onlyVAppOperator(bytes32 vappId) {
        VAppConfig memory config = vappConfigs[vappId];
        if (config.owner == address(0)) revert VAppNotFound();
        if (!config.active) revert VAppInactive();
        
        if (msg.sender != config.owner && !authorizedOperators[vappId][msg.sender]) {
            revert UnauthorizedAccess();
        }
        _;
    }
    
    /*//////////////////////////////////////////////////////////////
                            CONSTRUCTOR
    //////////////////////////////////////////////////////////////*/
    
    constructor(address _verifierGateway) {
        verifierGateway = SP1VerifierGateway(_verifierGateway);
        owner = msg.sender;
    }
    
    /*//////////////////////////////////////////////////////////////
                            VAPP MANAGEMENT
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Register a new vApp with BYO proving key
    /// @param vappId The unique identifier for the vApp
    /// @param name The name of the vApp
    /// @param vkeyHash The hash of the verification key for this vApp
    /// @param initialStateRoot The initial state root
    function registerVApp(
        bytes32 vappId,
        string calldata name,
        bytes32 vkeyHash,
        bytes32 initialStateRoot
    ) external {
        // Check if vApp already exists
        if (vappConfigs[vappId].owner != address(0)) revert VAppAlreadyExists();
        
        // Verify that the verification key is registered in the gateway
        if (!verifierGateway.isValidVerificationKey(vkeyHash)) {
            revert VerificationFailed();
        }
        
        // Create vApp configuration
        vappConfigs[vappId] = VAppConfig({
            vkeyHash: vkeyHash,
            name: name,
            owner: msg.sender,
            active: true,
            createdAt: block.timestamp
        });
        
        // Set initial state
        currentStateRoots[vappId] = initialStateRoot;
        
        // Add to enumeration
        allVAppIds.push(vappId);
        
        // Set owner as authorized operator
        authorizedOperators[vappId][msg.sender] = true;
        
        emit VAppRegistered(vappId, name, vkeyHash, msg.sender);
        emit VAppStateUpdated(vappId, initialStateRoot, block.timestamp);
        
        // Update event statistics
        _updateEventStats("vapp_registered");
    }
    
    /// @notice Update vApp state with verified proof
    /// @param vappId The vApp identifier
    /// @param prevStateRoot The previous state root
    /// @param nextStateRoot The next state root
    /// @param batchCommitment The batch commitment
    /// @param operationResult The operation result
    /// @param publicValues The encoded public values
    /// @param proofBytes The SP1 proof bytes
    function updateVAppState(
        bytes32 vappId,
        bytes32 prevStateRoot,
        bytes32 nextStateRoot,
        bytes32 batchCommitment,
        int32 operationResult,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external onlyVAppOperator(vappId) {
        VAppConfig memory config = vappConfigs[vappId];
        
        // Verify the current state matches expected previous state
        if (currentStateRoots[vappId] != prevStateRoot) {
            revert InvalidStateTransition();
        }
        
        // Verify the proof using the vApp's verification key
        bool verified = verifierGateway.tryVerifyProof(
            config.vkeyHash,
            publicValues,
            proofBytes
        );
        
        if (!verified) revert VerificationFailed();
        
        // Create state transition record
        StateTransition memory transition = StateTransition({
            prevStateRoot: prevStateRoot,
            nextStateRoot: nextStateRoot,
            batchCommitment: batchCommitment,
            operationResult: operationResult,
            timestamp: block.timestamp,
            submitter: msg.sender
        });
        
        // Update state
        currentStateRoots[vappId] = nextStateRoot;
        stateHistory[vappId].push(transition);
        latestTransitions[vappId] = transition;
        
        emit StateTransitionVerified(
            vappId,
            prevStateRoot,
            nextStateRoot,
            batchCommitment,
            msg.sender
        );
        
        emit VAppStateUpdated(vappId, nextStateRoot, block.timestamp);
        
        // Update event statistics
        _updateEventStats("state_transition");
    }
    
    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Get current state root for a vApp
    /// @param vappId The vApp identifier
    /// @return The current state root
    function getCurrentStateRoot(bytes32 vappId) external view returns (bytes32) {
        if (vappConfigs[vappId].owner == address(0)) revert VAppNotFound();
        return currentStateRoots[vappId];
    }
    
    /// @notice Get vApp configuration
    /// @param vappId The vApp identifier
    /// @return The vApp configuration
    function getVAppConfig(bytes32 vappId) external view returns (VAppConfig memory) {
        if (vappConfigs[vappId].owner == address(0)) revert VAppNotFound();
        return vappConfigs[vappId];
    }
    
    /// @notice Get state history length for a vApp
    /// @param vappId The vApp identifier
    /// @return The number of state transitions
    function getStateHistoryLength(bytes32 vappId) external view returns (uint256) {
        return stateHistory[vappId].length;
    }
    
    /// @notice Get state transition by index
    /// @param vappId The vApp identifier
    /// @param index The index in the history
    /// @return The state transition
    function getStateTransition(bytes32 vappId, uint256 index) external view returns (StateTransition memory) {
        require(index < stateHistory[vappId].length, "Index out of bounds");
        return stateHistory[vappId][index];
    }
    
    /// @notice Get latest state transition for a vApp
    /// @param vappId The vApp identifier
    /// @return The latest state transition
    function getLatestTransition(bytes32 vappId) external view returns (StateTransition memory) {
        if (vappConfigs[vappId].owner == address(0)) revert VAppNotFound();
        return latestTransitions[vappId];
    }
    
    /// @notice Get all registered vApp IDs
    /// @return Array of vApp IDs
    function getAllVAppIds() external view returns (bytes32[] memory) {
        return allVAppIds;
    }
    
    /// @notice Get the number of registered vApps
    /// @return The count of registered vApps
    function getVAppCount() external view returns (uint256) {
        return allVAppIds.length;
    }
    
    /*//////////////////////////////////////////////////////////////
                            ADMIN FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Set authorized operator for a vApp
    /// @param vappId The vApp identifier
    /// @param operator The operator address
    /// @param authorized Whether to authorize or deauthorize
    function setAuthorizedOperator(
        bytes32 vappId,
        address operator,
        bool authorized
    ) external {
        VAppConfig memory config = vappConfigs[vappId];
        if (config.owner == address(0)) revert VAppNotFound();
        if (msg.sender != config.owner) revert UnauthorizedAccess();
        
        authorizedOperators[vappId][operator] = authorized;
    }
    
    /// @notice Deactivate a vApp
    /// @param vappId The vApp identifier
    function deactivateVApp(bytes32 vappId) external onlyVAppOperator(vappId) {
        vappConfigs[vappId].active = false;
    }
    
    /// @notice Transfer ownership of the contract
    /// @param newOwner The new owner address
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "New owner is zero address");
        owner = newOwner;
    }
    
    /*//////////////////////////////////////////////////////////////
                        EVENT HELPER IMPLEMENTATIONS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Internal function to update event statistics
    function _updateEventStats(string memory eventType) internal override {
        eventStats.lastEventTimestamp = block.timestamp;
        
        bytes32 eventHash = keccak256(abi.encodePacked(eventType));
        
        if (eventHash == keccak256(abi.encodePacked("vapp_registered"))) {
            eventStats.totalStateUpdates++; // Reuse counter for vApp registrations
        } else if (eventHash == keccak256(abi.encodePacked("state_transition"))) {
            eventStats.totalStateUpdates++;
        }
    }
    
    /// @notice Internal function to track read events
    function _trackReadEvent(bytes32 stateId, bytes32 proofId) internal override {
        // Implementation for read event tracking if needed
        eventStats.lastEventTimestamp = block.timestamp;
    }
    
    /// @notice Get event statistics
    function getEventStats() external view override returns (EventStats memory) {
        return eventStats;
    }
    
    /// @notice Get event count by submitter
    function getEventCountBySubmitter(address submitter) external view override returns (uint256) {
        // Implementation would track per-submitter statistics
        return 0;
    }
    
    /// @notice Get event count by state ID
    function getEventCountByStateId(bytes32 stateId) external view override returns (uint256) {
        // Implementation would track per-state statistics
        return 0;
    }
    
    /// @notice Get daily event count
    function getDailyEventCount(uint256 day) external view override returns (uint256) {
        // Implementation would track daily statistics
        return 0;
    }
    
    /// @notice Set read event tracking
    function setReadEventTracking(bool enabled) external override onlyOwner {
        // Implementation for toggling read event tracking
    }
    
    /// @notice Get event count in range
    function getEventCountInRange(uint256 startTime, uint256 endTime) external view override returns (uint256) {
        // Implementation would count events in time range
        return 0;
    }
}
