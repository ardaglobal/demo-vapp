// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {StateInteraction} from "../libraries/StateInteraction.sol";

/// @title StateManagerExample
/// @notice Example contract demonstrating how to use the StateInteraction library
contract StateManagerExample {
    using StateInteraction for *;
    
    /// @notice Events for tracking operations
    event StatePosted(address indexed target, bytes32 indexed stateId, bool success);
    event BatchStatesRead(address indexed target, uint256 count);
    event GasEstimated(bytes32 indexed stateId, uint256 gasEstimate);
    
    /// @notice Post a state update to a target contract
    /// @param target The target contract address
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @param result The verification result
    /// @return success True if the operation succeeded
    function postState(
        address target,
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external returns (bool success) {
        // Validate parameters first
        if (!StateInteraction.validateStateUpdate(stateId, newState, proof)) {
            revert("Invalid parameters");
        }
        
        // Check if this would be a no-op update
        if (StateInteraction.isNoOpUpdate(target, stateId, newState)) {
            return true; // No need to update
        }
        
        // Execute the state update
        success = StateInteraction.executeStateUpdate(
            target,
            stateId,
            newState,
            proof,
            result
        );
        
        emit StatePosted(target, stateId, success);
    }
    
    /// @notice Read state from a target contract
    /// @param target The target contract address
    /// @param stateId The state identifier
    /// @return state The current state root
    /// @return exists True if the state exists
    function readState(
        address target,
        bytes32 stateId
    ) external view returns (bytes32 state, bool exists) {
        return StateInteraction.readStateFromContract(target, stateId);
    }
    
    /// @notice Read multiple states in a batch operation
    /// @param target The target contract address
    /// @param stateIds Array of state identifiers
    /// @return result The batch read result
    function batchReadStates(
        address target,
        bytes32[] calldata stateIds
    ) external returns (StateInteraction.BatchStateResult memory result) {
        result = StateInteraction.batchReadStates(target, stateIds);
        
        emit BatchStatesRead(target, stateIds.length);
    }
    
    /// @notice Get proof details from a target contract
    /// @param target The target contract address
    /// @param proofId The proof identifier
    /// @return details The proof details
    function getProofDetails(
        address target,
        bytes32 proofId
    ) external view returns (StateInteraction.ProofDetails memory details) {
        return StateInteraction.readProofFromContract(target, proofId);
    }
    
    /// @notice Estimate gas cost for a state update
    /// @param stateId The state identifier
    /// @param proof The ZK proof
    /// @return gasEstimate The estimated gas cost
    function estimateGas(
        bytes32 stateId,
        bytes calldata proof
    ) external returns (uint256 gasEstimate) {
        gasEstimate = StateInteraction.estimateUpdateGas(stateId, proof);
        
        emit GasEstimated(stateId, gasEstimate);
    }
    
    /// @notice Batch post multiple state updates
    /// @param target The target contract address
    /// @param params Array of state update parameters
    /// @return successes Array of success flags
    function batchPostStates(
        address target,
        StateInteraction.StateUpdateParams[] calldata params
    ) external returns (bool[] memory successes) {
        successes = new bool[](params.length);
        
        for (uint256 i = 0; i < params.length;) {
            // Validate each parameter set
            (bool isValid,) = StateInteraction.validateParameters(params[i]);
            
            if (isValid) {
                successes[i] = StateInteraction.executeStateUpdate(
                    target,
                    params[i].stateId,
                    params[i].newState,
                    params[i].proof,
                    params[i].result
                );
            }
            
            emit StatePosted(target, params[i].stateId, successes[i]);
            
            unchecked {
                ++i;
            }
        }
    }
    
    /// @notice Check if a target contract supports the state interface
    /// @param target The target contract address
    /// @return supported True if interface is supported
    function checkInterface(address target) external view returns (bool supported) {
        return StateInteraction.checkContractInterface(target);
    }
    
    /// @notice Prepare call data for a state update (for off-chain use)
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @param result The verification result
    /// @return callData The prepared call data
    function prepareCallData(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external pure returns (bytes memory callData) {
        return StateInteraction.prepareStateUpdate(stateId, newState, proof, result);
    }
    
    /// @notice Calculate hash of state update for tracking
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @return hash The calculated hash
    function calculateHash(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof
    ) external pure returns (bytes32 hash) {
        return StateInteraction.calculateStateHash(stateId, newState, proof);
    }
}