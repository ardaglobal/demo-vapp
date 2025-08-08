use alloy_sol_types::sol;

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    /// In true zero-knowledge fashion, only the result is public - the inputs a and b remain private.
    struct PublicValuesStruct {
        int32 result;
    }
}

/// Compute the result of the arithmetic operation (wrapping around on overflows), using normal Rust code.
#[must_use]
pub const fn addition(a: i32, b: i32) -> i32 {
    a + b
}
