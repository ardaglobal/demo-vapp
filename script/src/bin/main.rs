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
use sindri::{client::SindriClient, JobStatus, ProofInput};
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use sqlx::PgPool;
use std::io::{self, Write};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ARITHMETIC_ELF: &[u8] = include_elf!("arithmetic-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long, default_value = "1")]
    a: i32,
    #[arg(long, default_value = "1")]
    b: i32,

    #[arg(long)]
    verify: bool,

    #[arg(long, default_value = "20")]
    result: i32,
}

#[tokio::main]
async fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    // Setup the prover client and database pool.
    let client = ProverClient::from_env();
    let pool = init_db().await.expect("Failed to initialize database");

    if args.verify {
        run_verify_mode(&pool, args.result).await;
        return;
    } else if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    if args.execute {
        run_interactive_execute(&client, &pool).await;
        // This is now handled by run_interactive_execute
    } else if args.prove {
        run_prove_via_sindri(&pool, args.a, args.b, args.result).await;
    }
}

async fn run_interactive_execute(client: &sp1_sdk::EnvProver, pool: &PgPool) {
    println!("=== Interactive Arithmetic Execution ===");
    println!("Enter two numbers to add them together.");
    println!("Results will be stored in the database.");
    println!("Press 'q' + Enter to quit.\n");

    loop {
        // Get input for 'a'
        print!("Enter value for 'a' (or 'q' to quit): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Error reading input. Please try again.");
            continue;
        }

        let input = input.trim();
        if input == "q" || input == "Q" {
            println!("Goodbye!");
            break;
        }

        let a: i32 = if let Ok(num) = input.parse() {
            num
        } else {
            println!("Invalid number '{input}'. Please enter an integer.");
            continue;
        };

        // Get input for 'b'
        print!("Enter value for 'b': ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Error reading input. Please try again.");
            continue;
        }

        let b: i32 = if let Ok(num) = input.trim().parse() {
            num
        } else {
            println!(
                "Invalid number '{}'. Please enter an integer.",
                input.trim()
            );
            continue;
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
                        let PublicValuesStruct {
                            a: out_a,
                            b: out_b,
                            result,
                        } = decoded;
                        println!("✓ Computation successful: {out_a} + {out_b} = {result}");

                        let expected = arithmetic_lib::addition(a, b);
                        if result == expected {
                            println!("✓ Result verified");
                        } else {
                            println!("✗ Result mismatch (expected {expected})");
                            continue;
                        }

                        // Store in database
                        match store_arithmetic_transaction(pool, out_a, out_b, result).await {
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
    println!("=== Verify Mode ===");

    if result == 20 {
        // Default value
        // Interactive verify mode
        println!("Enter a result value to look up in the database.");
        println!("Press 'q' + Enter to quit.\n");

        loop {
            print!("Enter result to verify (or 'q' to quit): ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                println!("Error reading input. Please try again.");
                continue;
            }

            let input = input.trim();
            if input == "q" || input == "Q" {
                println!("Goodbye!");
                break;
            }

            let lookup_result: i32 = if let Ok(num) = input.parse() {
                num
            } else {
                println!("Invalid number '{input}'. Please enter an integer.");
                continue;
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
                        JobStatus::Ready => println!("✓ Proof is VALID for result = {result}"),
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
            println!(
                "✗ No Sindri proof stored for result = {result}. Run --prove to create one."
            );
        }
        Err(e) => println!("✗ Database error: {e}"),
    }
}

#[allow(clippy::future_not_send)]
async fn run_prove_via_sindri(pool: &PgPool, arg_a: i32, arg_b: i32, arg_result: i32) {
    // Prefer proving by result if provided (not default), otherwise use provided a and b
    let (a, b, result) = if arg_result == 20 {
        let result = arithmetic_lib::addition(arg_a, arg_b);
        (arg_a, arg_b, result)
    } else {
        match get_value_by_result(pool, arg_result).await {
            Ok(Some((a, b))) => (a, b, arg_result),
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

    println!("Proving that {a} + {b} = {result} via Sindri...");

    // Create SP1 inputs and serialize for Sindri
    let mut stdin = SP1Stdin::new();
    stdin.write(&a);
    stdin.write(&b);

    let stdin_json = match serde_json::to_string(&stdin) {
        Ok(s) => s,
        Err(e) => {
            println!("✗ Failed to serialize SP1Stdin: {e}");
            return;
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
            return;
        }
    };

    if proof_info.status == JobStatus::Failed {
        println!("✗ Proof generation failed: {:?}", proof_info.error);
        return;
    }

    println!("✓ Proof job submitted. Status: {:?}", proof_info.status);

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
            "✓ Stored Sindri proof metadata for result = {} (proof_id = {:?})",
            result, proof_info.proof_id
        );
    }
}
