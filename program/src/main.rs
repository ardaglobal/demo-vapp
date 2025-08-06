//! A simple program that takes two numbers `a` and `b` as input, and writes the result of the
//! arithmetic operation as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolType;
use arithmetic_lib::{addition, PublicValuesStruct};

pub fn main() {
    // Read an input to the program.
    //
    // Behind the scenes, this compiles down to a custom system call which handles reading inputs
    // from the prover.
    let a = sp1_zkvm::io::read::<i32>();
    let b = sp1_zkvm::io::read::<i32>();

    // Compute the result of the arithmetic operation using a function from the workspace lib crate.
    let result = addition(a, b);

    // Encode the public values of the program.
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct { a, b, result });

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to.
    sp1_zkvm::io::commit_slice(&bytes);
}
