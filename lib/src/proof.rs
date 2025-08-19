//! Unified proof generation and verification module
//!
//! This module provides a shared implementation for Sindri-based proof generation
//! and verification that can be used by both the CLI script and API server.
//! It consolidates the feature-rich logic from the script implementation.

use crate::PublicValuesStruct;
use alloy_sol_types::SolType;
use serde::{Deserialize, Serialize};
use sindri::integrations::sp1_v5::SP1ProofInfo;
use sindri::{client::SindriClient, JobStatus, ProofInfoResponse, ProofInput};
use sp1_sdk::{HashableKey, SP1Stdin};
use std::convert::TryInto;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};

/// Available EVM-compatible proof systems
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum ProofSystem {
    Plonk,
    #[default]
    Groth16,
}

impl ProofSystem {
    /// Convert to the proving scheme string expected by Sindri
    #[must_use]
    pub const fn to_sindri_scheme(&self) -> &'static str {
        match self {
            Self::Plonk => "plonk",
            Self::Groth16 => "groth16",
        }
    }
}

/// Request for proof generation
#[derive(Debug, Clone)]
pub struct ProofGenerationRequest {
    pub a: i32,
    pub b: i32,
    pub result: i32,
    pub proof_system: ProofSystem,
    pub generate_fixtures: bool,
}

/// Request for batch proof generation (new format for batch processing)
#[derive(Debug, Clone)]
pub struct BatchProofGenerationRequest {
    pub initial_balance: i32,
    pub transactions: Vec<i32>,
    pub proof_system: ProofSystem,
    pub generate_fixtures: bool,
}

/// Response from proof generation
#[derive(Debug, Clone)]
pub struct ProofGenerationResponse {
    pub proof_id: String,
    pub status: String,
    pub circuit_name: String,
    pub circuit_tag: String,
    pub verification_command: String,
    pub proof_info: ProofInfoResponse,
}

/// Request for proof verification
#[derive(Debug, Clone)]
pub struct ProofVerificationRequest {
    pub proof_id: String,
    pub expected_initial_balance: i32,
    pub expected_final_balance: i32,
}

/// Response from proof verification
#[derive(Debug, Clone)]
pub struct ProofVerificationResponse {
    pub is_valid: bool,
    pub cryptographic_proof_valid: bool,
    pub balances_match_expected: bool,
    pub actual_initial_balance: Option<i32>,
    pub actual_final_balance: Option<i32>,
    pub expected_initial_balance: i32,
    pub expected_final_balance: i32,
    pub verification_message: String,
    pub verification_time_ms: u64,
}

/// Errors that can occur during proof operations
#[derive(Error, Debug)]
pub enum ProofError {
    #[error("Failed to serialize SP1 stdin: {0}")]
    SerializationError(String),

    #[error("Sindri API error: {0}")]
    SindriError(String),

    #[error("Proof generation failed: {0}")]
    ProofGenerationFailed(String),

    #[error("Proof verification failed: {0}")]
    VerificationFailed(String),

    #[error("Failed to decode public values: {0}")]
    PublicValuesDecodeError(String),

    #[error("EVM fixture generation failed: {0}")]
    FixtureGenerationError(String),

    #[error("Environment configuration error: {0}")]
    ConfigError(String),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// A fixture that can be used to test the verification of SP1 zkVM proofs inside Solidity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SP1ArithmeticProofFixture {
    pub a: i32,
    pub b: i32,
    pub result: i32,
    pub vkey: String,
    pub public_values: String,
    pub proof: String,
}

/// A fixture for batch processing that can be used to test SP1 zkVM proofs inside Solidity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SP1BatchProofFixture {
    pub initial_balance: i32,
    pub transactions: Vec<i32>,
    pub final_balance: i32,
    pub vkey: String,
    pub public_values: String,
    pub proof: String,
}

/// Generate a batch proof via Sindri with full feature support
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Sindri API key is not set in environment
/// - Circuit is not found or not deployed
/// - Proof generation fails on Sindri servers
/// - Network communication errors occur
#[allow(clippy::cognitive_complexity)]
pub async fn generate_batch_proof(
    request: BatchProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    let final_balance = request.initial_balance + request.transactions.iter().sum::<i32>();

    info!(
        "🔐 Generating {} batch proof: {} + {:?} = {} via Sindri",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        request.initial_balance,
        request.transactions,
        final_balance
    );

    // Create SP1 inputs for batch processing and serialize for Sindri
    info!(
        "📝 Creating SP1 stdin with initial_balance={} and transactions={:?}",
        request.initial_balance, request.transactions
    );
    let mut stdin = SP1Stdin::new();
    stdin.write(&request.initial_balance);
    stdin.write(&request.transactions);
    info!("✅ SP1 stdin created successfully");

    info!("📝 Serializing SP1 stdin to JSON for Sindri");
    let stdin_json = serde_json::to_string(&stdin).map_err(|e| {
        error!("❌ Failed to serialize SP1 stdin: {}", e);
        ProofError::SerializationError(e.to_string())
    })?;
    info!(
        "✅ SP1 stdin serialized, JSON length: {} chars",
        stdin_json.len()
    );

    let proof_input = ProofInput::from(stdin_json.clone());
    info!("✅ ProofInput created from JSON");

    // Get circuit name with configurable tag from environment
    let circuit_tag = std::env::var("SINDRI_CIRCUIT_TAG").unwrap_or_else(|_| "latest".to_string());
    let circuit_name = format!("demo-vapp:{circuit_tag}");

    info!("📋 Using circuit: {} (tag: {})", circuit_name, circuit_tag);
    info!("🔑 Checking SINDRI_API_KEY availability...");

    if let Ok(key) = std::env::var("SINDRI_API_KEY") {
        info!("✅ SINDRI_API_KEY is set (length: {} chars)", key.len());
    } else {
        error!("❌ SINDRI_API_KEY is not set in environment!");
        return Err(ProofError::ConfigError(
            "SINDRI_API_KEY not set".to_string(),
        ));
    }

    info!("🌐 Creating Sindri client...");
    let client = SindriClient::default();
    info!("✅ Sindri client created");

    info!(
        "🚀 Submitting proof request to Sindri circuit: {}",
        circuit_name
    );
    info!(
        "📊 Request details: stdin_json preview: {}...",
        &stdin_json[..std::cmp::min(100, stdin_json.len())]
    );

    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, None, None, None)
        .await
        .map_err(|e| {
            error!("❌ Sindri API call failed: {}", e);
            ProofError::SindriError(e.to_string())
        })?;

    info!(
        "✅ Sindri API call successful! Proof ID: {}, Status: {:?}",
        proof_info.proof_id, proof_info.status
    );

    let status = match proof_info.status {
        JobStatus::Ready => "Ready".to_string(),
        JobStatus::Failed => "Failed".to_string(),
        _ => "Pending".to_string(),
    };

    let verification_command = format!(
        "cargo run --release -- --verify --proof-id {} --initial-balance {} --transactions {:?}",
        proof_info.proof_id, request.initial_balance, request.transactions
    );

    info!(
        "✅ {} batch proof submitted successfully - ID: {}",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        proof_info.proof_id
    );

    let response = ProofGenerationResponse {
        proof_id: proof_info.proof_id.clone(),
        status,
        circuit_name,
        circuit_tag,
        verification_command,
        proof_info,
    };

    // Generate EVM fixture if requested
    if request.generate_fixtures {
        if let Err(e) = create_batch_evm_fixture(
            &response.proof_info,
            request.initial_balance,
            &request.transactions,
            final_balance,
            request.proof_system,
        )
        .await
        {
            warn!("⚠️  Failed to generate EVM fixture: {}", e);
        }
    }

    Ok(response)
}

/// Generate a proof via Sindri with full feature support
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Sindri API key is not set in environment
/// - Circuit is not found or not deployed
/// - Proof generation fails on Sindri servers
/// - Network communication errors occur
#[allow(clippy::cognitive_complexity)]
pub async fn generate_sindri_proof(
    request: ProofGenerationRequest,
) -> Result<ProofGenerationResponse, ProofError> {
    info!(
        "🔐 Generating {} proof: {} + {} = {} via Sindri",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        request.a,
        request.b,
        request.result
    );

    // Create SP1 inputs and serialize for Sindri
    let mut stdin = SP1Stdin::new();
    stdin.write(&request.a);
    stdin.write(&request.b);

    let stdin_json =
        serde_json::to_string(&stdin).map_err(|e| ProofError::SerializationError(e.to_string()))?;
    let proof_input = ProofInput::from(stdin_json);

    // Get circuit name with configurable tag from environment
    let circuit_tag = std::env::var("SINDRI_CIRCUIT_TAG").unwrap_or_else(|_| "latest".to_string());
    let circuit_name = format!("demo-vapp:{circuit_tag}");

    info!("📋 Using circuit: {} (tag: {})", circuit_name, circuit_tag);

    let client = SindriClient::default();

    let proof_info = client
        .prove_circuit(&circuit_name, proof_input, None, None, None)
        .await
        .map_err(|e| ProofError::SindriError(e.to_string()))?;

    if proof_info.status == JobStatus::Failed {
        return Err(ProofError::ProofGenerationFailed(format!(
            "Sindri proof generation failed: {:?}",
            proof_info.error
        )));
    }

    let status = match proof_info.status {
        JobStatus::Ready => "Ready".to_string(),
        JobStatus::Failed => "Failed".to_string(),
        _ => "Pending".to_string(),
    };

    let verification_command = format!(
        "cargo run --release -- --verify --proof-id {} --result {}",
        proof_info.proof_id, request.result
    );

    info!(
        "✅ {} proof submitted successfully - ID: {}",
        request.proof_system.to_sindri_scheme().to_uppercase(),
        proof_info.proof_id
    );

    let response = ProofGenerationResponse {
        proof_id: proof_info.proof_id.clone(),
        status,
        circuit_name,
        circuit_tag,
        verification_command,
        proof_info,
    };

    // Generate EVM fixture if requested
    if request.generate_fixtures {
        if let Err(e) = create_evm_fixture(
            &response.proof_info,
            request.a,
            request.b,
            request.result,
            request.proof_system,
        )
        .await
        {
            warn!("⚠️  Failed to generate EVM fixture: {}", e);
        }
    }

    Ok(response)
}

/// Verify a proof via Sindri with comprehensive validation
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri
/// - Proof verification fails
/// - Network communication errors occur  
/// - SP1 proof extraction fails
pub async fn verify_sindri_proof(
    request: ProofVerificationRequest,
) -> Result<ProofVerificationResponse, ProofError> {
    let start_time = std::time::Instant::now();

    info!("🔍 Verifying proof ID: {}", request.proof_id);

    let client = SindriClient::default();

    let proof_info = client
        .get_proof(&request.proof_id, None, None, None)
        .await
        .map_err(|e| ProofError::SindriError(e.to_string()))?;

    if proof_info.status != JobStatus::Ready {
        return Ok(ProofVerificationResponse {
            is_valid: false,
            cryptographic_proof_valid: false,
            balances_match_expected: false,
            actual_initial_balance: None,
            actual_final_balance: None,
            expected_initial_balance: request.expected_initial_balance,
            expected_final_balance: request.expected_final_balance,
            verification_message: format!("Proof not ready. Status: {:?}", proof_info.status),
            verification_time_ms: start_time
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        });
    }

    // Extract SP1 proof and verification key from Sindri response
    let sp1_proof = proof_info
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::VerificationFailed(format!("Failed to extract SP1 proof: {e}")))?;

    let sindri_verifying_key = proof_info.get_sp1_verifying_key().map_err(|e| {
        ProofError::VerificationFailed(format!("Failed to extract verification key: {e}"))
    })?;

    // Perform local verification using Sindri's verification key
    let cryptographic_proof_valid = proof_info
        .verify_sp1_proof_locally(&sindri_verifying_key)
        .is_ok();

    if !cryptographic_proof_valid {
        return Ok(ProofVerificationResponse {
            is_valid: false,
            cryptographic_proof_valid: false,
            balances_match_expected: false,
            actual_initial_balance: None,
            actual_final_balance: None,
            expected_initial_balance: request.expected_initial_balance,
            expected_final_balance: request.expected_final_balance,
            verification_message: "Cryptographic proof verification failed".to_string(),
            verification_time_ms: start_time
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        });
    }

    // Verification successful - now validate the balance transition
    let decoded = PublicValuesStruct::abi_decode(sp1_proof.public_values.as_slice())
        .map_err(|e| ProofError::PublicValuesDecodeError(e.to_string()))?;

    let actual_initial_balance = decoded.initial_balance;
    let actual_final_balance = decoded.final_balance;
    let balances_match_expected = actual_initial_balance == request.expected_initial_balance
        && actual_final_balance == request.expected_final_balance;
    let is_valid = cryptographic_proof_valid && balances_match_expected;

    let verification_message = if is_valid {
        format!(
            "✅ CONTINUOUS BALANCE TRACKING VERIFIED: {actual_initial_balance} -> {actual_final_balance} (cryptographically verified)"
        )
    } else {
        format!(
            "❌ Balance transition verification failed: Expected {} -> {}, got {} -> {}",
            request.expected_initial_balance,
            request.expected_final_balance,
            actual_initial_balance,
            actual_final_balance
        )
    };

    info!("{}", verification_message);

    Ok(ProofVerificationResponse {
        is_valid,
        cryptographic_proof_valid,
        balances_match_expected,
        actual_initial_balance: Some(actual_initial_balance),
        actual_final_balance: Some(actual_final_balance),
        expected_initial_balance: request.expected_initial_balance,
        expected_final_balance: request.expected_final_balance,
        verification_message,
        verification_time_ms: start_time
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}

/// Create EVM-compatible fixture from Sindri proof for Solidity testing
#[allow(clippy::cognitive_complexity)]
async fn create_evm_fixture(
    proof_info: &ProofInfoResponse,
    _a: i32,
    _b: i32,
    result: i32,
    system: ProofSystem,
) -> Result<(), ProofError> {
    const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals

    info!(
        "🔧 Generating EVM fixture for {} proof...",
        system.to_sindri_scheme().to_uppercase()
    );

    // Wait for proof to be ready if it's still processing
    let client = SindriClient::default();
    let mut current_proof = proof_info.clone();

    // Poll until proof is ready (with timeout)
    let mut attempts = 0;

    while current_proof.status != JobStatus::Ready && attempts < MAX_ATTEMPTS {
        if current_proof.status == JobStatus::Failed {
            return Err(ProofError::FixtureGenerationError(format!(
                "Sindri proof generation failed: {:?}",
                current_proof.error
            )));
        }

        info!(
            "⏳ Waiting for proof to be ready... (attempt {}/{})",
            attempts + 1,
            MAX_ATTEMPTS
        );
        tokio::time::sleep(Duration::from_secs(5)).await;

        current_proof = client
            .get_proof(&proof_info.proof_id, None, None, Some(true))
            .await
            .map_err(|e| ProofError::SindriError(e.to_string()))?;
        attempts += 1;
    }

    if current_proof.status != JobStatus::Ready {
        return Err(ProofError::FixtureGenerationError(
            "Timeout waiting for Sindri proof to be ready".to_string(),
        ));
    }

    info!("✅ Sindri proof is ready, extracting EVM-compatible data...");

    // Extract SP1 proof data from Sindri response
    let sp1_proof = current_proof
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;
    let verification_key = current_proof
        .get_sp1_verifying_key()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;

    // Create the fixture
    // Note: In zero-knowledge mode, we use placeholder values for a and b since they're private
    let fixture = SP1ArithmeticProofFixture {
        a: 0, // Placeholder - actual value is private in ZK
        b: 0, // Placeholder - actual value is private in ZK
        result,
        vkey: verification_key.bytes32(),
        public_values: format!("0x{}", hex::encode(sp1_proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(sp1_proof.bytes())),
    };

    // Create fixtures directory and save the fixture
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            ProofError::FixtureGenerationError("Failed to get parent directory".to_string())
        })?
        .join("contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to create fixtures directory: {e}"))
    })?;

    let filename = format!("{}-fixture.json", system.to_sindri_scheme());
    let fixture_file = fixture_path.join(&filename);

    std::fs::write(&fixture_file, serde_json::to_string_pretty(&fixture)?).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to write fixture file: {e}"))
    })?;

    info!("✅ EVM fixture saved to: {}", fixture_file.display());
    info!("🔑 Verification Key: {}", fixture.vkey);
    info!("📊 Public Values: {}", fixture.public_values);
    info!(
        "🔒 Proof Bytes: {}...{}",
        &fixture.proof[..42],
        &fixture.proof[fixture.proof.len() - 6..]
    );

    Ok(())
}

/// Get proof information from Sindri
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri  
/// - Network communication errors occur
pub async fn get_sindri_proof_info(proof_id: &str) -> Result<ProofInfoResponse, ProofError> {
    let client = SindriClient::default();
    client
        .get_proof(proof_id, None, None, None)
        .await
        .map_err(|e| ProofError::SindriError(e.to_string()))
}

/// Check if a proof is ready on Sindri
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof is not found on Sindri  
/// - Network communication errors occur
pub async fn is_proof_ready(proof_id: &str) -> Result<bool, ProofError> {
    let proof_info = get_sindri_proof_info(proof_id).await?;
    Ok(proof_info.status == JobStatus::Ready)
}

/// Create EVM-compatible fixture from Sindri proof for batch processing
#[allow(clippy::cognitive_complexity)]
async fn create_batch_evm_fixture(
    proof_info: &ProofInfoResponse,
    initial_balance: i32,
    transactions: &[i32],
    final_balance: i32,
    system: ProofSystem,
) -> Result<(), ProofError> {
    const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals
    info!(
        "🔧 Generating batch EVM fixture for {} proof...",
        system.to_sindri_scheme().to_uppercase()
    );

    // Wait for proof to be ready if it's still processing
    let client = SindriClient::default();
    let mut current_proof = proof_info.clone();

    // Poll until proof is ready (with timeout)
    let mut attempts = 0;

    while current_proof.status != JobStatus::Ready && attempts < MAX_ATTEMPTS {
        if current_proof.status == JobStatus::Failed {
            return Err(ProofError::FixtureGenerationError(format!(
                "Sindri proof generation failed: {:?}",
                current_proof.error
            )));
        }

        info!(
            "⏳ Waiting for proof to be ready... (attempt {}/{})",
            attempts + 1,
            MAX_ATTEMPTS
        );
        tokio::time::sleep(Duration::from_secs(5)).await;

        current_proof = client
            .get_proof(&proof_info.proof_id, None, None, Some(true))
            .await
            .map_err(|e| ProofError::SindriError(e.to_string()))?;
        attempts += 1;
    }

    if current_proof.status != JobStatus::Ready {
        return Err(ProofError::FixtureGenerationError(
            "Timeout waiting for Sindri proof to be ready".to_string(),
        ));
    }

    info!("✅ Sindri proof is ready, extracting EVM-compatible data...");

    // Extract SP1 proof data from Sindri response
    let sp1_proof = current_proof
        .to_sp1_proof_with_public()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;
    let verification_key = current_proof
        .get_sp1_verifying_key()
        .map_err(|e| ProofError::FixtureGenerationError(e.to_string()))?;

    // Create the batch fixture
    let fixture = SP1BatchProofFixture {
        initial_balance,
        transactions: transactions.to_vec(),
        final_balance,
        vkey: verification_key.bytes32(),
        public_values: format!("0x{}", hex::encode(sp1_proof.public_values.as_slice())),
        proof: format!("0x{}", hex::encode(sp1_proof.bytes())),
    };

    // Create fixtures directory and save the fixture
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            ProofError::FixtureGenerationError("Failed to get parent directory".to_string())
        })?
        .join("contracts/src/fixtures");

    std::fs::create_dir_all(&fixture_path).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to create fixtures directory: {e}"))
    })?;

    let filename = format!("batch-{}-fixture.json", system.to_sindri_scheme());
    let fixture_file = fixture_path.join(&filename);

    std::fs::write(&fixture_file, serde_json::to_string_pretty(&fixture)?).map_err(|e| {
        ProofError::FixtureGenerationError(format!("Failed to write fixture file: {e}"))
    })?;

    info!("✅ Batch EVM fixture saved to: {}", fixture_file.display());
    info!("🔑 Verification Key: {}", fixture.vkey);
    info!("📊 Public Values: {}", fixture.public_values);
    info!(
        "🔒 Proof Bytes: {}...{}",
        &fixture.proof[..42],
        &fixture.proof[fixture.proof.len() - 6..]
    );

    Ok(())
}

/// Wait for a proof to be ready with timeout
///
/// # Errors
///
/// Returns `ProofError` if:
/// - Proof times out before becoming ready
/// - Proof fails during generation
/// - Network communication errors occur
pub async fn wait_for_proof_ready(
    proof_id: &str,
    timeout_seconds: u64,
) -> Result<ProofInfoResponse, ProofError> {
    let client = SindriClient::default();
    let mut attempts = 0;
    let max_attempts = timeout_seconds / 5; // Check every 5 seconds

    loop {
        let proof_info = client
            .get_proof(proof_id, None, None, None)
            .await
            .map_err(|e| ProofError::SindriError(e.to_string()))?;

        match proof_info.status {
            JobStatus::Ready => return Ok(proof_info),
            JobStatus::Failed => {
                return Err(ProofError::ProofGenerationFailed(format!(
                    "Proof generation failed: {:?}",
                    proof_info.error
                )));
            }
            _ => {
                if attempts >= max_attempts {
                    return Err(ProofError::ProofGenerationFailed(format!(
                        "Timeout waiting for proof to be ready after {timeout_seconds} seconds"
                    )));
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                attempts += 1;
            }
        }
    }
}
