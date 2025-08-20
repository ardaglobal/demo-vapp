use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::time::Duration;

use sqlx::PgPool;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::{info, instrument};

use crate::batch_processor::{create_batch_processor_config, start_batch_processor};
use crate::rest::{ApiConfig, ApiState};
use arithmetic_db::{init_db, AdsConfig, AdsServiceFactory, IndexedMerkleTreeADS};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// API SERVER CONFIGURATION
// ============================================================================

/// Configuration for the batch processing API server
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
    pub enable_playground: bool,

    /// Middleware configuration
    pub enable_compression: bool,
    pub enable_cors: bool,
    pub request_timeout_seconds: u64,
    pub max_request_size_bytes: usize,

    /// Security configuration
    pub cors_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            api_config: ApiConfig::default(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_rest: true,
            enable_playground: true,
            enable_compression: true,
            enable_cors: true,
            request_timeout_seconds: 30,
            max_request_size_bytes: 1024 * 1024, // 1MB
            cors_origins: vec!["*".to_string()],
            rate_limit_per_minute: 100,
        }
    }
}

// ============================================================================
// API SERVER IMPLEMENTATION
// ============================================================================

/// Batch processing API server
pub struct ApiServer {
    config: ApiServerConfig,
    state: ApiState,
}

impl ApiServer {
    /// Create new API server with database connection
    ///
    /// # Errors
    /// Returns error if database initialization fails
    #[instrument(skip(database_url), level = "info")]
    pub async fn new(
        database_url: Option<String>,
        config: ApiServerConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Initializing batch processing API server");

        // Initialize database connection
        let pool = if let Some(url) = database_url {
            arithmetic_db::init_db_with_url(&url).await?
        } else {
            init_db().await?
        };

        // Initialize ADS service with recovery from database
        info!("🔐 Initializing ADS service with database recovery");
        let ads_service = Self::initialize_ads_service(pool.clone()).await?;

        // Start background batch processor
        let batch_processor_config = create_batch_processor_config(&config.api_config);
        let batch_processor_handle =
            start_batch_processor(pool.clone(), batch_processor_config, ads_service.clone()).await;

        // Create API state
        let state = ApiState {
            pool,
            config: config.api_config.clone(),
            batch_processor: Some(batch_processor_handle),
            ads_service,
        };

        let server = Self { config, state };

        info!("✅ API server initialized successfully");
        Ok(server)
    }

    /// Create new API server with existing database pool
    pub async fn with_pool(
        pool: PgPool,
        config: ApiServerConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Creating API server with existing database pool");

        // Initialize ADS service with recovery from database
        info!("🔐 Initializing ADS service with database recovery");
        let ads_service = Self::initialize_ads_service(pool.clone()).await?;

        // Start background batch processor
        let batch_processor_config = create_batch_processor_config(&config.api_config);
        let batch_processor_handle =
            start_batch_processor(pool.clone(), batch_processor_config, ads_service.clone()).await;

        let state = ApiState {
            pool,
            config: config.api_config.clone(),
            batch_processor: Some(batch_processor_handle),
            ads_service,
        };

        Ok(Self { config, state })
    }

    /// Initialize ADS service with recovery from database state
    #[instrument(skip(pool), level = "info")]
    async fn initialize_ads_service(
        pool: PgPool,
    ) -> Result<Arc<RwLock<IndexedMerkleTreeADS>>, Box<dyn std::error::Error + Send + Sync>> {
        info!("🔐 Creating ADS service configuration");

        // Create ADS configuration for production use
        let ads_config = AdsConfig {
            settlement_contract: "0x742d35cc6640CA5AaAaB2AAD9d8e7f2B6E37b5D1".to_string(),
            chain_id: 1, // Mainnet - should be configurable
            audit_enabled: true,
            metrics_enabled: true,
            cache_size_limit: 50_000,
            batch_size_limit: 1_000,
            gas_price: 20_000_000_000, // 20 gwei
        };

        info!("🏭 Creating ADS service factory");
        let factory = AdsServiceFactory::with_config(pool.clone(), ads_config);

        info!("🌳 Initializing IndexedMerkleTreeADS (with database recovery)");
        let ads_service = factory
            .create_indexed_merkle_tree()
            .await
            .map_err(|e| format!("Failed to create ADS service: {}", e))?;

        let ads_service = Arc::new(RwLock::new(ads_service));

        info!("✅ ADS service initialized successfully with database recovery");
        Ok(ads_service)
    }

    /// Build the complete router with all endpoints
    #[instrument(skip(self), level = "info")]
    pub fn create_router(&self) -> Router {
        info!("🔧 Building API router");

        // Use REST API routes (includes health endpoints and state)
        let router = if self.config.enable_rest {
            info!("📡 Adding batch processing REST API routes");
            crate::rest::create_router(self.state.clone())
        } else {
            Router::new()
                .route("/health", get(health_check))
                .route("/", get(api_info_handler))
                .with_state(self.state.clone())
        };

        // Add middleware layers
        let router = self.add_middleware_with_state(router);

        info!("✅ API router built successfully");
        router
    }

    /// Add middleware layers to the router
    fn add_middleware_with_state(&self, router: Router) -> Router {
        let mut router = router
            .layer(TraceLayer::new_for_http())
            .layer(RequestBodyLimitLayer::new(
                self.config.max_request_size_bytes,
            ))
            .layer(TimeoutLayer::new(Duration::from_secs(
                self.config.request_timeout_seconds,
            )));

        if self.config.enable_compression {
            router = router.layer(CompressionLayer::new());
        }

        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers(Any);

            router = router.layer(cors);
        }

        router
    }

    /// Get server binding address
    #[must_use]
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Get API state reference
    #[must_use]
    pub const fn state(&self) -> &ApiState {
        &self.state
    }

    /// Get server configuration
    #[must_use]
    pub const fn config(&self) -> &ApiServerConfig {
        &self.config
    }
}

// ============================================================================
// HANDLERS
// ============================================================================

/// Health check endpoint
#[instrument(level = "info")]
async fn health_check() -> impl IntoResponse {
    info!("Health check requested");
    "OK"
}

/// API information handler
#[instrument(level = "info")]
async fn api_info_handler() -> Html<&'static str> {
    info!("API info page requested");
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Batch Processing API</title>
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
        <h1>🔄 Batch Processing API</h1>
        <p>Zero-knowledge proof system for batched transaction processing</p>
    </div>

    <div class="specs">
        <h3>📊 System Overview</h3>
        <ul>
            <li><strong>Architecture:</strong> Continuous balance tracking with ZK proofs</li>
            <li><strong>Batching:</strong> FIFO transaction processing with configurable batch sizes</li>
            <li><strong>Privacy:</strong> Individual transaction amounts remain private in ZK proofs</li>
            <li><strong>State Transitions:</strong> Proven counter transitions (e.g., 10 → 22)</li>
            <li><strong>Smart Contract Ready:</strong> Merkle roots and ZK proofs for on-chain verification</li>
            <li><strong>Database:</strong> PostgreSQL with atomic batch operations</li>
        </ul>
    </div>

    <div class="feature">
        <h3>🔗 REST API Endpoints</h3>
        <p>Version 2.0 batch processing endpoints:</p>
        <ul>
            <li><span class="endpoint">POST /api/v2/transactions</span> - Submit transaction</li>
            <li><span class="endpoint">GET /api/v2/transactions/pending</span> - View pending transactions</li>
            <li><span class="endpoint">POST /api/v2/batches</span> - Create batch from pending transactions</li>
            <li><span class="endpoint">GET /api/v2/batches</span> - List historical batches</li>
            <li><span class="endpoint">GET /api/v2/batches/:id</span> - Get specific batch</li>
            <li><span class="endpoint">POST /api/v2/batches/:id/proof</span> - Update batch with ZK proof</li>
            <li><span class="endpoint">GET /api/v2/state/current</span> - Get current counter state</li>
            <li><span class="endpoint">GET /api/v2/state/:id/contract</span> - Get contract submission data</li>
            <li><span class="endpoint">GET /api/v2/health</span> - Health check</li>
        </ul>
    </div>

    <div class="feature">
        <h3>🚀 Workflow</h3>
        <ol>
            <li><strong>Submit Transactions:</strong> Users submit integer amounts to be added to counter</li>
            <li><strong>Batch Creation:</strong> Accumulate transactions and create processing batches</li>
            <li><strong>ZK Proof Generation:</strong> Generate proofs showing correct state transitions</li>
            <li><strong>Merkle Tree Updates:</strong> Store authenticated data structure commits</li>
            <li><strong>Smart Contract Submission:</strong> Prepare data for on-chain verification</li>
        </ol>
    </div>

    <div class="feature">
        <h3>📚 Example Usage</h3>
        <p>Submit a transaction to add 5 to the counter:</p>
        <pre><code>curl -X POST http://localhost:8080/api/v2/transactions \
  -H "Content-Type: application/json" \
  -d '{"amount": 5}'</code></pre>
        
        <p>Create a batch of pending transactions:</p>
        <pre><code>curl -X POST http://localhost:8080/api/v2/batches \
  -H "Content-Type: application/json" \
  -d '{"batch_size": 10}'</code></pre>
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
    pub const fn enable_playground(mut self, enabled: bool) -> Self {
        self.config.enable_playground = enabled;
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
        database_url: Option<String>,
    ) -> Result<ApiServer, Box<dyn std::error::Error + Send + Sync>> {
        ApiServer::new(database_url, self.config).await
    }

    /// Build the API server with an existing database pool
    pub async fn build_with_pool(
        self,
        pool: PgPool,
    ) -> Result<ApiServer, Box<dyn std::error::Error + Send + Sync>> {
        ApiServer::with_pool(pool, self.config).await
    }
}

impl Default for ApiServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
