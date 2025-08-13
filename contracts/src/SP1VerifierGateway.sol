// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// @title SP1VerifierGateway
/// @author Arda Global
/// @notice Gateway contract for SP1 verification with BYO proving key support
/// @dev This contract acts as a gateway to SP1 verifiers and manages verification keys
///      for circuits using Bring Your Own Proving Key (BYO-PK) model.
contract SP1VerifierGateway {
    
    /*//////////////////////////////////////////////////////////////
                                EVENTS
    //////////////////////////////////////////////////////////////*/
    
    event VerificationKeyRegistered(
        bytes32 indexed vkeyHash,
        address indexed registrar,
        string circuitName
    );
    
    event ProofVerified(
        bytes32 indexed vkeyHash,
        bytes32 indexed proofHash,
        address indexed verifier,
        bool success
    );
    
    event VerifierUpdated(
        address indexed oldVerifier,
        address indexed newVerifier
    );
    
    /*//////////////////////////////////////////////////////////////
                                ERRORS
    //////////////////////////////////////////////////////////////*/
    
    error UnauthorizedAccess();
    error InvalidVerificationKey();
    error VerificationKeyNotRegistered();
    error VerificationFailed();
    error ZeroAddress();
    
    /*//////////////////////////////////////////////////////////////
                            STATE VARIABLES
    //////////////////////////////////////////////////////////////*/
    
    /// @notice The SP1 verifier contract (can be gateway or specific version)
    address public sp1Verifier;
    
    /// @notice Contract owner
    address public owner;
    
    /// @notice Authorized key registrars
    mapping(address => bool) public authorizedRegistrars;
    
    /// @notice Registry of verification keys by hash
    /// @dev Maps vkey hash to verification key data
    mapping(bytes32 => bytes32) public verificationKeys;
    
    /// @notice Circuit names by verification key hash
    mapping(bytes32 => string) public circuitNames;
    
    /// @notice Verification key metadata
    struct VKeyMetadata {
        bytes32 vkeyHash;
        string circuitName;
        address registrar;
        uint256 registeredAt;
        bool active;
    }
    
    /// @notice Metadata for each verification key
    mapping(bytes32 => VKeyMetadata) public vkeyMetadata;
    
    /// @notice Array of all registered verification key hashes
    bytes32[] public allVKeyHashes;
    
    /// @notice Verification statistics
    mapping(bytes32 => uint256) public verificationCounts;
    mapping(bytes32 => uint256) public lastVerificationTime;
    
    /*//////////////////////////////////////////////////////////////
                                MODIFIERS
    //////////////////////////////////////////////////////////////*/
    
    modifier onlyOwner() {
        if (msg.sender != owner) revert UnauthorizedAccess();
        _;
    }
    
    modifier onlyAuthorizedRegistrar() {
        if (!authorizedRegistrars[msg.sender] && msg.sender != owner) {
            revert UnauthorizedAccess();
        }
        _;
    }
    
    /*//////////////////////////////////////////////////////////////
                            CONSTRUCTOR
    //////////////////////////////////////////////////////////////*/
    
    constructor(address _sp1Verifier) {
        if (_sp1Verifier == address(0)) revert ZeroAddress();
        
        sp1Verifier = _sp1Verifier;
        owner = msg.sender;
        authorizedRegistrars[msg.sender] = true;
    }
    
    /*//////////////////////////////////////////////////////////////
                        VERIFICATION KEY MANAGEMENT
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Register a verification key for BYO-PK circuit
    /// @param vkey The verification key (32 bytes)
    /// @param circuitName The name of the circuit
    function registerVerificationKey(
        bytes32 vkey,
        string calldata circuitName
    ) external onlyAuthorizedRegistrar {
        if (vkey == bytes32(0)) revert InvalidVerificationKey();
        
        bytes32 vkeyHash = keccak256(abi.encode(vkey));
        
        // Store the verification key
        verificationKeys[vkeyHash] = vkey;
        circuitNames[vkeyHash] = circuitName;
        
        // Store metadata
        vkeyMetadata[vkeyHash] = VKeyMetadata({
            vkeyHash: vkeyHash,
            circuitName: circuitName,
            registrar: msg.sender,
            registeredAt: block.timestamp,
            active: true
        });
        
        // Add to enumeration
        allVKeyHashes.push(vkeyHash);
        
        emit VerificationKeyRegistered(vkeyHash, msg.sender, circuitName);
    }
    
    /// @notice Deactivate a verification key
    /// @param vkeyHash The hash of the verification key to deactivate
    function deactivateVerificationKey(bytes32 vkeyHash) external onlyOwner {
        if (verificationKeys[vkeyHash] == bytes32(0)) {
            revert VerificationKeyNotRegistered();
        }
        
        vkeyMetadata[vkeyHash].active = false;
    }
    
    /// @notice Get verification key by hash
    /// @param vkeyHash The hash of the verification key
    /// @return The verification key
    function getVerificationKey(bytes32 vkeyHash) external view returns (bytes32) {
        if (verificationKeys[vkeyHash] == bytes32(0)) {
            revert VerificationKeyNotRegistered();
        }
        return verificationKeys[vkeyHash];
    }
    
    /*//////////////////////////////////////////////////////////////
                            PROOF VERIFICATION
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Verify a proof using registered verification key
    /// @param vkeyHash The hash of the verification key to use
    /// @param publicValues The public values for the proof
    /// @param proofBytes The proof bytes
    /// @return success Whether the verification succeeded
    function verifyProof(
        bytes32 vkeyHash,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external returns (bool success) {
        // Check if verification key is registered and active
        bytes32 vkey = verificationKeys[vkeyHash];
        if (vkey == bytes32(0)) revert VerificationKeyNotRegistered();
        if (!vkeyMetadata[vkeyHash].active) revert InvalidVerificationKey();
        
        bytes32 proofHash = keccak256(proofBytes);
        
        try ISP1Verifier(sp1Verifier).verifyProof(vkey, publicValues, proofBytes) {
            success = true;
            
            // Update statistics
            verificationCounts[vkeyHash]++;
            lastVerificationTime[vkeyHash] = block.timestamp;
            
            emit ProofVerified(vkeyHash, proofHash, msg.sender, true);
        } catch {
            success = false;
            emit ProofVerified(vkeyHash, proofHash, msg.sender, false);
            revert VerificationFailed();
        }
    }
    
    /// @notice Verify a proof and return the result without reverting
    /// @param vkeyHash The hash of the verification key to use
    /// @param publicValues The public values for the proof
    /// @param proofBytes The proof bytes
    /// @return success Whether the verification succeeded
    function tryVerifyProof(
        bytes32 vkeyHash,
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external returns (bool success) {
        bytes32 vkey = verificationKeys[vkeyHash];
        if (vkey == bytes32(0)) return false;
        if (!vkeyMetadata[vkeyHash].active) return false;
        
        bytes32 proofHash = keccak256(proofBytes);
        
        try ISP1Verifier(sp1Verifier).verifyProof(vkey, publicValues, proofBytes) {
            success = true;
            verificationCounts[vkeyHash]++;
            lastVerificationTime[vkeyHash] = block.timestamp;
        } catch {
            success = false;
        }
        
        emit ProofVerified(vkeyHash, proofHash, msg.sender, success);
    }
    
    /*//////////////////////////////////////////////////////////////
                            VIEW FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Get the number of registered verification keys
    /// @return The count of registered verification keys
    function getVKeyCount() external view returns (uint256) {
        return allVKeyHashes.length;
    }
    
    /// @notice Get verification key hash by index
    /// @param index The index in the array
    /// @return The verification key hash
    function getVKeyHashByIndex(uint256 index) external view returns (bytes32) {
        require(index < allVKeyHashes.length, "Index out of bounds");
        return allVKeyHashes[index];
    }
    
    /// @notice Get verification statistics for a key
    /// @param vkeyHash The verification key hash
    /// @return count The number of verifications
    /// @return lastTime The timestamp of the last verification
    function getVerificationStats(bytes32 vkeyHash) external view returns (
        uint256 count,
        uint256 lastTime
    ) {
        count = verificationCounts[vkeyHash];
        lastTime = lastVerificationTime[vkeyHash];
    }
    
    /// @notice Check if a verification key is registered and active
    /// @param vkeyHash The verification key hash
    /// @return Whether the key is registered and active
    function isValidVerificationKey(bytes32 vkeyHash) external view returns (bool) {
        return verificationKeys[vkeyHash] != bytes32(0) && vkeyMetadata[vkeyHash].active;
    }
    
    /*//////////////////////////////////////////////////////////////
                            ADMIN FUNCTIONS
    //////////////////////////////////////////////////////////////*/
    
    /// @notice Update the SP1 verifier contract
    /// @param newVerifier The new verifier contract address
    function updateSP1Verifier(address newVerifier) external onlyOwner {
        if (newVerifier == address(0)) revert ZeroAddress();
        
        address oldVerifier = sp1Verifier;
        sp1Verifier = newVerifier;
        
        emit VerifierUpdated(oldVerifier, newVerifier);
    }
    
    /// @notice Set authorization for a registrar
    /// @param registrar The registrar address
    /// @param authorized Whether to authorize or deauthorize
    function setAuthorizedRegistrar(address registrar, bool authorized) external onlyOwner {
        authorizedRegistrars[registrar] = authorized;
    }
    
    /// @notice Transfer ownership
    /// @param newOwner The new owner address
    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert ZeroAddress();
        
        authorizedRegistrars[owner] = false;
        authorizedRegistrars[newOwner] = true;
        owner = newOwner;
    }
}
