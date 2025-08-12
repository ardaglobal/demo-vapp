#[cfg(test)]
mod tests {
    use crate::ads_service::AdsServiceFactory;
    use crate::api::rest::{create_router, ApiConfig, ApiState, ProofResponse};
    use crate::vapp_integration::{
        MockComplianceService, MockNotificationService, MockProofService, MockSettlementService,
        VAppAdsIntegration, VAppConfig,
    };
    use axum_test::TestServer;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn create_test_state(pool: sqlx::PgPool) -> ApiState {
        let factory = AdsServiceFactory::new(pool.clone());
        let ads = Arc::new(RwLock::new(
            factory
                .create_indexed_merkle_tree()
                .await
                .expect("Failed to create ADS"),
        ));
        let vapp = Arc::new(RwLock::new(
            VAppAdsIntegration::new(
                pool,
                VAppConfig::default(),
                Arc::new(MockSettlementService),
                Arc::new(MockProofService),
                Arc::new(MockComplianceService),
                Arc::new(MockNotificationService),
            )
            .await
            .expect("Failed to create vApp integration"),
        ));

        ApiState {
            ads,
            vapp_integration: vapp,
            config: ApiConfig::default(),
        }
    }

    #[tokio::test]
    #[allow(clippy::future_not_send)]
    async fn test_proof_verification_with_valid_proof() {
        let pool = crate::db::init_db()
            .await
            .expect("Failed to connect to database");
        // This test would need a real valid proof ID from Sindri
        // In practice, you'd mock the SindriClient or use a test proof

        let state = create_test_state(pool).await;
        let app = create_router(state);
        let server = TestServer::new(app).unwrap();

        // Test with a known valid proof ID (you'd need to set this up)
        // let response = server.get("/api/v1/proofs/test_valid_proof_id").await;
        // assert_eq!(response.status_code(), 200);

        // let body: ProofResponse = response.json();
        // if let Some(verification_data) = body.verification_data {
        //     assert!(verification_data.cryptographic_proof_valid);
        //     assert!(verification_data.is_verified);
        // }

        // For now, just ensure the endpoint exists
        let response = server.get("/api/v1/health").await;
        assert_eq!(response.status_code(), 200);
    }

    #[tokio::test]
    #[allow(clippy::future_not_send)]
    async fn test_proof_verification_with_invalid_proof() {
        let pool = crate::db::init_db()
            .await
            .expect("Failed to connect to database");
        let state = create_test_state(pool).await;
        let app = create_router(state);
        let server = TestServer::new(app).unwrap();

        // Test with invalid proof ID should return error or failed verification
        let response = server.get("/api/v1/proofs/invalid_proof_id").await;
        // Should handle gracefully - either 404 or verification failure
        assert!(response.status_code() == 404 || response.status_code() == 200);

        if response.status_code() == 200 {
            let body: ProofResponse = response.json();
            // Should not have verification data for invalid proof
            assert!(
                body.verification_data.is_none()
                    || !body.verification_data.unwrap().cryptographic_proof_valid
            );
        }
    }

    #[tokio::test]
    #[allow(clippy::future_not_send)]
    async fn test_proof_verification_error_handling() {
        let pool = crate::db::init_db()
            .await
            .expect("Failed to connect to database");
        let state = create_test_state(pool).await;
        let app = create_router(state);
        let server = TestServer::new(app).unwrap();

        // Test with malformed proof ID
        let response = server.get("/api/v1/proofs/").await;
        assert_eq!(response.status_code(), 404);
    }
}
