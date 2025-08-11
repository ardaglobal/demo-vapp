// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Arithmetic} from "../src/Arithmetic.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

contract ArithmeticEventSystemTest is Test {
    Arithmetic public arithmetic;
    address public verifier;
    address public user1 = address(0x1);
    address public user2 = address(0x2);
    
    bytes32 public constant TEST_VKEY = 0x1234567890123456789012345678901234567890123456789012345678901234;
    
    function setUp() public {
        verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, TEST_VKEY);
        
        // Grant authorization to test users
        arithmetic.setAuthorization(user1, true);
        arithmetic.setAuthorization(user2, true);
    }
    
    function testStateUpdatedEvent() public {
        bytes32 stateId = keccak256("test-state-1");
        bytes32 newState = keccak256("new-state-1");
        bytes memory proof = "test-proof-1";
        bytes memory result = "test-result-1";
        bytes32 proofId = keccak256(proof);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Expect the enhanced StateUpdated event
        vm.expectEmit(true, true, true, true);
        emit StateUpdated(stateId, newState, proofId, user1, block.timestamp);
        
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, newState, proof, result);
    }
    
    function testProofStoredEvent() public {
        bytes32 stateId = keccak256("test-state-2");
        bytes32 newState = keccak256("new-state-2");
        bytes memory proof = "test-proof-2";
        bytes memory result = "test-result-2";
        bytes32 proofId = keccak256(proof);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Expect the ProofStored event
        vm.expectEmit(true, true, true, true);
        emit ProofStored(proofId, stateId, user1, block.timestamp);
        
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, newState, proof, result);
    }
    
    function testProofVerifiedEvent() public {
        bytes32 stateId = keccak256("test-state-3");
        bytes32 newState = keccak256("new-state-3");
        bytes memory proof = "test-proof-3";
        bytes memory result = "test-result-3";
        bytes32 proofId = keccak256(proof);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Expect the enhanced ProofVerified event
        vm.expectEmit(true, true, false, true);
        emit ProofVerified(proofId, true, new bytes(0), block.timestamp);
        
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, newState, proof, result);
    }
    
    function testBatchStateUpdatedEvent() public {
        bytes32[] memory stateIds = new bytes32[](2);
        bytes32[] memory newStates = new bytes32[](2);
        bytes[] memory proofs = new bytes[](2);
        bytes[] memory results = new bytes[](2);
        
        stateIds[0] = keccak256("batch-state-1");
        stateIds[1] = keccak256("batch-state-2");
        newStates[0] = keccak256("batch-new-1");
        newStates[1] = keccak256("batch-new-2");
        proofs[0] = "batch-proof-1";
        proofs[1] = "batch-proof-2";
        results[0] = "batch-result-1";
        results[1] = "batch-result-2";
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Expect the BatchStateUpdated event
        vm.expectEmit(false, true, true, true);
        emit BatchStateUpdated(stateIds, newStates, user1, block.timestamp);
        
        vm.prank(user1);
        arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
    }
    
    function testStateReadRequestedEvent() public {
        bytes32 stateId = keccak256("read-test-state");
        
        // First post a state to read
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, keccak256("new-state"), "proof", "result");
        
        // Expect the StateReadRequested event
        vm.expectEmit(true, true, true, true);
        emit StateReadRequested(stateId, user2, block.timestamp);
        
        vm.prank(user2);
        arithmetic.readCurrentState(stateId);
    }
    
    function testProofReadRequestedEvent() public {
        bytes32 stateId = keccak256("proof-read-state");
        bytes memory proof = "proof-read-test";
        bytes32 proofId = keccak256(proof);
        
        // First store a proof
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, keccak256("new-state"), proof, "result");
        
        // Expect the ProofReadRequested event
        vm.expectEmit(true, true, true, true);
        emit ProofReadRequested(proofId, user2, block.timestamp);
        
        vm.prank(user2);
        arithmetic.getProofById(proofId);
    }
    
    function testEventStatisticsTracking() public {
        // Initial stats should be zero
        Arithmetic.EventStats memory initialStats = arithmetic.getEventStats();
        assertEq(initialStats.totalStateUpdates, 0);
        assertEq(initialStats.totalProofStored, 0);
        
        // Post a state update
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        bytes32 stateId = keccak256("stats-test-state");
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, keccak256("new-state"), "proof", "result");
        
        // Check updated stats
        Arithmetic.EventStats memory updatedStats = arithmetic.getEventStats();
        assertEq(updatedStats.totalStateUpdates, 1);
        assertEq(updatedStats.totalProofStored, 1);
        assertEq(updatedStats.lastEventTimestamp, block.timestamp);
    }
    
    function testEventCountBySubmitter() public {
        // Initial count should be zero
        assertEq(arithmetic.getEventCountBySubmitter(user1), 0);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Post multiple updates from user1
        for (uint i = 0; i < 3; i++) {
            bytes32 stateId = keccak256(abi.encodePacked("submitter-test-", i));
            vm.prank(user1);
            arithmetic.postStateUpdate(
                stateId, 
                keccak256(abi.encodePacked("new-state-", i)), 
                abi.encodePacked("proof-", i), 
                abi.encodePacked("result-", i)
            );
        }
        
        // Should have count for each state update + proof stored
        assertTrue(arithmetic.getEventCountBySubmitter(user1) >= 3);
    }
    
    function testEventCountByStateId() public {
        bytes32 stateId = keccak256("state-count-test");
        
        // Initial count should be zero
        assertEq(arithmetic.getEventCountByStateId(stateId), 0);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Post state update
        vm.prank(user1);
        arithmetic.postStateUpdate(stateId, keccak256("new-state"), "proof", "result");
        
        // Should have incremented count
        assertTrue(arithmetic.getEventCountByStateId(stateId) >= 1);
    }
    
    function testDailyEventCounts() public {
        uint256 today = block.timestamp / 86400;
        
        // Initial daily count should be zero
        assertEq(arithmetic.getDailyEventCount(today), 0);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Post state update
        vm.prank(user1);
        arithmetic.postStateUpdate(
            keccak256("daily-test"), 
            keccak256("new-state"), 
            "proof", 
            "result"
        );
        
        // Should have incremented daily count
        assertTrue(arithmetic.getDailyEventCount(today) >= 1);
    }
    
    function testTimeRangeInfo() public {
        uint256 startTime = block.timestamp - 86400; // 1 day ago
        uint256 endTime = block.timestamp;
        
        (bool isValid, uint256 dayCount) = arithmetic.getTimeRangeInfo(startTime, endTime);
        assertTrue(isValid);
        assertEq(dayCount, 2); // Should span 2 days
        
        // Test invalid range
        (bool invalidRange,) = arithmetic.getTimeRangeInfo(endTime, startTime);
        assertFalse(invalidRange);
    }
    
    function testEventCountInRange() public {
        uint256 startTime = block.timestamp;
        uint256 endTime = block.timestamp + 1;
        
        // Should start with zero events in range
        assertEq(arithmetic.getEventCountInRange(startTime, endTime), 0);
        
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Post an event
        vm.prank(user1);
        arithmetic.postStateUpdate(
            keccak256("range-test"), 
            keccak256("new-state"), 
            "proof", 
            "result"
        );
        
        // Should now have events in the range
        assertTrue(arithmetic.getEventCountInRange(startTime, endTime + 86400) >= 1);
    }
    
    function testReadEventTrackingToggle() public {
        // Only owner can toggle
        vm.expectRevert(Arithmetic.UnauthorizedAccess.selector);
        vm.prank(user1);
        arithmetic.setReadEventTracking(false);
        
        // Owner should be able to toggle
        arithmetic.setReadEventTracking(false);
        assertTrue(!arithmetic.readEventTrackingEnabled());
        
        arithmetic.setReadEventTracking(true);
        assertTrue(arithmetic.readEventTrackingEnabled());
    }
    
    // Events to match the contract
    event StateUpdated(
        bytes32 indexed stateId,
        bytes32 indexed newState,
        bytes32 indexed proofId,
        address updater,
        uint256 timestamp
    );
    
    event BatchStateUpdated(
        bytes32[] stateIds,
        bytes32[] newStates,
        address indexed updater,
        uint256 indexed timestamp
    );
    
    event StateReadRequested(
        bytes32 indexed stateId,
        address indexed reader,
        uint256 indexed timestamp
    );
    
    event ProofStored(
        bytes32 indexed proofId,
        bytes32 indexed stateId,
        address indexed submitter,
        uint256 timestamp
    );
    
    event ProofVerified(
        bytes32 indexed proofId,
        bool indexed success,
        bytes result,
        uint256 timestamp
    );
    
    event ProofReadRequested(
        bytes32 indexed proofId,
        address indexed reader,
        uint256 indexed timestamp
    );
}