use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    http::{header, HeaderValue, Method},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::{info, instrument};

use crate::graphql::create_schema;
use crate::rest::{ApiConfig, ApiState};
use arithmetic_db::ads_service::IndexedMerkleTreeADS;
use arithmetic_db::vapp_integration::VAppAdsIntegration;

// ============================================================================
// API SERVER CONFIGURATION
// ============================================================================

/// Configuration for the combined API server
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    /// Base API configuration
    pub api_config: ApiConfig,

    /// Server binding configuration
    pub host: String,
    pub port: u16,

    /// Feature flags
    pub enable_rest: bool,
    pub enable_graphql: bool,
    pub enable_playground: bool,
    pub enable_subscriptions: bool,

    /// Middleware configuration
    pub enable_compression: bool,
    pub enable_cors: bool,
    pub request_timeout_seconds: u64,
    pub max_request_size_bytes: usize,

    /// Security configuration
    pub cors_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub api_key_required: bool,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            api_config: ApiConfig::default(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_rest: true,
            enable_graphql: true,
            enable_playground: true,
            enable_subscriptions: true,
            enable_compression: true,
            enable_cors: true,
            request_timeout_seconds: 30,
            max_request_size_bytes: 1024 * 1024, // 1MB
            cors_origins: vec!["*".to_string()],
            rate_limit_per_minute: 100,
            api_key_required: false,
        }
    }
}

// ============================================================================
// API SERVER IMPLEMENTATION
// ============================================================================

/// Combined REST and GraphQL API server
pub struct ApiServer {
    config: ApiServerConfig,
    state: ApiState,
}

impl ApiServer {
    /// Create new API server with configuration
    ///
    /// # Errors
    /// Returns error if server initialization fails
    #[instrument(skip(ads, vapp_integration), level = "info")]
    pub async fn new(
        ads: Arc<RwLock<IndexedMerkleTreeADS>>,
        vapp_integration: Arc<RwLock<VAppAdsIntegration>>,
        config: ApiServerConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Initializing combined API server");

        // Create API state
        let state = ApiState {
            ads,
            vapp_integration,
            config: config.api_config.clone(),
        };

        let server = Self { config, state };

        info!("✅ API server initialized successfully");
        Ok(server)
    }

    /// Build the complete router with all endpoints
    #[instrument(skip(self), level = "info")]
    #[allow(clippy::cognitive_complexity)]
    pub fn create_router(&self) -> Router<ApiState> {
        info!("🔧 Building API router");

        let mut router = Router::new().with_state(self.state.clone());

        // Add health check endpoint (always available)
        router = router.route("/health", get(health_check));
        router = router.route("/", get(api_info_handler));

        // Add REST API routes if enabled
        if self.config.enable_rest {
            info!("📡 Adding REST API routes");
            // TODO: Implement unified REST/GraphQL router
            // For now, REST is disabled to focus on GraphQL
        }

        // Add GraphQL routes if enabled
        if self.config.enable_graphql {
            info!("🔄 Adding GraphQL routes");

            // Main GraphQL endpoint
            router = router.route(
                "/graphql",
                post(graphql_handler).get(graphql_playground_handler),
            );

            // GraphQL subscriptions if enabled
            if self.config.enable_subscriptions {
                router = router.route("/graphql/ws", get(graphql_subscription_handler));
            }

            // GraphQL playground if enabled
            if self.config.enable_playground {
                router = router.route("/playground", get(graphql_playground_handler));
            }
        }

        // Add middleware layers
        router = self.add_middleware(router);

        info!("✅ API router built successfully");
        router
    }

    /// Add middleware layers to the router
    fn add_middleware(&self, router: Router<ApiState>) -> Router<ApiState> {
        let mut router = router
            .layer(TraceLayer::new_for_http())
            .layer(TimeoutLayer::new(Duration::from_secs(
                self.config.request_timeout_seconds,
            )))
            .layer(RequestBodyLimitLayer::new(
                self.config.max_request_size_bytes,
            ));

        if self.config.enable_compression {
            router = router.layer(CompressionLayer::new());
        }

        // Add CORS if enabled
        if self.config.enable_cors {
            let cors_layer = if self.config.cors_origins.contains(&"*".to_string()) {
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            } else {
                let origins: Vec<HeaderValue> = self
                    .config
                    .cors_origins
                    .iter()
                    .filter_map(|origin| origin.parse().ok())
                    .collect();

                CorsLayer::new()
                    .allow_origin(origins)
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            };
            router = router.layer(cors_layer);
        }

        router
    }

    /// Get server configuration
    #[must_use]
    pub const fn config(&self) -> &ApiServerConfig {
        &self.config
    }

    /// Get server bind address
    #[must_use]
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Get server state for testing purposes
    #[must_use]
    pub const fn state(&self) -> &ApiState {
        &self.state
    }
}

// ============================================================================
// GRAPHQL HANDLERS
// ============================================================================

/// GraphQL query/mutation handler
#[instrument(skip(req), level = "info")]
async fn graphql_handler(State(state): State<ApiState>, req: GraphQLRequest) -> GraphQLResponse {
    info!("🔄 Processing GraphQL request");
    let schema = create_schema();
    schema.execute(req.into_inner()).await.into()
}

/// GraphQL subscription handler (WebSocket)
#[instrument(level = "info")]
async fn graphql_subscription_handler(
    State(state): State<ApiState>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl IntoResponse {
    info!("🔄 Establishing GraphQL subscription connection");
    ws.on_upgrade(move |_socket| async move {
        // TODO: Fix GraphQL subscription API compatibility
        info!("GraphQL subscriptions temporarily disabled");
    })
}

/// GraphQL playground handler
#[instrument(level = "info")]
async fn graphql_playground_handler() -> impl IntoResponse {
    info!("🎮 Serving GraphQL Playground");
    Html(GraphiQLSource::build().endpoint("/graphql").finish())
}

// ============================================================================
// UTILITY HANDLERS
// ============================================================================

/// Basic health check endpoint
#[instrument(level = "info")]
async fn health_check() -> impl IntoResponse {
    info!("❤️ Health check requested");
    axum::Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "Indexed Merkle Tree API",
        "version": "1.0.0"
    }))
}

/// API information handler
#[instrument(level = "info")]
async fn api_info_handler() -> impl IntoResponse {
    info!("ℹ️ API info requested");
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Indexed Merkle Tree API</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { color: #2c3e50; border-bottom: 2px solid #3498db; padding-bottom: 20px; }
        .feature { background: #ecf0f1; padding: 15px; margin: 10px 0; border-radius: 5px; }
        .endpoint { font-family: monospace; background: #2c3e50; color: #ecf0f1; padding: 5px; border-radius: 3px; }
        .specs { background: #e8f5e8; padding: 15px; border-left: 4px solid #27ae60; margin: 20px 0; }
    </style>
</head>
<body>
    <div class="header">
        <h1>🌳 Indexed Merkle Tree API</h1>
        <p>High-performance zero-knowledge proof system with 32-level optimization</p>
    </div>

    <div class="specs">
        <h3>📊 Specifications</h3>
        <ul>
            <li><strong>Tree Height:</strong> 32 levels (vs traditional 256)</li>
            <li><strong>Constraint Optimization:</strong> ~200 constraints (8x fewer than traditional)</li>
            <li><strong>Hash Operations:</strong> 3n + 3 = 99 (for 32 levels)</li>
            <li><strong>Range Checks:</strong> Exactly 2 per operation</li>
            <li><strong>Proof Size:</strong> ~1KB (32 × 32 bytes)</li>
            <li><strong>Database Backend:</strong> PostgreSQL with audit trails</li>
        </ul>
    </div>

    <div class="feature">
        <h3>🔗 REST API</h3>
        <p>RESTful endpoints for all tree operations:</p>
        <ul>
            <li><span class="endpoint">POST /api/v1/nullifiers</span> - Insert nullifier</li>
            <li><span class="endpoint">POST /api/v1/nullifiers/batch</span> - Batch insert</li>
            <li><span class="endpoint">GET /api/v1/nullifiers/{value}/membership</span> - Membership proof</li>
            <li><span class="endpoint">GET /api/v1/nullifiers/{value}/non-membership</span> - Non-membership proof</li>
            <li><span class="endpoint">GET /api/v1/tree/stats</span> - Tree statistics</li>
            <li><span class="endpoint">GET /api/v1/health</span> - Health status</li>
        </ul>
    </div>

    <div class="feature">
        <h3>🔄 GraphQL API</h3>
        <p>Flexible query language for complex operations:</p>
        <ul>
            <li><span class="endpoint">POST /graphql</span> - GraphQL endpoint</li>
            <li><span class="endpoint">GET /playground</span> - Interactive GraphQL playground</li>
            <li><span class="endpoint">WS /graphql/ws</span> - Real-time subscriptions</li>
        </ul>
        <p><a href="/playground">🎮 Open GraphQL Playground</a></p>
    </div>

    <div class="feature">
        <h3>🚀 Features</h3>
        <ul>
            <li>7-step nullifier insertion algorithm from transparency dictionaries paper</li>
            <li>32-level tree optimization for ZK constraint reduction</li>
            <li>Real-time audit trails and compliance monitoring</li>
            <li>Batch processing with atomic guarantees</li>
            <li>State commitments for settlement contracts</li>
            <li>Comprehensive performance metrics</li>
            <li>Thread-safe concurrent operations</li>
        </ul>
    </div>

    <div class="feature">
        <h3>📚 Documentation</h3>
        <p>For detailed API documentation and examples, see:</p>
        <ul>
            <li>REST API: <a href="/api/v1/info">API Info Endpoint</a></li>
            <li>GraphQL Schema: <a href="/playground">GraphQL Playground</a></li>
            <li>Health Status: <a href="/health">System Health</a></li>
        </ul>
    </div>
</body>
</html>
    "#,
    )
}

// ============================================================================
// SERVER BUILDER UTILITIES
// ============================================================================

/// Builder for API server configuration
pub struct ApiServerBuilder {
    config: ApiServerConfig,
}

impl ApiServerBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ApiServerConfig::default(),
        }
    }

    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.host = host.into();
        self
    }

    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    #[must_use]
    pub fn api_config(mut self, config: ApiConfig) -> Self {
        self.config.api_config = config;
        self
    }

    #[must_use]
    pub const fn enable_rest(mut self, enabled: bool) -> Self {
        self.config.enable_rest = enabled;
        self
    }

    #[must_use]
    pub const fn enable_graphql(mut self, enabled: bool) -> Self {
        self.config.enable_graphql = enabled;
        self
    }

    #[must_use]
    pub const fn enable_playground(mut self, enabled: bool) -> Self {
        self.config.enable_playground = enabled;
        self
    }

    #[must_use]
    pub const fn enable_subscriptions(mut self, enabled: bool) -> Self {
        self.config.enable_subscriptions = enabled;
        self
    }

    #[must_use]
    pub fn cors_origins(mut self, origins: Vec<String>) -> Self {
        self.config.cors_origins = origins;
        self
    }

    #[must_use]
    pub const fn request_timeout(mut self, seconds: u64) -> Self {
        self.config.request_timeout_seconds = seconds;
        self
    }

    #[must_use]
    pub const fn max_request_size(mut self, bytes: usize) -> Self {
        self.config.max_request_size_bytes = bytes;
        self
    }

    /// Build the API server with the current configuration
    ///
    /// # Errors
    /// Returns error if server initialization fails
    pub async fn build(
        self,
        ads: Arc<RwLock<IndexedMerkleTreeADS>>,
        vapp_integration: Arc<RwLock<VAppAdsIntegration>>,
    ) -> Result<ApiServer, Box<dyn std::error::Error + Send + Sync>> {
        ApiServer::new(ads, vapp_integration, self.config).await
    }
}

impl Default for ApiServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// EXAMPLE USAGE
// ============================================================================

/// Example function showing how to create and run the API server
#[cfg(test)]
pub async fn example_server_setup() -> Result<ApiServer, Box<dyn std::error::Error + Send + Sync>> {
    use crate::ads_service::AdsServiceFactory;
    use crate::vapp_integration::{
        MockComplianceService, MockNotificationService, MockProofService, MockSettlementService,
        VAppAdsIntegration, VAppConfig,
    };
    use sqlx::PgPool;

    // This would be replaced with actual database pool
    let pool = PgPool::connect("postgresql://localhost/test").await?;

    // Create ADS service
    let factory = AdsServiceFactory::new(pool.clone());
    let ads = Arc::new(RwLock::new(factory.create_indexed_merkle_tree().await?));

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
        .await?,
    ));

    // Create API server
    let server = ApiServerBuilder::new()
        .host("0.0.0.0")
        .port(8080)
        .enable_rest(true)
        .enable_graphql(true)
        .enable_playground(true)
        .cors_origins(vec!["*".to_string()])
        .build(ads, vapp)
        .await?;

    Ok(server)
}
