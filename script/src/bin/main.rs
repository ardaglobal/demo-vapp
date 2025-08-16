//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```
//!
//! Customize background processor (execute mode only):
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute --bg-interval 10 --bg-batch-size 50
//! ```
//!
//! Run background processor once then exit:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute --bg-one-shot
//! ```

use alloy_primitives::{Bytes, FixedBytes};
use alloy_sol_types::SolType;
use arithmetic_db::db::{
    get_sindri_proof_by_result, get_value_by_result, init_db, store_arithmetic_transaction,
    upsert_sindri_proof,
};
use arithmetic_db::ProcessorBuilder;
use arithmetic_lib::PublicValuesStruct;
use clap::{Parser, ValueEnum};
use ethereum_client::{Config as EthereumConfig, EthereumClient};
use serde::{Deserialize, Serialize};
use sindri::integrations::sp1_v5::SP1ProofInfo;
use sindri::{client::SindriClient, JobStatus, ProofInfoResponse, ProofInput};
use sp1_sdk::{include_elf, HashableKey, Prover, ProverClient, SP1Stdin};
use sqlx::PgPool;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::info;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ARITHMETIC_ELF: &[u8] = include_elf!("arithmetic-program");

/// Enum representing the available EVM-compatible proof systems
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum ProofSystem {
    Plonk,
    Groth16,
}

impl ProofSystem {
    /// Convert to the proving scheme string expected by Sindri
    const fn as_sindri_scheme(self) -> &'static str {
        match self {
            Self::Plonk => "plonk",
            Self::Groth16 => "groth16",
        }
    }
}

/// A fixture that can be used to test the verification of SP1 zkVM proofs inside Solidity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SP1ArithmeticProofFixture {
    a: i32,
    b: i32,
    result: i32,
    vkey: String,
    public_values: String,
    proof: String,
}

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    // Program execution mode
    #[arg(long)]
    execute: bool, // Run the program in interactive mode
    #[arg(long)]
    prove: bool, // Run the program in prove mode
    #[arg(long)]
    verify: bool, // Run the program in verify mode
    #[arg(long)]
    vkey: bool, // Print the vkey for the program

    // Arithmetic inputs
    #[arg(long, default_value = "1")]
    a: i32,
    #[arg(long, default_value = "1")]
    b: i32,
    #[arg(long, default_value = "20")]
    result: i32,

    // EVM-compatible proof system selection
    #[arg(
        long,
        value_enum,
        default_value = "groth16",
        help = "EVM-compatible proof system to use"
    )]
    system: ProofSystem,

    // Proof ID for external verification
    #[arg(long)]
    proof_id: Option<String>,

    // Generate EVM fixture files (only used with --prove)
    #[arg(long, help = "Generate Solidity test fixtures for EVM verification")]
    generate_fixture: bool,

    // Skip smart contract submission (only used with --prove)
    #[arg(
        long,
        help = "Skip automatic smart contract submission after proof generation"
    )]
    skip_contract_submission: bool,

    // Background processor configuration (only used with --execute)
    #[arg(
        long,
        default_value = "30",
        help = "Background processor polling interval in seconds"
    )]
    bg_interval: u64,
    #[arg(
        long,
        default_value = "100",
        help = "Background processor batch size for processing transactions"
    )]
    bg_batch_size: usize,
    #[arg(
        long,
        help = "Run background processor once and exit (default: continuous mode)"
    )]
    bg_one_shot: bool,
}

#[tokio::main]
async fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    // Determine the mode of operation.
    // If multiple modes are specified, the modes are executed in alphabetic order, which also happens to be the execute, prove, verify order.
    if args.execute {
        // Execute mode requires database for storing results
        let client = ProverClient::from_env();
        let pool = init_db().await.expect("Failed to initialize database");

        // Start background processor for indexed Merkle tree construction with user configuration
        let _background_handle = start_background_processor(
            pool.clone(),
            args.bg_interval,
            args.bg_batch_size,
            !args.bg_one_shot, // continuous = !one_shot
        )
        .await;

        run_interactive_execute(&client, &pool).await;
    }

    if args.prove {
        // Determine if we need database based on whether user provided a specific result to lookup
        // vs. using provided/default a and b values
        let using_default_inputs = args.a == 1 && args.b == 1; // Default values from clap
        let using_specific_result = args.result != 20; // Non-default result value

        let needs_database = using_specific_result && using_default_inputs;

        if needs_database {
            // Need database to lookup inputs by result
            println!(
                "🔍 Looking up inputs for result = {} in database",
                args.result
            );
            let pool = init_db().await.expect("Failed to initialize database - required to lookup inputs for the specified result");
            run_prove_via_sindri(
                &pool,
                args.a,
                args.b,
                args.result,
                args.system,
                args.generate_fixture,
                !args.skip_contract_submission,
            )
            .await;
        } else {
            // Have explicit inputs or using default calculation - no database needed
            if using_specific_result && !using_default_inputs {
                println!(
                    "ℹ️  Using provided inputs (a={}, b={}) - ignoring --result parameter",
                    args.a, args.b
                );
            } else {
                println!("ℹ️  Using provided inputs - database not required for proving");
            }
            run_prove_via_sindri_no_db(
                args.a,
                args.b,
                args.result,
                args.system,
                args.generate_fixture,
                !args.skip_contract_submission,
            )
            .await;
        }
    }

    if args.verify {
        if let Some(proof_id) = args.proof_id {
            // External verification flow - no database dependency
            run_external_verify(&proof_id, args.result).await;
        } else {
            // Legacy database-based verification flow - requires database
            let pool = init_db().await.expect("Failed to initialize database");
            run_verify_mode(&pool, args.result).await;
        }
    }

    if args.vkey {
        let prover = ProverClient::builder().cpu().build();
        let (_, vk) = prover.setup(ARITHMETIC_ELF);
        println!("{}", vk.bytes32());
    }
}

/// Start background processor for indexed Merkle tree construction
/// Returns a join handle for the background task
fn start_background_processor(
    pool: PgPool,
    interval_secs: u64,
    batch_size: usize,
    continuous: bool,
) -> JoinHandle<()> {
    info!("🚀 Starting background processor for indexed Merkle tree construction...");
    info!(
        "⚙️  Configuration: interval={}s, batch_size={}, continuous={}",
        interval_secs, batch_size, continuous
    );

    // Create and configure processor with user-provided settings
    let mut processor = ProcessorBuilder::new()
        .polling_interval(Duration::from_secs(interval_secs))
        .batch_size(batch_size)
        .continuous(continuous)
        .build(pool);

    // Spawn background task
    tokio::spawn(async move {
        let mode = if continuous { "continuous" } else { "one-shot" };
        info!("📊 Background processor started in {} mode - monitoring for new arithmetic transactions", mode);

        if let Err(e) = processor.start().await {
            eprintln!("❌ Background processor error: {e}");
        } else if !continuous {
            info!("✅ Background processor completed one-shot processing");
        }
    })
}

/// Helper function to get integer input from user with quit option
/// Returns None if user wants to quit, Some(value) if valid integer entered
fn get_integer_input(prompt: &str) -> Option<i32> {
    loop {
        print!("{prompt}");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Error reading input. Please try again.");
            continue;
        }

        let input = input.trim();
        if input == "q" || input == "Q" {
            return None; // User wants to quit
        }

        match input.parse::<i32>() {
            Ok(num) => return Some(num),
            Err(_) => {
                println!("Invalid number '{input}'. Please enter an integer or 'q' to quit.");
            }
        }
    }
}

async fn run_interactive_execute(client: &sp1_sdk::EnvProver, pool: &PgPool) {
    println!("=== Interactive Arithmetic Execution ===");
    println!("Enter two numbers to add them together.");
    println!("Results will be stored in the database.");
    println!("📊 Background processor is running - building indexed Merkle tree automatically.");
    println!("💡 Tip: Use --bg-interval, --bg-batch-size, and --bg-one-shot to customize background processing.");
    println!("Press 'q' + Enter to quit.\n");

    loop {
        // Get input for 'a'
        let Some(a) = get_integer_input("Enter value for 'a' (or 'q' to quit): ") else {
            println!("Goodbye!");
            break;
        };

        // Get input for 'b'
        let Some(b) = get_integer_input("Enter value for 'b' (or 'q' to quit): ") else {
            println!("Goodbye!");
            break;
        };

        // Execute the computation
        println!("\nExecuting: {a} + {b} ...");

        let mut stdin = SP1Stdin::new();
        stdin.write(&a);
        stdin.write(&b);

        match client.execute(ARITHMETIC_ELF, &stdin).run() {
            Ok((output, report)) => {
                // Read the output
                match PublicValuesStruct::abi_decode(output.as_slice()) {
                    Ok(decoded) => {
                        let PublicValuesStruct { result } = decoded;
                        println!("✓ Computation successful: {a} + {b} = {result}");

                        let expected = arithmetic_lib::addition(a, b);
                        if result == expected {
                            println!("✓ Result verified");
                        } else {
                            println!("✗ Result mismatch (expected {expected})");
                            continue;
                        }

                        // Store in database
                        match store_arithmetic_transaction(pool, a, b, result).await {
                            Ok(()) => {
                                println!("✓ Stored in database");
                            }
                            Err(e) => {
                                println!("✗ Failed to store in database: {e}");
                            }
                        }

                        println!("Cycles executed: {}\n", report.total_instruction_count());
                    }
                    Err(e) => {
                        println!("✗ Failed to decode output: {e}\n");
                    }
                }
            }
            Err(e) => {
                println!("✗ Execution failed: {e}\n");
            }
        }
    }
}

async fn run_verify_mode(pool: &PgPool, result: i32) {
    println!("=== Database Verification Mode ===");
    println!("⚠️  This mode requires database access. For external verification, use --proof-id instead.");

    if result == 20 {
        // Default value
        // Interactive verify mode
        println!("Enter a result value to look up in the database.");
        println!("Press 'q' + Enter to quit.\n");

        loop {
            let Some(lookup_result) =
                get_integer_input("Enter result to verify (or 'q' to quit): ")
            else {
                println!("Goodbye!");
                break;
            };

            verify_result_via_sindri(pool, lookup_result).await;
            println!();
        }
    } else {
        // Single verify mode
        verify_result_via_sindri(pool, result).await;
    }
}

/// Get proof from Sindri by `proof_id`
async fn get_sindri_proof(proof_id: &str) -> Option<ProofInfoResponse> {
    let client = SindriClient::default();
    match client.get_proof(proof_id, None, None, None).await {
        Ok(verification_result) => {
            println!(
                "Verification status from Sindri: {:?}",
                verification_result.status
            );

            match verification_result.status {
                JobStatus::Ready => {
                    println!("✓ Proof is READY on Sindri");
                    Some(verification_result)
                }
                JobStatus::Failed => {
                    println!(
                        "✗ Proof verification FAILED: {:?}",
                        verification_result.error
                    );
                    None
                }
                other => {
                    println!("⏳ Proof status: {other:?}");
                    None
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to retrieve proof from Sindri: {e}");
            None
        }
    }
}

/// Core verification function - handles the actual proof verification
fn verify_proof_core(proof_info: &ProofInfoResponse, expected_result: i32) -> bool {
    println!("🔍 Performing local SP1 proof verification...");

    // Extract SP1 proof and verification key from Sindri response
    let sp1_proof = match proof_info.to_sp1_proof_with_public() {
        Ok(proof) => proof,
        Err(e) => {
            println!("✗ Failed to extract SP1 proof: {e}");
            return false;
        }
    };

    let sindri_verifying_key = match proof_info.get_sp1_verifying_key() {
        Ok(vk) => vk,
        Err(e) => {
            println!("✗ Failed to extract verification key: {e}");
            return false;
        }
    };

    // Perform local verification using Sindri's verification key
    if let Err(e) = proof_info.verify_sp1_proof_locally(&sindri_verifying_key) {
        println!("✗ Local proof verification FAILED: {e}");
        return false;
    }

    // Verification successful - now validate the computation result
    let decoded = match PublicValuesStruct::abi_decode(sp1_proof.public_values.as_slice()) {
        Ok(decoded) => decoded,
        Err(e) => {
            println!("✗ Failed to decode public values from proof: {e}");
            return false;
        }
    };

    let PublicValuesStruct { result } = decoded;
    let result_valid = result == expected_result;

    // Color codes for output
    let color_code = if result_valid { "\x1b[32m" } else { "\x1b[31m" };
    let reset_code = "\x1b[0m";

    if result_valid {
        println!("{color_code}✓ ZERO-KNOWLEDGE PROOF VERIFIED: result = {result} (ZKP verified){reset_code}");
        println!("🔐 Proof cryptographically verified - computation integrity confirmed");
        println!("🎭 Private inputs remain hidden - only the result is revealed");
        println!("📊 The prover demonstrated knowledge of inputs that produce result = {result}");
        true
    } else {
        println!("{color_code}✗ Proof verification FAILED: Expected {expected_result}, got {result}{reset_code}");
        false
    }
}

/// Verify proof by looking up result in database
async fn verify_result_via_sindri(pool: &PgPool, result: i32) {
    println!("Verifying proof for result: {result} via Sindri...");

    // Get proof_id from database
    let proof_id = match get_sindri_proof_by_result(pool, result).await {
        Ok(Some(record)) => record.proof_id,
        Ok(None) => {
            println!("✗ No Sindri proof stored for result = {result}. Run --prove to create one.");
            return;
        }
        Err(e) => {
            println!("✗ Database error: {e}");
            return;
        }
    };

    // Get proof from Sindri
    let Some(proof_info) = get_sindri_proof(&proof_id).await else {
        return; // Error already printed
    };

    // Perform verification
    let verification_success = verify_proof_core(&proof_info, result);

    // Update database with latest status (only for database-driven verification)
    if verification_success {
        let _ = upsert_sindri_proof(
            pool,
            result,
            &proof_id,
            Some(proof_info.circuit_id.clone()),
            Some("Ready".to_string()),
        )
        .await;
    }
}

/// Verify proof directly by `proof_id` (no database required)
async fn run_external_verify(proof_id: &str, expected_result: i32) {
    println!("=== External Verification Mode ===");
    println!("Verifying proof ID: {proof_id}");
    println!("Expected result: {expected_result}");

    // Get proof from Sindri
    let Some(proof_info) = get_sindri_proof(proof_id).await else {
        println!("💡 Make sure the proof ID is correct and the proof exists on Sindri");
        return;
    };

    // Perform verification (no database updates for external verification)
    verify_proof_core(&proof_info, expected_result);
}

/// Core proving function that handles Sindri circuit proving without database dependencies
///
/// Returns the proof info and computed values on success
#[allow(clippy::future_not_send)]
async fn prove_via_sindri_core(
    a: i32,
    b: i32,
    result: i32,
    system: ProofSystem,
) -> Option<ProofInfoResponse> {
    println!("Proving that {a} + {b} = {result} via Sindri...");

    // Create SP1 inputs and serialize for Sindri
    let mut stdin = SP1Stdin::new();
    stdin.write(&a);
    stdin.write(&b);

    let stdin_json = match serde_json::to_string(&stdin) {
        Ok(s) => s,
        Err(e) => {
            println!("✗ Failed to serialize SP1Stdin: {e}");
            return None;
        }
    };
    let proof_input = ProofInput::from(stdin_json);

    let client = SindriClient::default();
    println!("Submitting proof request to Sindri...");
    let proof_info = client
        .prove_circuit(
            "demo-vapp", // Circuit name as defined in sindri.json manifest
            proof_input,
            None,
            None,
            None,
        )
        .await;

    let proof_info = match proof_info {
        Ok(info) => info,
        Err(e) => {
            println!("✗ Failed to submit proof request: {e}");
            return None;
        }
    };

    if proof_info.status == JobStatus::Failed {
        println!("✗ Proof generation failed: {:?}", proof_info.error);
        return None;
    }

    println!(
        "✓ {} proof job submitted. Status: {:?}",
        system.as_sindri_scheme().to_uppercase(),
        proof_info.status
    );
    println!("\n🔗 PROOF ID FOR EXTERNAL VERIFICATION:");
    println!("   {}", proof_info.proof_id);
    println!("\n📋 To verify this proof externally, use:");
    println!(
        "   cargo run --release -- --verify --proof-id {} --result {}",
        proof_info.proof_id, result
    );

    Some(proof_info)
}

#[allow(clippy::future_not_send)]
async fn run_prove_via_sindri(
    pool: &PgPool,
    arg_a: i32,
    arg_b: i32,
    arg_result: i32,
    system: ProofSystem,
    generate_fixture: bool,
    submit_to_contract: bool,
) {
    // Prefer proving by result if provided (not default), otherwise use provided a and b
    let (a, b, result) = if arg_result == 20 {
        let result = arithmetic_lib::addition(arg_a, arg_b);
        (arg_a, arg_b, result)
    } else {
        match get_value_by_result(pool, arg_result).await {
            Ok(Some((a, b, _))) => (a, b, arg_result),
            Ok(None) => {
                println!("✗ No stored transaction found with result = {arg_result}. Run --execute first.");
                return;
            }
            Err(e) => {
                println!("✗ Database error: {e}");
                return;
            }
        }
    };

    // Use the common proving core
    let Some(proof_info) = prove_via_sindri_core(a, b, result, system).await else {
        return; // Error already printed in core function
    };

    // Generate EVM fixture if requested
    if generate_fixture {
        if let Err(e) = create_evm_fixture_from_sindri(&proof_info, a, b, result, system).await {
            println!("⚠️  Failed to generate EVM fixture: {e}");
        }
    }

    // Submit to smart contract if requested
    if submit_to_contract {
        if let Err(e) = submit_proof_to_contract(&proof_info, result).await {
            println!("⚠️  Failed to submit proof to smart contract: {e}");
        }
    }

    // Store proof metadata by result for later verification
    if let Err(e) = upsert_sindri_proof(
        pool,
        result,
        &proof_info.proof_id,
        Some(proof_info.circuit_id.clone()),
        Some(match proof_info.status {
            JobStatus::Ready => "Ready".to_string(),
            JobStatus::Failed => "Failed".to_string(),
            _ => "Other".to_string(),
        }),
    )
    .await
    {
        println!("✗ Failed to store proof metadata: {e}");
    } else {
        println!(
            "✓ Stored Sindri proof metadata for result = {} (proof_id = {})",
            result, proof_info.proof_id
        );
    }
}

async fn run_prove_via_sindri_no_db(
    arg_a: i32,
    arg_b: i32,
    arg_result: i32,
    system: ProofSystem,
    generate_fixture: bool,
    submit_to_contract: bool,
) {
    // Calculate result from inputs (no database lookup needed)
    // For database-free mode, we always calculate from provided inputs
    if arg_result != 20 {
        println!("⚠️  Database-free mode: Using provided inputs and ignoring --result parameter");
    }
    let result = arithmetic_lib::addition(arg_a, arg_b);
    let (a, b) = (arg_a, arg_b);

    println!("Database-free mode:");

    // Use the common proving core
    let Some(proof_info) = prove_via_sindri_core(a, b, result, system).await else {
        return; // Error already printed in core function
    };

    // Generate EVM fixture if requested
    if generate_fixture {
        if let Err(e) = create_evm_fixture_from_sindri(&proof_info, a, b, result, system).await {
            println!("⚠️  Failed to generate EVM fixture: {e}");
        }
    }

    // Submit to smart contract if requested
    if submit_to_contract {
        if let Err(e) = submit_proof_to_contract(&proof_info, result).await {
            println!("⚠️  Failed to submit proof to smart contract: {e}");
        }
    }

    println!("ℹ️  Note: Proof metadata not stored (database-free mode)");
}

/// Create EVM-compatible fixture from Sindri proof for Solidity testing
async fn create_evm_fixture_from_sindri(
    proof_info: &ProofInfoResponse,
    _a: i32,
    _b: i32,
    result: i32,
    system: ProofSystem,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "🔧 Generating EVM fixture for {} proof...",
        system.as_sindri_scheme().to_uppercase()
    );

    // Wait for proof to be ready if it's still processing
    let client = SindriClient::default();
    let mut current_proof = proof_info.clone();

    // Poll until proof is ready (with timeout)
    #[allow(clippy::items_after_statements)]
    const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals
    let mut attempts = 0;

    while current_proof.status != JobStatus::Ready && attempts < MAX_ATTEMPTS {
        if current_proof.status == JobStatus::Failed {
            return Err(
                format!("Sindri proof generation failed: {:?}", current_proof.error).into(),
            );
        }

        println!(
            "⏳ Waiting for proof to be ready... (attempt {}/{})",
            attempts + 1,
            MAX_ATTEMPTS
        );
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        current_proof = client
            .get_proof(&proof_info.proof_id, None, None, Some(true))
            .await?;
        attempts += 1;
    }

    if current_proof.status != JobStatus::Ready {
        return Err("Timeout waiting for Sindri proof to be ready".into());
    }

    println!("✅ Sindri proof is ready, extracting EVM-compatible data...");

    // Extract SP1 proof data from Sindri response
    let sp1_proof = current_proof.to_sp1_proof_with_public()?;
    let verification_key = current_proof.get_sp1_verifying_key()?;

    // Create the fixture matching evm.rs format
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
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/src/fixtures");
    std::fs::create_dir_all(&fixture_path)?;

    let filename = format!("{}-fixture.json", system.as_sindri_scheme());
    let fixture_file = fixture_path.join(&filename);

    std::fs::write(&fixture_file, serde_json::to_string_pretty(&fixture)?)?;

    println!("✅ EVM fixture saved to: {}", fixture_file.display());
    println!("🔑 Verification Key: {}", fixture.vkey);
    println!("📊 Public Values: {}", fixture.public_values);
    println!(
        "🔒 Proof Bytes: {}...{}",
        &fixture.proof[..42],
        &fixture.proof[fixture.proof.len() - 6..]
    );

    Ok(())
}

/// Submit proof to smart contract using ethereum client
async fn submit_proof_to_contract(
    proof_info: &ProofInfoResponse,
    result: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Submitting proof to smart contract...");

    // Create ethereum client configuration
    let eth_config = EthereumConfig::from_env()?;

    // Initialize ethereum client
    let eth_client = EthereumClient::new(eth_config).await?;

    // Check if client has signing capability
    if !eth_client.has_signer() {
        return Err("Ethereum client requires a signer for contract submission. Please set private key in environment.".into());
    }

    println!("✅ Ethereum client initialized successfully");

    // Extract SP1 proof and verification key
    let sp1_proof = proof_info.to_sp1_proof_with_public()?;
    let _verification_key = proof_info.get_sp1_verifying_key()?;

    // Convert SP1 proof to bytes for contract submission
    let proof_bytes = Bytes::from(sp1_proof.bytes());

    // Create public values (the arithmetic result as encoded bytes)
    let public_values = Bytes::from(sp1_proof.public_values.as_slice().to_vec());

    // Generate state ID (can be customized based on application logic)
    let state_id = FixedBytes::from_slice(
        &alloy_primitives::keccak256(format!("arithmetic_result_{result}").as_bytes())[..32],
    );

    // Generate new state root (for this demo, we'll use a simple hash of the result)
    let new_state_root =
        FixedBytes::from_slice(&alloy_primitives::keccak256(result.to_be_bytes())[..32]);

    println!("📊 Proof data prepared:");
    println!("  State ID: 0x{}", hex::encode(state_id.as_slice()));
    println!(
        "  New State Root: 0x{}",
        hex::encode(new_state_root.as_slice())
    );
    println!("  Proof size: {} bytes", proof_bytes.len());
    println!("  Public values size: {} bytes", public_values.len());

    // Submit to smart contract
    let state_update = eth_client
        .update_state(state_id, new_state_root, proof_bytes, public_values)
        .await?;

    println!("✅ Proof submitted to smart contract successfully!");
    println!("🔗 Transaction details:");
    if let Some(tx_hash) = state_update.transaction_hash {
        println!("  Transaction hash: 0x{}", hex::encode(tx_hash.as_slice()));
    }
    if let Some(block_number) = state_update.block_number {
        println!("  Block number: {block_number}");
    }
    println!(
        "  State ID: 0x{}",
        hex::encode(state_update.state_id.as_slice())
    );

    Ok(())
}
