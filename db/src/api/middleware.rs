use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::api::rest::ApiState;

// ============================================================================
// RATE LIMITING MIDDLEWARE
// ============================================================================

/// Rate limiter based on token bucket algorithm
#[derive(Debug, Clone)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    requests_per_minute: u32,
    bucket_capacity: u32,
    window_duration: Duration,
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
    capacity: u32,
    refill_rate_per_second: f64,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate_per_minute: u32) -> Self {
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
            capacity,
            refill_rate_per_second: refill_rate_per_minute as f64 / 60.0,
        }
    }

    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = (elapsed * self.refill_rate_per_second) as u32;
        
        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_refill = now;
        }
    }
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            requests_per_minute,
            bucket_capacity: requests_per_minute,
            window_duration: Duration::from_secs(60),
        }
    }

    pub fn check_rate_limit(&self, client_id: &str, tokens: u32) -> Result<(), RateLimitError> {
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets.entry(client_id.to_string()).or_insert_with(|| {
            TokenBucket::new(self.bucket_capacity, self.requests_per_minute)
        });

        if bucket.try_consume(tokens) {
            Ok(())
        } else {
            Err(RateLimitError {
                client_id: client_id.to_string(),
                limit: self.requests_per_minute,
                window_seconds: 60,
                retry_after_seconds: 60,
            })
        }
    }

    // Clean up old buckets periodically
    pub fn cleanup_old_buckets(&self) {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) < Duration::from_secs(300) // Keep for 5 minutes
        });
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RateLimitError {
    pub client_id: String,
    pub limit: u32,
    pub window_seconds: u64,
    pub retry_after_seconds: u64,
}

/// Rate limiting middleware
#[instrument(skip(rate_limiter, request, next), level = "debug")]
pub async fn rate_limit_middleware(
    State(rate_limiter): State<Arc<RateLimiter>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Extract client identifier (IP address, API key, etc.)
    let client_id = extract_client_id(&request);
    
    // Determine token cost based on endpoint
    let token_cost = calculate_token_cost(&request);
    
    match rate_limiter.check_rate_limit(&client_id, token_cost) {
        Ok(_) => {
            debug!("Rate limit check passed for client: {}", client_id);
            
            // Add rate limit headers to response
            let response = next.run(request).await;
            add_rate_limit_headers(response, &rate_limiter, &client_id)
        }
        Err(error) => {
            warn!("Rate limit exceeded for client: {} (limit: {} req/min)", 
                  client_id, error.limit);
            
            // Return rate limit error response
            let error_response = Json(serde_json::json!({
                "error": "RATE_LIMIT_EXCEEDED",
                "message": "Rate limit exceeded",
                "details": {
                    "client_id": error.client_id,
                    "limit": error.limit,
                    "window_seconds": error.window_seconds,
                    "retry_after_seconds": error.retry_after_seconds
                },
                "timestamp": Utc::now()
            }));

            (StatusCode::TOO_MANY_REQUESTS, error_response).into_response()
        }
    }
}

fn extract_client_id(request: &Request<Body>) -> String {
    // Try API key first
    if let Some(api_key) = request.headers().get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            return format!("api_key:{}", key_str);
        }
    }
    
    // Try authorization header
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str.starts_with("Bearer ") {
                return format!("bearer:{}", &auth_str[7..]);
            }
        }
    }
    
    // Fall back to IP address (simplified - would need proper extraction in real implementation)
    request
        .headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .and_then(|ip| ip.to_str().ok())
        .map(|ip| format!("ip:{}", ip))
        .unwrap_or_else(|| "unknown".to_string())
}

fn calculate_token_cost(request: &Request<Body>) -> u32 {
    match (request.method(), request.uri().path()) {
        // High-cost operations
        (&Method::POST, path) if path.contains("batch") => 10,
        (&Method::POST, path) if path.contains("nullifiers") => 5,
        
        // Medium-cost operations
        (&Method::GET, path) if path.contains("proof") => 3,
        (&Method::GET, path) if path.contains("audit") => 3,
        
        // Low-cost operations
        (&Method::GET, path) if path.contains("stats") => 1,
        (&Method::GET, path) if path.contains("health") => 1,
        
        // GraphQL operations (variable cost)
        (&Method::POST, "/graphql") => 5, // Would analyze query complexity in real implementation
        
        // Default cost
        _ => 1,
    }
}

fn add_rate_limit_headers(mut response: Response, rate_limiter: &RateLimiter, client_id: &str) -> Response {
    let headers = response.headers_mut();
    
    // Get current bucket state
    if let Ok(buckets) = rate_limiter.buckets.lock() {
        if let Some(bucket) = buckets.get(client_id) {
            headers.insert("X-RateLimit-Limit", HeaderValue::from(rate_limiter.requests_per_minute));
            headers.insert("X-RateLimit-Remaining", HeaderValue::from(bucket.tokens));
            headers.insert("X-RateLimit-Window", HeaderValue::from_static("60"));
        }
    }
    
    response
}

// ============================================================================
// REQUEST VALIDATION MIDDLEWARE
// ============================================================================

/// Request validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub max_nullifier_value: i64,
    pub min_nullifier_value: i64,
    pub max_batch_size: usize,
    pub allowed_content_types: Vec<String>,
    pub required_headers: Vec<String>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_nullifier_value: i64::MAX - 1000,
            min_nullifier_value: 1,
            max_batch_size: 1000,
            allowed_content_types: vec![
                "application/json".to_string(),
                "application/graphql".to_string(),
            ],
            required_headers: vec![],
        }
    }
}

/// Request validation middleware
#[instrument(skip(config, request, next), level = "debug")]
pub async fn validation_middleware(
    State(config): State<ValidationConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Validate content type for POST requests
    if request.method() == Method::POST {
        if let Some(content_type) = request.headers().get("content-type") {
            if let Ok(ct_str) = content_type.to_str() {
                let ct_base = ct_str.split(';').next().unwrap_or(ct_str);
                if !config.allowed_content_types.contains(&ct_base.to_string()) {
                    return validation_error(
                        "INVALID_CONTENT_TYPE",
                        &format!("Content-Type '{}' not allowed", ct_base),
                        Some(&format!("Allowed types: {}", config.allowed_content_types.join(", ")))
                    );
                }
            }
        } else {
            return validation_error(
                "MISSING_CONTENT_TYPE",
                "Content-Type header is required for POST requests",
                None
            );
        }
    }
    
    // Validate required headers
    for required_header in &config.required_headers {
        if request.headers().get(required_header).is_none() {
            return validation_error(
                "MISSING_REQUIRED_HEADER",
                &format!("Required header '{}' is missing", required_header),
                None
            );
        }
    }
    
    // Validate path parameters
    if let Err(error_response) = validate_path_parameters(&request, &config) {
        return error_response;
    }
    
    debug!("Request validation passed for: {} {}", request.method(), request.uri().path());
    next.run(request).await
}

fn validate_path_parameters(request: &Request<Body>, config: &ValidationConfig) -> Result<(), Response> {
    let path = request.uri().path();
    
    // Extract nullifier value from path if present
    if let Some(captures) = regex::Regex::new(r"/nullifiers/([0-9]+)/").unwrap().captures(path) {
        if let Some(value_str) = captures.get(1) {
            match value_str.as_str().parse::<i64>() {
                Ok(value) => {
                    if value < config.min_nullifier_value || value > config.max_nullifier_value {
                        return Err(validation_error(
                            "INVALID_NULLIFIER_VALUE",
                            &format!("Nullifier value {} is out of range", value),
                            Some(&format!("Range: {} to {}", config.min_nullifier_value, config.max_nullifier_value))
                        ));
                    }
                }
                Err(_) => {
                    return Err(validation_error(
                        "INVALID_NULLIFIER_FORMAT",
                        "Nullifier value must be a valid integer",
                        None
                    ));
                }
            }
        }
    }
    
    Ok(())
}

fn validation_error(error_code: &str, message: &str, details: Option<&str>) -> Response {
    let mut error_json = serde_json::json!({
        "error": error_code,
        "message": message,
        "timestamp": Utc::now()
    });
    
    if let Some(details) = details {
        error_json["details"] = serde_json::Value::String(details.to_string());
    }
    
    (StatusCode::BAD_REQUEST, Json(error_json)).into_response()
}

// ============================================================================
// AUTHENTICATION MIDDLEWARE
// ============================================================================

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub valid_api_keys: Vec<String>,
    pub jwt_secret: Option<String>,
    pub allow_anonymous_endpoints: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            valid_api_keys: vec![],
            jwt_secret: None,
            allow_anonymous_endpoints: vec![
                "/health".to_string(),
                "/".to_string(),
                "/playground".to_string(),
            ],
        }
    }
}

/// Authentication middleware
#[instrument(skip(config, request, next), level = "debug")]
pub async fn auth_middleware(
    State(config): State<AuthConfig>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !config.enabled {
        debug!("Authentication disabled, skipping");
        return next.run(request).await;
    }
    
    let path = request.uri().path();
    
    // Check if endpoint allows anonymous access
    if config.allow_anonymous_endpoints.iter().any(|ep| path.starts_with(ep)) {
        debug!("Anonymous access allowed for path: {}", path);
        return next.run(request).await;
    }
    
    // Check for API key authentication
    if let Some(api_key) = request.headers().get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            if config.valid_api_keys.contains(&key_str.to_string()) {
                debug!("API key authentication successful");
                return next.run(request).await;
            }
        }
    }
    
    // Check for Bearer token authentication (JWT)
    if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if let Some(jwt_secret) = &config.jwt_secret {
                    if validate_jwt_token(token, jwt_secret) {
                        debug!("JWT authentication successful");
                        return next.run(request).await;
                    }
                }
            }
        }
    }
    
    warn!("Authentication failed for path: {}", path);
    
    let error_response = Json(serde_json::json!({
        "error": "AUTHENTICATION_REQUIRED",
        "message": "Valid API key or Bearer token required",
        "timestamp": Utc::now()
    }));

    (StatusCode::UNAUTHORIZED, error_response).into_response()
}

fn validate_jwt_token(token: &str, secret: &str) -> bool {
    // Simplified JWT validation - would use proper JWT library in real implementation
    // For now, just check if token is non-empty and matches a simple pattern
    !token.is_empty() && token.len() > 20 && token.contains('.')
}

// ============================================================================
// REQUEST LOGGING MIDDLEWARE
// ============================================================================

/// Request logging information
#[derive(Debug, Serialize)]
pub struct RequestLog {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub query_string: Option<String>,
    pub user_agent: Option<String>,
    pub client_ip: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_ms: u64,
    pub status_code: u16,
    pub response_size: Option<u64>,
    pub error: Option<String>,
}

/// Request logging middleware
#[instrument(skip(request, next), level = "info")]
pub async fn logging_middleware(
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let start_time = Utc::now();
    let start_instant = Instant::now();
    
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query_string = request.uri().query().map(|s| s.to_string());
    let user_agent = request.headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let client_ip = extract_client_ip(&request);
    
    // Add request ID to headers for downstream use
    request.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("invalid"))
    );
    
    info!("🔄 Request started: {} {} [{}]", method, path, request_id);
    
    let response = next.run(request).await;
    
    let end_time = Utc::now();
    let duration_ms = start_instant.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();
    
    let log = RequestLog {
        request_id: request_id.clone(),
        method: method.to_string(),
        path,
        query_string,
        user_agent,
        client_ip,
        start_time,
        end_time,
        duration_ms,
        status_code,
        response_size: None, // Would extract from response body if needed
        error: if status_code >= 400 { Some(format!("HTTP {}", status_code)) } else { None },
    };
    
    if status_code >= 400 {
        warn!("❌ Request failed: {} [{}] - {}ms - {}", 
              log.method, request_id, duration_ms, status_code);
    } else {
        info!("✅ Request completed: {} [{}] - {}ms - {}", 
              log.method, request_id, duration_ms, status_code);
    }
    
    // In a real implementation, you might want to send this to a logging service
    debug!("Request log: {}", serde_json::to_string(&log).unwrap_or_default());
    
    response
}

fn extract_client_ip(request: &Request<Body>) -> Option<String> {
    request
        .headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .and_then(|ip| ip.to_str().ok())
        .map(|ip| ip.split(',').next().unwrap_or(ip).trim().to_string())
}

// ============================================================================
// METRICS MIDDLEWARE
// ============================================================================

/// Metrics collection middleware
#[instrument(skip(request, next), level = "debug")]
pub async fn metrics_middleware(
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    
    let response = next.run(request).await;
    
    let duration = start.elapsed();
    let status = response.status().as_u16();
    
    // Record metrics (would integrate with actual metrics system like Prometheus)
    debug!(
        "📊 Metrics - Method: {}, Path: {}, Status: {}, Duration: {:?}",
        method, path, status, duration
    );
    
    // In real implementation, would increment counters, histograms, etc.
    // For example:
    // - http_requests_total.with_label_values(&[method.as_str(), &status.to_string()]).inc();
    // - http_request_duration_seconds.with_label_values(&[method.as_str(), &path]).observe(duration.as_secs_f64());
    
    response
}

// ============================================================================
// MIDDLEWARE CONFIGURATION BUILDER
// ============================================================================

/// Builder for configuring middleware stack
pub struct MiddlewareBuilder {
    pub rate_limiter: Option<Arc<RateLimiter>>,
    pub validation_config: ValidationConfig,
    pub auth_config: AuthConfig,
    pub enable_logging: bool,
    pub enable_metrics: bool,
}

impl MiddlewareBuilder {
    pub fn new() -> Self {
        Self {
            rate_limiter: None,
            validation_config: ValidationConfig::default(),
            auth_config: AuthConfig::default(),
            enable_logging: true,
            enable_metrics: true,
        }
    }

    pub fn with_rate_limiting(mut self, requests_per_minute: u32) -> Self {
        self.rate_limiter = Some(Arc::new(RateLimiter::new(requests_per_minute)));
        self
    }

    pub fn with_validation(mut self, config: ValidationConfig) -> Self {
        self.validation_config = config;
        self
    }

    pub fn with_auth(mut self, config: AuthConfig) -> Self {
        self.auth_config = config;
        self
    }

    pub fn enable_logging(mut self, enabled: bool) -> Self {
        self.enable_logging = enabled;
        self
    }

    pub fn enable_metrics(mut self, enabled: bool) -> Self {
        self.enable_metrics = enabled;
        self
    }
}