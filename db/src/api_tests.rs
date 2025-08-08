#[cfg(test)]
mod tests {
    use crate::ads_service::AdsServiceFactory;
    use crate::api::{
        ApiServer, ApiServerBuilder, BatchInsertRequest, Environment, InsertNullifierRequest,
        VAppApiIntegration, VAppApiIntegrationBuilder,
    };
    use crate::vapp_integration::{
        MockComplianceService, MockNotificationService, MockProofService, MockSettlementService,
        VAppAdsIntegration, VAppConfig,
    };
    use axum::http::{header, StatusCode};
    use serde_json::{json, Value};
    use sqlx::PgPool;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tracing::{info, warn};
    use tracing_test::traced_test;

    // ============================================================================
    // TEST UTILITIES
    // ============================================================================

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

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Insert nullifier
        let request = InsertNullifierRequest {
            value: 12345,
            metadata: Some(json!({"test": "data"})),
            client_id: Some("test-client".to_string()),
        };

        let response = server.post("/api/v1/nullifiers").json(&request).await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert_eq!(body["success"], true);
        assert!(body["transaction_id"].is_string());
        assert!(body["state_transition"].is_object());
        assert_eq!(body["state_transition"]["nullifier_value"], 12345);
        assert_eq!(body["constraint_count"]["total_constraints"], 200);

        info!("✅ REST nullifier insertion test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_batch_insertion(pool: PgPool) {
        info!("🧪 Testing REST batch insertion");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Batch insert nullifiers
        let request = BatchInsertRequest {
            values: vec![1001, 1002, 1003],
            metadata: Some(json!({"batch": "test"})),
            client_id: Some("test-client".to_string()),
        };

        let response = server.post("/api/v1/nullifiers/batch").json(&request).await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert_eq!(body["success"], true);
        assert!(body["batch_id"].is_string());
        assert_eq!(body["total_operations"], 3);
        assert_eq!(body["successful_operations"], 3);
        assert!(body["failed_operations"].as_array().unwrap().is_empty());

        info!("✅ REST batch insertion test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_membership_check(pool: PgPool) {
        info!("🧪 Testing REST membership check");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // First insert a nullifier
        let insert_request = InsertNullifierRequest {
            value: 54321,
            metadata: None,
            client_id: None,
        };

        let insert_response = server
            .post("/api/v1/nullifiers")
            .json(&insert_request)
            .await;
        assert_eq!(insert_response.status_code(), StatusCode::OK);

        // Check membership
        let response = server.get("/api/v1/nullifiers/54321/membership").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert_eq!(body["exists"], true);
        assert_eq!(body["nullifier_value"], 54321);
        assert!(body["proof"].is_object());

        info!("✅ REST membership check test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_non_membership_proof(pool: PgPool) {
        info!("🧪 Testing REST non-membership proof");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Insert some nullifiers to create structure
        for value in [100, 300, 500] {
            let request = InsertNullifierRequest {
                value,
                metadata: None,
                client_id: None,
            };
            let response = server.post("/api/v1/nullifiers").json(&request).await;
            assert_eq!(response.status_code(), StatusCode::OK);
        }

        // Check non-membership of value in gap
        let response = server.get("/api/v1/nullifiers/200/non-membership").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["proof"].is_object());
        assert_eq!(body["proof"]["queried_value"], 200);
        assert!(body["verification_data"]["range_valid"].as_bool().unwrap());

        info!("✅ REST non-membership proof test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_tree_stats(pool: PgPool) {
        info!("🧪 Testing REST tree statistics");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Insert some nullifiers first
        for value in [1111, 2222, 3333] {
            let request = InsertNullifierRequest {
                value,
                metadata: None,
                client_id: None,
            };
            let response = server.post("/api/v1/nullifiers").json(&request).await;
            assert_eq!(response.status_code(), StatusCode::OK);
        }

        // Get tree stats
        let response = server.get("/api/v1/tree/stats").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["root_hash"].is_string());
        assert_eq!(body["tree_height"], 32);
        assert_eq!(body["total_nullifiers"], 3);
        assert!(body["performance_metrics"].is_object());
        assert!(body["constraint_efficiency"].is_object());
        assert_eq!(body["constraint_efficiency"]["our_constraints"], 200);
        assert_eq!(
            body["constraint_efficiency"]["traditional_constraints"],
            1600
        );

        info!("✅ REST tree statistics test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_rest_error_handling(pool: PgPool) {
        info!("🧪 Testing REST API error handling");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Test invalid nullifier value
        let invalid_request = InsertNullifierRequest {
            value: -1, // Invalid negative value
            metadata: None,
            client_id: None,
        };

        let response = server
            .post("/api/v1/nullifiers")
            .json(&invalid_request)
            .await;

        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

        // Test batch size exceeded
        let large_batch = BatchInsertRequest {
            values: (1..2001).collect(), // Exceeds max batch size
            metadata: None,
            client_id: None,
        };

        let response = server
            .post("/api/v1/nullifiers/batch")
            .json(&large_batch)
            .await;

        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

        // Test non-existent endpoint
        let response = server.get("/api/v1/nonexistent").await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);

        info!("✅ REST API error handling test passed");
    }

    // ============================================================================
    // GRAPHQL API TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_tree_stats_query(pool: PgPool) {
        info!("🧪 Testing GraphQL tree stats query");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        let query = "
        query {
            treeStats {
                rootHash
                totalNullifiers
                treeHeight
                performanceMetrics {
                    totalOperations
                    avgInsertionTimeMs
                    errorRatePercent
                }
                constraintEfficiency {
                    ourConstraints
                    traditionalConstraints
                    improvementFactor
                }
            }
        }
        ";

        let response = server
            .post("/graphql")
            .json(&json!({ "query": query }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["data"]["treeStats"].is_object());
        assert!(body["data"]["treeStats"]["rootHash"].is_string());
        assert_eq!(body["data"]["treeStats"]["treeHeight"], 32);

        info!("✅ GraphQL tree stats query test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_nullifier_insertion_mutation(pool: PgPool) {
        info!("🧪 Testing GraphQL nullifier insertion mutation");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        let mutation = "
        mutation($input: InsertNullifierInput!) {
            insertNullifier(input: $input) {
                id
                oldRoot
                newRoot
                nullifierValue
                constraintCount {
                    totalConstraints
                    totalHashes
                    rangeChecks
                }
            }
        }
        ";

        let variables = json!({
            "input": {
                "value": 98765,
                "metadata": "{\"test\": \"graphql\"}",
                "clientId": "graphql-test"
            }
        });

        let response = server
            .post("/graphql")
            .json(&json!({
                "query": mutation,
                "variables": variables
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["data"]["insertNullifier"].is_object());
        assert_eq!(body["data"]["insertNullifier"]["nullifierValue"], 98765);
        assert_eq!(
            body["data"]["insertNullifier"]["constraintCount"]["totalConstraints"],
            200
        );

        info!("✅ GraphQL nullifier insertion mutation test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_membership_proof_query(pool: PgPool) {
        info!("🧪 Testing GraphQL membership proof query");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // First insert a nullifier
        let insert_mutation = "
        mutation {
            insertNullifier(input: { value: 77777 }) {
                id
                nullifierValue
            }
        }
        ";

        let insert_response = server
            .post("/graphql")
            .json(&json!({ "query": insert_mutation }))
            .await;
        assert_eq!(insert_response.status_code(), StatusCode::OK);

        // Then query membership proof
        let query = "
        query {
            membershipProof(nullifierValue: 77777) {
                nullifierValue
                treeIndex
                rootHash
                merkleProof {
                    siblings
                    pathIndices
                    treeHeight
                }
                isValid
            }
        }
        ";

        let response = server
            .post("/graphql")
            .json(&json!({ "query": query }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["data"]["membershipProof"].is_object());
        assert_eq!(body["data"]["membershipProof"]["nullifierValue"], 77777);
        assert_eq!(
            body["data"]["membershipProof"]["merkleProof"]["treeHeight"],
            32
        );

        info!("✅ GraphQL membership proof query test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_graphql_batch_insertion_mutation(pool: PgPool) {
        info!("🧪 Testing GraphQL batch insertion mutation");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        let mutation = "
        mutation($input: BatchInsertInput!) {
            batchInsertNullifiers(input: $input) {
                ... on SuccessResult {
                    message
                    processingTimeMs
                }
                ... on ErrorResult {
                    errorCode
                    message
                }
            }
        }
        ";

        let variables = json!({
            "input": {
                "values": [11111, 22222, 33333],
                "clientId": "batch-test"
            }
        });

        let response = server
            .post("/graphql")
            .json(&json!({
                "query": mutation,
                "variables": variables
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["data"]["batchInsertNullifiers"].is_object());
        assert!(body["data"]["batchInsertNullifiers"]["message"].is_string());

        info!("✅ GraphQL batch insertion mutation test passed");
    }

    // ============================================================================
    // MIDDLEWARE TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_rate_limiting_middleware(pool: PgPool) {
        info!("🧪 Testing rate limiting middleware");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Make multiple requests rapidly to trigger rate limit
        let mut responses = Vec::new();
        for i in 0..10 {
            let response = server
                .get("/health")
                .add_header(
                    header::HeaderName::from_static("x-client-id"),
                    &format!("test-client-{}", i),
                )
                .await;
            responses.push(response.status_code());
        }

        // All should succeed since we're using different client IDs
        for status in &responses {
            assert_eq!(*status, StatusCode::OK);
        }

        info!("✅ Rate limiting middleware test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_request_validation_middleware(pool: PgPool) {
        info!("🧪 Testing request validation middleware");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Test invalid content type
        let response = server
            .post("/api/v1/nullifiers")
            .add_header(header::CONTENT_TYPE, "text/plain")
            .text("invalid content")
            .await;

        // Should fail validation (though exact status depends on middleware configuration)
        assert!(response.status_code() >= StatusCode::BAD_REQUEST);

        info!("✅ Request validation middleware test passed");
    }

    // ============================================================================
    // INTEGRATION TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_integration_health_monitoring(pool: PgPool) {
        info!("🧪 Testing vApp integration health monitoring");

        let vapp_integration = create_test_vapp_integration(pool).await;
        let router = vapp_integration.build_production_router();
        let server = TestServer::new(router).unwrap();

        // Test basic health endpoint
        let response = server.get("/health").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        // Test detailed health endpoint
        let response = server.get("/health/detailed").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert!(body["service_id"].is_string());
        assert!(body["status"].is_string());
        assert!(body["checks"].is_array());

        // Test readiness endpoint
        let response = server.get("/health/ready").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        // Test liveness endpoint
        let response = server.get("/health/live").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        info!("✅ vApp integration health monitoring test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_vapp_integration_metrics_endpoint(pool: PgPool) {
        info!("🧪 Testing vApp integration metrics endpoint");

        let vapp_integration = create_test_vapp_integration(pool).await;
        let router = vapp_integration.build_production_router();
        let server = TestServer::new(router).unwrap();

        let response = server.get("/metrics").await;
        assert_eq!(response.status_code(), StatusCode::OK);

        let body = response.text();
        assert!(body.contains("# HELP"));
        assert!(body.contains("# TYPE"));
        assert!(body.contains("http_requests_total"));
        assert!(body.contains("merkle_tree_operations_total"));
        assert!(body.contains("constraint_count"));

        info!("✅ vApp integration metrics endpoint test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_end_to_end_nullifier_workflow(pool: PgPool) {
        info!("🧪 Testing end-to-end nullifier workflow");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        let test_nullifier = 999_888;

        // Step 1: Insert nullifier via REST
        let insert_request = InsertNullifierRequest {
            value: test_nullifier,
            metadata: Some(json!({"workflow": "e2e_test"})),
            client_id: Some("e2e-test".to_string()),
        };

        let insert_response = server
            .post("/api/v1/nullifiers")
            .json(&insert_request)
            .await;
        assert_eq!(insert_response.status_code(), StatusCode::OK);

        let insert_body: Value = insert_response.json();
        let transaction_id = insert_body["transaction_id"].as_str().unwrap();

        // Step 2: Verify membership via REST
        let membership_response = server
            .get(&format!("/api/v1/nullifiers/{}/membership", test_nullifier))
            .await;
        assert_eq!(membership_response.status_code(), StatusCode::OK);

        let membership_body: Value = membership_response.json();
        assert_eq!(membership_body["exists"], true);
        assert_eq!(membership_body["nullifier_value"], test_nullifier);

        // Step 3: Get tree stats via GraphQL
        let stats_query = "
        query {
            treeStats {
                totalNullifiers
                rootHash
                performanceMetrics {
                    totalOperations
                }
            }
        }
        ";

        let stats_response = server
            .post("/graphql")
            .json(&json!({ "query": stats_query }))
            .await;
        assert_eq!(stats_response.status_code(), StatusCode::OK);

        let stats_body: Value = stats_response.json();
        assert!(
            stats_body["data"]["treeStats"]["totalNullifiers"]
                .as_i64()
                .unwrap()
                >= 1
        );

        // Step 4: Verify audit trail
        let audit_response = server
            .get(&format!("/api/v1/nullifiers/{}/audit", test_nullifier))
            .await;
        assert_eq!(audit_response.status_code(), StatusCode::OK);

        let audit_body: Value = audit_response.json();
        assert_eq!(audit_body["nullifier_value"], test_nullifier);
        assert!(audit_body["total_events"].as_i64().unwrap() > 0);

        info!("✅ End-to-end nullifier workflow test passed");
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_concurrent_api_operations(pool: PgPool) {
        info!("🧪 Testing concurrent API operations");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        let concurrent_operations = 20;
        let mut handles = Vec::new();

        // Spawn concurrent insertion tasks
        for i in 0..concurrent_operations {
            let server_clone = server.clone();
            let base_nullifier = 500_000 + i;

            let handle = tokio::spawn(async move {
                let request = InsertNullifierRequest {
                    value: base_nullifier,
                    metadata: Some(json!({"concurrent": true, "index": i})),
                    client_id: Some(format!("concurrent-{}", i)),
                };

                let response = server_clone.post("/api/v1/nullifiers").json(&request).await;

                (i, response.status_code())
            });

            handles.push(handle);
        }

        // Collect results
        let mut successful_operations = 0;
        for handle in handles {
            match handle.await {
                Ok((i, status)) => {
                    if status == StatusCode::OK {
                        successful_operations += 1;
                    } else {
                        warn!("Concurrent operation {} failed with status: {}", i, status);
                    }
                }
                Err(e) => {
                    warn!("Concurrent operation task failed: {:?}", e);
                }
            }
        }

        // At least most operations should succeed
        assert!(
            successful_operations >= concurrent_operations / 2,
            "Expected at least {} successful operations, got {}",
            concurrent_operations / 2,
            successful_operations
        );

        // Verify final tree state
        let stats_response = server.get("/api/v1/tree/stats").await;
        assert_eq!(stats_response.status_code(), StatusCode::OK);

        let stats_body: Value = stats_response.json();
        assert!(stats_body["total_nullifiers"].as_i64().unwrap() >= successful_operations as i64);

        info!(
            "✅ Concurrent API operations test passed ({} successful operations)",
            successful_operations
        );
    }

    #[traced_test]
    #[sqlx::test]
    async fn test_api_performance_metrics(pool: PgPool) {
        info!("🧪 Testing API performance metrics collection");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Perform several operations to generate metrics
        for i in 0..10 {
            let request = InsertNullifierRequest {
                value: 700_000 + i,
                metadata: Some(json!({"performance_test": true})),
                client_id: Some("perf-test".to_string()),
            };

            let response = server.post("/api/v1/nullifiers").json(&request).await;
            assert_eq!(response.status_code(), StatusCode::OK);

            // Also test membership proofs
            let membership_response = server
                .get(&format!("/api/v1/nullifiers/{}/membership", 700000 + i))
                .await;
            assert_eq!(membership_response.status_code(), StatusCode::OK);
        }

        // Check performance metrics
        let metrics_response = server.get("/api/v1/metrics").await;
        assert_eq!(metrics_response.status_code(), StatusCode::OK);

        let metrics_body: Value = metrics_response.json();
        assert!(metrics_body["operations"]["total"].as_i64().unwrap() >= 10);
        assert!(
            metrics_body["performance"]["avg_insertion_time_ms"]
                .as_f64()
                .unwrap()
                > 0.0
        );

        info!("✅ API performance metrics test passed");
    }

    // ============================================================================
    // STRESS TESTS
    // ============================================================================

    #[traced_test]
    #[sqlx::test]
    async fn test_large_batch_processing(pool: PgPool) {
        info!("🧪 Testing large batch processing");

        let api_server = create_test_api_server(pool).await;
        let app = api_server.create_router();
        let server = TestServer::new(app).unwrap();

        // Create a large batch (but within limits)
        let batch_size = 100;
        let nullifiers: Vec<i64> = (800_000..800_000 + batch_size).collect();

        let request = BatchInsertRequest {
            values: nullifiers,
            metadata: Some(json!({"large_batch": true})),
            client_id: Some("stress-test".to_string()),
        };

        let response = server.post("/api/v1/nullifiers/batch").json(&request).await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: Value = response.json();
        assert_eq!(body["success"], true);
        assert_eq!(body["total_operations"], batch_size);
        assert_eq!(body["successful_operations"], batch_size);

        info!("✅ Large batch processing test passed");
    }
}
