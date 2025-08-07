use crate::db::init_db;
use std::env;

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_invalid_database_url() {
        env::set_var("DATABASE_URL", "invalid_url");
        
        let result = init_db().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_connection_failure() {
        env::set_var("DATABASE_URL", "postgres://user:password@nonexistent:5432/db");
        
        let result = init_db().await;
        assert!(result.is_err());
    }
}