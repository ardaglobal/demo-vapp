use alloy_sol_types::sol;

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        uint32 a;
        uint32 b;
        uint32 result;
    }
}

/// Compute the result of the arithmetic operation (wrapping around on overflows), using normal Rust code.
#[must_use]
pub const fn addition(a: u32, b: u32) -> u32 {
    a + b
}
