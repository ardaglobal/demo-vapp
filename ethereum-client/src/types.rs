use alloy_primitives::{Address, Bytes, FixedBytes, U256};
use serde::{Deserialize, Serialize};

pub type StateRoot = FixedBytes<32>;
pub type ProofId = FixedBytes<32>;
pub type StateId = FixedBytes<32>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    pub state_id: StateId,
    pub new_state_root: StateRoot,
    pub proof: Bytes,
    pub public_values: Bytes,
    pub block_number: Option<u64>,
    pub transaction_hash: Option<FixedBytes<32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofSubmission {
    pub proof_id: ProofId,
    pub state_id: StateId,
    pub proof: Bytes,
    pub result: Bytes,
    pub submitter: Address,
    pub block_number: u64,
    pub transaction_hash: FixedBytes<32>,
    pub gas_used: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateQuery {
    pub state_id: StateId,
    pub block_number: Option<u64>,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponse {
    pub state_id: StateId,
    pub state_root: StateRoot,
    pub block_number: u64,
    pub timestamp: u64,
    pub proof_id: Option<ProofId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalState {
    pub state_id: StateId,
    pub state_roots: Vec<StateRoot>,
    pub block_numbers: Vec<u64>,
    pub timestamps: Vec<u64>,
    pub proof_ids: Vec<ProofId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateHistory {
    pub state_id: StateId,
    pub state_roots: Vec<StateRoot>,
    pub block_numbers: Vec<u64>,
    pub timestamps: Vec<u64>,
    pub proof_ids: Vec<Option<ProofId>>,
    pub limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofVerificationResult {
    pub proof_id: ProofId,
    pub verified: bool,
    pub result: Option<Bytes>,
    pub block_number: u64,
    pub gas_used: U256,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InclusionProof {
    pub leaf_hash: FixedBytes<32>,
    pub leaf_index: u64,
    pub siblings: Vec<FixedBytes<32>>,
    pub root: StateRoot,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStateUpdate {
    pub state_ids: Vec<StateId>,
    pub new_state_roots: Vec<StateRoot>,
    pub proofs: Vec<Bytes>,
    pub results: Vec<Bytes>,
    pub transaction_hash: FixedBytes<32>,
    pub block_number: u64,
    pub gas_used: U256,
    pub success_flags: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEvent {
    pub event_type: String,
    pub state_id: Option<StateId>,
    pub proof_id: Option<ProofId>,
    pub block_number: u64,
    pub transaction_hash: FixedBytes<32>,
    pub log_index: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub chain_id: u64,
    pub block_number: u64,
    pub gas_price: U256,
    pub base_fee: Option<U256>,
    pub network_name: String,
    pub sync_status: bool,
}

// ==========================================
// INDEPENDENT VERIFICATION TYPES
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationData {
    pub proof_id: ProofId,
    pub state_id: StateId,
    pub proof_bytes: Bytes,
    pub public_values: Bytes,
    pub verifier_key: FixedBytes<32>,
    pub state_root: StateRoot,
    pub submitter: Address,
    pub timestamp: u64,
    pub verified_on_chain: bool,
    pub block_number: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndependentVerificationResult {
    pub proof_id: ProofId,
    pub sp1_verification_passed: bool,
    pub on_chain_verification_status: bool,
    pub consistency_checks_passed: bool,
    pub consistency_details: ConsistencyChecks,
    pub verification_data: VerificationData,
    pub verified_at: u64,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsistencyChecks {
    pub proof_id_matches_hash: bool,
    pub state_exists: bool,
    pub proof_data_present: bool,
    pub timestamp_reasonable: bool,
    pub verifier_key_valid: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractQueryResult {
    pub verifier_key: FixedBytes<32>,
    pub proof_result: Option<Bytes>,
    pub proof_data: Option<Bytes>,
    pub state_root: Option<StateRoot>,
    pub block_number: u64,
    pub query_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustlessVerificationSummary {
    pub proof_id: ProofId,
    pub verification_status: VerificationStatus,
    pub verifier_key: FixedBytes<32>,
    pub state_root: StateRoot,
    pub independent_verification_passed: bool,
    pub verification_details: String,
    pub retrieved_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Verified,
    Failed,
    NotFound,
    Pending,
}
