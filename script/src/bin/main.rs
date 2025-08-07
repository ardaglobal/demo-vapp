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

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    if args.execute {
        run_interactive_execute(&client, &pool).await;
        // This is now handled by run_interactive_execute
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
    }
}

async fn run_interactive_execute(client: &sp1_sdk::EnvProver, pool: &sqlx::PgPool) {
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

async fn run_verify_mode(pool: &sqlx::PgPool, result: i32) {
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

            verify_result(pool, lookup_result).await;
            println!();
        }
    } else {
        // Single verify mode
        verify_result(pool, result).await;
    }
}

async fn verify_result(pool: &sqlx::PgPool, result: i32) {
    println!("Looking for transactions with result: {result}");

    match get_value_by_result(pool, result).await {
        Ok(Some((a, b))) => {
            println!("✓ Found in database: {a} + {b} = {result}");
        }
        Ok(None) => {
            println!("✗ No transactions found with result = {result}");
        }
        Err(e) => {
            println!("✗ Database error: {e}");
        }
    }
}
