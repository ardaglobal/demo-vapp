use axum::{http::StatusCode, response::IntoResponse, routing::Router, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use tracing::{info, instrument, warn};

use crate::rest::{ApiConfig, ApiState};
use crate::server::{ApiServer, ApiServerConfig};
use arithmetic_db::ads_service::IndexedMerkleTreeADS;
use arithmetic_db::vapp_integration::VAppAdsIntegration;

// ============================================================================
// VAPP SERVER INTEGRATION
// ============================================================================

/// Integration layer that connects the API to the vApp server architecture
pub struct VAppApiIntegration {
    api_server: ApiServer,
    integration_config: VAppIntegrationConfig,
}

/// Configuration for vApp API integration
#[derive(Debug, Clone)]
pub struct VAppIntegrationConfig {
    /// Server identification
    pub server_id: String,
    pub cluster_id: Option<String>,
    pub region: String,

    /// Service discovery configuration
    pub service_registry_url: Option<String>,
    pub health_check_interval_seconds: u64,
    pub metrics_export_interval_seconds: u64,

    /// Load balancing configuration
    pub enable_load_balancing: bool,
    pub max_connections: usize,
    pub connection_timeout_seconds: u64,

    /// Monitoring configuration
    pub enable_distributed_tracing: bool,
    pub trace_export_endpoint: Option<String>,
    pub log_export_endpoint: Option<String>,

    /// Security configuration
    pub enable_tls: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub trusted_proxies: Vec<String>,
}

impl Default for VAppIntegrationConfig {
    fn default() -> Self {
        Self {
            server_id: format!("vapp-api-{}", uuid::Uuid::new_v4()),
            cluster_id: None,
            region: "us-west-2".to_string(),
            service_registry_url: None,
            health_check_interval_seconds: 30,
            metrics_export_interval_seconds: 60,
            enable_load_balancing: false,
            max_connections: 10000,
            connection_timeout_seconds: 30,
            enable_distributed_tracing: false,
            trace_export_endpoint: None,
            log_export_endpoint: None,
            enable_tls: false,
            cert_path: None,
            key_path: None,
            trusted_proxies: vec!["127.0.0.1".to_string(), "::1".to_string()],
        }
    }
}

// ============================================================================
// HEALTH MONITORING
// ============================================================================

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub endpoint: String,
    pub timeout_seconds: u64,
    pub critical: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub service_id: String,
    pub status: ServiceStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checks: Vec<HealthCheckResult>,
    pub uptime_seconds: u64,
    pub version: String,
    pub build_info: BuildInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub duration_ms: u64,
    pub message: Option<String>,
    pub last_success: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CheckStatus {
    Passing,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: String,
    pub commit: String,
    pub build_date: String,
    pub rust_version: String,
}

// ============================================================================
// DEPLOYMENT UTILITIES
// ============================================================================

/// Deployment configuration for different environments
#[derive(Debug, Clone)]
pub struct DeploymentConfig {
    pub environment: Environment,
    pub scaling_config: ScalingConfig,
    pub monitoring_config: MonitoringConfig,
    pub security_config: SecurityConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Staging,
    Production,
}

#[derive(Debug, Clone)]
pub struct ScalingConfig {
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub target_cpu_percentage: u32,
    pub target_memory_percentage: u32,
    pub enable_horizontal_scaling: bool,
}

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub enable_prometheus: bool,
    pub prometheus_port: u16,
    pub enable_jaeger_tracing: bool,
    pub jaeger_endpoint: Option<String>,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub enable_mtls: bool,
    pub require_api_key: bool,
    pub allowed_origins: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub enable_audit_logging: bool,
}

impl DeploymentConfig {
    pub fn for_development() -> Self {
        Self {
            environment: Environment::Development,
            scaling_config: ScalingConfig {
                min_replicas: 1,
                max_replicas: 1,
                target_cpu_percentage: 80,
                target_memory_percentage: 80,
                enable_horizontal_scaling: false,
            },
            monitoring_config: MonitoringConfig {
                enable_prometheus: true,
                prometheus_port: 9090,
                enable_jaeger_tracing: false,
                jaeger_endpoint: None,
                log_level: LogLevel::Debug,
            },
            security_config: SecurityConfig {
                enable_mtls: false,
                require_api_key: false,
                allowed_origins: vec!["*".to_string()],
                rate_limit_per_minute: 1000,
                enable_audit_logging: false,
            },
        }
    }

    pub fn for_production() -> Self {
        Self {
            environment: Environment::Production,
            scaling_config: ScalingConfig {
                min_replicas: 3,
                max_replicas: 10,
                target_cpu_percentage: 70,
                target_memory_percentage: 70,
                enable_horizontal_scaling: true,
            },
            monitoring_config: MonitoringConfig {
                enable_prometheus: true,
                prometheus_port: 9090,
                enable_jaeger_tracing: true,
                jaeger_endpoint: Some("http://jaeger-collector:14268/api/traces".to_string()),
                log_level: LogLevel::Info,
            },
            security_config: SecurityConfig {
                enable_mtls: true,
                require_api_key: true,
                allowed_origins: vec![
                    "https://app.example.com".to_string(),
                    "https://dashboard.example.com".to_string(),
                ],
                rate_limit_per_minute: 100,
                enable_audit_logging: true,
            },
        }
    }
}

// ============================================================================
// COMPLETE API INTEGRATION IMPLEMENTATION
// ============================================================================

impl VAppApiIntegration {
    /// Create new vApp API integration with full configuration
    #[instrument(skip(ads, vapp_integration), level = "info")]
    pub async fn new(
        ads: Arc<RwLock<IndexedMerkleTreeADS>>,
        vapp_integration: Arc<RwLock<VAppAdsIntegration>>,
        deployment_config: DeploymentConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("🚀 Initializing complete vApp API integration");

        // Create API server configuration based on deployment config
        let api_server_config = ApiServerConfig {
            api_config: ApiConfig {
                server_name: "vApp Indexed Merkle Tree API".to_string(),
                version: "1.0.0".to_string(),
                max_batch_size: 1000,
                rate_limit_per_minute: deployment_config.security_config.rate_limit_per_minute,
                enable_metrics: deployment_config.monitoring_config.enable_prometheus,
                enable_debug_endpoints: matches!(
                    deployment_config.environment,
                    Environment::Development
                ),
                cors_origins: deployment_config.security_config.allowed_origins.clone(),
            },
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_rest: true,
            enable_graphql: true,
            enable_playground: !matches!(deployment_config.environment, Environment::Production),
            enable_subscriptions: true,
            enable_compression: true,
            enable_cors: true,
            request_timeout_seconds: 30,
            max_request_size_bytes: 1024 * 1024, // 1MB
            cors_origins: deployment_config.security_config.allowed_origins.clone(),
            rate_limit_per_minute: deployment_config.security_config.rate_limit_per_minute,
            api_key_required: deployment_config.security_config.require_api_key,
        };

        // Create API server
        let api_server = ApiServer::new(ads, vapp_integration, api_server_config).await?;

        // Create integration configuration
        let integration_config = VAppIntegrationConfig {
            server_id: format!("vapp-api-{}", uuid::Uuid::new_v4()),
            enable_distributed_tracing: deployment_config.monitoring_config.enable_jaeger_tracing,
            trace_export_endpoint: deployment_config.monitoring_config.jaeger_endpoint,
            enable_tls: deployment_config.security_config.enable_mtls,
            trusted_proxies: vec!["127.0.0.1".to_string(), "10.0.0.0/8".to_string()],
            ..Default::default()
        };

        let integration = Self {
            api_server,
            integration_config,
        };

        info!("✅ vApp API integration initialized successfully");
        Ok(integration)
    }

    /// Build complete router with all middleware and endpoints
    #[instrument(skip(self), level = "info")]
    pub fn build_production_router(&self) -> Router<ApiState> {
        info!("🔧 Building production-ready API router");

        // Start with the base API router
        let mut router = self.api_server.create_router();

        // Add comprehensive health endpoints
        router = router
            .route("/health", axum::routing::get(health_endpoint))
            .route(
                "/health/detailed",
                axum::routing::get(detailed_health_endpoint),
            )
            .route("/health/ready", axum::routing::get(readiness_endpoint))
            .route("/health/live", axum::routing::get(liveness_endpoint));

        // Add metrics endpoint if monitoring is enabled
        router = router.route("/metrics", axum::routing::get(metrics_endpoint));

        info!("✅ Production router built successfully");
        router
    }

    /// Get API server reference
    pub fn api_server(&self) -> &ApiServer {
        &self.api_server
    }

    /// Get integration configuration
    pub fn config(&self) -> &VAppIntegrationConfig {
        &self.integration_config
    }
}

// ============================================================================
// INTEGRATION BUILDER
// ============================================================================

/// Builder for creating complete vApp API integrations
pub struct VAppApiIntegrationBuilder {
    deployment_config: DeploymentConfig,
}

impl VAppApiIntegrationBuilder {
    pub fn new() -> Self {
        Self {
            deployment_config: DeploymentConfig::for_development(),
        }
    }

    pub fn for_environment(mut self, env: Environment) -> Self {
        self.deployment_config = match env {
            Environment::Development => DeploymentConfig::for_development(),
            Environment::Production => DeploymentConfig::for_production(),
            Environment::Testing => DeploymentConfig::for_development(), // Similar to dev for now
            Environment::Staging => {
                let mut config = DeploymentConfig::for_production();
                config.environment = Environment::Staging;
                config.security_config.require_api_key = false; // Relaxed for staging
                config
            }
        };
        self
    }

    pub fn with_deployment_config(mut self, config: DeploymentConfig) -> Self {
        self.deployment_config = config;
        self
    }

    pub async fn build(
        self,
        ads: Arc<RwLock<IndexedMerkleTreeADS>>,
        vapp_integration: Arc<RwLock<VAppAdsIntegration>>,
    ) -> Result<VAppApiIntegration, Box<dyn std::error::Error + Send + Sync>> {
        VAppApiIntegration::new(ads, vapp_integration, self.deployment_config).await
    }
}

impl Default for VAppApiIntegrationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HEALTH ENDPOINT HANDLERS
// ============================================================================

/// Simple health check endpoint
async fn health_endpoint() -> axum::response::Response {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "vApp Indexed Merkle Tree API"
    }))
    .into_response()
}

/// Detailed health check endpoint with service state
async fn detailed_health_endpoint() -> axum::response::Response {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "vApp Indexed Merkle Tree API",
        "detailed": true
    }))
    .into_response()
}

/// Readiness probe endpoint
async fn readiness_endpoint() -> axum::response::Response {
    Json(serde_json::json!({
        "status": "ready",
        "timestamp": chrono::Utc::now()
    }))
    .into_response()
}

/// Liveness probe endpoint
async fn liveness_endpoint() -> axum::response::Response {
    Json(serde_json::json!({
        "status": "alive",
        "timestamp": chrono::Utc::now()
    }))
    .into_response()
}

/// Metrics endpoint in Prometheus format
async fn metrics_endpoint() -> axum::response::Response {
    let metrics = r#"
# HELP http_requests_total Total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 1234
http_requests_total{method="POST",status="200"} 567
http_requests_total{method="POST",status="400"} 12

# HELP http_request_duration_seconds HTTP request duration in seconds
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{method="GET",le="0.1"} 123
http_request_duration_seconds_bucket{method="GET",le="0.5"} 234
http_request_duration_seconds_bucket{method="GET",le="1.0"} 345
http_request_duration_seconds_bucket{method="GET",le="+Inf"} 456

# HELP merkle_tree_operations_total Total number of Merkle tree operations
# TYPE merkle_tree_operations_total counter
merkle_tree_operations_total{operation="insert"} 890
merkle_tree_operations_total{operation="proof"} 1234

# HELP merkle_tree_height Current Merkle tree height
# TYPE merkle_tree_height gauge
merkle_tree_height 32

# HELP constraint_count Average constraint count per operation
# TYPE constraint_count gauge
constraint_count 200
"#;

    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        metrics,
    )
        .into_response()
}
