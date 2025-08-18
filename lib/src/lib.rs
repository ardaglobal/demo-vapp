use alloy_sol_types::sol;

// Proof module only available for host-side operations
#[cfg(feature = "sp1")]
pub mod proof;

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    /// In true zero-knowledge fashion, only the initial and final balances are public.
    /// The individual transactions remain private within the zkVM execution.
    struct PublicValuesStruct {
        int32 initial_balance;
        int32 final_balance;
    }
}

/// Compute the result of the arithmetic operation (wrapping around on overflows), using normal Rust code.
#[must_use]
pub const fn addition(a: i32, b: i32) -> i32 {
    a + b
}

/// Process a series of addition transactions starting from an initial balance.
/// 
/// This function applies each transaction in sequence and returns the final balance.
/// All intermediate steps remain private - only initial and final balances are exposed.
#[must_use]
pub fn process_transactions(initial_balance: i32, transactions: &[i32]) -> i32 {
    let mut balance = initial_balance;
    for &transaction in transactions {
        balance = addition(balance, transaction);
    }
    balance
}
