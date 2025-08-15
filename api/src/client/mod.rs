//! HTTP API Client for arithmetic operations
//!
//! This client provides a typed interface for interacting with the arithmetic API server.
//! The CLI uses this instead of direct database access.

use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Client for interacting with the arithmetic API
#[derive(Debug, Clone)]
pub struct ArithmeticApiClient {
    client: Client,
    base_url: String,
}

/// API client errors
#[derive(Error, Debug)]
pub enum ApiClientError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    
    #[error("API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },
    
    #[error("Failed to serialize/deserialize: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Request to store an arithmetic transaction
#[derive(Debug, Serialize)]
pub struct StoreTransactionRequest {
    pub a: i32,
    pub b: i32,
    pub result: i32,
}

/// Response from storing an arithmetic transaction
#[derive(Debug, Deserialize)]
pub struct StoreTransactionResponse {
    pub transaction_id: i32,
    pub success: bool,
}

/// Request to get a transaction by result
#[derive(Debug, Serialize)]
pub struct GetTransactionRequest {
    pub result: i32,
}

/// Transaction data response
#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub id: i32,
    pub a: i32,
    pub b: i32,
    pub result: i32,
    pub created_at: String,
}

impl ArithmeticApiClient {
    /// Create a new API client
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            client,
            base_url: base_url.into(),
        }
    }
    
    /// Store an arithmetic transaction
    pub async fn store_transaction(
        &self,
        a: i32,
        b: i32,
        result: i32,
    ) -> Result<StoreTransactionResponse, ApiClientError> {
        let url = format!("{}/api/v1/transactions", self.base_url);
        let request = StoreTransactionRequest { a, b, result };
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
            
        self.handle_response(response).await
    }
    
    /// Get transaction by result value
    pub async fn get_transaction_by_result(
        &self,
        result: i32,
    ) -> Result<Option<Transaction>, ApiClientError> {
        let url = format!("{}/api/v1/transactions/by-result/{}", self.base_url, result);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        match response.status().as_u16() {
            200 => Ok(Some(self.handle_response(response).await?)),
            404 => Ok(None),
            _ => Err(self.handle_error_response(response).await),
        }
    }
    
    /// Check API health
    pub async fn health_check(&self) -> Result<bool, ApiClientError> {
        let url = format!("{}/api/v1/health", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;
            
        Ok(response.status().is_success())
    }
    
    /// Generic response handler
    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: Response,
    ) -> Result<T, ApiClientError> {
        if response.status().is_success() {
            let json = response.json().await?;
            Ok(json)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }
    
    /// Handle error responses
    async fn handle_error_response(&self, response: Response) -> ApiClientError {
        let status = response.status().as_u16();
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
            
        ApiClientError::ApiError { status, message }
    }
}

/// Default API client using localhost:8080
impl Default for ArithmeticApiClient {
    fn default() -> Self {
        Self::new("http://localhost:8080")
    }
}