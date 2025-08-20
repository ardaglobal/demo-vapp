// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {stdJson} from "forge-std/StdJson.sol";
import {Arithmetic} from "../src/Arithmetic.sol";
import {SP1VerifierGateway} from "@sp1-contracts/SP1VerifierGateway.sol";

struct SP1ProofFixtureJson {
    int32 initial_balance;
    int32 final_balance;
    bytes proof;
    bytes publicValues;
    bytes32 vkey;
}

contract ArithmeticGroth16Test is Test {
    using stdJson for string;

    address verifier;
    Arithmetic public arithmetic;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/src/fixtures/groth16-fixture.json"
        );
        string memory json = vm.readFile(path);
        bytes memory jsonBytes = json.parseRaw(".");
        return abi.decode(jsonBytes, (SP1ProofFixtureJson));
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, fixture.vkey);
    }

    function test_ValidArithmeticProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        int32 result = arithmetic.verifyArithmeticProof(
            fixture.publicValues,
            fixture.proof
        );
        
        // Function returns final_balance which should be 12 based on our public values
        assert(result == 12);
    }

    function testRevert_InvalidArithmeticProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory fakeProof = new bytes(fixture.proof.length);

        arithmetic.verifyArithmeticProof(fixture.publicValues, fakeProof);
    }
}

contract ArithmeticPlonkTest is Test {
    using stdJson for string;

    address verifier;
    Arithmetic public arithmetic;

    function loadFixture() public view returns (SP1ProofFixtureJson memory) {
        string memory root = vm.projectRoot();
        string memory path = string.concat(
            root,
            "/src/fixtures/plonk-fixture.json"
        );
        string memory json = vm.readFile(path);
        bytes memory jsonBytes = json.parseRaw(".");
        return abi.decode(jsonBytes, (SP1ProofFixtureJson));
    }

    function setUp() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, fixture.vkey);
    }

    function test_ValidArithmeticProof() public {
        SP1ProofFixtureJson memory fixture = loadFixture();

        vm.mockCall(
            verifier,
            abi.encodeWithSelector(SP1VerifierGateway.verifyProof.selector),
            abi.encode(true)
        );

        int32 result = arithmetic.verifyArithmeticProof(
            fixture.publicValues,
            fixture.proof
        );
        
        // Function returns final_balance which should be 12 based on our public values  
        assert(result == 12);
    }

    function testRevert_InvalidArithmeticProof() public {
        vm.expectRevert();

        SP1ProofFixtureJson memory fixture = loadFixture();

        // Create a fake proof.
        bytes memory fakeProof = new bytes(fixture.proof.length);

        arithmetic.verifyArithmeticProof(fixture.publicValues, fakeProof);
    }
}

contract ArithmeticVerificationKeyTest is Test {
    Arithmetic public arithmetic;
    bytes32 constant TEST_VKEY = 0x035a6b230490471fe1a84470ae9bf66a6521fd76d559d50488f30d5b1ccbfc2b;
    bytes32 constant WRONG_VKEY = 0x1234567890123456789012345678901234567890123456789012345678901234;

    function setUp() public {
        address verifier = address(new SP1VerifierGateway(address(1)));
        arithmetic = new Arithmetic(verifier, TEST_VKEY);
    }

    function test_GetProgramVerificationKey() public view {
        bytes32 vkey = arithmetic.getProgramVerificationKey();
        assertEq(vkey, TEST_VKEY);
    }

    function test_IsValidProgramVerificationKey_ValidKey() public view {
        bool isValid = arithmetic.isValidProgramVerificationKey(TEST_VKEY);
        assertTrue(isValid);
    }

    function test_IsValidProgramVerificationKey_InvalidKey() public view {
        bool isValid = arithmetic.isValidProgramVerificationKey(WRONG_VKEY);
        assertFalse(isValid);
    }

    function test_GetVerificationInfo() public view {
        (bytes32 vkey, address verifierAddr) = arithmetic.getVerificationInfo();
        assertEq(vkey, TEST_VKEY);
        assertTrue(verifierAddr != address(0));
    }

    function test_ValidateProofCompatibility_ValidVKey() public view {
        bytes memory mockProof = "mock_proof_data";
        bytes memory mockPublicValues = "mock_public_values";
        
        (bool isValid, string memory message) = arithmetic.validateProofCompatibility(
            TEST_VKEY,
            mockPublicValues,
            mockProof
        );
        
        assertTrue(isValid);
        assertEq(message, "Proof format is compatible with this contract");
    }

    function test_ValidateProofCompatibility_InvalidVKey() public view {
        bytes memory mockProof = "mock_proof_data";
        bytes memory mockPublicValues = "mock_public_values";
        
        (bool isValid, ) = arithmetic.validateProofCompatibility(
            WRONG_VKEY,
            mockPublicValues,
            mockProof
        );
        
        assertFalse(isValid);
    }
}
