// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {Arithmetic} from "../src/Arithmetic.sol";
import {IStateManager} from "../src/interfaces/IStateManager.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";
import {stdJson} from "forge-std/StdJson.sol";

contract StateManagementTest is Test {
    using stdJson for string;
    
    Arithmetic public arithmetic;
    address public verifier;
    
    // Test constants
    bytes32 public constant TEST_VKEY = 0x00b51cef3572d1a49ae7f4a332221cab31cdb72b131dbf28fb6ab26e15458fe2;
    
    // Test data from fixtures
    struct FixtureData {
        uint256 a;
        uint256 b;
        uint256 n;
        bytes32 vkey;
        bytes publicValues;
        bytes proof;
    }
    
    FixtureData public groth16Fixture;
    FixtureData public plonkFixture;
    
    // Test actors
    address public owner;
    address public authorizedPoster;
    address public unauthorizedUser;
    address public reader;
    
    // Test state identifiers
    bytes32 public constant STATE_ID_1 = keccak256("test-state-1");
    bytes32 public constant STATE_ID_2 = keccak256("test-state-2");
    bytes32 public constant STATE_ID_3 = keccak256("test-state-3");
    
    // Test state values
    bytes32 public constant NEW_STATE_1 = keccak256("new-state-1");
    bytes32 public constant NEW_STATE_2 = keccak256("new-state-2");
    bytes32 public constant NEW_STATE_3 = keccak256("new-state-3");
    
    // Events for testing
    event StateUpdated(bytes32 indexed stateId, bytes32 indexed newState, bytes32 indexed proofId, address updater, uint256 timestamp);
    event BatchStateUpdated(bytes32[] stateIds, bytes32[] newStates, address indexed updater, uint256 indexed timestamp);
    event ProofStored(bytes32 indexed proofId, bytes32 indexed stateId, address indexed submitter, uint256 timestamp);
    
    function setUp() public {
        // Load fixture data
        _loadFixtures();
        
        // Setup test actors
        owner = address(this);
        authorizedPoster = makeAddr("authorizedPoster");
        unauthorizedUser = makeAddr("unauthorizedUser");
        reader = makeAddr("reader");
        
        // Deploy verifier and arithmetic contract
        verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, groth16Fixture.vkey);
        
        // Grant authorization to test poster
        arithmetic.setAuthorization(authorizedPoster, true);
        
        // Mock the verifier to always return true for successful verification
        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );
        
        // Fund test accounts
        vm.deal(owner, 100 ether);
        vm.deal(authorizedPoster, 100 ether);
        vm.deal(unauthorizedUser, 100 ether);
        vm.deal(reader, 100 ether);
    }
    
    function _loadFixtures() internal {
        // Load Groth16 fixture
        string memory groth16Json = vm.readFile("src/fixtures/groth16-fixture.json");
        groth16Fixture = FixtureData({
            a: groth16Json.readUint(".a"),
            b: groth16Json.readUint(".b"),
            n: groth16Json.readUint(".n"),
            vkey: bytes32(groth16Json.readBytes(".vkey")),
            publicValues: groth16Json.readBytes(".publicValues"),
            proof: groth16Json.readBytes(".proof")
        });
        
        // Load PLONK fixture
        string memory plonkJson = vm.readFile("src/fixtures/plonk-fixture.json");
        plonkFixture = FixtureData({
            a: plonkJson.readUint(".a"),
            b: plonkJson.readUint(".b"),
            n: plonkJson.readUint(".n"),
            vkey: bytes32(plonkJson.readBytes(".vkey")),
            publicValues: plonkJson.readBytes(".publicValues"),
            proof: plonkJson.readBytes(".proof")
        });
    }

    /*//////////////////////////////////////////////////////////////
                    CORE STATE FUNCTION TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_UpdateState_ValidProof_Success() public {
        vm.prank(authorizedPoster);
        
        vm.expectEmit(true, true, true, false);
        emit StateUpdated(STATE_ID_1, NEW_STATE_1, keccak256(groth16Fixture.proof), authorizedPoster, block.timestamp);
        
        bool success = arithmetic.postStateUpdate(
            STATE_ID_1,
            NEW_STATE_1,
            groth16Fixture.proof,
            groth16Fixture.publicValues
        );
        
        assertTrue(success, "State update should succeed with valid proof");
        
        // Verify state was stored
        bytes32 storedState = arithmetic.getCurrentState(STATE_ID_1);
        assertEq(storedState, NEW_STATE_1, "State should be updated correctly");
        
        // Verify proof was stored
        bytes32 proofId = keccak256(groth16Fixture.proof);
        bytes memory storedProof = arithmetic.getStoredProof(proofId);
        assertEq(storedProof, groth16Fixture.proof, "Proof should be stored correctly");
        
        // Verify result was stored
        bytes memory storedResult = arithmetic.getStoredResult(proofId);
        assertEq(storedResult, groth16Fixture.publicValues, "Result should be stored correctly");
        
        // Verify proof is marked as verified
        assertTrue(arithmetic.isProofVerified(proofId), "Proof should be marked as verified");
    }
    
    function test_GetCurrentState_ExistingState_ReturnsCorrectState() public {
        // First update a state
        vm.prank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        // Test reading the state
        bytes32 currentState = arithmetic.getCurrentState(STATE_ID_1);
        assertEq(currentState, NEW_STATE_1, "Should return correct current state");
    }
    
    function test_GetCurrentState_NonExistentState_ReturnsZero() public {
        bytes32 currentState = arithmetic.getCurrentState(STATE_ID_1);
        assertEq(currentState, bytes32(0), "Should return zero for non-existent state");
    }
    
    function test_GetStoredProof_ExistingProof_ReturnsCorrectProof() public {
        // Store a proof first
        vm.prank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        // Test reading the proof
        bytes32 proofId = keccak256(groth16Fixture.proof);
        bytes memory storedProof = arithmetic.getStoredProof(proofId);
        assertEq(storedProof, groth16Fixture.proof, "Should return correct stored proof");
    }
    
    function test_GetStoredResult_ExistingResult_ReturnsCorrectResult() public {
        // Store a result first
        vm.prank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        // Test reading the result
        bytes32 proofId = keccak256(groth16Fixture.proof);
        bytes memory storedResult = arithmetic.getStoredResult(proofId);
        assertEq(storedResult, groth16Fixture.publicValues, "Should return correct stored result");
    }

    /*//////////////////////////////////////////////////////////////
                    BATCH OPERATIONS TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_BatchUpdateStates_ValidProofs_AllSucceed() public {
        // Prepare batch data
        bytes32[] memory stateIds = new bytes32[](2);
        stateIds[0] = STATE_ID_1;
        stateIds[1] = STATE_ID_2;
        
        bytes32[] memory newStates = new bytes32[](2);
        newStates[0] = NEW_STATE_1;
        newStates[1] = NEW_STATE_2;
        
        bytes[] memory proofs = new bytes[](2);
        proofs[0] = groth16Fixture.proof;
        proofs[1] = plonkFixture.proof;
        
        bytes[] memory results = new bytes[](2);
        results[0] = groth16Fixture.publicValues;
        results[1] = plonkFixture.publicValues;
        
        vm.prank(authorizedPoster);
        
        vm.expectEmit(true, true, false, false);
        emit BatchStateUpdated(stateIds, newStates, authorizedPoster, block.timestamp);
        
        bool[] memory successes = arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
        
        // Verify all updates succeeded
        assertTrue(successes[0], "First update should succeed");
        assertTrue(successes[1], "Second update should succeed");
        
        // Verify states were stored
        assertEq(arithmetic.getCurrentState(STATE_ID_1), NEW_STATE_1, "First state should be updated");
        assertEq(arithmetic.getCurrentState(STATE_ID_2), NEW_STATE_2, "Second state should be updated");
    }
    
    function test_BatchReadStates_ExistingStates_ReturnsCorrectStates() public {
        // Setup test states
        vm.startPrank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        arithmetic.postStateUpdate(STATE_ID_2, NEW_STATE_2, plonkFixture.proof, plonkFixture.publicValues);
        vm.stopPrank();
        
        // Prepare batch read
        bytes32[] memory stateIds = new bytes32[](2);
        stateIds[0] = STATE_ID_1;
        stateIds[1] = STATE_ID_2;
        
        // Test batch read
        bytes32[] memory states = arithmetic.batchReadStates(stateIds);
        
        assertEq(states.length, 2, "Should return two states");
        assertEq(states[0], NEW_STATE_1, "First state should be correct");
        assertEq(states[1], NEW_STATE_2, "Second state should be correct");
    }
    
    function test_BatchUpdateStates_MismatchedArrays_Reverts() public {
        bytes32[] memory stateIds = new bytes32[](2);
        bytes32[] memory newStates = new bytes32[](1); // Mismatched length
        bytes[] memory proofs = new bytes[](2);
        bytes[] memory results = new bytes[](2);
        
        vm.prank(authorizedPoster);
        vm.expectRevert(Arithmetic.InvalidArrayLength.selector);
        arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
    }

    /*//////////////////////////////////////////////////////////////
                    PROOF MANAGEMENT TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_GetProofById_ExistingProof_ReturnsProofAndTrue() public {
        // Store a proof first
        vm.prank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        bytes32 proofId = keccak256(groth16Fixture.proof);
        (bytes memory proof, bool exists) = arithmetic.getProofById(proofId);
        
        assertTrue(exists, "Proof should exist");
        assertEq(proof, groth16Fixture.proof, "Should return correct proof");
    }
    
    function test_GetProofById_NonExistentProof_ReturnsEmptyAndFalse() public {
        bytes32 fakeProofId = keccak256("fake-proof");
        (bytes memory proof, bool exists) = arithmetic.getProofById(fakeProofId);
        
        assertFalse(exists, "Proof should not exist");
        assertEq(proof.length, 0, "Should return empty proof");
    }
    
    function test_IsProofVerified_VerifiedProof_ReturnsTrue() public {
        // Store a verified proof
        vm.prank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        bytes32 proofId = keccak256(groth16Fixture.proof);
        assertTrue(arithmetic.isProofVerified(proofId), "Proof should be verified");
    }
    
    function test_IsProofVerified_NonExistentProof_ReturnsFalse() public {
        bytes32 fakeProofId = keccak256("fake-proof");
        assertFalse(arithmetic.isProofVerified(fakeProofId), "Non-existent proof should not be verified");
    }
    
    function test_GetVerificationResult_VerifiedProof_ReturnsCorrectData() public {
        // Store a verified proof
        vm.prank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        bytes32 proofId = keccak256(groth16Fixture.proof);
        (bool verified, bytes memory result) = arithmetic.getVerificationResult(proofId);
        
        assertTrue(verified, "Proof should be verified");
        assertEq(result, groth16Fixture.publicValues, "Should return correct result");
    }

    /*//////////////////////////////////////////////////////////////
                    ACCESS CONTROL TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_UpdateState_UnauthorizedUser_Reverts() public {
        vm.prank(unauthorizedUser);
        vm.expectRevert(Arithmetic.UnauthorizedAccess.selector);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
    }
    
    function test_UpdateState_Owner_Success() public {
        vm.prank(owner);
        bool success = arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        assertTrue(success, "Owner should be able to update state");
    }
    
    function test_BatchUpdateStates_UnauthorizedUser_Reverts() public {
        bytes32[] memory stateIds = new bytes32[](1);
        bytes32[] memory newStates = new bytes32[](1);
        bytes[] memory proofs = new bytes[](1);
        bytes[] memory results = new bytes[](1);
        
        vm.prank(unauthorizedUser);
        vm.expectRevert(Arithmetic.UnauthorizedAccess.selector);
        arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
    }

    /*//////////////////////////////////////////////////////////////
                    ERROR CONDITION TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_UpdateState_InvalidProof_ReturnsFalse() public {
        bytes memory invalidProof = "invalid-proof-data";
        
        // Mock the verifier to revert for this specific invalid proof
        vm.mockCallRevert(
            verifier,
            abi.encodeWithSelector(
                SP1VerifierGateway.verifyProof.selector,
                groth16Fixture.vkey,
                groth16Fixture.publicValues,
                invalidProof
            ),
            "Proof verification failed"
        );
        
        vm.prank(authorizedPoster);
        bool success = arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, invalidProof, groth16Fixture.publicValues);
        
        assertFalse(success, "Invalid proof should cause update to fail");
        
        // Verify state was not updated
        bytes32 storedState = arithmetic.getCurrentState(STATE_ID_1);
        assertEq(storedState, bytes32(0), "State should not be updated with invalid proof");
    }
    
    function test_ReadProofDetails_NonExistentProof_Reverts() public {
        bytes32 fakeProofId = keccak256("fake-proof");
        vm.expectRevert(Arithmetic.ProofNotFound.selector);
        arithmetic.readProofDetails(fakeProofId);
    }
    
    function test_GetProofMetadata_NonExistentProof_Reverts() public {
        bytes32 fakeProofId = keccak256("fake-proof");
        vm.expectRevert(Arithmetic.ProofNotFound.selector);
        arithmetic.getProofMetadata(fakeProofId);
    }

    /*//////////////////////////////////////////////////////////////
                    GAS OPTIMIZATION TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_Gas_SingleStateUpdate() public {
        vm.prank(authorizedPoster);
        
        uint256 gasBefore = gasleft();
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        uint256 gasUsed = gasBefore - gasleft();
        
        console.log("Gas used for single state update:", gasUsed);
        // Expect reasonable gas usage (should be less than 1M gas for complex state management with ZK proof verification)
        assertLt(gasUsed, 1_000_000, "Single state update should use reasonable gas");
    }
    
    function test_Gas_BatchVsSingleUpdates() public {
        // Test single updates
        vm.startPrank(authorizedPoster);
        
        uint256 singleUpdateGas = 0;
        for (uint256 i = 0; i < 3; i++) {
            bytes32 stateId = bytes32(uint256(STATE_ID_1) + i);
            bytes32 newState = bytes32(uint256(NEW_STATE_1) + i);
            
            // Create unique proof by modifying the last byte
            bytes memory uniqueProof = groth16Fixture.proof;
            uniqueProof[uniqueProof.length - 1] = bytes1(uint8(i));
            
            uint256 gasBefore = gasleft();
            arithmetic.postStateUpdate(stateId, newState, uniqueProof, groth16Fixture.publicValues);
            singleUpdateGas += gasBefore - gasleft();
        }
        
        vm.stopPrank();
        
        // Reset contract state for batch test
        arithmetic = new Arithmetic(verifier, groth16Fixture.vkey);
        arithmetic.setAuthorization(authorizedPoster, true);
        
        // Test batch update
        bytes32[] memory stateIds = new bytes32[](3);
        bytes32[] memory newStates = new bytes32[](3);
        bytes[] memory proofs = new bytes[](3);
        bytes[] memory results = new bytes[](3);
        
        for (uint256 i = 0; i < 3; i++) {
            stateIds[i] = bytes32(uint256(STATE_ID_1) + i);
            newStates[i] = bytes32(uint256(NEW_STATE_1) + i);
            
            // Create unique proof by modifying the last byte
            bytes memory uniqueProof = groth16Fixture.proof;
            uniqueProof[uniqueProof.length - 1] = bytes1(uint8(i + 10)); // Offset by 10 to avoid collision with single updates
            proofs[i] = uniqueProof;
            results[i] = groth16Fixture.publicValues;
        }
        
        vm.prank(authorizedPoster);
        uint256 batchGasBefore = gasleft();
        arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
        uint256 batchUpdateGas = batchGasBefore - gasleft();
        
        console.log("Gas used for 3 single updates:", singleUpdateGas);
        console.log("Gas used for 1 batch update (3 items):", batchUpdateGas);
        
        // Batch should be more efficient than individual calls
        assertLt(batchUpdateGas, singleUpdateGas, "Batch update should be more gas efficient");
    }
    
    function test_Gas_StateReading() public {
        // Setup some states
        vm.startPrank(authorizedPoster);
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        arithmetic.postStateUpdate(STATE_ID_2, NEW_STATE_2, plonkFixture.proof, plonkFixture.publicValues);
        vm.stopPrank();
        
        // Test single state reads
        uint256 gasBefore = gasleft();
        arithmetic.getCurrentState(STATE_ID_1);
        uint256 singleReadGas = gasBefore - gasleft();
        
        // Test batch state reads
        bytes32[] memory stateIds = new bytes32[](2);
        stateIds[0] = STATE_ID_1;
        stateIds[1] = STATE_ID_2;
        
        gasBefore = gasleft();
        arithmetic.batchReadStates(stateIds);
        uint256 batchReadGas = gasBefore - gasleft();
        
        console.log("Gas used for single state read:", singleReadGas);
        console.log("Gas used for batch read (2 states):", batchReadGas);
        
        // Verify reasonable gas usage
        assertLt(singleReadGas, 10_000, "Single read should be efficient");
        assertLt(batchReadGas, 20_000, "Batch read should be efficient");
    }
    
    function test_Gas_ProofStorage() public {
        vm.prank(authorizedPoster);
        
        uint256 gasBefore = gasleft();
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        uint256 gasUsed = gasBefore - gasleft();
        
        console.log("Gas used for proof storage:", gasUsed);
        
        // Now test reading the proof
        bytes32 proofId = keccak256(groth16Fixture.proof);
        gasBefore = gasleft();
        arithmetic.getStoredProof(proofId);
        uint256 readGas = gasBefore - gasleft();
        
        console.log("Gas used for proof reading:", readGas);
        assertLt(readGas, 50_000, "Proof reading should be efficient");
    }

    /*//////////////////////////////////////////////////////////////
                    INTEGRATION TESTS
    //////////////////////////////////////////////////////////////*/
    
    function test_Integration_CompleteWorkflow() public {
        // 1. Authorized poster updates multiple states
        vm.startPrank(authorizedPoster);
        
        bytes32[] memory stateIds = new bytes32[](2);
        stateIds[0] = STATE_ID_1;
        stateIds[1] = STATE_ID_2;
        
        bytes32[] memory newStates = new bytes32[](2);
        newStates[0] = NEW_STATE_1;
        newStates[1] = NEW_STATE_2;
        
        bytes[] memory proofs = new bytes[](2);
        proofs[0] = groth16Fixture.proof;
        proofs[1] = plonkFixture.proof;
        
        bytes[] memory results = new bytes[](2);
        results[0] = groth16Fixture.publicValues;
        results[1] = plonkFixture.publicValues;
        
        bool[] memory successes = arithmetic.batchUpdateStates(stateIds, newStates, proofs, results);
        vm.stopPrank();
        
        // Verify batch success
        assertTrue(successes[0] && successes[1], "Batch update should succeed");
        
        // 2. Reader queries states
        vm.prank(reader);
        bytes32[] memory readStates = arithmetic.batchReadStates(stateIds);
        
        assertEq(readStates[0], NEW_STATE_1, "First state should be readable");
        assertEq(readStates[1], NEW_STATE_2, "Second state should be readable");
        
        // 3. Verify proofs are accessible
        bytes32 proofId1 = keccak256(groth16Fixture.proof);
        bytes32 proofId2 = keccak256(plonkFixture.proof);
        
        assertTrue(arithmetic.isProofVerified(proofId1), "First proof should be verified");
        assertTrue(arithmetic.isProofVerified(proofId2), "Second proof should be verified");
        
        // 4. Check metadata
        Arithmetic.ProofMetadata memory metadata1 = arithmetic.getProofMetadata(proofId1);
        assertEq(metadata1.stateId, STATE_ID_1, "Metadata should link to correct state");
        assertEq(metadata1.submitter, authorizedPoster, "Metadata should record correct submitter");
        assertTrue(metadata1.verified, "Metadata should show verification");
    }
    
    function test_Integration_StateHistory() public {
        // Update same state multiple times
        vm.startPrank(authorizedPoster);
        
        // First update
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_1, groth16Fixture.proof, groth16Fixture.publicValues);
        
        // Second update with different proof
        arithmetic.postStateUpdate(STATE_ID_1, NEW_STATE_2, plonkFixture.proof, plonkFixture.publicValues);
        
        vm.stopPrank();
        
        // Verify current state is latest
        assertEq(arithmetic.getCurrentState(STATE_ID_1), NEW_STATE_2, "Current state should be latest");
        
        // Verify history length
        assertEq(arithmetic.getStateHistoryLength(STATE_ID_1), 2, "Should have 2 states in history");
        
        // Verify history contents
        bytes32[] memory history = arithmetic.readStateHistory(STATE_ID_1, 0);
        assertEq(history.length, 2, "History should contain 2 states");
        assertEq(history[0], NEW_STATE_1, "First history entry should be first state");
        assertEq(history[1], NEW_STATE_2, "Second history entry should be second state");
    }
}