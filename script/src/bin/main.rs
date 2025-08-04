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
use clap::Parser;
use fibonacci_db::db::{create_simple_task_with_addition, get_value, update_db};
use fibonacci_lib::PublicValuesStruct;
use parking_lot::lock_api::RwLock;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const FIBONACCI_ELF: &[u8] = include_elf!("fibonacci-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    #[arg(long)]
    verify: bool,

    #[arg(long, default_value = "20")]
    n: u32,
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
    let mut ads = fibonacci_db::db::init_db();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    stdin.write(&args.n);

    println!("n: {}", args.n);

    if args.execute {
        // Execute the program
        let (output, report) = client.execute(FIBONACCI_ELF, &stdin).run().unwrap();
        println!("Program executed successfully.");

        // Read the output.
        let decoded = PublicValuesStruct::abi_decode(output.as_slice()).unwrap();
        let PublicValuesStruct { n, a, b } = decoded;
        println!("n: {n}");
        println!("a: {a}");
        println!("b: {b}");

        let (expected_a, expected_b) = fibonacci_lib::fibonacci(n);
        assert_eq!(a, expected_a);
        assert_eq!(b, expected_b);
        println!("Values are correct!");

        println!("Storing in database");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&expected_a.to_le_bytes());
        bytes.extend_from_slice(&expected_b.to_le_bytes());
        let task = create_simple_task_with_addition(n.to_string().as_bytes(), &bytes);
        let task_with_lock = RwLock::new(Some(task));
        update_db(&mut ads, &[task_with_lock], 0);
        println!("Stored in database");

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else if args.prove {
        // Setup the program for proving.
        let (pk, vk) = client.setup(FIBONACCI_ELF);

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
        let key_string = args.n.to_string();
        let key = key_string.as_bytes();
        match get_value(&ads, key) {
            Some(value) => {
                if value.len() >= 8 {
                    let a = u32::from_le_bytes([value[0], value[1], value[2], value[3]]);
                    let b = u32::from_le_bytes([value[4], value[5], value[6], value[7]]);
                    println!(
                        "Retrieved from database for n = {}: a = {}, b = {}",
                        args.n, a, b
                    );
                } else {
                    println!(
                        "Value found in database for n = {}, but data is incomplete.",
                        args.n
                    );
                }
            }
            None => {
                println!("No value found in database for n = {}", args.n);
            }
        }
    }
}
