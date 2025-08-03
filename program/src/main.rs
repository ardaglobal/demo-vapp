//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolType;
use fibonacci_lib::{fibonacci, PublicValuesStruct};
use parking_lot::RwLock;

const KEY: &[u8] = b"key";

/// # Panics
/// Panics if the database cannot be initialized
pub fn main() {
    // Read an input to the program.
    //
    // Behind the scenes, this compiles down to a custom system call which handles reading inputs
    // from the prover.
    let n = sp1_zkvm::io::read::<u32>();

    // Initialize the database.
    let mut ads = fibonacci_db::db::init_db();

    // Compute the n'th fibonacci number using a function from the workspace lib crate.
    let (a, b) = fibonacci(n);

    // Encode the public values of the program.
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct { n, a, b });

    // Create the task to update the database.
    let task = fibonacci_db::db::create_simple_task_with_addition(KEY, &bytes);
    let task_with_lock = RwLock::new(Some(task));

    fibonacci_db::db::update_db(&mut ads, &[task_with_lock], 0);

    // Now read from the database
    if let Some(retrieved_value) = fibonacci_db::db::get_value(&ads, KEY) {
        println!("Retrieved value: {retrieved_value:?}");
        assert_eq!(retrieved_value, bytes);
    } else {
        println!("Key not found!");
    }

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to.
    sp1_zkvm::io::commit_slice(&bytes);
}
