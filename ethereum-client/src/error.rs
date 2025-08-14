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
    AlchemyApi {
        status_code: u16,
        message: String,
    },
    
    #[cfg(feature = "database")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),
    
    #[error("Array conversion error: {0}")]
    ArrayConversion(#[from] std::array::TryFromSliceError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("General error: {0}")]
    General(#[from] eyre::Error),
}