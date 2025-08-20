use alloy_primitives::Address;
use alloy_sol_types::sol;

sol! {
    #[sol(rpc)]
    interface IArithmetic {
        struct ProofMetadata {
            bytes32 proofId;
            bytes32 stateId;
            address submitter;
            uint256 timestamp;
            bool verified;
            bool exists;
        }

        function updateState(
            bytes32 stateId,
            bytes32 newStateRoot,
            bytes calldata proof,
            bytes calldata publicValues
        ) external;

        function postStateUpdate(
            bytes32 stateId,
            bytes32 newState,
            bytes calldata proof,
            bytes calldata result
        ) external returns (bool success);

        function batchUpdateStates(
            bytes32[] calldata stateIds,
            bytes32[] calldata newStates,
            bytes[] calldata proofs,
            bytes[] calldata results
        ) external returns (bool[] memory successes);

        function getCurrentState(bytes32 stateId) external view returns (bytes32);

        function readCurrentState(bytes32 stateId) external returns (bytes32 state, bool exists);

        function readStateHistory(bytes32 stateId, uint256 limit) external view returns (bytes32[] memory states);

        function getStoredProof(bytes32 proofId) external view returns (bytes memory);

        function getStoredResult(bytes32 proofId) external view returns (bytes memory);

        function readProofDetails(bytes32 proofId) external view returns (
            bytes memory proof,
            bytes memory result,
            bool verified
        );

        function batchReadStates(bytes32[] calldata stateIds) external view returns (bytes32[] memory states);

        function getProofById(bytes32 proofId) external returns (bytes memory proof, bool exists);

        function getProofByStateId(bytes32 stateId) external returns (bytes memory proof, bytes32 proofId);

        function getLatestProof(bytes32 stateId) external view returns (
            bytes memory proof,
            bytes32 proofId,
            uint256 timestamp
        );

        function isProofVerified(bytes32 proofId) external view returns (bool);

        function getVerificationResult(bytes32 proofId) external view returns (bool verified, bytes memory result);

        function getProofTimestamp(bytes32 proofId) external view returns (uint256);

        function getProofCount() external view returns (uint256);

        function getProofByIndex(uint256 index) external view returns (bytes32 proofId, bytes memory proof);

        function getRecentProofs(uint256 limit) external view returns (
            bytes32[] memory proofIds,
            bytes[] memory proofs
        );

        function getProofsByStateId(bytes32 stateId) external view returns (bytes32[] memory proofIds);

        function getProofMetadata(bytes32 proofId) external view returns (ProofMetadata memory metadata);

        function getProofSubmitter(bytes32 proofId) external view returns (address submitter);

        function verifyArithmeticProof(
            bytes calldata publicValues,
            bytes calldata proofBytes
        ) external view returns (int32);

        function isAuthorized(address account) external view returns (bool);

        function proofExists(bytes calldata proof) external view returns (bool);

        function setAuthorization(address account, bool authorized) external;

        function transferOwnership(address newOwner) external;

        function owner() external view returns (address);

        function verifier() external view returns (address);

        function arithmeticProgramVKey() external view returns (bytes32);

        #[derive(Debug, PartialEq, Eq)]
        event StateUpdated(
            bytes32 indexed stateId,
            bytes32 indexed newState,
            bytes32 indexed proofId,
            address updater,
            uint256 timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event BatchStateUpdated(
            bytes32[] stateIds,
            bytes32[] newStates,
            address indexed updater,
            uint256 indexed timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event StateReadRequested(
            bytes32 indexed stateId,
            address indexed reader,
            uint256 indexed timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event ProofStored(
            bytes32 indexed proofId,
            bytes32 indexed stateId,
            address indexed submitter,
            uint256 timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event ProofVerified(
            bytes32 indexed proofId,
            bool indexed success,
            bytes result,
            uint256 timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event ProofReadRequested(
            bytes32 indexed proofId,
            address indexed reader,
            uint256 indexed timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event AuthorizationChanged(address indexed account, bool authorized);

        #[derive(Debug, PartialEq, Eq)]
        event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

        #[derive(Debug, PartialEq, Eq)]
        event BulkOperationExecuted(
            string indexed operationType,
            uint256 indexed itemCount,
            address indexed executor,
            uint256 timestamp
        );

        #[derive(Debug, PartialEq, Eq)]
        event ContractStateQueried(
            address indexed querier,
            string queryType,
            uint256 timestamp
        );
    }
}

sol! {
    #[sol(rpc)]
    interface ISP1Verifier {
        function verifyProof(
            bytes32 programVKey,
            bytes calldata publicValues,
            bytes calldata proofBytes
        ) external view;

        function VERSION() external view returns (string memory);
    }
}

pub use IArithmetic::{IArithmeticCalls, IArithmeticInstance};
pub use ISP1Verifier::{ISP1VerifierCalls, ISP1VerifierInstance};

#[derive(Debug, Clone)]
pub struct ContractAddresses {
    pub arithmetic: Address,
    pub verifier: Address,
}

impl ContractAddresses {
    #[must_use]
    pub const fn new(arithmetic: Address, verifier: Address) -> Self {
        Self {
            arithmetic,
            verifier,
        }
    }
}
