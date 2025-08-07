pub mod rest;
pub mod graphql;
pub mod server;
pub mod middleware;
pub mod integration;

// Re-export main API types for convenience
pub use rest::{
    ApiState, ApiConfig, create_router,
    InsertNullifierRequest, InsertNullifierResponse, 
    BatchInsertRequest, BatchInsertResponse,
    MembershipCheckResponse, NonMembershipResponse,
    TreeStatsResponse, AuditTrailResponse, HealthResponse,
};

pub use graphql::{
    QueryRoot, MutationRoot, SubscriptionRoot, GraphQLSchema,
    create_schema, NullifierType, StateTransitionType, 
    MembershipProofType, NonMembershipProofType, TreeStatsType,
    InsertNullifierInput, BatchInsertInput, NullifierQueryInput,
    ProofResult, OperationResult,
};

pub use server::{
    ApiServer, ApiServerConfig, ApiServerBuilder,
};

pub use middleware::{
    RateLimiter, ValidationConfig, AuthConfig, MiddlewareBuilder,
    rate_limit_middleware, validation_middleware, auth_middleware,
    logging_middleware, metrics_middleware,
};

pub use integration::{
    VAppApiIntegration, VAppIntegrationConfig, VAppApiIntegrationBuilder,
    HealthMonitor, HealthStatus, DeploymentConfig, Environment,
    ScalingConfig, MonitoringConfig, SecurityConfig,
};