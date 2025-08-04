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
use arithmetic_db::db::{create_simple_task_with_addition, get_value, init_db, update_db};
use arithmetic_lib::PublicValuesStruct;
use clap::Parser;
use parking_lot::RwLock;
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
    a: u64,
    #[arg(long, default_value = "1")]
    b: u64,

    #[arg(long)]
    verify: bool,

    #[arg(long, default_value = "20")]
    result: u32,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();
    let mut ads = init_db();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    stdin.write(&args.a);
    stdin.write(&args.b);

    println!("a: {}", args.a);
    println!("b: {}", args.b);

    if args.execute {
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
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&result.to_le_bytes());
        let task = create_simple_task_with_addition(result.to_string().as_bytes(), &bytes);
        let task_with_lock = RwLock::new(Some(task));
        update_db(&mut ads, &[task_with_lock], 0);
        println!("Stored in database");

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else if args.prove {
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
        let key_string = args.result.to_string();
        let key = key_string.as_bytes();
        match get_value(&ads, key) {
            Some(value) => {
                if value.len() >= 8 {
                    let a = u32::from_le_bytes([value[0], value[1], value[2], value[3]]);
                    let b = u32::from_le_bytes([value[4], value[5], value[6], value[7]]);
                    println!(
                        "Retrieved from database for result = {}: a = {}, b = {}",
                        args.result, a, b
                    );
                } else {
                    println!(
                        "Value found in database for result = {}, but data is incomplete.",
                        args.result
                    );
                }
            }
            None => {
                println!("No value found in database for result = {}", args.result);
            }
        }
    }
}
