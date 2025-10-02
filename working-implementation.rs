//! AMP (Asset Management Platform) controller module
//!
//! This module provides integration with Blockstream's AMP API for managing
//! Liquid assets, users, categories, and distributions.

use crate::helpers::job_dispatcher::{dispatch_amp_job, JobDispatchResponse};
use crate::retry::{AmpRetryPolicy, RetryClient, RetryConfig};
use crate::workers::amp_worker::JobKind;
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use loco_rs::model::ModelError;
use loco_rs::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

/// Default AMP API base URL (used only if `AMP_API_BASE_URL` env var is not set)
const DEFAULT_AMP_API_BASE_URL: &str = "https://amp.blockstream.com";

/// Thread-safe storage for the AMP authentication token
static AMP_TOKEN: OnceCell<Arc<Mutex<Option<String>>>> = OnceCell::const_new();

/// Get the AMP API base URL from environment variable or use default
#[must_use]
pub fn get_amp_api_base_url() -> String {
    env::var("AMP_API_BASE_URL").unwrap_or_else(|_| DEFAULT_AMP_API_BASE_URL.to_string())
}

// Debug utilities - only available in debug builds
#[cfg(debug_assertions)]
mod debug_utils {

    /// Returns a shortened version of a token for debug logging
    /// Shows first 6 chars (or less if token is shorter) and total length
    pub fn short_token(t: &str) -> String {
        format!("{}…({} chars)", &t[..6.min(t.len())], t.len())
    }

    /// Debug logs an outgoing HTTP request with headers and body
    /// Uses clear separators and ensures tokens are shortened
    #[allow(clippy::cognitive_complexity)]
    pub fn debug_request(
        label: &str,
        url: &str,
        headers: &[(&str, String)],
        body: &serde_json::Value,
    ) {
        tracing::debug!("➡️  {} REQUEST", label.to_uppercase());
        tracing::debug!("URL: {}", url);
        tracing::debug!("Headers:");

        for (name, value) in headers {
            if name.to_lowercase() == "authorization" && value.starts_with("token ") {
                let token = &value[6..];
                tracing::debug!("  {}: token {}", name, short_token(token));
            } else {
                tracing::debug!("  {}: {}", name, value);
            }
        }

        tracing::debug!(
            "Body:\n{}",
            serde_json::to_string_pretty(body).unwrap_or_else(|_| "<invalid json>".to_string())
        );
    }

    /// Debug logs an HTTP response with status and body
    /// Returns the response body as a string for further processing
    pub fn debug_response(resp: &reqwest::Response) -> String {
        let status = resp.status();
        let headers = resp.headers();

        tracing::debug!("⬅️  RESPONSE");
        tracing::debug!("Status: {}", status);

        // Log interesting response headers
        if let Some(content_type) = headers.get("content-type") {
            tracing::debug!("Content-Type: {:?}", content_type);
        }

        // Note: This function doesn't consume the response body
        // The caller is responsible for reading the body
        String::new()
    }
}

// Re-export debug utilities for use in this module
#[cfg(debug_assertions)]
use debug_utils::{debug_request, debug_response, short_token};

/// Request payload for AMP token acquisition
#[derive(Debug, Serialize)]
struct TokenRequest {
    username: String,
    password: String,
}

/// Response from AMP token acquisition
#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
}

/// Obtains an API authentication token from Blockstream's AMP API
///
/// This function retrieves credentials from environment variables `AMP_USERNAME` and `AMP_PASSWORD`,
/// makes a POST request to the `/user/obtain_token` endpoint, and stores the token securely.
///
/// # Errors
///
/// Returns an error if:
/// - Required environment variables are missing
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn obtain_amp_token() -> Result<String, Error> {
    // Get credentials from environment variables
    let username = env::var("AMP_USERNAME")
        .map_err(|_| Error::string("Missing AMP_USERNAME environment variable"))?;
    let password = env::var("AMP_PASSWORD")
        .map_err(|_| Error::string("Missing AMP_PASSWORD environment variable"))?;

    // Create retry client
    let mut retry_client = create_amp_retry_client();

    // Prepare request payload
    let request_payload = TokenRequest { username, password };

    // Make POST request to obtain token with retry
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/user/obtain_token");

    let client = retry_client.inner().clone();
    let response = retry_client
        .execute_with_retry(|| client.post(&url).json(&request_payload).send())
        .await
        .map_err(|e| Error::string(&format!("AMP token request failed: {}", e)))?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "AMP token request failed with status {status}: {error_text}"
        )));
    }

    // Parse response
    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse AMP token response: {e}")))?;

    // Store token securely
    let token_storage = AMP_TOKEN
        .get_or_init(|| async { Arc::new(Mutex::new(None)) })
        .await;

    let expiry_storage = AMP_TOKEN_EXPIRY
        .get_or_init(|| async { Arc::new(Mutex::new(None)) })
        .await;

    {
        let mut token_guard = token_storage.lock().await;
        *token_guard = Some(token_response.token.clone());
        drop(token_guard);

        let mut expiry_guard = expiry_storage.lock().await;
        // Set expiry to 1 day from now
        *expiry_guard = Some(Utc::now() + Duration::days(1));
    }

    tracing::info!("AMP authentication token obtained successfully");
    Ok(token_response.token)
}

/// Thread-safe storage for the AMP token expiry information
static AMP_TOKEN_EXPIRY: OnceCell<Arc<Mutex<Option<DateTime<Utc>>>>> = OnceCell::const_new();

/// Create a `RetryClient` configured for AMP API requests
///
/// This function creates a new `RetryClient` with:
/// - AMP-specific retry policy that handles 429 (Too Many Requests)
/// - Configuration from environment variables or defaults
/// - Proper error handling for 401 (authentication) errors
pub fn create_amp_retry_client() -> RetryClient {
    let mut config = RetryConfig::from_env().unwrap_or_default();

    // In test environment, use shorter timeouts to prevent hanging
    if cfg!(test) || std::env::var("RUST_TEST_THREADS").is_ok() {
        config = RetryConfig::new(
            true, // enabled
            2,    // max_attempts (reduced from 3)
            500,  // base_delay_ms (reduced from 1000)
            5000, // max_delay_ms (reduced from 30000)
        )
        .unwrap_or(config);
    }

    let policy = Box::new(AmpRetryPolicy::new(config.max_attempts));

    // Create client with timeout to prevent hanging
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    RetryClient::with_client_and_policy(client, config, policy)
}

/// Delete a manager by ID (helper function for internal use)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
pub async fn delete_manager_by_id(manager_id: i32) -> Result<(), Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!("{amp_base_url}/managers/{manager_id}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to delete manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to delete manager with status {status}: {error_text}"
        )));
    }

    Ok(())
}

/// Delete a registered user by ID (helper function for internal use)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
pub async fn delete_registered_user_by_id(user_id: i32) -> Result<(), Error> {
    let token = get_amp_token().await?;
    let mut retry_client = create_amp_retry_client();
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/registered_users/{user_id}/delete");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .delete(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .send()
        })
        .await
        .map_err(|e| Error::string(&format!("Failed to delete registered user: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to delete registered user with status {status}: {error_text}"
        )));
    }

    Ok(())
}

/// Refreshes the AMP authentication token using the refresh endpoint
///
/// # Errors
///
/// Returns an error if:
/// - No current token exists to refresh
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
///
/// # Panics
///
/// Panics if the token storage was not properly initialized
pub async fn refresh_amp_token() -> Result<String, Error> {
    println!("\n=== Refreshing AMP Token ===");

    // Get current token and check expiry
    let (current_token, has_expired) = {
        let token_storage = AMP_TOKEN
            .get_or_init(|| async { Arc::new(Mutex::new(None)) })
            .await;
        let expiry_storage = AMP_TOKEN_EXPIRY
            .get_or_init(|| async { Arc::new(Mutex::new(None)) })
            .await;

        let token_guard = token_storage.lock().await;
        if let Some(token) = token_guard.as_ref() {
            let token_clone = token.clone();
            drop(token_guard);

            // Check if token has expired
            let has_expired = expiry_storage.lock().await.is_some_and(|expires_at| {
                let now = Utc::now();
                expires_at <= now
            });
            (token_clone, has_expired)
        } else {
            println!("No token available to refresh");
            println!("=== End Refreshing AMP Token ===");
            return Err(Error::string("No token available to refresh"));
        }
    };

    // If token has expired, obtain a new one instead of refreshing
    if has_expired {
        println!("Token has already expired, obtaining new token instead of refreshing");
        let result = obtain_amp_token().await;
        println!("=== End Refreshing AMP Token ===");
        return result;
    }

    // Create retry client
    let mut retry_client = create_amp_retry_client();

    // Make GET request to refresh token with retry
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/user/refresh-token");

    let client = retry_client.inner().clone();
    let current_token_clone = current_token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .get(&url)
                .header(
                    "Authorization",
                    format!("token {}", current_token_clone.clone()),
                )
                .send()
        })
        .await
        .map_err(|e| Error::string(&format!("AMP token refresh failed: {}", e)))?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "AMP token refresh failed with status {status}: {error_text}"
        )));
    }

    // Parse response
    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse AMP token refresh response: {e}")))?;

    // Update stored token and expiry
    let token_storage = AMP_TOKEN.get().unwrap();
    let expiry_storage = AMP_TOKEN_EXPIRY.get().unwrap();

    {
        let mut token_guard = token_storage.lock().await;
        *token_guard = Some(token_response.token.clone());
        drop(token_guard);

        let mut expiry_guard = expiry_storage.lock().await;
        // Set expiry to 1 day from now
        *expiry_guard = Some(Utc::now() + Duration::days(1));
    }

    tracing::info!("AMP authentication token refreshed successfully");
    println!("Token refreshed successfully!");
    println!(
        "New token length: {} characters",
        token_response.token.len()
    );
    println!(
        "New token preview: {}...",
        &token_response.token[..20.min(token_response.token.len())]
    );
    println!("=== End Refreshing AMP Token ===");
    Ok(token_response.token)
}

/// Retrieves the stored AMP authentication token, obtaining a new one if necessary
///
/// If no token is currently stored, this function will automatically call
/// `obtain_amp_token()` to get a new token. If a token exists but expires in the
/// next 5 minutes or has already expired, it will refresh the token.
///
/// # Errors
///
/// Returns an error if:
/// - Required environment variables are missing
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn get_amp_token() -> Result<String, Error> {
    let token_storage = AMP_TOKEN
        .get_or_init(|| async { Arc::new(Mutex::new(None)) })
        .await;

    let expiry_storage = AMP_TOKEN_EXPIRY
        .get_or_init(|| async { Arc::new(Mutex::new(None)) })
        .await;

    // Check if we have a token and if it needs refresh
    {
        let token_guard = token_storage.lock().await;
        let expiry_guard = expiry_storage.lock().await;

        if let Some(token) = token_guard.as_ref() {
            // Check if token needs refresh
            if let Some(expires_at) = *expiry_guard {
                let now = Utc::now();
                let five_minutes_from_now = now + Duration::minutes(5);

                if expires_at <= five_minutes_from_now {
                    // Token expires soon or has expired, need to refresh
                    drop(token_guard);
                    drop(expiry_guard);
                    tracing::info!(
                        "Token expires at {} (in {} minutes), refreshing...",
                        expires_at.format("%Y-%m-%d %H:%M:%S UTC"),
                        (expires_at - now).num_minutes()
                    );
                    return refresh_amp_token().await;
                }
            }
            return Ok(token.clone());
        }
    }

    // No token exists, obtain a new one
    obtain_amp_token().await
}

/// Token information including the token string and expiry time
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub token: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Retrieves the stored AMP authentication token with expiry info, obtaining a new one if necessary
///
/// If no token is currently stored, this function will automatically call
/// `obtain_amp_token()` to get a new token. If a token exists but expires in the
/// next 5 minutes or has already expired, it will refresh the token.
///
/// # Errors
///
/// Returns an error if:
/// - Required environment variables are missing
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn get_amp_token_info() -> Result<TokenInfo, Error> {
    // This will handle obtaining/refreshing the token as needed
    let token = get_amp_token().await?;

    // Get the expiry info
    let expiry_storage = AMP_TOKEN_EXPIRY
        .get_or_init(|| async { Arc::new(Mutex::new(None)) })
        .await;

    let expiry_guard = expiry_storage.lock().await;
    Ok(TokenInfo {
        token,
        expires_at: *expiry_guard,
    })
}

/// Clears the stored AMP authentication token (primarily for testing)
#[doc(hidden)]
pub async fn clear_amp_token() {
    if let Some(token_storage) = AMP_TOKEN.get() {
        let mut token_guard = token_storage.lock().await;
        *token_guard = None;
    }
    if let Some(expiry_storage) = AMP_TOKEN_EXPIRY.get() {
        let mut expiry_guard = expiry_storage.lock().await;
        *expiry_guard = None;
    }
}

/// Request structure for adding a registered user to AMP API
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisteredUserAdd {
    /// User's GAID (Green Address ID)
    #[serde(rename = "GAID")]
    pub gaid: String,
    /// Whether the user represents a company
    pub is_company: bool,
    /// User's full name
    pub name: String,
}

/// Request structure for editing a registered user in AMP API
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisteredUserEdit {
    /// Updated user name (optional)
    pub name: Option<String>,
    /// Updated company status (optional)
    pub is_company: Option<bool>,
}

/// Obtain and return AMP authentication token
///
/// # Errors
///
/// Returns an error if token acquisition fails
#[axum::debug_handler]
pub async fn obtain_token_handler() -> Result<Json<Value>, Error> {
    let token = obtain_amp_token().await?;

    let response = json!({
        "status": "success",
        "message": "AMP authentication token obtained",
        "token_length": token.len()
    });

    Ok(Json(response))
}

// User Management Endpoints

/// Get all registered users from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn get_registered_users(State(ctx): State<AppContext>) -> Result<Response> {
    let job_id = dispatch_amp_job(&ctx, JobKind::GetRegisteredUsers, json!({})).await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Get a specific registered user from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn get_registered_user(
    State(ctx): State<AppContext>,
    Path(user_id): Path<i32>,
) -> Result<Response> {
    let job_id = dispatch_amp_job(
        &ctx,
        JobKind::GetRegisteredUser,
        json!({ "user_id": user_id }),
    )
    .await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Add a new registered user to AMP API (internal synchronous version for models)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn add_registered_user_sync(body: RegisteredUserAdd) -> Result<Value, Error> {
    let token = get_amp_token().await?;
    let mut retry_client = create_amp_retry_client();
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/registered_users/add");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .json(&body)
                .send()
        })
        .await
        .map_err(|e| Error::string(&format!("Failed to add registered user: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to add registered user with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse add user response: {e}")))?;

    Ok(result)
}

/// Add a registered user to a category (internal synchronous version for models)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn add_registered_user_to_category_sync(
    category_id: i32,
    registered_user_id: i32,
) -> Result<Value, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/categories/{category_id}/registered_users/{registered_user_id}/add"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to add user to category: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to add user to category with status {status}: {error_text}"
        )));
    }

    let result: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse add user to category response: {e}"
        ))
    })?;

    Ok(result)
}

/// Add a new manager to AMP API (internal synchronous version for models)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn add_manager_sync(body: ManagerCreate) -> Result<Value, Error> {
    tracing::debug!("Creating manager in AMP API: {:?}", body);

    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/managers"))
        .header("Authorization", format!("token {token}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to create manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to create manager with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse create manager response: {e}")))?;

    tracing::debug!("Manager created successfully");
    Ok(result)
}

/// Edit an existing manager (internal synchronous version for models)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn edit_manager_sync(manager_id: i32, body: Value) -> Result<Value, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .patch(format!("{amp_base_url}/managers/{manager_id}"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to edit manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to edit manager with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse edit manager response: {e}")))?;

    Ok(result)
}

/// Add a new registered user to AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn add_registered_user(
    State(ctx): State<AppContext>,
    Json(body): Json<RegisteredUserAdd>,
) -> Result<Response> {
    let job_id = dispatch_amp_job(
        &ctx,
        JobKind::AddRegisteredUser,
        serde_json::to_value(body)?,
    )
    .await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Edit an existing registered user in AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn edit_registered_user(
    State(ctx): State<AppContext>,
    Path(user_id): Path<i32>,
    Json(body): Json<RegisteredUserEdit>,
) -> Result<Response> {
    let payload = json!({
        "user_id": user_id,
        "update_data": body
    });
    let job_id = dispatch_amp_job(&ctx, JobKind::EditRegisteredUser, payload).await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Delete a registered user from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn delete_registered_user(
    State(ctx): State<AppContext>,
    Path(user_id): Path<i32>,
) -> Result<Response> {
    let job_id = dispatch_amp_job(
        &ctx,
        JobKind::DeleteRegisteredUser,
        json!({ "user_id": user_id }),
    )
    .await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

// Additional User Management Endpoints
/// Get summary information for a registered user
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn get_registered_user_summary(
    State(ctx): State<AppContext>,
    Path(user_id): Path<i32>,
) -> Result<Response> {
    let job_id = dispatch_amp_job(
        &ctx,
        JobKind::GetRegisteredUserSummary,
        json!({ "user_id": user_id }),
    )
    .await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Get all GAIDs associated with a registered user
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn get_registered_user_gaids(
    State(ctx): State<AppContext>,
    Path(user_id): Path<i32>,
) -> Result<Response> {
    let job_id = dispatch_amp_job(
        &ctx,
        JobKind::GetRegisteredUserGaids,
        json!({ "user_id": user_id }),
    )
    .await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Add a new GAID to a registered user
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn add_registered_user_gaid(
    State(ctx): State<AppContext>,
    Path(user_id): Path<i32>,
    body: String,
) -> Result<Response> {
    let payload = json!({
        "user_id": user_id,
        "gaid": body
    });
    let job_id = dispatch_amp_job(&ctx, JobKind::AddRegisteredUserGaid, payload).await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Set the default GAID for a registered user
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn set_default_registered_user_gaid(
    Path(user_id): Path<i32>,
    body: String,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let mut retry_client = create_amp_retry_client();
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/registered_users/{user_id}/gaids/set-default");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let body_clone = body.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .header("Content-Type", "text/plain")
                .body(body_clone.clone())
                .send()
        })
        .await
        .map_err(|e| Error::string(&format!("Failed to set default GAID: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to set default GAID with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse set default GAID response: {e}")))?;

    Ok(Json(result))
}

/// Handler to check current token status
///
/// # Errors
///
/// Returns an error if no token is available
///
/// # Panics
///
/// Panics if the token storage was not properly initialized
#[axum::debug_handler]
pub async fn token_status_handler() -> Result<Json<Value>, Error> {
    // Check if we have a token stored without attempting to obtain one
    let has_token = if let Some(token_storage) = AMP_TOKEN.get() {
        let token_guard = token_storage.lock().await;
        token_guard.is_some()
    } else {
        false
    };

    if has_token {
        // Get the token to check its length
        let token_storage = AMP_TOKEN.get().unwrap();
        let token_guard = token_storage.lock().await;
        let token_length = token_guard.as_ref().unwrap().len();
        drop(token_guard);

        let response = json!({
            "status": "success",
            "message": "AMP token is available",
            "token_length": token_length
        });
        Ok(Json(response))
    } else {
        let response = json!({
            "status": "error",
            "message": "No AMP token available"
        });
        Ok(Json(response))
    }
}
/// Asset issuance request structure matching AMP API requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issuance {
    pub name: String,
    pub amount: i64,
    pub destination_address: String,
    pub domain: String,
    pub ticker: String,
    pub precision: i32,
    pub pubkey: String,
    pub is_confidential: bool,
    pub is_reissuable: bool,
    pub reissuance_amount: i64,
    pub reissuance_address: String,
    pub transfer_restricted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<i32>,
}

/// Context struct for asset issuance to reduce function parameters
/// This wraps the Issuance struct to provide a cleaner API while maintaining
/// backward compatibility with the AMP API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuanceContext {
    /// The issuance parameters
    pub issuance: Issuance,
}

impl IssuanceContext {
    /// Create a new issuance context from issuance parameters
    #[must_use]
    pub fn new(issuance: Issuance) -> Self {
        Self { issuance }
    }

    /// Get a reference to the issuance parameters
    #[must_use]
    pub fn params(&self) -> &Issuance {
        &self.issuance
    }

    /// Convert context into the inner issuance parameters
    #[must_use]
    pub fn into_params(self) -> Issuance {
        self.issuance
    }
}

/// Backward compatibility: Allow creating `IssuanceContext` from Issuance
impl From<Issuance> for IssuanceContext {
    fn from(issuance: Issuance) -> Self {
        Self::new(issuance)
    }
}

/// Asset issuance response from AMP API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuanceResponse {
    pub asset_id: String,
    pub reissuance_token_id: Option<String>,
    pub asset_uuid: String,
    pub txid: String,
    pub vin: i32,
    pub asset_vout: i32,
    pub reissuance_vout: Option<i32>,
    pub issuer_authorization_endpoint: Option<String>,
    pub issuance_assetblinder: String,
    pub issuance_tokenblinder: Option<String>,
}

/// Issue an asset through the AMP API
///
/// This function sends an asset issuance request to the AMP API with proper authentication.
/// It automatically handles token acquisition/refresh through the `get_amp_token` function.
///
/// # Arguments
///
/// * `issuance_params` - The asset issuance parameters
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn issue_asset(issuance_params: &Issuance) -> Result<IssuanceResponse, ModelError> {
    // Get authentication token
    let token = get_amp_token()
        .await
        .map_err(|e| ModelError::msg(&format!("Failed to get AMP token: {e}")))?;

    // Create retry client
    let mut retry_client = create_amp_retry_client();

    // Make POST request to issue asset with retry
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/assets");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .json(issuance_params)
                .send()
        })
        .await
        .map_err(|e| ModelError::msg(&format!("AMP asset issuance request failed: {}", e)))?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(ModelError::msg(&format!(
            "AMP asset issuance failed with status {status}: {error_text}"
        )));
    }

    // Parse response
    let issuance_response: IssuanceResponse = response
        .json()
        .await
        .map_err(|e| ModelError::msg(&format!("Failed to parse AMP issuance response: {e}")))?;

    tracing::info!(
        "Asset issued successfully: asset_id={}, txid={}",
        issuance_response.asset_id,
        issuance_response.txid
    );

    Ok(issuance_response)
}

/// Issue an asset through the AMP API using `IssuanceContext`
///
/// This is a wrapper function that accepts `IssuanceContext` for cleaner API design
/// while maintaining backward compatibility.
///
/// # Arguments
///
/// * `context` - The issuance context containing asset parameters
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn issue_asset_with_context(
    context: &IssuanceContext,
) -> Result<IssuanceResponse, ModelError> {
    issue_asset(context.params()).await
}

/// Register an asset in the AMP API
///
/// This function registers an asset that has been issued on the Liquid Network
/// with the AMP API to enable tracking and management features.
///
/// # Arguments
///
/// * `asset_uuid` - The UUID of the asset to register
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn register_asset(asset_uuid: &str) -> Result<Value, ModelError> {
    // Get authentication token
    let token = get_amp_token()
        .await
        .map_err(|e| ModelError::msg(&format!("Failed to get AMP token: {e}")))?;

    // Create retry client
    let mut retry_client = create_amp_retry_client();

    // Make POST request to register asset with retry
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/assets/{asset_uuid}/register");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .send()
        })
        .await
        .map_err(|e| ModelError::msg(&format!("AMP asset registration request failed: {}", e)))?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(ModelError::msg(&format!(
            "AMP asset registration failed with status {status}: {error_text}"
        )));
    }

    // Parse response
    let result: Value = response
        .json()
        .await
        .map_err(|e| ModelError::msg(&format!("Failed to parse AMP registration response: {e}")))?;

    tracing::info!("Asset registered successfully: asset_uuid={}", asset_uuid);

    Ok(result)
}

/// Register an authorized asset in the AMP API
///
/// This function registers an asset as an authorized asset in the AMP API,
/// which enables additional compliance and regulatory features.
///
/// # Arguments
///
/// * `asset_uuid` - The UUID of the asset to register as authorized
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn register_authorized_asset(asset_uuid: &str) -> Result<Value, ModelError> {
    // Get authentication token
    let token = get_amp_token()
        .await
        .map_err(|e| ModelError::msg(&format!("Failed to get AMP token: {e}")))?;

    // Create retry client
    let mut retry_client = create_amp_retry_client();

    // Make POST request to register authorized asset with retry
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/assets/{asset_uuid}/register-authorized");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .send()
        })
        .await
        .map_err(|e| {
            ModelError::msg(&format!(
                "AMP authorized asset registration request failed: {}",
                e
            ))
        })?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(ModelError::msg(&format!(
            "AMP authorized asset registration failed with status {status}: {error_text}"
        )));
    }

    // Parse response
    let result: Value = response.json().await.map_err(|e| {
        ModelError::msg(&format!(
            "Failed to parse AMP authorized registration response: {e}"
        ))
    })?;

    tracing::info!(
        "Authorized asset registered successfully: asset_uuid={}",
        asset_uuid
    );

    Ok(result)
}

// Category Management Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryAdd {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryEdit {
    pub name: Option<String>,
    pub description: Option<String>,
}

// Asset Management Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct EditAssetRequest {
    pub authorization_endpoint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReissueRequest {
    pub amount: i64,
    pub address: String,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReissueConfirm {
    pub txid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BurnRequest {
    pub amount: i64,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BurnConfirm {
    pub txid: String,
}

// Additional Asset Management Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBlindersRequest {
    pub txid: String,
    pub vout: i32,
    pub assetblinder: String,
    pub amountblinder: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: i32,
}

// Assignment Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct AssignmentCreateBody {
    pub registered_user_id: i32,
    pub amount: i64,
    pub is_locked: bool,
    pub vesting_timestamp: Option<i64>,
    pub comment: Option<String>,
}

// Distribution Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct DistributionConfirm {
    pub txids: Vec<String>,
}

// Manager Structures
#[derive(Debug, Serialize, Deserialize)]
pub struct ManagerCreate {
    pub username: String,
    pub password: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ManagerPasswordChange {
    pub old_password: String,
    pub new_password: String,
}

// Category Management Endpoints
/// Get all categories from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_categories() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let mut retry_client = create_amp_retry_client();
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/categories");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .get(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .send()
        })
        .await
        .map_err(|e| Error::string(&format!("Failed to get categories: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get categories with status {status}: {error_text}"
        )));
    }

    let categories: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse categories response: {e}")))?;

    Ok(Json(categories))
}

/// Add a new category to AMP API
///
/// # Debug Logging Format
///
/// This function follows the standard AMP debug format:
///
/// ```text
/// ➡️  ADD-CATEGORY REQUEST
/// URL: https://amp.blockstream.com/categories/add
/// Headers:
///   Authorization: token abc123...
///   Content-Type: application/json
/// Body:
/// {
///   "name": "Category Name",
///   "description": "Category Description"
/// }
/// Token: Using cached token (6 chars: abc123...)
///
/// ⬅️  ADD-CATEGORY RESPONSE
/// Status: 200 OK
/// Body:
/// {
///   "id": 123,
///   "name": "Category Name",
///   "description": "Category Description"
/// }
/// ```
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
#[tracing::instrument(skip(body))]
pub async fn add_category(Json(body): Json<CategoryAdd>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let mut retry_client = create_amp_retry_client();
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/categories/add");

    // Debug log the request
    #[cfg(debug_assertions)]
    {
        let headers = vec![
            ("Authorization", format!("token {}", short_token(&token))),
            ("Content-Type", "application/json".to_string()),
        ];
        let body_json = serde_json::to_value(&body)
            .unwrap_or_else(|_| json!({"error": "Failed to serialize body"}));
        debug_request("ADD-CATEGORY", &url, &headers, &body_json);
    }

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .json(&body)
                .send()
        })
        .await
        .map_err(|e| Error::string(&format!("Failed to add category: {}", e)))?;

    let status = response.status();

    // Debug log the response status
    #[cfg(debug_assertions)]
    {
        debug_response(&response);
    }

    // Read response body for both success and error cases
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Unknown error".to_string());

    if !status.is_success() {
        #[cfg(debug_assertions)]
        {
            tracing::debug!("Body:\n{}", response_text);
        }
        return Err(Error::string(&format!(
            "Failed to add category with status {status}: {response_text}"
        )));
    }

    // Try to parse as JSON
    let result: Value = serde_json::from_str(&response_text).map_err(|e| {
        #[cfg(debug_assertions)]
        {
            tracing::debug!("Body (raw):\n{}", response_text);
        }
        Error::string(&format!(
            "Failed to parse add category response: {e}. Response body: {response_text}"
        ))
    })?;

    #[cfg(debug_assertions)]
    {
        tracing::debug!(
            "Body:\n{}",
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| response_text.clone())
        );
    }

    Ok(Json(result))
}

// Complete Category Management Endpoints
/// Get a specific category from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_category(Path(category_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/categories/{category_id}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get category: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get category with status {status}: {error_text}"
        )));
    }

    let category: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse category response: {e}")))?;

    Ok(Json(category))
}

/// Edit an existing category in AMP API
///
/// This function updates a category in the AMP API and is designed for both HTTP
/// requests and internal model calls.
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
///
/// # Note
///
/// Confirm with Blockstream (business@blockstream.com) if `PUT /categories/{id}` is
/// the correct endpoint for updating categories.
#[axum::debug_handler]
pub async fn edit_category(
    Path(category_id): Path<i32>,
    Json(body): Json<CategoryEdit>,
) -> Result<Json<Value>, Error> {
    tracing::debug!("Updating category {} in AMP API: {:?}", category_id, body);

    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .put(format!("{amp_base_url}/categories/{category_id}/edit"))
        .header("Authorization", format!("token {token}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to edit category: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to edit category with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse edit category response: {e}")))?;

    tracing::debug!("Category {} updated successfully", category_id);
    Ok(Json(result))
}

/// Delete a category from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
#[allow(clippy::cognitive_complexity)]
pub async fn delete_category(Path(category_id): Path<i32>) -> Result<Json<Value>, Error> {
    tracing::debug!("=== DELETE_CATEGORY: Starting AMP API category deletion ===");
    tracing::debug!("Category ID to delete: {}", category_id);

    // Get token with debugging
    tracing::debug!("Acquiring AMP token...");
    let token = match get_amp_token().await {
        Ok(t) => {
            tracing::debug!("Token acquired successfully (length: {} chars)", t.len());
            t
        }
        Err(e) => {
            tracing::error!("Failed to acquire AMP token: {}", e);
            return Err(e);
        }
    };

    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();
    let delete_url = format!("{amp_base_url}/categories/{category_id}/delete");

    tracing::debug!("AMP API Base URL: {}", amp_base_url);
    tracing::debug!("Full delete URL: {}", delete_url);
    tracing::debug!(
        "Authorization header: token {} (first 10 chars: {}...)",
        if token.len() > 10 {
            &token[..10]
        } else {
            &token
        },
        if token.len() > 10 {
            &token[..10]
        } else {
            &token
        }
    );

    tracing::debug!("Sending DELETE request to AMP API...");
    let response = client
        .delete(&delete_url)
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("HTTP request failed: {}", e);
            tracing::error!("Request details - URL: {}, Method: DELETE", delete_url);
            Error::string(&format!("Failed to delete category: {e}"))
        })?;

    let status = response.status();
    tracing::debug!("Response status: {}", status);
    tracing::debug!("Response headers: {:?}", response.headers());

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|e| {
            tracing::error!("Failed to read error response body: {}", e);
            "Unknown error".to_string()
        });

        tracing::error!(
            "AMP API delete failed with status {}: {}",
            status,
            error_text
        );
        tracing::error!("Full error response body: {}", error_text);

        return Err(Error::string(&format!(
            "Failed to delete category with status {status}: {error_text}"
        )));
    }

    // Read successful response body for debugging
    let success_body = response.text().await.unwrap_or_else(|e| {
        tracing::warn!("Failed to read success response body: {}", e);
        "No body".to_string()
    });

    tracing::debug!("Successful response body: {}", success_body);
    tracing::debug!("Category {} deleted successfully from AMP", category_id);
    tracing::debug!("=== DELETE_CATEGORY: Completed successfully ===");

    Ok(Json(
        json!({ "message": "Category deleted successfully", "category_id": category_id }),
    ))
}

/// Add a registered user to a category
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn add_registered_user_to_category(
    Path((category_id, registered_user_id)): Path<(i32, i32)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/categories/{category_id}/registered_users/{registered_user_id}/add"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to add user to category: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to add user to category with status {status}: {error_text}"
        )));
    }

    let result: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse add user to category response: {e}"
        ))
    })?;

    Ok(Json(result))
}

/// Remove a registered user from a category
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn remove_registered_user_from_category(
    Path((category_id, registered_user_id)): Path<(i32, i32)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!(
            "{amp_base_url}/categories/{category_id}/registered_users/{registered_user_id}/remove"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to remove user from category: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to remove user from category with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "User removed from category successfully" }),
    ))
}

/// Add an asset to a category (internal function for model use)
///
/// This function associates an asset with a category in the AMP API.
/// It uses the category's `registered_id` and the asset's UUID to make the association.
///
/// # Arguments
///
/// * `category_registered_id` - The `registered_id` of the category in AMP
/// * `asset_uuid` - The UUID of the asset to associate with the category
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
pub async fn add_asset_to_category(
    category_registered_id: i32,
    asset_uuid: &str,
) -> Result<Value, ModelError> {
    // Get authentication token
    let token = get_amp_token()
        .await
        .map_err(|e| ModelError::msg(&format!("Failed to get AMP token: {e}")))?;

    // Create retry client
    let mut retry_client = create_amp_retry_client();

    // Make POST request to add asset to category with retry
    let amp_base_url = get_amp_api_base_url();
    let url = format!("{amp_base_url}/categories/{category_registered_id}/assets/{asset_uuid}/add");

    let client = retry_client.inner().clone();
    let token_clone = token.clone();
    let response = retry_client
        .execute_with_retry(|| {
            client
                .post(&url)
                .header("Authorization", format!("token {}", token_clone.clone()))
                .send()
        })
        .await
        .map_err(|e| {
            ModelError::msg(&format!("AMP add asset to category request failed: {}", e))
        })?;

    // Check if request was successful
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(ModelError::msg(&format!(
            "AMP add asset to category failed with status {status}: {error_text}"
        )));
    }

    // Parse response
    let result: Value = response.json().await.map_err(|e| {
        ModelError::msg(&format!(
            "Failed to parse AMP add asset to category response: {e}"
        ))
    })?;

    tracing::info!(
        "Asset added to category successfully: category_registered_id={}, asset_uuid={}",
        category_registered_id,
        asset_uuid
    );

    Ok(result)
}

/// Add an asset to a category (HTTP handler)
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn add_asset_to_category_handler(
    Path((category_id, asset_uuid)): Path<(i32, String)>,
) -> Result<Json<Value>, Error> {
    let result = add_asset_to_category(category_id, &asset_uuid)
        .await
        .map_err(|e| Error::string(&e.to_string()))?;

    Ok(Json(result))
}

/// Remove an asset from a category
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn remove_asset_from_category(
    Path((category_id, asset_uuid)): Path<(i32, String)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!(
            "{amp_base_url}/categories/{category_id}/assets/{asset_uuid}/remove"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to remove asset from category: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to remove asset from category with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "Asset removed from category successfully" }),
    ))
}

// Asset Management Endpoints
/// Get all assets from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_assets() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get assets: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get assets with status {status}: {error_text}"
        )));
    }

    let assets: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse assets response: {e}")))?;

    Ok(Json(assets))
}

/// Get a specific asset from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset with status {status}: {error_text}"
        )));
    }

    let asset: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset response: {e}")))?;

    Ok(Json(asset))
}

/// Edit an existing asset in AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn edit_asset(
    Path(asset_uuid): Path<String>,
    Json(body): Json<EditAssetRequest>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .put(format!("{amp_base_url}/assets/{asset_uuid}/edit"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to edit asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to edit asset with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse edit asset response: {e}")))?;

    Ok(Json(result))
}

/// Delete an asset from AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn delete_asset(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!("{amp_base_url}/assets/{asset_uuid}/delete"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to delete asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to delete asset with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "Asset deleted successfully", "asset_uuid": asset_uuid }),
    ))
}

/// Lock an asset in AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn lock_asset(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/assets/{asset_uuid}/lock"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to lock asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to lock asset with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse lock asset response: {e}")))?;

    Ok(Json(result))
}

/// Unlock an asset in AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn unlock_asset(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/assets/{asset_uuid}/unlock"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to unlock asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to unlock asset with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse unlock asset response: {e}")))?;

    Ok(Json(result))
}

// Additional Asset Endpoints
/// Get activities for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_activities(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/activities"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset activities: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset activities with status {status}: {error_text}"
        )));
    }

    let activities: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset activities response: {e}")))?;

    Ok(Json(activities))
}

/// Get ownerships for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_ownerships(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/ownerships"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset ownerships: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset ownerships with status {status}: {error_text}"
        )));
    }

    let ownerships: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset ownerships response: {e}")))?;

    Ok(Json(ownerships))
}

/// Get balance for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_balance(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/balance"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset balance: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset balance with status {status}: {error_text}"
        )));
    }

    let balance: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset balance response: {e}")))?;

    Ok(Json(balance))
}

/// Get lost outputs for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_lost_outputs(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/lost_outputs"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset lost outputs: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset lost outputs with status {status}: {error_text}"
        )));
    }

    let lost_outputs: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset lost outputs response: {e}")))?;

    Ok(Json(lost_outputs))
}

/// Get summary information for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_summary(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/summary"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset summary: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset summary with status {status}: {error_text}"
        )));
    }

    let summary: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset summary response: {e}")))?;

    Ok(Json(summary))
}

/// Get UTXOs for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_utxos(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/utxos"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset UTXOs: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset UTXOs with status {status}: {error_text}"
        )));
    }

    let utxos: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset UTXOs response: {e}")))?;

    Ok(Json(utxos))
}

/// Blacklist UTXOs for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn blacklist_asset_utxos(
    Path(asset_uuid): Path<String>,
    Json(body): Json<Vec<Utxo>>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/utxos/blacklist"
        ))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to blacklist UTXOs: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to blacklist UTXOs with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse blacklist UTXOs response: {e}")))?;

    Ok(Json(result))
}

/// Whitelist UTXOs for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn whitelist_asset_utxos(
    Path(asset_uuid): Path<String>,
    Json(body): Json<Vec<Utxo>>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/utxos/whitelist"
        ))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to whitelist UTXOs: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to whitelist UTXOs with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse whitelist UTXOs response: {e}")))?;

    Ok(Json(result))
}

/// Get reissuances for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_reissuances(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/reissuances"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset reissuances: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset reissuances with status {status}: {error_text}"
        )));
    }

    let reissuances: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset reissuances response: {e}")))?;

    Ok(Json(reissuances))
}

/// Request asset reissuance
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn request_asset_reissue(
    Path(asset_uuid): Path<String>,
    Json(body): Json<ReissueRequest>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/reissue/request"
        ))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to request asset reissue: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to request asset reissue with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse reissue request response: {e}")))?;

    Ok(Json(result))
}

/// Confirm asset reissuance
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn confirm_asset_reissue(
    Path(asset_uuid): Path<String>,
    Json(body): Json<ReissueConfirm>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/reissue/confirm"
        ))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to confirm asset reissue: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to confirm asset reissue with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse reissue confirm response: {e}")))?;

    Ok(Json(result))
}

/// Request asset burn
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn request_asset_burn(
    Path(asset_uuid): Path<String>,
    Json(body): Json<BurnRequest>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/assets/{asset_uuid}/burn/request"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to request asset burn: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to request asset burn with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse burn request response: {e}")))?;

    Ok(Json(result))
}

/// Confirm asset burn
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn confirm_asset_burn(
    Path(asset_uuid): Path<String>,
    Json(body): Json<BurnConfirm>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/assets/{asset_uuid}/burn/confirm"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to confirm asset burn: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to confirm asset burn with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse burn confirm response: {e}")))?;

    Ok(Json(result))
}

// Assignments endpoints
/// Get assignments for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_assignments(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/assignments"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset assignments: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset assignments with status {status}: {error_text}"
        )));
    }

    let assignments: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset assignments response: {e}")))?;

    Ok(Json(assignments))
}

/// Get a specific assignment for an asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_assignment(
    Path((asset_uuid, assignment_id)): Path<(String, i32)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!(
            "{amp_base_url}/assets/{asset_uuid}/assignments/{assignment_id}"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset assignment: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset assignment with status {status}: {error_text}"
        )));
    }

    let assignment: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset assignment response: {e}")))?;

    Ok(Json(assignment))
}

/// Create a new assignment for an asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn create_asset_assignment(
    Path(asset_uuid): Path<String>,

    Json(body): Json<AssignmentCreateBody>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/assets/{asset_uuid}/assignments"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to create asset assignment: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to create asset assignment with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse create assignment response: {e}")))?;

    Ok(Json(result))
}

/// Lock an asset assignment
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn lock_asset_assignment(
    Path((asset_uuid, assignment_id)): Path<(String, i32)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/assignments/{assignment_id}/lock"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to lock assignment: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to lock assignment with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse lock assignment response: {e}")))?;

    Ok(Json(result))
}

/// Unlock an asset assignment
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn unlock_asset_assignment(
    Path((asset_uuid, assignment_id)): Path<(String, i32)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/assignments/{assignment_id}/unlock"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to unlock assignment: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to unlock assignment with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse unlock assignment response: {e}")))?;

    Ok(Json(result))
}

/// Delete an asset assignment
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn delete_asset_assignment(
    Path((asset_uuid, assignment_id)): Path<(String, i32)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!(
            "{amp_base_url}/assets/{asset_uuid}/assignments/{assignment_id}"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to delete assignment: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to delete assignment with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "Assignment deleted successfully" }),
    ))
}

// Distribution endpoints
/// Get distributions for a specific asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_distributions(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/assets/{asset_uuid}/distributions"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset distributions: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset distributions with status {status}: {error_text}"
        )));
    }

    let distributions: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse asset distributions response: {e}"
        ))
    })?;

    Ok(Json(distributions))
}

/// Get a specific distribution for an asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_asset_distribution(
    Path((asset_uuid, distribution_uuid)): Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!(
            "{amp_base_url}/assets/{asset_uuid}/distributions/{distribution_uuid}"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get asset distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get asset distribution with status {status}: {error_text}"
        )));
    }

    let distribution: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse asset distribution response: {e}")))?;

    Ok(Json(distribution))
}

/// Create a new distribution for an asset
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn create_asset_distribution(
    Path(asset_uuid): Path<String>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/assets/{asset_uuid}/distributions"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to create asset distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to create asset distribution with status {status}: {error_text}"
        )));
    }

    let result: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse create distribution response: {e}"
        ))
    })?;

    Ok(Json(result))
}

/// Confirm an asset distribution
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn confirm_asset_distribution(
    Path((asset_uuid, distribution_uuid)): Path<(String, String)>,

    Json(body): Json<DistributionConfirm>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/distributions/{distribution_uuid}/confirm"
        ))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to confirm distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to confirm distribution with status {status}: {error_text}"
        )));
    }

    let result: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse confirm distribution response: {e}"
        ))
    })?;

    Ok(Json(result))
}

/// Cancel an asset distribution
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn cancel_asset_distribution(
    Path((asset_uuid, distribution_uuid)): Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/assets/{asset_uuid}/distributions/{distribution_uuid}/cancel"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to cancel distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to cancel distribution with status {status}: {error_text}"
        )));
    }

    let result: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse cancel distribution response: {e}"
        ))
    })?;

    Ok(Json(result))
}

// Info endpoints
/// Get general information about the AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_info() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/info"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get info: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get info with status {status}: {error_text}"
        )));
    }

    let info: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse info response: {e}")))?;

    Ok(Json(info))
}

/// Get the changelog of the AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_changelog() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/changelog"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get changelog: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get changelog with status {status}: {error_text}"
        )));
    }

    let changelog: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse changelog response: {e}")))?;

    Ok(Json(changelog))
}

// GAID endpoints
/// Validate a GAID
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn validate_gaid(Path(gaid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/gaids/{gaid}/validate"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to validate GAID: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to validate GAID with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse validate GAID response: {e}")))?;

    Ok(Json(result))
}

/// Get GAID address
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_gaid_address(Path(gaid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/gaids/{gaid}/address"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get GAID address: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get GAID address with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse GAID address response: {e}")))?;

    Ok(Json(result))
}

/// Get GAID registered user
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_gaid_registered_user(Path(gaid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/gaids/{gaid}/registered_user"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get GAID registered user: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get GAID registered user with status {status}: {error_text}"
        )));
    }

    let result: Value = response.json().await.map_err(|e| {
        Error::string(&format!(
            "Failed to parse GAID registered user response: {e}"
        ))
    })?;

    Ok(Json(result))
}

/// Get GAID balance
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_gaid_balance(Path(gaid): Path<String>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/gaids/{gaid}/balance"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get GAID balance: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get GAID balance with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse GAID balance response: {e}")))?;

    Ok(Json(result))
}

/// Get GAID asset balance
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_gaid_asset_balance(
    Path((gaid, asset_uuid)): Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!(
            "{amp_base_url}/gaids/{gaid}/assets/{asset_uuid}/balance"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get GAID asset balance: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get GAID asset balance with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse GAID asset balance response: {e}")))?;

    Ok(Json(result))
}

// Manager endpoints
/// Get managers
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_managers() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/managers"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get managers: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get managers with status {status}: {error_text}"
        )));
    }

    let managers: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse managers response: {e}")))?;

    Ok(Json(managers))
}

/// Get current manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_me_manager() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/managers/me"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get current manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get current manager with status {status}: {error_text}"
        )));
    }

    let manager: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse current manager response: {e}")))?;

    Ok(Json(manager))
}

/// Get a specific manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_manager(Path(manager_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/managers/{manager_id}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get manager with status {status}: {error_text}"
        )));
    }

    let manager: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse manager response: {e}")))?;

    Ok(Json(manager))
}

/// Change manager password
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn change_manager_password(
    Path(manager_id): Path<i32>,

    Json(body): Json<ManagerPasswordChange>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/managers/{manager_id}/password"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to change manager password: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to change manager password with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse change password response: {e}")))?;

    Ok(Json(result))
}

/// Lock a manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn lock_manager(Path(manager_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/managers/{manager_id}/lock"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to lock manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to lock manager with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse lock manager response: {e}")))?;

    Ok(Json(result))
}

/// Unlock a manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn unlock_manager(Path(manager_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/managers/{manager_id}/unlock"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to unlock manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to unlock manager with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse unlock manager response: {e}")))?;

    Ok(Json(result))
}

/// Add an asset to a manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn add_manager_asset(
    Path((manager_id, asset_uuid)): Path<(i32, String)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!(
            "{amp_base_url}/managers/{manager_id}/assets/{asset_uuid}"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to add manager asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to add manager asset with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse add manager asset response: {e}")))?;

    Ok(Json(result))
}

/// Remove an asset from a manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn remove_manager_asset(
    Path((manager_id, asset_uuid)): Path<(i32, String)>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!(
            "{amp_base_url}/managers/{manager_id}/assets/{asset_uuid}"
        ))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to remove manager asset: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to remove manager asset with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "Manager asset removed successfully" }),
    ))
}

// Stub implementations for missing handlers
/// Refresh the AMP authentication token
///
/// # Errors
///
/// Returns an error if:
/// - No current token exists to refresh
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn refresh_token_handler() -> Result<Json<Value>, Error> {
    let token = refresh_amp_token().await?;
    Ok(Json(json!({
        "status": "success",
        "message": "Token refreshed successfully",
        "token_length": token.len()
    })))
}

/// Issue a new asset
///
/// # Errors
///
/// Returns an error if:
/// - Job dispatch fails
#[axum::debug_handler]
pub async fn issue_asset_handler(
    State(ctx): State<AppContext>,
    Json(issuance): Json<Issuance>,
) -> Result<Response> {
    let job_id =
        dispatch_amp_job(&ctx, JobKind::IssueAsset, serde_json::to_value(issuance)?).await?;

    let response = JobDispatchResponse {
        job_id,
        status: "accepted".to_string(),
    };

    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// Add a new manager to AMP API
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn add_manager(Json(body): Json<ManagerCreate>) -> Result<Json<Value>, Error> {
    tracing::debug!("Creating manager in AMP API: {:?}", body);

    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/managers"))
        .header("Authorization", format!("token {token}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to create manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to create manager with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse create manager response: {e}")))?;

    tracing::debug!("Manager created successfully");
    Ok(Json(result))
}

/// Edit an existing manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn edit_manager(
    Path(manager_id): Path<i32>,

    Json(body): Json<Value>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .patch(format!("{amp_base_url}/managers/{manager_id}"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to edit manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to edit manager with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse edit manager response: {e}")))?;

    Ok(Json(result))
}

/// Delete a manager
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn delete_manager(Path(manager_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!("{amp_base_url}/managers/{manager_id}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to delete manager: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to delete manager with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "Manager deleted successfully", "manager_id": manager_id }),
    ))
}

/// Get manager authentication information
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_manager_auth(Path(manager_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/managers/{manager_id}/auth"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get manager auth: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get manager auth with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse manager auth response: {e}")))?;

    Ok(Json(result))
}

// Distribution stub implementations
/// Get all distributions
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_distributions() -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/distributions"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get distributions: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get distributions with status {status}: {error_text}"
        )));
    }

    let distributions: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse distributions response: {e}")))?;

    Ok(Json(distributions))
}

/// Get a specific distribution
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn get_distribution(Path(distribution_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .get(format!("{amp_base_url}/distributions/{distribution_id}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to get distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to get distribution with status {status}: {error_text}"
        )));
    }

    let distribution: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse distribution response: {e}")))?;

    Ok(Json(distribution))
}

/// Add a new distribution
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn add_distribution(Json(body): Json<Value>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .post(format!("{amp_base_url}/distributions"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to add distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to add distribution with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse add distribution response: {e}")))?;

    Ok(Json(result))
}

/// Edit an existing distribution
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn edit_distribution(
    Path(distribution_id): Path<i32>,

    Json(body): Json<Value>,
) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .patch(format!("{amp_base_url}/distributions/{distribution_id}"))
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to edit distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to edit distribution with status {status}: {error_text}"
        )));
    }

    let result: Value = response
        .json()
        .await
        .map_err(|e| Error::string(&format!("Failed to parse edit distribution response: {e}")))?;

    Ok(Json(result))
}

/// Delete a distribution
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
#[axum::debug_handler]
pub async fn delete_distribution(Path(distribution_id): Path<i32>) -> Result<Json<Value>, Error> {
    let token = get_amp_token().await?;
    let client = Client::new();
    let amp_base_url = get_amp_api_base_url();

    let response = client
        .delete(format!("{amp_base_url}/distributions/{distribution_id}"))
        .header("Authorization", format!("token {token}"))
        .send()
        .await
        .map_err(|e| Error::string(&format!("Failed to delete distribution: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(Error::string(&format!(
            "Failed to delete distribution with status {status}: {error_text}"
        )));
    }

    Ok(Json(
        json!({ "message": "Distribution deleted successfully", "distribution_id": distribution_id }),
    ))
}

/// Register an asset handler for HTTP routes
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn register_asset_handler(Path(asset_uuid): Path<String>) -> Result<Json<Value>, Error> {
    let result = register_asset(&asset_uuid)
        .await
        .map_err(|e| Error::string(&format!("Failed to register asset: {e}")))?;
    Ok(Json(result))
}

/// Register an authorized asset handler for HTTP routes
///
/// # Errors
///
/// Returns an error if:
/// - Token acquisition fails
/// - The HTTP request fails
/// - The API returns an error response
/// - JSON parsing fails
#[axum::debug_handler]
pub async fn register_authorized_asset_handler(
    Path(asset_uuid): Path<String>,
) -> Result<Json<Value>, Error> {
    let result = register_authorized_asset(&asset_uuid)
        .await
        .map_err(|e| Error::string(&format!("Failed to register authorized asset: {e}")))?;
    Ok(Json(result))
}

// Helper function to configure token management routes
fn configure_token_routes(routes: Routes) -> Routes {
    routes
        .add("/obtain_token", post(obtain_token_handler))
        .add("/token_status", get(token_status_handler))
        .add("/refresh_token", post(refresh_token_handler))
}

// Helper function to configure user management routes
fn configure_user_routes(routes: Routes) -> Routes {
    routes
        .add("/registered_users", get(get_registered_users))
        .add("/registered_users/add", post(add_registered_user))
        .add("/registered_users/{id}", get(get_registered_user))
        .add("/registered_users/{id}/edit", put(edit_registered_user))
        .add(
            "/registered_users/{id}/delete",
            delete(delete_registered_user),
        )
        .add(
            "/registered_users/{id}/summary",
            get(get_registered_user_summary),
        )
        .add(
            "/registered_users/{id}/gaids",
            get(get_registered_user_gaids),
        )
        .add(
            "/registered_users/{id}/gaids/add",
            post(add_registered_user_gaid),
        )
        .add(
            "/registered_users/{id}/gaids/set-default",
            post(set_default_registered_user_gaid),
        )
}

// Helper function to configure category management routes
fn configure_category_routes(routes: Routes) -> Routes {
    routes
        .add("/categories", get(get_categories))
        .add("/categories/add", post(add_category))
        .add("/categories/{id}", get(get_category))
        .add("/categories/{id}/edit", put(edit_category))
        .add("/categories/{id}/delete", delete(delete_category))
        .add(
            "/categories/{category_id}/registered_users/{registered_user_id}/add",
            post(add_registered_user_to_category),
        )
        .add(
            "/categories/{category_id}/registered_users/{registered_user_id}/remove",
            delete(remove_registered_user_from_category),
        )
        .add(
            "/categories/{category_id}/assets/{asset_uuid}/add",
            post(add_asset_to_category_handler),
        )
        .add(
            "/categories/{category_id}/assets/{asset_uuid}/remove",
            delete(remove_asset_from_category),
        )
}

// Helper function to configure asset management routes
fn configure_asset_routes(routes: Routes) -> Routes {
    routes
        .add("/assets", get(get_assets))
        .add("/assets/{uuid}", get(get_asset))
        .add("/assets/{uuid}/edit", put(edit_asset))
        .add("/assets/{uuid}/delete", delete(delete_asset))
        .add("/assets/{uuid}/register", post(register_asset_handler))
        .add(
            "/assets/{uuid}/register-authorized",
            post(register_authorized_asset_handler),
        )
        .add("/assets/{uuid}/lock", post(lock_asset))
        .add("/assets/{uuid}/unlock", post(unlock_asset))
        .add("/assets/{uuid}/activities", get(get_asset_activities))
        .add("/assets/{uuid}/ownerships", get(get_asset_ownerships))
        .add("/assets/{uuid}/balance", get(get_asset_balance))
        .add("/assets/{uuid}/lost_outputs", get(get_asset_lost_outputs))
        .add("/assets/{uuid}/summary", get(get_asset_summary))
        .add("/assets/{uuid}/utxos", get(get_asset_utxos))
        .add(
            "/assets/{uuid}/utxos/blacklist",
            post(blacklist_asset_utxos),
        )
        .add(
            "/assets/{uuid}/utxos/whitelist",
            post(whitelist_asset_utxos),
        )
        .add("/assets/{uuid}/reissuances", get(get_asset_reissuances))
        .add(
            "/assets/{uuid}/reissue/request",
            post(request_asset_reissue),
        )
        .add(
            "/assets/{uuid}/reissue/confirm",
            post(confirm_asset_reissue),
        )
        .add("/assets/{uuid}/burn/request", post(request_asset_burn))
        .add("/assets/{uuid}/burn/confirm", post(confirm_asset_burn))
        .add("/assets/{uuid}/assignments", get(get_asset_assignments))
        .add("/assets/{uuid}/assignments", post(create_asset_assignment))
        .add(
            "/assets/{uuid}/assignments/{assignment_id}",
            get(get_asset_assignment),
        )
        .add(
            "/assets/{uuid}/assignments/{assignment_id}",
            delete(delete_asset_assignment),
        )
        .add(
            "/assets/{uuid}/assignments/{assignment_id}/lock",
            post(lock_asset_assignment),
        )
        .add(
            "/assets/{uuid}/assignments/{assignment_id}/unlock",
            post(unlock_asset_assignment),
        )
        .add("/assets/{uuid}/distributions", get(get_asset_distributions))
        .add(
            "/assets/{uuid}/distributions",
            post(create_asset_distribution),
        )
        .add(
            "/assets/{uuid}/distributions/{distribution_uuid}",
            get(get_asset_distribution),
        )
        .add(
            "/assets/{uuid}/distributions/{distribution_uuid}/confirm",
            post(confirm_asset_distribution),
        )
        .add(
            "/assets/{uuid}/distributions/{distribution_uuid}/cancel",
            post(cancel_asset_distribution),
        )
        .add("/issue_asset", post(issue_asset_handler))
}

// Helper function to configure manager routes
fn configure_manager_routes(routes: Routes) -> Routes {
    routes
        .add("/managers", get(get_managers))
        .add("/managers/{id}", get(get_manager))
        .add("/managers", post(add_manager))
        .add("/managers/{id}", patch(edit_manager))
        .add("/managers/{id}", delete(delete_manager))
        .add("/managers/{id}/auth", get(get_manager_auth))
        .add("/managers/{id}/unlock", post(unlock_manager))
        .add(
            "/managers/{manager_id}/assets/{asset_uuid}",
            post(add_manager_asset),
        )
        .add(
            "/managers/{manager_id}/assets/{asset_uuid}",
            delete(remove_manager_asset),
        )
}

// Helper function to configure distribution routes
fn configure_distribution_routes(routes: Routes) -> Routes {
    routes
        .add("/distributions", get(get_distributions))
        .add("/distributions/{id}", get(get_distribution))
        .add("/distributions", post(add_distribution))
        .add("/distributions/{id}", patch(edit_distribution))
        .add("/distributions/{id}", delete(delete_distribution))
}

// Define all routes for AMP controller
pub fn routes() -> Routes {
    let mut routes = Routes::new().prefix("/api/amp");

    // Configure each group of routes
    routes = configure_token_routes(routes);
    routes = configure_user_routes(routes);
    routes = configure_category_routes(routes);
    routes = configure_asset_routes(routes);
    routes = configure_manager_routes(routes);
    routes = configure_distribution_routes(routes);

    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_info() {
        // Set up test environment variables if needed
        std::env::set_var("AMP_API_BASE_URL", "https://test.amp.blockstream.com");
        std::env::set_var("AMP_USERNAME", "test_user");
        std::env::set_var("AMP_PASSWORD", "test_password");

        // Since we can't use AppContext::default(), we'll test the functions that don't need real context
        // Testing that the functions compile and have the right signatures is valuable

        // Test get_amp_api_base_url
        let base_url = get_amp_api_base_url();
        assert_eq!(base_url, "https://test.amp.blockstream.com");
    }

    #[tokio::test]
    async fn test_token_request_response_types() {
        // Test that our types serialize/deserialize correctly
        let token_request = TokenRequest {
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        let json = serde_json::to_string(&token_request).unwrap();
        assert!(json.contains("username"));
        assert!(json.contains("password"));

        let token_response_json = r#"{"token": "test_token"}"#;
        let token_response: TokenResponse = serde_json::from_str(token_response_json).unwrap();
        assert_eq!(token_response.token, "test_token");
    }

    #[tokio::test]
    async fn test_issuance_types() {
        // Test Issuance struct serialization
        let issuance = Issuance {
            name: "Test Asset".to_string(),
            amount: 1000,
            destination_address: "test_address".to_string(),
            domain: "test.com".to_string(),
            ticker: "TEST".to_string(),
            precision: 8,
            pubkey: "test_pubkey".to_string(),
            is_confidential: false,
            is_reissuable: true,
            reissuance_amount: 100,
            reissuance_address: "reissue_address".to_string(),
            transfer_restricted: false,
            category_id: Some(1),
        };

        let json = serde_json::to_string(&issuance).unwrap();
        assert!(json.contains("\"name\":\"Test Asset\""));
        assert!(json.contains("\"amount\":1000"));
        assert!(json.contains("\"is_reissuable\":true"));
    }
}
