//! A continuous balance tracking program that takes an initial balance and a list of addition
//! transactions as input, processes all transactions in sequence, and commits the initial and
//! final balances as public values while keeping individual transactions private.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolType;
use arithmetic_lib::{process_transactions, PublicValuesStruct};

pub fn main() {
    // Read the initial balance from the prover.
    let initial_balance = sp1_zkvm::io::read::<i32>();

    // Read the list of transactions from the prover.
    let transactions = sp1_zkvm::io::read::<Vec<i32>>();

    // Process all transactions in sequence starting from the initial balance.
    // Each transaction is added to the running balance, but the individual transaction
    // amounts remain private within the zkVM execution.
    let final_balance = process_transactions(initial_balance, &transactions);

    // Encode the public values of the program.
    // In true zero-knowledge fashion, we only commit the initial and final balances as public.
    // The individual transaction amounts remain private within the zkVM execution.
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct {
        initial_balance,
        final_balance,
    });

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to.
    sp1_zkvm::io::commit_slice(&bytes);
}
