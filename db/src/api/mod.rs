pub mod graphql;
pub mod integration;
pub mod middleware;
pub mod rest;
pub mod server;

// Re-export main API types for convenience
pub use rest::{
    create_router, ApiConfig, ApiState, AuditTrailResponse, BatchInsertRequest,
    BatchInsertResponse, HealthResponse, InsertNullifierRequest, InsertNullifierResponse,
    MembershipCheckResponse, NonMembershipResponse, TreeStatsResponse,
};

pub use graphql::{
    create_schema, BatchInsertInput, GraphQLSchema, InsertNullifierInput, MembershipProofType,
    MutationRoot, NonMembershipProofType, NullifierQueryInput, NullifierType, OperationResult,
    ProofResult, QueryRoot, StateTransitionType, SubscriptionRoot, TreeStatsType,
};

pub use server::{ApiServer, ApiServerBuilder, ApiServerConfig};

pub use middleware::{
    auth_middleware, logging_middleware, metrics_middleware, rate_limit_middleware,
    validation_middleware, AuthConfig, MiddlewareBuilder, RateLimiter, ValidationConfig,
};

pub use integration::{
    DeploymentConfig, Environment, HealthStatus, MonitoringConfig, ScalingConfig, SecurityConfig,
    VAppApiIntegration, VAppApiIntegrationBuilder, VAppIntegrationConfig,
};
