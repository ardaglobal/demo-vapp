// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title StateInteraction
/// @author Arda Global
/// @notice Library for interacting with state management contracts
/// @dev Provides reusable functions for state posting, reading, and validation
library StateInteraction {
    
    /// @notice Custom errors for better gas efficiency and debugging
    error ContractNotFound();
    error InvalidContractInterface();
    error CallFailed(bytes reason);
    error InvalidParameters();
    error ArrayLengthMismatch();
    error GasEstimationFailed();
    
    /// @notice Struct for state update parameters
    struct StateUpdateParams {
        bytes32 stateId;
        bytes32 newState;
        bytes proof;
        bytes result;
    }
    
    /// @notice Struct for batch state reading results
    struct BatchStateResult {
        bytes32[] states;
        bool[] exists;
    }
    
    /// @notice Struct for proof details
    struct ProofDetails {
        bytes proof;
        bytes result;
        bool verified;
        bool exists;
    }
    
    /// @notice Events for tracking library operations
    event StateUpdatePrepared(bytes32 indexed stateId, bytes32 newState);
    event StateUpdateExecuted(address indexed contractAddr, bytes32 indexed stateId, bool success);
    event BatchStateRead(address indexed contractAddr, uint256 count);
    
    /*//////////////////////////////////////////////////////////////
                        STATE POSTING HELPERS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Prepare state update call data (public for testing)
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @param result The verification result
    /// @return callData The encoded function call data
    function prepareStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) internal pure returns (bytes memory callData) {
        if (stateId == bytes32(0) || newState == bytes32(0)) {
            revert InvalidParameters();
        }
        
        callData = abi.encodeWithSignature(
            "postStateUpdate(bytes32,bytes32,bytes,bytes)",
            stateId,
            newState,
            proof,
            result
        );
    }
    
    /// @notice Execute state update on target contract
    /// @param contractAddr The target contract address
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @param result The verification result
    /// @return success True if the update succeeded
    function executeStateUpdate(
        address contractAddr,
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external returns (bool success) {
        if (!_isContract(contractAddr)) {
            revert ContractNotFound();
        }
        
        bytes memory callData = prepareStateUpdate(stateId, newState, proof, result);
        
        (bool callSuccess, bytes memory returnData) = contractAddr.call(callData);
        
        if (!callSuccess) {
            revert CallFailed(returnData);
        }
        
        success = abi.decode(returnData, (bool));
        
        emit StateUpdateExecuted(contractAddr, stateId, success);
    }
    
    /// @notice Validate state update parameters before posting
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @return valid True if parameters are valid
    function validateStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof
    ) external pure returns (bool valid) {
        if (stateId == bytes32(0)) return false;
        if (newState == bytes32(0)) return false;
        if (proof.length == 0) return false;
        
        return true;
    }
    
    /*//////////////////////////////////////////////////////////////
                        STATE READING HELPERS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Read current state from any contract
    /// @param contractAddr The target contract address
    /// @param stateId The state identifier
    /// @return state The current state root
    /// @return exists True if the state exists
    function readStateFromContract(
        address contractAddr,
        bytes32 stateId
    ) internal view returns (bytes32 state, bool exists) {
        if (!_isContract(contractAddr)) {
            revert ContractNotFound();
        }
        
        bytes memory callData = abi.encodeWithSignature(
            "readCurrentState(bytes32)",
            stateId
        );
        
        (bool success, bytes memory returnData) = contractAddr.staticcall(callData);
        
        if (!success) {
            return (bytes32(0), false);
        }
        
        (state, exists) = abi.decode(returnData, (bytes32, bool));
    }
    
    /// @notice Read proof details from any contract
    /// @param contractAddr The target contract address
    /// @param proofId The proof identifier (hash)
    /// @return details The proof details struct
    function readProofFromContract(
        address contractAddr,
        bytes32 proofId
    ) external view returns (ProofDetails memory details) {
        if (!_isContract(contractAddr)) {
            revert ContractNotFound();
        }
        
        bytes memory callData = abi.encodeWithSignature(
            "readProofDetails(bytes32)",
            proofId
        );
        
        (bool success, bytes memory returnData) = contractAddr.staticcall(callData);
        
        if (!success) {
            return ProofDetails({
                proof: new bytes(0),
                result: new bytes(0),
                verified: false,
                exists: false
            });
        }
        
        (details.proof, details.result, details.verified) = abi.decode(
            returnData, 
            (bytes, bytes, bool)
        );
        details.exists = true;
    }
    
    /// @notice Batch read multiple states from contract
    /// @param contractAddr The target contract address
    /// @param stateIds Array of state identifiers
    /// @return result The batch read result
    function batchReadStates(
        address contractAddr,
        bytes32[] calldata stateIds
    ) external view returns (BatchStateResult memory result) {
        if (!_isContract(contractAddr)) {
            revert ContractNotFound();
        }
        
        if (stateIds.length == 0) {
            revert InvalidParameters();
        }
        
        result.states = new bytes32[](stateIds.length);
        result.exists = new bool[](stateIds.length);
        
        for (uint256 i = 0; i < stateIds.length;) {
            (result.states[i], result.exists[i]) = readStateFromContract(
                contractAddr,
                stateIds[i]
            );
            
            unchecked {
                ++i;
            }
        }
    }
    
    /*//////////////////////////////////////////////////////////////
                    CONTRACT INTERACTION UTILITIES
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Encode state update call data
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @param result The verification result
    /// @return encoded The encoded call data
    function encodeStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external pure returns (bytes memory encoded) {
        encoded = abi.encode(stateId, newState, proof, result);
    }
    
    /// @notice Decode state response data
    /// @param response The response bytes from contract call
    /// @return state The decoded state root
    /// @return exists True if state exists
    function decodeStateResponse(
        bytes calldata response
    ) external pure returns (bytes32 state, bool exists) {
        if (response.length < 64) {
            return (bytes32(0), false);
        }
        
        (state, exists) = abi.decode(response, (bytes32, bool));
    }
    
    /// @notice Estimate gas cost for state update
    /// @param stateId The state identifier
    /// @param proof The ZK proof
    /// @return gasEstimate The estimated gas cost
    function estimateUpdateGas(
        bytes32 stateId,
        bytes calldata proof
    ) external pure returns (uint256 gasEstimate) {
        uint256 baseGas = 21000;
        uint256 callDataGas = (proof.length + 64) * 16;
        uint256 storageGas = 20000 * 3;
        uint256 computationGas = 5000;
        
        gasEstimate = baseGas + callDataGas + storageGas + computationGas;
        
        if (gasEstimate == 0) {
            revert GasEstimationFailed();
        }
    }
    
    /*//////////////////////////////////////////////////////////////
                    ERROR HANDLING AND VALIDATION
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Check if contract exists and has required functions
    /// @param contractAddr The contract address to check
    /// @return hasInterface True if contract supports the interface
    function checkContractInterface(
        address contractAddr
    ) external view returns (bool hasInterface) {
        if (!_isContract(contractAddr)) {
            return false;
        }
        
        bytes memory callData = abi.encodeWithSignature("postStateUpdate(bytes32,bytes32,bytes,bytes)", bytes32(0), bytes32(0), "", "");
        
        (bool success,) = contractAddr.staticcall{gas: 10000}(callData);
        
        return success;
    }
    
    /// @notice Validate state update parameters comprehensively
    /// @param params The state update parameters
    /// @return isValid True if all parameters are valid
    /// @return errorMessage Error description if invalid
    function validateParameters(
        StateUpdateParams calldata params
    ) external pure returns (bool isValid, string memory errorMessage) {
        if (params.stateId == bytes32(0)) {
            return (false, "Invalid state ID");
        }
        
        if (params.newState == bytes32(0)) {
            return (false, "Invalid new state");
        }
        
        if (params.proof.length == 0) {
            return (false, "Empty proof");
        }
        
        if (params.result.length == 0) {
            return (false, "Empty result");
        }
        
        return (true, "");
    }
    
    /// @notice Handle call failures gracefully
    /// @param success The call success status
    /// @param returnData The return data from failed call
    /// @return handled True if error was handled
    /// @return reason The error reason
    function handleCallFailure(
        bool success,
        bytes memory returnData
    ) external pure returns (bool handled, string memory reason) {
        if (success) {
            return (true, "");
        }
        
        if (returnData.length == 0) {
            return (false, "Call reverted without reason");
        }
        
        if (returnData.length < 4) {
            return (false, "Invalid return data");
        }
        
        bytes4 selector = bytes4(returnData);
        
        if (selector == ContractNotFound.selector) {
            return (true, "Contract not found");
        } else if (selector == InvalidParameters.selector) {
            return (true, "Invalid parameters");
        } else if (selector == CallFailed.selector) {
            return (true, "Call failed");
        }
        
        return (false, "Unknown error");
    }
    
    /// @notice Batch validate array parameters
    /// @param stateIds Array of state identifiers
    /// @param newStates Array of new state roots
    /// @param proofs Array of proofs
    /// @param results Array of results
    /// @return isValid True if all arrays are valid
    function validateBatchParameters(
        bytes32[] calldata stateIds,
        bytes32[] calldata newStates,
        bytes[] calldata proofs,
        bytes[] calldata results
    ) external pure returns (bool isValid) {
        if (stateIds.length == 0) return false;
        
        if (stateIds.length != newStates.length ||
            stateIds.length != proofs.length ||
            stateIds.length != results.length) {
            return false;
        }
        
        for (uint256 i = 0; i < stateIds.length;) {
            if (stateIds[i] == bytes32(0) || newStates[i] == bytes32(0)) {
                return false;
            }
            
            unchecked {
                ++i;
            }
        }
        
        return true;
    }
    
    /*//////////////////////////////////////////////////////////////
                        INTERNAL HELPERS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Check if address is a contract
    /// @param addr The address to check
    /// @return True if address is a contract
    function _isContract(address addr) internal view returns (bool) {
        uint256 size;
        assembly {
            size := extcodesize(addr)
        }
        return size > 0;
    }
    
    /*//////////////////////////////////////////////////////////////
                        UTILITY FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Calculate keccak256 hash of state update
    /// @param stateId The state identifier
    /// @param newState The new state root
    /// @param proof The ZK proof
    /// @return hash The calculated hash
    function calculateStateHash(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof
    ) external pure returns (bytes32 hash) {
        hash = keccak256(abi.encodePacked(stateId, newState, proof));
    }
    
    /// @notice Get function selector for state operations
    /// @param functionName The function name
    /// @return selector The 4-byte function selector
    function getFunctionSelector(
        string calldata functionName
    ) external pure returns (bytes4 selector) {
        selector = bytes4(keccak256(bytes(functionName)));
    }
    
    /// @notice Check if state update would be a no-op
    /// @param contractAddr The target contract address
    /// @param stateId The state identifier
    /// @param newState The proposed new state
    /// @return isNoOp True if the update would not change state
    function isNoOpUpdate(
        address contractAddr,
        bytes32 stateId,
        bytes32 newState
    ) external view returns (bool isNoOp) {
        (bytes32 currentState, bool exists) = readStateFromContract(contractAddr, stateId);
        
        if (!exists) {
            return false;
        }
        
        return currentState == newState;
    }
}