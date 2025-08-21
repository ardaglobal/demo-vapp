// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Arithmetic} from "../src/Arithmetic.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

contract ArithmeticProofReadingTest is Test {
    Arithmetic public arithmetic;
    address public verifier;
    address public user1 = address(0x1);
    address public user2 = address(0x2);

    bytes32 public constant TEST_VKEY =
        0x1234567890123456789012345678901234567890123456789012345678901234;

    function setUp() public {
        verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, TEST_VKEY);

        // Grant authorization to test users
        arithmetic.setAuthorization(user1, true);
        arithmetic.setAuthorization(user2, true);
    }

    function testPostStateUpdateStoresMetadata() public {
        bytes32 stateId = keccak256("test-state-1");
        bytes32 newState = keccak256("new-state-1");
        bytes memory proof = "test-proof-1";
        bytes memory result = "test-result-1";

        // Mock the verifier call
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Expect events
        bytes32 expectedProofId = keccak256(proof);
        vm.expectEmit(true, true, true, false);
        emit StateUpdated(
            stateId,
            newState,
            expectedProofId,
            user1,
            block.timestamp
        );

        vm.prank(user1);
        arithmetic.updateState(stateId, newState, proof, result);

        // Verify metadata was stored
        Arithmetic.ProofMetadata memory metadata = arithmetic.getProofMetadata(
            expectedProofId
        );
        assertEq(metadata.proofId, expectedProofId);
        assertEq(metadata.stateId, stateId);
        assertEq(metadata.submitter, user1);
        assertTrue(metadata.verified);
        assertTrue(metadata.exists);
        assertEq(metadata.timestamp, block.timestamp);
    }

    function testGetProofById() public {
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

        vm.prank(user1);
        arithmetic.updateState(stateId, newState, proof, result);

        // Test getProofById
        (bytes memory retrievedProof, bool exists) = arithmetic.getProofById(
            proofId
        );
        assertTrue(exists);
        assertEq(retrievedProof, proof);

        // Test non-existent proof
        (bytes memory noProof, bool notExists) = arithmetic.getProofById(
            keccak256("non-existent")
        );
        assertFalse(notExists);
        assertEq(noProof.length, 0);
    }

    function testGetProofByStateId() public {
        bytes32 stateId = keccak256("test-state-3");
        bytes32 newState = keccak256("new-state-3");
        bytes memory proof = "test-proof-3";
        bytes memory result = "test-result-3";

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        vm.prank(user1);
        arithmetic.updateState(stateId, newState, proof, result);

        // Test getProofByStateId
        (bytes memory retrievedProof, bytes32 proofId) = arithmetic
            .getProofByStateId(stateId);
        assertEq(retrievedProof, proof);
        assertEq(proofId, keccak256(proof));

        // Test non-existent state
        (bytes memory noProof, bytes32 noProofId) = arithmetic
            .getProofByStateId(keccak256("non-existent"));
        assertEq(noProof.length, 0);
        assertEq(noProofId, bytes32(0));
    }

    function testGetLatestProof() public {
        bytes32 stateId = keccak256("test-state-4");

        // Post first proof
        bytes memory proof1 = "test-proof-4a";
        bytes memory result1 = "test-result-4a";
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        vm.prank(user1);
        arithmetic.updateState(
            stateId,
            keccak256("new-state-4a"),
            proof1,
            result1
        );
        uint256 firstTimestamp = block.timestamp;

        // Advance time
        vm.warp(block.timestamp + 100);

        // Post second proof (should be the latest)
        bytes memory proof2 = "test-proof-4b";
        bytes memory result2 = "test-result-4b";

        vm.prank(user2);
        arithmetic.updateState(
            stateId,
            keccak256("new-state-4b"),
            proof2,
            result2
        );
        uint256 secondTimestamp = block.timestamp;

        // Test getLatestProof
        (
            bytes memory latestProof,
            bytes32 proofId,
            uint256 timestamp
        ) = arithmetic.getLatestProof(stateId);
        assertEq(latestProof, proof2);
        assertEq(proofId, keccak256(proof2));
        assertEq(timestamp, secondTimestamp);

        // Verify first proof is still accessible
        (bytes memory firstProof, bool exists) = arithmetic.getProofById(
            keccak256(proof1)
        );
        assertTrue(exists);
        assertEq(firstProof, proof1);
    }

    function testProofVerificationStatus() public {
        bytes32 stateId = keccak256("test-state-5");
        bytes32 newState = keccak256("new-state-5");
        bytes memory proof = "test-proof-5";
        bytes memory result = "test-result-5";
        bytes32 proofId = keccak256(proof);

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        vm.prank(user1);
        arithmetic.updateState(stateId, newState, proof, result);

        // Test verification status functions
        assertTrue(arithmetic.isProofVerified(proofId));

        (bool verified, bytes memory retrievedResult) = arithmetic
            .getVerificationResult(proofId);
        assertTrue(verified);
        assertEq(retrievedResult, result);

        uint256 timestamp = arithmetic.getProofTimestamp(proofId);
        assertEq(timestamp, block.timestamp);

        address submitter = arithmetic.getProofSubmitter(proofId);
        assertEq(submitter, user1);
    }

    function testProofEnumeration() public {
        // Start with empty count
        assertEq(arithmetic.getProofCount(), 0);

        // Post multiple proofs
        bytes32[] memory stateIds = new bytes32[](3);
        bytes32[] memory proofIds = new bytes32[](3);

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        for (uint256 i = 0; i < 3; i++) {
            stateIds[i] = keccak256(abi.encodePacked("test-state-", i));
            bytes memory proof = abi.encodePacked("test-proof-", i);
            bytes memory result = abi.encodePacked("test-result-", i);
            proofIds[i] = keccak256(proof);

            vm.prank(user1);
            arithmetic.updateState(
                stateIds[i],
                keccak256(abi.encodePacked("new-state-", i)),
                proof,
                result
            );
        }

        // Test proof count
        assertEq(arithmetic.getProofCount(), 3);

        // Test getProofByIndex
        for (uint256 i = 0; i < 3; i++) {
            (bytes32 indexProofId, bytes memory indexProof) = arithmetic
                .getProofByIndex(i);
            assertEq(indexProofId, proofIds[i]);
            assertEq(indexProof, abi.encodePacked("test-proof-", i));
        }

        // Test invalid index
        vm.expectRevert(Arithmetic.InvalidIndex.selector);
        arithmetic.getProofByIndex(3);
    }

    function testGetRecentProofs() public {
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Post 5 proofs
        for (uint256 i = 0; i < 5; i++) {
            bytes32 stateId = keccak256(abi.encodePacked("test-state-", i));
            bytes memory proof = abi.encodePacked("test-proof-", i);
            bytes memory result = abi.encodePacked("test-result-", i);

            vm.prank(user1);
            arithmetic.updateState(
                stateId,
                keccak256(abi.encodePacked("new-state-", i)),
                proof,
                result
            );
        }

        // Test getting recent 3 proofs
        (
            bytes32[] memory recentProofIds,
            bytes[] memory recentProofs
        ) = arithmetic.getRecentProofs(3);
        assertEq(recentProofIds.length, 3);
        assertEq(recentProofs.length, 3);

        // Should return most recent proofs (indices 2, 3, 4)
        for (uint256 i = 0; i < 3; i++) {
            assertEq(recentProofs[i], abi.encodePacked("test-proof-", i + 2));
        }

        // Test getting all proofs (limit 0)
        (bytes32[] memory allProofIds, bytes[] memory allProofs) = arithmetic
            .getRecentProofs(0);
        assertEq(allProofIds.length, 5);
        assertEq(allProofs.length, 5);

        // Test limit larger than total
        (
            bytes32[] memory limitedProofIds,
            bytes[] memory limitedProofs
        ) = arithmetic.getRecentProofs(10);
        assertEq(limitedProofIds.length, 5);
        assertEq(limitedProofs.length, 5);
    }

    function testGetProofsByStateId() public {
        bytes32 stateId = keccak256("test-state-multi");

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        // Post multiple proofs for the same state
        for (uint256 i = 0; i < 3; i++) {
            bytes memory proof = abi.encodePacked("test-proof-multi-", i);
            bytes memory result = abi.encodePacked("test-result-multi-", i);

            vm.prank(user1);
            arithmetic.updateState(
                stateId,
                keccak256(abi.encodePacked("new-state-multi-", i)),
                proof,
                result
            );
        }

        // Test getProofsByStateId
        bytes32[] memory stateProofIds = arithmetic.getProofsByStateId(stateId);
        assertEq(stateProofIds.length, 3);

        // Verify each proof ID corresponds to the correct proof
        for (uint256 i = 0; i < 3; i++) {
            bytes memory expectedProof = abi.encodePacked(
                "test-proof-multi-",
                i
            );
            assertEq(stateProofIds[i], keccak256(expectedProof));
        }
    }

    function testEmptyStateReturns() public {
        // Test empty state returns
        (bytes memory noProof, bytes32 noProofId) = arithmetic
            .getProofByStateId(keccak256("non-existent"));
        assertEq(noProof.length, 0);
        assertEq(noProofId, bytes32(0));

        (
            bytes memory noLatestProof,
            bytes32 noLatestProofId,
            uint256 noTimestamp
        ) = arithmetic.getLatestProof(keccak256("non-existent"));
        assertEq(noLatestProof.length, 0);
        assertEq(noLatestProofId, bytes32(0));
        assertEq(noTimestamp, 0);

        (
            bytes32[] memory emptyRecentIds,
            bytes[] memory emptyRecentProofs
        ) = arithmetic.getRecentProofs(5);
        assertEq(emptyRecentIds.length, 0);
        assertEq(emptyRecentProofs.length, 0);
    }

    function testProofNotFoundErrors() public {
        bytes32 nonExistentProofId = keccak256("non-existent-proof");

        vm.expectRevert(Arithmetic.ProofNotFound.selector);
        arithmetic.getProofMetadata(nonExistentProofId);

        vm.expectRevert(Arithmetic.ProofNotFound.selector);
        arithmetic.getProofSubmitter(nonExistentProofId);
    }

    // Events to match the contract
    event StateUpdated(
        bytes32 indexed stateId,
        bytes32 indexed newState,
        bytes32 indexed proofId,
        address updater,
        uint256 timestamp
    );
}
