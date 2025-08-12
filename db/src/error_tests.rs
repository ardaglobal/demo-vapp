use crate::db::init_db_with_url;

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_invalid_database_url() {
        let result = init_db_with_url("invalid_url").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[traced_test]
    async fn test_connection_failure() {
        let result = init_db_with_url("postgres://user:password@nonexistent:5432/db").await;
        assert!(result.is_err());
    }
}
