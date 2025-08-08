use thiserror::Error;

/// Custom error types for the indexed Merkle tree database operations
#[derive(Error, Debug)]
pub enum DbError {
    /// Database connection or query errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Nullifier already exists in the tree
    #[error("Nullifier with value {0} already exists")]
    NullifierExists(i64),

    /// Failed to insert nullifier using atomic procedure
    #[error("Failed to insert nullifier with value {0}")]
    InsertionFailed(i64),

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Invalid hash length (must be 32 bytes)
    #[error("Invalid hash length: expected 32 bytes, got {0}")]
    InvalidHashLength(usize),

    /// Chain validation failed
    #[error("Nullifier chain validation failed - linked list integrity compromised")]
    ChainValidationFailed,

    /// Invalid tree parameters
    #[error("Invalid tree parameter: {0}")]
    InvalidTreeParameter(String),

    /// Tree is full (reached maximum capacity)
    #[error("Tree is full: cannot insert more nullifiers")]
    TreeFull,

    /// Invalid nullifier value
    #[error("Invalid nullifier value: {0}")]
    InvalidNullifierValue(String),

    /// Transaction failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    /// Connection pool error
    #[error("Connection pool error: {0}")]
    PoolError(String),

    /// Migration error
    #[error("Migration error: {0}")]
    MigrationError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type alias for database operations
pub type DbResult<T> = Result<T, DbError>;

impl DbError {
    /// Check if the error is recoverable (can retry the operation)
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        match self {
            Self::Database(sqlx_error) => {
                matches!(
                    sqlx_error,
                    sqlx::Error::Io(_) | 
                    sqlx::Error::PoolTimedOut |
                    sqlx::Error::PoolClosed
                )
            }
            Self::PoolError(_) | Self::TransactionFailed(_) => true,
            _ => false,
        }
    }

    /// Check if the error indicates a constraint violation
    #[must_use]
    pub fn is_constraint_violation(&self) -> bool {
        match self {
            Self::Database(sqlx::Error::Database(db_err)) => {
                db_err.constraint().is_some()
            }
            Self::NullifierExists(_) | Self::ChainValidationFailed => true,
            _ => false,
        }
    }

    /// Get error code for logging and monitoring
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::Database(_) => "DB_ERROR",
            Self::NullifierExists(_) => "NULLIFIER_EXISTS",
            Self::InsertionFailed(_) => "INSERTION_FAILED",
            Self::NotFound(_) => "NOT_FOUND",
            Self::InvalidHashLength(_) => "INVALID_HASH_LENGTH",
            Self::ChainValidationFailed => "CHAIN_VALIDATION_FAILED",
            Self::InvalidTreeParameter(_) => "INVALID_TREE_PARAMETER",
            Self::TreeFull => "TREE_FULL",
            Self::InvalidNullifierValue(_) => "INVALID_NULLIFIER_VALUE",
            Self::TransactionFailed(_) => "TRANSACTION_FAILED",
            Self::PoolError(_) => "POOL_ERROR",
            Self::MigrationError(_) => "MIGRATION_ERROR",
            Self::ConfigError(_) => "CONFIG_ERROR",
        }
    }
}

/// Convert database URL parse errors to DbError
impl From<url::ParseError> for DbError {
    fn from(err: url::ParseError) -> Self {
Self::ConfigError(format!("Invalid database URL: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(DbError::NullifierExists(123).error_code(), "NULLIFIER_EXISTS");
        assert_eq!(DbError::ChainValidationFailed.error_code(), "CHAIN_VALIDATION_FAILED");
        assert_eq!(DbError::TreeFull.error_code(), "TREE_FULL");
    }

    #[test]
    fn test_recoverable_errors() {
        assert!(!DbError::NullifierExists(123).is_recoverable());
        assert!(!DbError::ChainValidationFailed.is_recoverable());
        assert!(DbError::PoolError("connection timeout".to_string()).is_recoverable());
        assert!(DbError::TransactionFailed("deadlock".to_string()).is_recoverable());
    }

    #[test]
    fn test_constraint_violations() {
        assert!(DbError::NullifierExists(123).is_constraint_violation());
        assert!(DbError::ChainValidationFailed.is_constraint_violation());
        assert!(!DbError::NotFound("test".to_string()).is_constraint_violation());
    }
}