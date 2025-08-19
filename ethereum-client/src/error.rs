use thiserror::Error;

pub type Result<T> = std::result::Result<T, EthereumError>;

#[derive(Error, Debug)]
pub enum EthereumError {
    #[error("Provider error: {0}")]
    Provider(#[from] alloy_transport::TransportError),

    #[error("RPC error: {0}")]
    Rpc(#[from] alloy_json_rpc::RpcError<alloy_transport::TransportError>),

    #[error("Contract error: {0}")]
    Contract(String),

    #[error("Signer error: {0}")]
    Signer(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Invalid transaction hash: {0}")]
    InvalidTransactionHash(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Proof verification failed: {0}")]
    ProofVerificationFailed(String),

    #[error("State not found: {0}")]
    StateNotFound(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Alchemy API error: {status_code} - {message}")]
    AlchemyApi { status_code: u16, message: String },

    #[cfg(feature = "database")]
    #[error("Database error: {0}")]
    DatabaseSqlx(#[from] sqlx::Error),

    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    #[error("Array conversion error: {0}")]
    ArrayConversion(#[from] std::array::TryFromSliceError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("General error: {0}")]
    General(#[from] eyre::Error),

    #[error("External service error: {0}")]
    External(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[cfg(feature = "database")]
    #[error("Database error: {0}")]
    Database(String),

    #[error("Sindri error: {0}")]
    Sindri(String),

    // Custom contract errors
    #[error("Unauthorized access: The caller is not authorized to perform this operation")]
    UnauthorizedAccess,

    #[error("Invalid array length: Input arrays have mismatched lengths")]
    InvalidArrayLength,

    #[error("State not found: The requested state does not exist")]
    ContractStateNotFound,

    #[error("Proof not found: The requested proof does not exist")]
    ContractProofNotFound,

    #[error("Invalid limit: The provided limit parameter is invalid")]
    InvalidLimit,

    #[error("Invalid index: The provided index is out of bounds")]
    InvalidIndex,

    #[error("Proof already exists: A proof with this ID already exists")]
    ProofAlreadyExists,
}

impl EthereumError {
    /// Parse contract revert error and convert to specific custom error if recognized
    pub fn from_contract_error(error_msg: &str) -> Self {
        // Look for revert error signature in the message
        if let Some(sig_start) = error_msg.find("0x") {
            let sig_end = sig_start + 10; // "0x" + 8 hex characters
            if sig_end <= error_msg.len() {
                let signature = &error_msg[sig_start..sig_end];
                match signature {
                    "0x344fd586" => return EthereumError::UnauthorizedAccess,
                    "0x9d89020a" => return EthereumError::InvalidArrayLength,
                    "0xfa8d84c7" => return EthereumError::ContractStateNotFound,
                    "0x36131e57" => return EthereumError::ContractProofNotFound,
                    "0xe55fb509" => return EthereumError::InvalidLimit,
                    "0x63df8171" => return EthereumError::InvalidIndex,
                    "0xb8cdb9bd" => return EthereumError::ProofAlreadyExists,
                    _ => {} // Unknown signature, fall through to generic error
                }
            }
        }
        
        // If no signature match, return generic contract error
        EthereumError::Contract(error_msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unauthorized_access_error() {
        let error_msg = "Transaction failed: 0x344fd586";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::UnauthorizedAccess => {},
            _ => panic!("Expected UnauthorizedAccess error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_invalid_array_length_error() {
        let error_msg = "Batch transaction failed: execution reverted with custom error 0x9d89020a";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::InvalidArrayLength => {},
            _ => panic!("Expected InvalidArrayLength error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_state_not_found_error() {
        let error_msg = "State query failed: revert 0xfa8d84c7";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::ContractStateNotFound => {},
            _ => panic!("Expected ContractStateNotFound error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_proof_not_found_error() {
        let error_msg = "Proof data query failed: 0x36131e57 ProofNotFound()";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::ContractProofNotFound => {},
            _ => panic!("Expected ContractProofNotFound error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_invalid_limit_error() {
        let error_msg = "Contract call failed: 0xe55fb509";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::InvalidLimit => {},
            _ => panic!("Expected InvalidLimit error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_invalid_index_error() {
        let error_msg = "Array access failed: 0x63df8171";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::InvalidIndex => {},
            _ => panic!("Expected InvalidIndex error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_proof_already_exists_error() {
        let error_msg = "Transaction failed: execution reverted 0xb8cdb9bd";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::ProofAlreadyExists => {},
            _ => panic!("Expected ProofAlreadyExists error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_unknown_error_signature() {
        let error_msg = "Transaction failed: 0x12345678 unknown error";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::Contract(msg) => {
                assert_eq!(msg, "Transaction failed: 0x12345678 unknown error");
            },
            _ => panic!("Expected Contract error for unknown signature, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_error_without_signature() {
        let error_msg = "Transaction failed: out of gas";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::Contract(msg) => {
                assert_eq!(msg, "Transaction failed: out of gas");
            },
            _ => panic!("Expected Contract error for message without signature, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_malformed_signature() {
        let error_msg = "Transaction failed: 0x123"; // Too short
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::Contract(msg) => {
                assert_eq!(msg, "Transaction failed: 0x123");
            },
            _ => panic!("Expected Contract error for malformed signature, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_signature_at_end_of_message() {
        let error_msg = "Contract execution reverted with error 0x344fd586";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::UnauthorizedAccess => {},
            _ => panic!("Expected UnauthorizedAccess error, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_parse_multiple_signatures_uses_first() {
        let error_msg = "Error 0x344fd586 followed by 0x9d89020a";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        // Should match the first signature (UnauthorizedAccess)
        match parsed_error {
            EthereumError::UnauthorizedAccess => {},
            _ => panic!("Expected UnauthorizedAccess error (first signature), got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_error_message_display() {
        // Test that custom errors display meaningful messages
        let unauthorized = EthereumError::UnauthorizedAccess;
        assert_eq!(unauthorized.to_string(), "Unauthorized access: The caller is not authorized to perform this operation");

        let invalid_array = EthereumError::InvalidArrayLength;
        assert_eq!(invalid_array.to_string(), "Invalid array length: Input arrays have mismatched lengths");

        let state_not_found = EthereumError::ContractStateNotFound;
        assert_eq!(state_not_found.to_string(), "State not found: The requested state does not exist");

        let proof_not_found = EthereumError::ContractProofNotFound;
        assert_eq!(proof_not_found.to_string(), "Proof not found: The requested proof does not exist");

        let invalid_limit = EthereumError::InvalidLimit;
        assert_eq!(invalid_limit.to_string(), "Invalid limit: The provided limit parameter is invalid");

        let invalid_index = EthereumError::InvalidIndex;
        assert_eq!(invalid_index.to_string(), "Invalid index: The provided index is out of bounds");

        let proof_exists = EthereumError::ProofAlreadyExists;
        assert_eq!(proof_exists.to_string(), "Proof already exists: A proof with this ID already exists");
    }

    #[test]
    fn test_case_insensitive_signature_parsing() {
        // Test with uppercase hex
        let error_msg = "Transaction failed: 0X344FD586";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        // Should still return generic contract error since we're looking for lowercase
        match parsed_error {
            EthereumError::Contract(_) => {},
            _ => panic!("Expected Contract error for uppercase signature, got: {:?}", parsed_error),
        }
    }

    #[test]
    fn test_signature_with_extra_data() {
        // Test signature with additional data after it
        let error_msg = "Transaction failed: 0x344fd586000000000000000000000000";
        let parsed_error = EthereumError::from_contract_error(error_msg);
        
        match parsed_error {
            EthereumError::UnauthorizedAccess => {},
            _ => panic!("Expected UnauthorizedAccess error, got: {:?}", parsed_error),
        }
    }
}
