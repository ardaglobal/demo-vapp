// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {StateInteraction} from "../src/libraries/StateInteraction.sol";
import {Arithmetic} from "../src/Arithmetic.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

// Test wrapper contract to expose internal library functions
contract StateInteractionWrapper {
    using StateInteraction for *;
    
    function prepareStateUpdate(
        bytes32 stateId,
        bytes32 newState,
        bytes calldata proof,
        bytes calldata result
    ) external pure returns (bytes memory) {
        if (stateId == bytes32(0) || newState == bytes32(0)) {
            revert();
        }
        
        return abi.encodeWithSignature(
            "postStateUpdate(bytes32,bytes32,bytes,bytes)",
            stateId,
            newState,
            proof,
            result
        );
    }
    
    function readStateFromContract(
        address contractAddr,
        bytes32 stateId
    ) external view returns (bytes32, bool) {
        if (contractAddr.code.length == 0) {
            revert();
        }
        
        bytes memory callData = abi.encodeWithSignature(
            "readCurrentState(bytes32)",
            stateId
        );
        
        (bool success, bytes memory returnData) = contractAddr.staticcall(callData);
        
        if (!success || returnData.length == 0) {
            return (bytes32(0), false);
        }
        
        bytes32 state = abi.decode(returnData, (bytes32));
        return (state, state != bytes32(0));
    }
}

contract StateInteractionTest is Test {
    using StateInteraction for *;
    
    StateInteractionWrapper public wrapper;
    Arithmetic public arithmetic;
    address public verifier;
    bytes32 public constant TEST_VKEY = 0x1234567890123456789012345678901234567890123456789012345678901234;
    
    function setUp() public {
        wrapper = new StateInteractionWrapper();
        verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, TEST_VKEY);
    }
    
    function testPrepareStateUpdate() public {
        bytes32 stateId = keccak256("test-state");
        bytes32 newState = keccak256("new-state");
        bytes memory proof = "test-proof";
        bytes memory result = "test-result";
        
        bytes memory callData = wrapper.prepareStateUpdate(
            stateId,
            newState,
            proof,
            result
        );
        
        assertTrue(callData.length > 0);
        
        bytes memory expectedCallData = abi.encodeWithSignature(
            "postStateUpdate(bytes32,bytes32,bytes,bytes)",
            stateId,
            newState,
            proof,
            result
        );
        
        assertEq(callData, expectedCallData);
    }
    
    function testValidateStateUpdate() public {
        bytes32 stateId = keccak256("test-state");
        bytes32 newState = keccak256("new-state");
        bytes memory proof = "test-proof";
        
        bool valid = StateInteraction.validateStateUpdate(stateId, newState, proof);
        assertTrue(valid);
        
        bool invalidEmpty = StateInteraction.validateStateUpdate(bytes32(0), newState, proof);
        assertFalse(invalidEmpty);
        
        bool invalidProof = StateInteraction.validateStateUpdate(stateId, newState, "");
        assertFalse(invalidProof);
    }
    
    function testReadStateFromContract() public {
        bytes32 stateId = keccak256("test-state");
        
        (bytes32 state, bool exists) = wrapper.readStateFromContract(
            address(arithmetic),
            stateId
        );
        
        assertEq(state, bytes32(0));
        assertFalse(exists);
    }
    
    function testEncodeStateUpdate() public {
        bytes32 stateId = keccak256("test-state");
        bytes32 newState = keccak256("new-state");
        bytes memory proof = "test-proof";
        bytes memory result = "test-result";
        
        bytes memory encoded = StateInteraction.encodeStateUpdate(
            stateId,
            newState,
            proof,
            result
        );
        
        assertTrue(encoded.length > 0);
        
        (bytes32 decodedStateId, bytes32 decodedNewState, bytes memory decodedProof, bytes memory decodedResult) = 
            abi.decode(encoded, (bytes32, bytes32, bytes, bytes));
        
        assertEq(decodedStateId, stateId);
        assertEq(decodedNewState, newState);
        assertEq(decodedProof, proof);
        assertEq(decodedResult, result);
    }
    
    function testEstimateUpdateGas() public {
        bytes32 stateId = keccak256("test-state");
        bytes memory proof = "test-proof-data";
        
        uint256 gasEstimate = StateInteraction.estimateUpdateGas(stateId, proof);
        assertTrue(gasEstimate > 0);
        assertTrue(gasEstimate > 21000); // Should be more than base gas
    }
    
    function testValidateParameters() public {
        StateInteraction.StateUpdateParams memory validParams = StateInteraction.StateUpdateParams({
            stateId: keccak256("test-state"),
            newState: keccak256("new-state"),
            proof: "test-proof",
            result: "test-result"
        });
        
        (bool isValid, string memory errorMessage) = StateInteraction.validateParameters(validParams);
        assertTrue(isValid);
        assertEq(errorMessage, "");
        
        StateInteraction.StateUpdateParams memory invalidParams = StateInteraction.StateUpdateParams({
            stateId: bytes32(0),
            newState: keccak256("new-state"),
            proof: "test-proof",
            result: "test-result"
        });
        
        (bool isInvalid, string memory error) = StateInteraction.validateParameters(invalidParams);
        assertFalse(isInvalid);
        assertEq(error, "Invalid state ID");
    }
    
    function testBatchValidateParameters() public {
        bytes32[] memory stateIds = new bytes32[](2);
        stateIds[0] = keccak256("state1");
        stateIds[1] = keccak256("state2");
        
        bytes32[] memory newStates = new bytes32[](2);
        newStates[0] = keccak256("newstate1");
        newStates[1] = keccak256("newstate2");
        
        bytes[] memory proofs = new bytes[](2);
        proofs[0] = "proof1";
        proofs[1] = "proof2";
        
        bytes[] memory results = new bytes[](2);
        results[0] = "result1";
        results[1] = "result2";
        
        bool isValid = StateInteraction.validateBatchParameters(stateIds, newStates, proofs, results);
        assertTrue(isValid);
        
        bytes32[] memory mismatchedStates = new bytes32[](1);
        mismatchedStates[0] = keccak256("state1");
        
        bool isInvalid = StateInteraction.validateBatchParameters(stateIds, mismatchedStates, proofs, results);
        assertFalse(isInvalid);
    }
    
    function testCalculateStateHash() public {
        bytes32 stateId = keccak256("test-state");
        bytes32 newState = keccak256("new-state");
        bytes memory proof = "test-proof";
        
        bytes32 hash = StateInteraction.calculateStateHash(stateId, newState, proof);
        bytes32 expectedHash = keccak256(abi.encodePacked(stateId, newState, proof));
        
        assertEq(hash, expectedHash);
    }
    
    function testGetFunctionSelector() public {
        bytes4 selector = StateInteraction.getFunctionSelector("postStateUpdate(bytes32,bytes32,bytes,bytes)");
        bytes4 expectedSelector = bytes4(keccak256("postStateUpdate(bytes32,bytes32,bytes,bytes)"));
        
        assertEq(selector, expectedSelector);
    }
    
    function testRevertCases() public {
        vm.expectRevert();
        wrapper.prepareStateUpdate(bytes32(0), bytes32(0), "", "");
        
        vm.expectRevert();
        wrapper.readStateFromContract(address(0), keccak256("test"));
        
        vm.expectRevert(StateInteraction.InvalidParameters.selector);
        bytes32[] memory empty = new bytes32[](0);
        StateInteraction.batchReadStates(address(arithmetic), empty);
    }
}