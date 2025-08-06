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
use arithmetic_db::db::{get_value_by_result, init_db, store_arithmetic_transaction};
use arithmetic_lib::PublicValuesStruct;
use clap::Parser;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};

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

    if args.verify {
        // Verify mode is separate from execute/prove
    } else if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();
    let pool = init_db().await.expect("Failed to initialize database");

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    if args.execute {
        stdin.write(&args.a);
        stdin.write(&args.b);

        println!("a: {}", args.a);
        println!("b: {}", args.b);
        // Execute the program
        let (output, report) = client.execute(ARITHMETIC_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Read the output.
        let decoded = PublicValuesStruct::abi_decode(output.as_slice()).unwrap();
        let PublicValuesStruct { a, b, result } = decoded;
        println!("a: {a}");
        println!("b: {b}");
        println!("result: {result}");

        let expected_result = arithmetic_lib::addition(a, b);
        assert_eq!(result, expected_result);
        println!("Values are correct!");

        println!("Storing in database");
        match store_arithmetic_transaction(&pool, a, b, result).await {
            Ok(()) => {
                println!("Stored in database successfully");

                // Test immediate retrieval in the same process
                println!("Testing immediate retrieval...");
                match get_value_by_result(&pool, result).await {
                    Ok(Some((retrieved_a, retrieved_b))) => {
                        println!(
                            "✓ Successfully retrieved: a = {retrieved_a}, b = {retrieved_b} for result = {result}"
                        );
                    }
                    Ok(None) => {
                        println!("✗ Failed to retrieve stored data immediately");
                    }
                    Err(e) => {
                        println!("✗ Database error during retrieval: {e}");
                    }
                }
            }
            Err(e) => {
                println!("✗ Failed to store in database: {e}");
            }
        }

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else if args.prove {
        stdin.write(&args.a);
        stdin.write(&args.b);

        // Setup the program for proving.
        let (pk, vk) = client.setup(ARITHMETIC_ELF);

        // Generate the proof
        let proof = client
            .prove(&pk, &stdin)
            .run()
            .expect("failed to generate proof");

        println!("Successfully generated proof!");

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    } else if args.verify {
        println!("Looking for transactions with result: {}", args.result);
        println!("Database initialized, attempting to get value...");
        match get_value_by_result(&pool, args.result).await {
            Ok(Some((a, b))) => {
                println!(
                    "Retrieved from database for result = {}: a = {}, b = {}",
                    args.result, a, b
                );
            }
            Ok(None) => {
                println!("No value found in database for result = {}", args.result);
            }
            Err(e) => {
                println!("Database error: {e}");
            }
        }
    }
}
