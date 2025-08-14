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

use alloy_sol_types::SolType;
use arithmetic_db::db::{
    get_sindri_proof_by_result, get_value_by_result, init_db, store_arithmetic_transaction,
    upsert_sindri_proof,
};
use arithmetic_lib::PublicValuesStruct;
use clap::Parser;
use sindri::integrations::sp1_v5::SP1ProofInfo;
use sindri::{client::SindriClient, JobStatus, ProofInfo, ProofInput};
use sp1_sdk::{include_elf, HashableKey, Prover, ProverClient, SP1Stdin};
use sqlx::PgPool;
use std::io::{self, Write};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ARITHMETIC_ELF: &[u8] = include_elf!("arithmetic-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
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

    // Proof ID for external verification
    #[arg(long)]
    proof_id: Option<String>,
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
        run_interactive_execute(&client, &pool).await;
    }
    
    if args.prove {
        // Intelligently determine if we need database based on arguments
        let needs_database = (args.a != 0 && args.b != 0) && args.result == 0;

        if needs_database {
            // Need database to lookup inputs by result
            let pool = init_db().await.expect("Failed to initialize database - required to lookup inputs for the specified result");
            run_prove_via_sindri(&pool, args.a, args.b, args.result).await;
        } else {
            // Have explicit inputs or using default calculation - no database needed
            println!("ℹ️  Using provided inputs - database not required for proving");
            run_prove_via_sindri_no_db(args.a, args.b, args.result).await;
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

/// Helper function to get integer input from user with quit option
/// Returns None if user wants to quit, Some(value) if valid integer entered
fn get_integer_input(prompt: &str) -> Option<i32> {
    loop {
        print!("{}", prompt);
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
                continue;
            }
        }
    }
}

async fn run_interactive_execute(client: &sp1_sdk::EnvProver, pool: &PgPool) {
    println!("=== Interactive Arithmetic Execution ===");
    println!("Enter two numbers to add them together.");
    println!("Results will be stored in the database.");
    println!("Press 'q' + Enter to quit.\n");

    loop {
        // Get input for 'a'
        let a = match get_integer_input("Enter value for 'a' (or 'q' to quit): ") {
            Some(value) => value,
            None => {
                println!("Goodbye!");
                break;
            }
        };

        // Get input for 'b'
        let b = match get_integer_input("Enter value for 'b' (or 'q' to quit): ") {
            Some(value) => value,
            None => {
                println!("Goodbye!");
                break;
            }
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
            let lookup_result = match get_integer_input("Enter result to verify (or 'q' to quit): ") {
                Some(value) => value,
                None => {
                    println!("Goodbye!");
                    break;
                }
            };

            verify_result_via_sindri(pool, lookup_result).await;
            println!();
        }
    } else {
        // Single verify mode
        verify_result_via_sindri(pool, result).await;
    }
}

async fn verify_result_via_sindri(pool: &PgPool, result: i32) {
    println!("Verifying proof for result: {result} via Sindri...");

    match get_sindri_proof_by_result(pool, result).await {
        Ok(Some(record)) => {
            let client = SindriClient::default();
            let proof_id: String = record.proof_id.clone();
            match client.get_proof(&proof_id, None, None, None).await {
                Ok(verification_result) => {
                    println!(
                        "Verification status from Sindri: {:?}",
                        verification_result.status
                    );
                    // Update stored status
                    let _ = upsert_sindri_proof(
                        pool,
                        result,
                        &proof_id,
                        Some(verification_result.circuit_id.clone()),
                        Some(match verification_result.status {
                            JobStatus::Ready => "Ready".to_string(),
                            JobStatus::Failed => "Failed".to_string(),
                            _ => "Other".to_string(),
                        }),
                    )
                    .await;

                    match verification_result.status {
                        JobStatus::Ready => {
                            println!("✓ Proof is READY on Sindri for result = {result}");

                            // Perform local verification using Sindri's verification key
                            perform_local_verification(&verification_result, result).await;
                        }
                        JobStatus::Failed => println!(
                            "✗ Proof verification FAILED for result = {result}: {:?}",
                            verification_result.error
                        ),
                        other => println!("⏳ Proof status: {other:?}"),
                    }
                }
                Err(e) => {
                    println!("✗ Failed to verify proof via Sindri: {e}");
                }
            }
        }
        Ok(None) => {
            println!("✗ No Sindri proof stored for result = {result}. Run --prove to create one.");
        }
        Err(e) => println!("✗ Database error: {e}"),
    }
}

async fn run_external_verify(proof_id: &str, expected_result: i32) {
    println!("=== External Verification Mode ===");
    println!("Verifying proof ID: {proof_id}");
    println!("Expected result: {expected_result}");

    let client = SindriClient::default();
    match client.get_proof(proof_id, None, None, None).await {
        Ok(verification_result) => {
            println!(
                "Verification status from Sindri: {:?}",
                verification_result.status
            );

            match verification_result.status {
                JobStatus::Ready => {
                    println!("✓ Proof is READY on Sindri for proof ID: {proof_id}");

                    // Perform local verification using Sindri's verification key
                    perform_local_verification(&verification_result, expected_result).await;
                }
                JobStatus::Failed => println!(
                    "✗ Proof verification FAILED for proof ID {proof_id}: {:?}",
                    verification_result.error
                ),
                other => println!("⏳ Proof status: {other:?}"),
            }
        }
        Err(e) => {
            println!("✗ Failed to retrieve proof from Sindri: {e}");
            println!("💡 Make sure the proof ID is correct and the proof exists on Sindri");
        }
    }
}

#[allow(clippy::future_not_send)]
#[allow(clippy::unused_async)]
async fn perform_local_verification<T>(verification_result: &T, expected_result: i32)
where
    T: ProofInfo + SP1ProofInfo,
{
    println!("🔍 Performing local SP1 proof verification...");

    // Extract SP1 proof and verification key from Sindri response
    match verification_result.to_sp1_proof_with_public() {
        Ok(sp1_proof) => {
            match verification_result.get_sp1_verifying_key() {
                Ok(sindri_verifying_key) => {
                    // Perform local verification using Sindri's verification key
                    match verification_result.verify_sp1_proof_locally(&sindri_verifying_key) {
                        Ok(()) => {
                            // Verification successful - now validate the computation
                            match PublicValuesStruct::abi_decode(sp1_proof.public_values.as_slice())
                            {
                                Ok(decoded) => {
                                    let PublicValuesStruct { result } = decoded;

                                    // In true zero-knowledge verification, we only see the result
                                    // We cannot see the private inputs 'a' and 'b' that were used
                                    let result_valid = result == expected_result;

                                    // Color codes for output
                                    let color_code =
                                        if result_valid { "\x1b[32m" } else { "\x1b[31m" }; // Green for valid, Red for invalid
                                    let reset_code = "\x1b[0m"; // Reset color

                                    if result_valid {
                                        println!(
                                            "{color_code}✓ ZERO-KNOWLEDGE PROOF VERIFIED: result = {result} (ZKP verified){reset_code}"
                                        );
                                        println!("🔐 Proof cryptographically verified - computation integrity confirmed");
                                        println!("🎭 Private inputs remain hidden - only the result is revealed");
                                        println!("📊 The prover demonstrated knowledge of inputs that produce result = {result}");
                                    } else {
                                        println!(
                                            "{color_code}✗ Proof verification FAILED: Expected {expected_result}, got {result}{reset_code}"
                                        );
                                    }
                                }
                                Err(e) => {
                                    println!("✗ Failed to decode public values from proof: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            println!("✗ Local proof verification FAILED: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("✗ Failed to extract verification key from Sindri response: {e}");
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to extract SP1 proof from Sindri response: {e}");
        }
    }
}

/// Core proving function that handles Sindri circuit proving without database dependencies
///
/// Returns the proof info and computed values on success
#[allow(clippy::future_not_send)]
async fn prove_via_sindri_core(a: i32, b: i32, result: i32) -> Option<sindri::ProofInfo> {
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

    println!("✓ Proof job submitted. Status: {:?}", proof_info.status);
    Some(proof_info);

    println!("\n🔗 PROOF ID FOR EXTERNAL VERIFICATION:");
    println!("   {}", proof_id);
    println!("\n📋 To verify this proof externally, use:");
    println!(
        "   cargo run --release -- --verify --proof-id {} --result {}",
        proof_id, result
    );
}

#[allow(clippy::future_not_send)]
async fn run_prove_via_sindri(pool: &PgPool, arg_a: i32, arg_b: i32, arg_result: i32) {
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
    let proof_info = match prove_via_sindri_core(a, b, result).await {
        Some(info) => info,
        None => return, // Error already printed in core function
    };

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

async fn run_prove_via_sindri_no_db(arg_a: i32, arg_b: i32, arg_result: i32) {
    // Calculate result from inputs (no database lookup needed)
    // For database-free mode, we always calculate from provided inputs
    if arg_result != 20 {
        println!("⚠️  Database-free mode: Using provided inputs and ignoring --result parameter");
    }
    let result = arithmetic_lib::addition(arg_a, arg_b);
    let (a, b) = (arg_a, arg_b);

    println!("Database-free mode:");
    
    // Use the common proving core
    let proof_info = match prove_via_sindri_core(a, b, result).await {
        Some(info) => info,
        None => return, // Error already printed in core function
    };

    println!("ℹ️  Note: Proof metadata not stored (database-free mode)");
}
