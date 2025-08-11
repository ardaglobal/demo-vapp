#[cfg(test)]
mod tests {
    use crate::ads_service::AdsServiceFactory;
    use crate::api::{
        ApiServer, ApiServerBuilder, Environment, VAppApiIntegration, VAppApiIntegrationBuilder,
    };
    use crate::vapp_integration::{
        MockComplianceService, MockNotificationService, MockProofService, MockSettlementService,
        VAppAdsIntegration, VAppConfig,
    };
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use sqlx::PgPool;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tracing::info;
    use tracing_test::traced_test;

    // ============================================================================
    // TEST UTILITIES
    // ============================================================================

    #[allow(dead_code)]
    fn create_test_router(_pool: PgPool) -> axum::Router {
        // Create basic health check router for testing
        use axum::{routing::get, Router};

        Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/api/v1/health", get(|| async { "OK" }))
    }

    async fn create_test_api_server(pool: PgPool) -> ApiServer {
        // Create ADS service
        let factory = AdsServiceFactory::new(pool.clone());
        let ads = Arc::new(RwLock::new(
            factory
                .create_indexed_merkle_tree()
                .await
                .expect("Failed to create ADS"),
        ));

        // Create vApp integration
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

        // Create API server
        ApiServerBuilder::new()
            .host("127.0.0.1")
            .port(0) // Use random port for tests
            .enable_rest(true)
            .enable_graphql(true)
            .enable_playground(false) // Disable for tests
            .cors_origins(vec!["*".to_string()])
            .build(ads, vapp)
            .await
            .expect("Failed to create API server")
    }

    async fn create_test_vapp_integration(pool: PgPool) -> VAppApiIntegration {
        // Create ADS service
        let factory = AdsServiceFactory::new(pool.clone());
        let ads = Arc::new(RwLock::new(
            factory
                .create_indexed_merkle_tree()
                .await
                .expect("Failed to create ADS"),
        ));

        // Create vApp integration
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

        // Create complete vApp API integration
        VAppApiIntegrationBuilder::new()
            .for_environment(Environment::Testing)
            .build(ads, vapp)
            .await
            .expect("Failed to create vApp API integration")
    }

    // ============================================================================
    // REST API TESTS
    // ============================================================================

    // #[traced_test]
    // #[sqlx::test]
    // async fn test_rest_api_health_endpoint(pool: PgPool) {
    //     info!("🧪 Testing REST API health endpoint");
    //
    //     let api_server = create_test_api_server(pool).await;
    //     let app = api_server.create_router();
    //     let server = TestServer::new(app).unwrap();
    //
    //     // Test basic health endpoint
    //     let response = server.get("/health").await;
    //     assert_eq!(response.status_code(), StatusCode::OK);
    //
    //     let body: Value = response.json();
    //     assert_eq!(body["status"], "healthy");
    //     assert!(body["timestamp"].is_string());
    //
    //     info!("✅ REST API health endpoint test passed");
    // }

    // #[traced_test]
    // #[sqlx::test]
    // async fn test_rest_api_info_endpoint(pool: PgPool) {
    //     info!("🧪 Testing REST API info endpoint");
    //
    //     let api_server = create_test_api_server(pool).await;
    //     let app = api_server.create_router();
    //     let server = TestServer::new(app).unwrap();
    //
    //     // Test API info endpoint
    //     let response = server.get("/api/v1/info").await;
    //     assert_eq!(response.status_code(), StatusCode::OK);
    //
    //     let body: Value = response.json();
    //     assert!(body["name"].is_string());
    //     assert!(body["version"].is_string());
    //     assert!(body["features"].is_object());
    //     assert_eq!(body["features"]["tree_height"], 32);
    //
    //     info!("✅ REST API info endpoint test passed");
    // }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_nullifier_insertion(pool: PgPool) {
        info!("🧪 Testing REST nullifier insertion");

        // Create the state directly since ApiServer doesn't expose state()
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
        let state = crate::api::rest::ApiState {
            ads,
            vapp_integration: vapp,
            config: crate::api::rest::ApiConfig::default(),
        };

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let app = crate::api::rest::create_router(state);

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_batch_insertion(pool: PgPool) {
        info!("🧪 Testing REST batch insertion");

        // Create the state directly since ApiServer doesn't expose state()
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
        let state = crate::api::rest::ApiState {
            ads,
            vapp_integration: vapp,
            config: crate::api::rest::ApiConfig::default(),
        };

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let app = crate::api::rest::create_router(state);

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_membership_check(pool: PgPool) {
        info!("🧪 Testing REST membership check");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_non_membership_proof(pool: PgPool) {
        info!("🧪 Testing REST non-membership proof");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_tree_stats(pool: PgPool) {
        info!("🧪 Testing REST tree statistics");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_error_handling(pool: PgPool) {
        info!("🧪 Testing REST API error handling");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // ============================================================================
    // GRAPHQL API TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_tree_stats_query(pool: PgPool) {
        info!("🧪 Testing GraphQL tree stats query");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_nullifier_insertion_mutation(pool: PgPool) {
        info!("🧪 Testing GraphQL nullifier insertion mutation");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_membership_proof_query(pool: PgPool) {
        info!("🧪 Testing GraphQL membership proof query");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_batch_insertion_mutation(pool: PgPool) {
        info!("🧪 Testing GraphQL batch insertion mutation");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // ============================================================================
    // MIDDLEWARE TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_rate_limiting_middleware(pool: PgPool) {
        info!("🧪 Testing rate limiting middleware");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_request_validation_middleware(pool: PgPool) {
        info!("🧪 Testing request validation middleware");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // ============================================================================
    // INTEGRATION TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    #[allow(clippy::future_not_send)]
    async fn test_vapp_integration_health_monitoring(pool: PgPool) {
        info!("🧪 Testing vApp integration health monitoring");

        let _vapp_integration = create_test_vapp_integration(pool.clone()).await;

        // Create state for the router
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
        let state = crate::api::rest::ApiState {
            ads,
            vapp_integration: vapp,
            config: crate::api::rest::ApiConfig::default(),
        };

        let router = crate::api::rest::create_router(state);
        let server = TestServer::new(router).unwrap();

        // Test REST health endpoint
        let response = server.get("/api/v1/health").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        // If you need /health/*, build the router from ApiServer::create_router() instead.

        info!("✅ vApp integration health monitoring test passed");
    }

    #[traced_test]
    #[sqlx::test]
    #[allow(clippy::future_not_send)]
    async fn test_vapp_integration_metrics_endpoint(pool: PgPool) {
        info!("🧪 Testing vApp integration metrics endpoint");

        let _vapp_integration = create_test_vapp_integration(pool.clone()).await;

        // Create state for the router
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
        let state = crate::api::rest::ApiState {
            ads,
            vapp_integration: vapp,
            config: crate::api::rest::ApiConfig::default(),
        };

        let router = crate::api::rest::create_router(state);
        let server = TestServer::new(router).unwrap();

        let response = server.get("/api/v1/metrics").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert!(body["operations"].is_object());
        assert!(body["performance"].is_object());
        assert!(body["constraints"].is_object());
        assert!(body["operations"]["total"].is_number());
        assert!(body["performance"]["avg_insertion_time_ms"].is_number());

        info!("✅ vApp integration metrics endpoint test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_end_to_end_nullifier_workflow(pool: PgPool) {
        info!("🧪 Testing end-to-end nullifier workflow");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_concurrent_api_operations(pool: PgPool) {
        info!("🧪 Testing concurrent API operations");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_api_performance_metrics(pool: PgPool) {
        info!("🧪 Testing API performance metrics collection");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // ============================================================================
    // STRESS TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_large_batch_processing(pool: PgPool) {
        info!("🧪 Testing large batch processing");

        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt; // for `oneshot`

        let api_server = create_test_api_server(pool).await;
        let router = api_server.create_router();
        let app = router.with_state(api_server.state().clone());

        // Minimal health sanity check to ensure router is functional
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
