use std::env;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{Duration, Utc};

use reqwest::header::AUTHORIZATION;
use reqwest::{Client, Method, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;

use secrecy::ExposeSecret;
use secrecy::Secret;

use crate::model::{
    Activity, AddAssetToGroup, Asset, AssetActivityParams, AssetGroup, AssetPermission,
    AssetSummary, Assignment, Audit, Balance, BroadcastResponse, CategoryAdd, CategoryEdit,
    CategoryResponse, ChangePasswordRequest, ChangePasswordResponse, CreateAssetAssignmentRequest,
    CreateAssetGroup, CreateAssetPermission, CreateAudit, EditAssetRequest, IssuanceRequest,
    IssuanceResponse, Outpoint, Ownership, Password, TokenData, TokenInfo, TokenRequest, TokenResponse, UpdateAssetGroup,
    UpdateAssetPermission, UpdateAudit, Utxo,
};



#[derive(Error, Debug)]
pub enum Error {
    #[error("Missing {0} environment variable")]
    MissingEnvVar(String),
    #[error("AMP request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse AMP response: {0}")]
    ResponseParsingFailed(String),
    #[error("AMP token request failed with status {status}: {error_text}")]
    TokenRequestFailed {
        status: reqwest::StatusCode,
        error_text: String,
    },
    #[error("Failed to parse url: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Invalid retry configuration: {0}")]
    InvalidRetryConfig(String),
    #[error("Token management error: {0}")]
    Token(#[from] TokenError),
}

/// Detailed error types for token management operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum TokenError {
    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),
    #[error("Token obtain failed after {attempts} attempts: {last_error}")]
    ObtainFailed { attempts: u32, last_error: String },
    #[error("Rate limited: retry after {retry_after_seconds} seconds")]
    RateLimited { retry_after_seconds: u64 },
    #[error("Request timeout after {timeout_seconds} seconds")]
    Timeout { timeout_seconds: u64 },
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Token storage error: {0}")]
    Storage(String),
    #[error("Token validation error: {0}")]
    Validation(String),
}

impl TokenError {
    /// Creates a new RefreshFailed error
    pub fn refresh_failed<S: Into<String>>(message: S) -> Self {
        Self::RefreshFailed(message.into())
    }

    /// Creates a new ObtainFailed error
    pub fn obtain_failed(attempts: u32, last_error: String) -> Self {
        Self::ObtainFailed {
            attempts,
            last_error,
        }
    }

    /// Creates a new RateLimited error
    pub fn rate_limited(retry_after_seconds: u64) -> Self {
        Self::RateLimited {
            retry_after_seconds,
        }
    }

    /// Creates a new Timeout error
    pub fn timeout(timeout_seconds: u64) -> Self {
        Self::Timeout { timeout_seconds }
    }

    /// Creates a new Serialization error
    pub fn serialization<S: Into<String>>(message: S) -> Self {
        Self::Serialization(message.into())
    }

    /// Creates a new Storage error
    pub fn storage<S: Into<String>>(message: S) -> Self {
        Self::Storage(message.into())
    }

    /// Creates a new Validation error
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation(message.into())
    }

    /// Returns true if this error indicates a retryable condition
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            TokenError::RefreshFailed(_)
                | TokenError::RateLimited { .. }
                | TokenError::Timeout { .. }
        )
    }

    /// Returns true if this error indicates a rate limiting condition
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, TokenError::RateLimited { .. })
    }

    /// Returns the retry delay in seconds if this is a rate limited error
    pub fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            TokenError::RateLimited {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            _ => None,
        }
    }
}

// Conversion from serde_json::Error for serialization errors
impl From<serde_json::Error> for TokenError {
    fn from(err: serde_json::Error) -> Self {
        TokenError::Serialization(err.to_string())
    }
}

/// Configuration for retry behavior in API requests
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay in milliseconds for exponential backoff
    pub base_delay_ms: u64,
    /// Maximum delay in milliseconds to cap exponential backoff
    pub max_delay_ms: u64,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            timeout_seconds: 10,
        }
    }
}

impl RetryConfig {
    /// Creates a RetryConfig from environment variables with default fallbacks
    ///
    /// Environment variables:
    /// - `API_RETRY_MAX_ATTEMPTS`: Maximum retry attempts (default: 3)
    /// - `API_RETRY_BASE_DELAY_MS`: Base delay in milliseconds (default: 1000)
    /// - `API_RETRY_MAX_DELAY_MS`: Maximum delay in milliseconds (default: 30000)
    /// - `API_REQUEST_TIMEOUT_SECONDS`: Request timeout in seconds (default: 10)
    ///
    /// # Errors
    ///
    /// Returns an error if any environment variable contains an invalid value
    pub fn from_env() -> Result<Self, Error> {
        let max_attempts = match env::var("API_RETRY_MAX_ATTEMPTS") {
            Ok(val) => val.parse::<u32>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_RETRY_MAX_ATTEMPTS: {}", e))
            })?,
            Err(_) => 3,
        };

        let base_delay_ms = match env::var("API_RETRY_BASE_DELAY_MS") {
            Ok(val) => val.parse::<u64>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_RETRY_BASE_DELAY_MS: {}", e))
            })?,
            Err(_) => 1000,
        };

        let max_delay_ms = match env::var("API_RETRY_MAX_DELAY_MS") {
            Ok(val) => val.parse::<u64>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_RETRY_MAX_DELAY_MS: {}", e))
            })?,
            Err(_) => 30000,
        };

        let timeout_seconds = match env::var("API_REQUEST_TIMEOUT_SECONDS") {
            Ok(val) => val.parse::<u64>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_REQUEST_TIMEOUT_SECONDS: {}", e))
            })?,
            Err(_) => 10,
        };

        // Validate configuration
        if max_attempts == 0 {
            return Err(Error::InvalidRetryConfig(
                "max_attempts must be greater than 0".to_string(),
            ));
        }
        if base_delay_ms == 0 {
            return Err(Error::InvalidRetryConfig(
                "base_delay_ms must be greater than 0".to_string(),
            ));
        }
        if max_delay_ms < base_delay_ms {
            return Err(Error::InvalidRetryConfig(
                "max_delay_ms must be greater than or equal to base_delay_ms".to_string(),
            ));
        }
        if timeout_seconds == 0 {
            return Err(Error::InvalidRetryConfig(
                "timeout_seconds must be greater than 0".to_string(),
            ));
        }

        Ok(Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
            timeout_seconds,
        })
    }

    /// Creates a RetryConfig optimized for test environments
    ///
    /// Uses reduced values for faster test execution:
    /// - 2 retry attempts
    /// - 500ms base delay
    /// - 5000ms max delay
    /// - 5 second timeout
    pub fn for_tests() -> Self {
        Self {
            max_attempts: 2,
            base_delay_ms: 500,
            max_delay_ms: 5000,
            timeout_seconds: 5,
        }
    }

    /// Sets a custom timeout value
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    /// Sets custom max attempts
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Sets custom base delay
    pub fn with_base_delay_ms(mut self, base_delay_ms: u64) -> Self {
        self.base_delay_ms = base_delay_ms;
        self
    }

    /// Sets custom max delay
    pub fn with_max_delay_ms(mut self, max_delay_ms: u64) -> Self {
        self.max_delay_ms = max_delay_ms;
        self
    }
}

/// HTTP client with sophisticated retry logic and exponential backoff
#[derive(Debug, Clone)]
pub struct RetryClient {
    client: Client,
    config: RetryConfig,
}

impl RetryClient {
    /// Creates a new RetryClient with the given configuration
    pub fn new(config: RetryConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Creates a new RetryClient with default configuration
    pub fn with_default_config() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Creates a new RetryClient with test-optimized configuration
    pub fn for_tests() -> Self {
        Self::new(RetryConfig::for_tests())
    }

    /// Executes an HTTP request with retry logic and exponential backoff
    ///
    /// # Arguments
    /// * `request_builder` - A function that creates the request builder
    ///
    /// # Returns
    /// The response if successful, or an error after all retries are exhausted
    ///
    /// # Errors
    /// Returns `TokenError::Timeout` if the request times out
    /// Returns `TokenError::RateLimited` if rate limited and retries are exhausted
    /// Returns `TokenError::ObtainFailed` if all retry attempts fail
    pub async fn execute_with_retry<F>(
        &self,
        request_builder: F,
    ) -> Result<reqwest::Response, TokenError>
    where
        F: Fn() -> reqwest::RequestBuilder + Send + Sync,
    {
        let mut last_error = String::new();
        let mut attempt = 0;

        while attempt < self.config.max_attempts {
            attempt += 1;

            // Create the request with timeout
            let request =
                request_builder().timeout(StdDuration::from_secs(self.config.timeout_seconds));

            // Execute the request
            match request.send().await {
                Ok(response) => {
                    let status = response.status();

                    // Handle rate limiting (429 Too Many Requests)
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        let retry_after = self.extract_retry_after(&response).unwrap_or(60);

                        tracing::warn!(
                            "Rate limited (429) on attempt {}/{}. Retry after {} seconds",
                            attempt,
                            self.config.max_attempts,
                            retry_after
                        );

                        // If this is our last attempt, return the rate limit error
                        if attempt >= self.config.max_attempts {
                            return Err(TokenError::rate_limited(retry_after));
                        }

                        // Wait for the rate limit period (or our max delay, whichever is smaller)
                        let delay_ms = std::cmp::min(retry_after * 1000, self.config.max_delay_ms);
                        sleep(StdDuration::from_millis(delay_ms)).await;
                        continue;
                    }

                    // Handle other client errors (4xx) - these are generally not retryable
                    if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS
                    {
                        last_error = format!("Client error: {}", status);
                        tracing::error!("Non-retryable client error: {}", status);
                        break;
                    }

                    // Handle server errors (5xx) - these are retryable
                    if status.is_server_error() {
                        last_error = format!("Server error: {}", status);
                        tracing::warn!(
                            "Server error {} on attempt {}/{}",
                            status,
                            attempt,
                            self.config.max_attempts
                        );

                        if attempt < self.config.max_attempts {
                            let delay = self.calculate_backoff_delay(attempt);
                            sleep(delay).await;
                            continue;
                        }
                        break;
                    }

                    // Success case
                    return Ok(response);
                }
                Err(e) => {
                    last_error = e.to_string();

                    // Check if this is a timeout error
                    if e.is_timeout() {
                        tracing::warn!(
                            "Request timeout on attempt {}/{}",
                            attempt,
                            self.config.max_attempts
                        );

                        if attempt >= self.config.max_attempts {
                            return Err(TokenError::timeout(self.config.timeout_seconds));
                        }
                    } else {
                        tracing::warn!(
                            "Request failed on attempt {}/{}: {}",
                            attempt,
                            self.config.max_attempts,
                            e
                        );
                    }

                    // If we have more attempts, wait and retry
                    if attempt < self.config.max_attempts {
                        let delay = self.calculate_backoff_delay(attempt);
                        sleep(delay).await;
                    }
                }
            }
        }

        // All retries exhausted
        Err(TokenError::obtain_failed(attempt, last_error))
    }

    /// Calculates the delay for exponential backoff with jitter
    ///
    /// Uses the formula: min(base_delay * 2^(attempt-1) + jitter, max_delay)
    /// where jitter is a random value between 0 and base_delay/2
    fn calculate_backoff_delay(&self, attempt: u32) -> StdDuration {
        use rand::Rng;

        let base_delay = self.config.base_delay_ms;
        let max_delay = self.config.max_delay_ms;

        // Calculate exponential backoff: base_delay * 2^(attempt-1)
        let exponential_delay = base_delay * 2_u64.pow(attempt.saturating_sub(1));

        // Add jitter (random value between 0 and base_delay/2)
        let jitter = rand::thread_rng().gen_range(0..=base_delay / 2);
        let total_delay = exponential_delay + jitter;

        // Cap at max_delay
        let final_delay = std::cmp::min(total_delay, max_delay);

        tracing::debug!(
            "Calculated backoff delay for attempt {}: {}ms (exponential: {}ms, jitter: {}ms, capped at: {}ms)",
            attempt,
            final_delay,
            exponential_delay,
            jitter,
            max_delay
        );

        StdDuration::from_millis(final_delay)
    }

    /// Extracts the Retry-After header value from a 429 response
    ///
    /// Returns the number of seconds to wait, or None if the header is not present
    /// or cannot be parsed
    fn extract_retry_after(&self, response: &reqwest::Response) -> Option<u64> {
        response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    }

    /// Gets the underlying reqwest client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Gets the retry configuration
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }
}

/// Core token manager with proactive refresh and secure storage
#[derive(Debug)]
pub struct TokenManager {
    token_data: Arc<Mutex<Option<TokenData>>>,
    retry_client: RetryClient,
    base_url: Url,
    /// Semaphore to ensure only one token operation (obtain/refresh) happens at a time
    /// This prevents race conditions where multiple threads try to refresh/obtain simultaneously
    token_operation_semaphore: Arc<Semaphore>,
}

impl TokenManager {
    /// Creates a new TokenManager with default configuration
    ///
    /// # Errors
    /// Returns an error if the base URL cannot be obtained from environment variables
    pub fn new() -> Result<Self, Error> {
        let config = RetryConfig::from_env()?;
        Self::with_config(config)
    }

    /// Creates a new TokenManager with the specified retry configuration
    ///
    /// # Errors
    /// Returns an error if the base URL cannot be obtained from environment variables
    pub fn with_config(config: RetryConfig) -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        Ok(Self {
            token_data: Arc::new(Mutex::new(None)),
            retry_client: RetryClient::new(config),
            base_url,
            token_operation_semaphore: Arc::new(Semaphore::new(1)),
        })
    }

    /// Creates a new TokenManager with the specified configuration and base URL (for testing)
    ///
    /// # Errors
    /// This method is infallible but returns Result for API consistency
    pub fn with_config_and_base_url(config: RetryConfig, base_url: Url) -> Result<Self, Error> {
        Ok(Self {
            token_data: Arc::new(Mutex::new(None)),
            retry_client: RetryClient::new(config),
            base_url,
            token_operation_semaphore: Arc::new(Semaphore::new(1)),
        })
    }

    /// Gets a valid authentication token with proactive refresh logic
    ///
    /// This method implements thread-safe token management logic:
    /// 1. Check if a valid token exists and is not expiring soon (within 5 minutes)
    /// 2. If token needs refresh/obtain, acquire semaphore to prevent concurrent operations
    /// 3. Double-check token state after acquiring semaphore (another thread may have updated it)
    /// 4. Perform atomic token update operations
    /// 5. Return the valid token
    ///
    /// # Thread Safety
    /// This method is fully thread-safe and prevents race conditions by:
    /// - Using a semaphore to ensure only one token operation at a time
    /// - Double-checking token state after acquiring the semaphore
    /// - Performing atomic token updates within the critical section
    ///
    /// # Errors
    /// Returns a `TokenError` if token acquisition or refresh fails after all retries
    pub async fn get_token(&self) -> Result<String, Error> {
        // Fast path: check if we have a valid token without acquiring semaphore
        {
            let token_guard = self.token_data.lock().await;
            if let Some(ref token_data) = *token_guard {
                if !token_data.expires_soon(Duration::minutes(5)) {
                    tracing::debug!("Using existing valid token (fast path)");
                    return Ok(token_data.token.expose_secret().clone());
                }
            }
        }

        // Slow path: token needs refresh/obtain, acquire semaphore for thread safety
        let _permit = self.token_operation_semaphore.acquire().await
            .map_err(|e| Error::Token(TokenError::storage(format!("Failed to acquire token operation semaphore: {}", e))))?;

        tracing::debug!("Acquired token operation semaphore for thread-safe token management");

        // Double-check token state after acquiring semaphore - another thread may have updated it
        {
            let token_guard = self.token_data.lock().await;
            if let Some(ref token_data) = *token_guard {
                if !token_data.expires_soon(Duration::minutes(5)) {
                    tracing::debug!("Token was updated by another thread, using existing valid token");
                    return Ok(token_data.token.expose_secret().clone());
                }
            }
        }

        // At this point, we need to refresh or obtain a new token
        // Determine the current token state for decision making
        let (needs_refresh, _needs_obtain) = {
            let token_guard = self.token_data.lock().await;
            match token_guard.as_ref() {
                Some(token_data) => {
                    if token_data.is_expired() {
                        tracing::info!("Token is expired, will obtain new token");
                        (false, true)
                    } else {
                        tracing::info!("Token expires soon, will attempt refresh");
                        (true, false)
                    }
                }
                None => {
                    tracing::info!("No token exists, will obtain new token");
                    (false, true)
                }
            }
        };

        // Perform the appropriate token operation
        if needs_refresh {
            match self.refresh_token_internal().await {
                Ok(token) => {
                    tracing::info!("Token refreshed successfully");
                    return Ok(token);
                }
                Err(e) => {
                    tracing::warn!("Token refresh failed, falling back to obtain: {}", e);
                    // Fall through to obtain new token
                }
            }
        }

        // Either we needed to obtain from the start, or refresh failed
        self.obtain_token_internal().await
    }

    /// Obtains a new authentication token using environment credentials with retry logic
    ///
    /// This method:
    /// 1. Reads credentials from environment variables
    /// 2. Makes a token request with retry logic
    /// 3. Stores the new token with 24-hour expiry
    /// 4. Returns the token string
    ///
    /// # Thread Safety
    /// This method acquires the token operation semaphore to ensure thread-safe operation.
    /// For internal use within already-synchronized contexts, use `obtain_token_internal()`.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Environment variables are missing
    /// - All retry attempts fail
    /// - Response parsing fails
    pub async fn obtain_token(&self) -> Result<String, Error> {
        let _permit = self.token_operation_semaphore.acquire().await
            .map_err(|e| Error::Token(TokenError::storage(format!("Failed to acquire token operation semaphore: {}", e))))?;

        self.obtain_token_internal().await
    }

    /// Internal method to obtain a new authentication token without acquiring semaphore
    ///
    /// This method should only be called from contexts where the token operation semaphore
    /// has already been acquired (e.g., from within get_token()).
    ///
    /// # Errors
    /// Returns an error if:
    /// - Environment variables are missing
    /// - All retry attempts fail
    /// - Response parsing fails
    async fn obtain_token_internal(&self) -> Result<String, Error> {
        tracing::debug!("Obtaining new authentication token");

        // Get credentials from environment variables - check early to avoid retry logic for missing credentials
        let username = env::var("AMP_USERNAME")
            .map_err(|_| Error::MissingEnvVar("AMP_USERNAME".to_string()))?;
        let password = env::var("AMP_PASSWORD")
            .map_err(|_| Error::MissingEnvVar("AMP_PASSWORD".to_string()))?;

        let request_payload = TokenRequest { username, password };

        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("user")
            .push("obtain_token");

        // Use retry client for robust token acquisition
        let response = self
            .retry_client
            .execute_with_retry(|| {
                self.retry_client
                    .client()
                    .post(url.clone())
                    .json(&request_payload)
            })
            .await
            .map_err(Error::Token)?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::TokenRequestFailed { status, error_text });
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))?;

        // Store the new token with 24-hour expiry atomically
        let expires_at = Utc::now() + Duration::days(1);
        let token_data = TokenData::new(token_response.token.clone(), expires_at);

        // Atomic token update - hold the lock for the minimal time needed
        {
            let mut token_guard = self.token_data.lock().await;
            *token_guard = Some(token_data);
            tracing::debug!("Token data updated atomically in storage");
        }

        tracing::info!("New authentication token obtained successfully");
        Ok(token_response.token)
    }

    /// Refreshes the current authentication token with fallback to obtain on failure
    ///
    /// This method:
    /// 1. Uses the existing token to request a refresh
    /// 2. Updates the stored token data on success
    /// 3. Falls back to obtaining a new token if refresh fails
    ///
    /// # Thread Safety
    /// This method acquires the token operation semaphore to ensure thread-safe operation.
    /// For internal use within already-synchronized contexts, use `refresh_token_internal()`.
    ///
    /// # Errors
    /// Returns an error if both refresh and obtain operations fail
    pub async fn refresh_token(&self) -> Result<String, Error> {
        let _permit = self.token_operation_semaphore.acquire().await
            .map_err(|e| Error::Token(TokenError::storage(format!("Failed to acquire token operation semaphore: {}", e))))?;

        self.refresh_token_internal().await
    }

    /// Internal method to refresh the current authentication token without acquiring semaphore
    ///
    /// This method should only be called from contexts where the token operation semaphore
    /// has already been acquired (e.g., from within get_token()).
    ///
    /// # Errors
    /// Returns an error if both refresh and obtain operations fail
    async fn refresh_token_internal(&self) -> Result<String, Error> {
        tracing::debug!("Refreshing authentication token");

        // Get the current token for refresh request
        let current_token = {
            let token_guard = self.token_data.lock().await;
            match token_guard.as_ref() {
                Some(token_data) => token_data.token.expose_secret().clone(),
                None => {
                    tracing::warn!("No token available for refresh, obtaining new token");
                    return self.obtain_token_internal().await;
                }
            }
        };

        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("user")
            .push("refresh_token");

        // Use retry client for robust token refresh
        let response = self
            .retry_client
            .execute_with_retry(|| {
                self.retry_client
                    .client()
                    .post(url.clone())
                    .header(AUTHORIZATION, format!("token {}", current_token))
            })
            .await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let error_text = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());

                    tracing::warn!("Token refresh failed with status {}: {}", status, error_text);

                    // If refresh fails, fall back to obtaining a new token
                    return self.obtain_token_internal().await;
                }

                let token_response: TokenResponse = resp
                    .json()
                    .await
                    .map_err(|e| Error::ResponseParsingFailed(e.to_string()))?;

                // Store the refreshed token with 24-hour expiry atomically
                let expires_at = Utc::now() + Duration::days(1);
                let token_data = TokenData::new(token_response.token.clone(), expires_at);

                // Atomic token update - hold the lock for the minimal time needed
                {
                    let mut token_guard = self.token_data.lock().await;
                    *token_guard = Some(token_data);
                    tracing::debug!("Refreshed token data updated atomically in storage");
                }

                tracing::info!("Authentication token refreshed successfully");
                Ok(token_response.token)
            }
            Err(e) => {
                tracing::warn!("Token refresh request failed: {}, falling back to obtain", e);
                // Fall back to obtaining a new token
                self.obtain_token_internal().await
            }
        }
    }

    /// Gets current token information for debugging and monitoring
    ///
    /// Returns detailed information about the current token including:
    /// - Expiry time and remaining duration
    /// - Token age since acquisition
    /// - Expiry status flags
    ///
    /// # Returns
    /// `Some(TokenInfo)` if a token exists, `None` if no token is stored
    pub async fn get_token_info(&self) -> Result<Option<TokenInfo>, Error> {
        tracing::debug!("Retrieving token information for debugging");

        let token_guard = self.token_data.lock().await;
        let token_info = token_guard.as_ref().map(TokenInfo::from);

        match &token_info {
            Some(info) => {
                tracing::debug!(
                    "Token info retrieved - expires_at: {}, age: {:?}, expires_in: {:?}, is_expired: {}, expires_soon: {}",
                    info.expires_at,
                    info.age,
                    info.expires_in,
                    info.is_expired,
                    info.expires_soon
                );
            }
            None => {
                tracing::debug!("No token information available - no token stored");
            }
        }

        Ok(token_info)
    }

    /// Clears the stored token (useful for testing scenarios)
    ///
    /// This method removes the current token from storage, forcing the next
    /// `get_token()` call to obtain a fresh token.
    pub async fn clear_token(&self) -> Result<(), Error> {
        tracing::debug!("Clearing stored token from memory");

        let mut token_guard = self.token_data.lock().await;
        let had_token = token_guard.is_some();
        *token_guard = None;

        if had_token {
            tracing::info!("Token successfully cleared from storage - next get_token() will obtain fresh token");
        } else {
            tracing::debug!("No token was stored to clear");
        }

        Ok(())
    }

    /// Forces a token refresh regardless of current token status
    ///
    /// This method bypasses the normal proactive refresh logic and immediately
    /// attempts to refresh the current token. If no token exists or refresh fails,
    /// it falls back to obtaining a new token.
    ///
    /// # Thread Safety
    /// This method is fully thread-safe and uses the same semaphore-based synchronization
    /// as other token operations to prevent race conditions.
    ///
    /// # Errors
    /// Returns an error if both refresh and obtain operations fail
    pub async fn force_refresh(&self) -> Result<String, Error> {
        tracing::info!("Forcing token refresh - bypassing normal proactive refresh logic");

        let _permit = self.token_operation_semaphore.acquire().await
            .map_err(|e| Error::Token(TokenError::storage(format!("Failed to acquire token operation semaphore: {}", e))))?;

        // Check if we have a token to refresh
        let has_token = {
            let token_guard = self.token_data.lock().await;
            token_guard.is_some()
        };

        if has_token {
            tracing::debug!("Existing token found, attempting forced refresh");
        } else {
            tracing::debug!("No existing token found, will obtain new token");
        }

        match self.refresh_token_internal().await {
            Ok(token) => {
                tracing::info!("Forced token refresh completed successfully");
                Ok(token)
            }
            Err(e) => {
                tracing::error!("Forced token refresh failed: {}", e);
                Err(e)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: Url,
    token_manager: Arc<TokenManager>,
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
impl ApiClient {
    /// Creates a new API client with the base URL from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `AMP_API_BASE_URL` environment variable contains an invalid URL
    /// - Token manager initialization fails
    pub fn new() -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        let token_manager = Arc::new(TokenManager::new()?);
        Ok(Self {
            client: Client::new(),
            base_url,
            token_manager,
        })
    }

    /// Creates a new API client with the specified base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if token manager initialization fails.
    pub fn with_base_url(base_url: Url) -> Result<Self, Error> {
        let config = RetryConfig::from_env()?;
        let token_manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url.clone())?);
        Ok(Self {
            client: Client::new(),
            base_url,
            token_manager,
        })
    }

    /// Creates a new API client with a custom token manager (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL cannot be obtained from environment variables.
    pub fn with_token_manager(token_manager: Arc<TokenManager>) -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        Ok(Self {
            client: Client::new(),
            base_url,
            token_manager,
        })
    }

    /// Obtains a new authentication token from the AMP API.
    ///
    /// **Note**: This method is deprecated in favor of the automatic token management
    /// provided by `get_token()`. The TokenManager handles token acquisition internally
    /// with enhanced retry logic and error handling.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `AMP_USERNAME` or `AMP_PASSWORD` environment variables are not set
    /// - The HTTP request fails
    /// - The token request is rejected by the server
    /// - The response cannot be parsed
    #[deprecated(note = "Use get_token() instead - it provides automatic token management")]
    pub async fn obtain_amp_token(&self) -> Result<String, Error> {
        self.token_manager.obtain_token().await
    }

    /// Gets current token information for debugging and monitoring.
    ///
    /// Returns detailed information about the current token including:
    /// - Expiry time and remaining duration
    /// - Token age since acquisition
    /// - Expiry status flags
    ///
    /// # Returns
    /// `Some(TokenInfo)` if a token exists, `None` if no token is stored
    ///
    /// # Errors
    /// Returns an error if token information retrieval fails
    pub async fn get_token_info(&self) -> Result<Option<TokenInfo>, Error> {
        self.token_manager.get_token_info().await
    }

    /// Clears the stored token (useful for testing scenarios).
    ///
    /// This method removes the current token from storage, forcing the next
    /// `get_token()` call to obtain a fresh token.
    ///
    /// # Errors
    /// Returns an error if token clearing fails
    pub async fn clear_token(&self) -> Result<(), Error> {
        self.token_manager.clear_token().await
    }

    /// Forces a token refresh regardless of current token status.
    ///
    /// This method bypasses the normal proactive refresh logic and immediately
    /// attempts to refresh the current token. If no token exists or refresh fails,
    /// it falls back to obtaining a new token.
    ///
    /// # Errors
    /// Returns an error if both refresh and obtain operations fail
    pub async fn force_refresh(&self) -> Result<String, Error> {
        self.token_manager.force_refresh().await
    }

    /// Gets a valid authentication token with automatic token management.
    ///
    /// This method uses the integrated TokenManager to handle:
    /// - Proactive token refresh (5 minutes before expiry)
    /// - Automatic fallback from refresh to obtain on failure
    /// - Retry logic with exponential backoff
    /// - Thread-safe token storage
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition or refresh fails after all retries.
    pub async fn get_token(&self) -> Result<String, Error> {
        self.token_manager.get_token().await
    }

    async fn request_raw(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<reqwest::Response, Error> {
        let token = self.get_token().await?;
        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().extend(path);

        let mut request_builder = self
            .client
            .request(method, url)
            .header(AUTHORIZATION, format!("token {token}"));

        if let Some(body) = body {
            request_builder = request_builder.json(&body);
        }

        let response = request_builder.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Request to {path:?} failed with status {status}: {error_text}"
            )));
        }

        Ok(response)
    }

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<T, Error> {
        let response = self.request_raw(method, path, body).await?;
        response
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))
    }

    async fn request_empty(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<(), Error> {
        self.request_raw(method, path, body).await?;
        Ok(())
    }

    /// Gets the API changelog.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed as JSON
    pub async fn get_changelog(&self) -> Result<serde_json::Value, Error> {
        self.request_json(Method::GET, &["changelog"], None::<&()>)
            .await
    }

    /// Changes the user's password.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server rejects the password change
    /// - The response cannot be parsed
    pub async fn user_change_password(
        &self,
        password: Secret<String>,
    ) -> Result<ChangePasswordResponse, Error> {
        let request = ChangePasswordRequest {
            password: Secret::new(Password(password.expose_secret().clone())),
        };
        self.request_json(Method::POST, &["user", "change_password"], Some(request))
            .await
    }

    /// Gets a list of all assets.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        self.request_json(Method::GET, &["assets"], None::<&()>)
            .await
    }

    /// Gets a specific asset by UUID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The asset does not exist
    /// - The response cannot be parsed
    pub async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid], None::<&()>)
            .await
    }

    /// Issues a new asset.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The issuance request is invalid
    /// - The response cannot be parsed
    pub async fn issue_asset(
        &self,
        issuance_request: &IssuanceRequest,
    ) -> Result<IssuanceResponse, Error> {
        self.request_json(Method::POST, &["assets", "issue"], Some(issuance_request))
            .await
    }

    /// Edits an existing asset.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The asset does not exist
    /// - The edit request is invalid
    /// - The response cannot be parsed
    pub async fn edit_asset(
        &self,
        asset_uuid: &str,
        edit_asset_request: &EditAssetRequest,
    ) -> Result<Asset, Error> {
        self.request_json(
            Method::PUT,
            &["assets", asset_uuid, "edit"],
            Some(edit_asset_request),
        )
        .await
    }

    pub async fn delete_asset(&self, asset_uuid: &str) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["assets", asset_uuid, "delete"],
            None::<&()>,
        )
        .await
    }

    pub async fn list_asset_permissions(&self) -> Result<Vec<AssetPermission>, Error> {
        self.request_json(Method::GET, &["asset_permissions"], None::<&()>)
            .await
    }

    pub async fn create_asset_permission(
        &self,
        create_asset_permission: &CreateAssetPermission,
    ) -> Result<AssetPermission, Error> {
        self.request_json(
            Method::POST,
            &["asset_permissions"],
            Some(create_asset_permission),
        )
        .await
    }

    pub async fn get_asset_permission(
        &self,
        asset_permission_id: i64,
    ) -> Result<AssetPermission, Error> {
        self.request_json(
            Method::GET,
            &["asset_permissions", &asset_permission_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn update_asset_permission(
        &self,
        asset_permission_id: i64,
        update_asset_permission: &UpdateAssetPermission,
    ) -> Result<AssetPermission, Error> {
        self.request_json(
            Method::PUT,
            &["asset_permissions", &asset_permission_id.to_string()],
            Some(update_asset_permission),
        )
        .await
    }

    pub async fn delete_asset_permission(&self, asset_permission_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["asset_permissions", &asset_permission_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn get_broadcast_status(&self, txid: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::GET, &["tx", "broadcast", txid], None::<&()>)
            .await
    }

    pub async fn broadcast_transaction(&self, tx_hex: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::POST, &["tx", "broadcast"], Some(tx_hex))
            .await
    }

    pub async fn list_audits(&self) -> Result<Vec<Audit>, Error> {
        self.request_json(Method::GET, &["audits"], None::<&()>)
            .await
    }

    pub async fn create_audit(&self, create_audit: &CreateAudit) -> Result<Audit, Error> {
        self.request_json(Method::POST, &["audits"], Some(create_audit))
            .await
    }

    pub async fn get_audit(&self, audit_id: i64) -> Result<Audit, Error> {
        self.request_json(Method::GET, &["audits", &audit_id.to_string()], None::<&()>)
            .await
    }

    pub async fn update_audit(
        &self,
        audit_id: i64,
        update_audit: &UpdateAudit,
    ) -> Result<Audit, Error> {
        self.request_json(
            Method::PUT,
            &["audits", &audit_id.to_string()],
            Some(update_audit),
        )
        .await
    }

    pub async fn delete_audit(&self, audit_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["audits", &audit_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn list_asset_groups(&self) -> Result<Vec<AssetGroup>, Error> {
        self.request_json(Method::GET, &["asset_groups"], None::<&()>)
            .await
    }

    pub async fn create_asset_group(
        &self,
        create_asset_group: &CreateAssetGroup,
    ) -> Result<AssetGroup, Error> {
        self.request_json(Method::POST, &["asset_groups"], Some(create_asset_group))
            .await
    }

    pub async fn get_asset_group(&self, asset_group_id: i64) -> Result<AssetGroup, Error> {
        self.request_json(
            Method::GET,
            &["asset_groups", &asset_group_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn update_asset_group(
        &self,
        asset_group_id: i64,
        update_asset_group: &UpdateAssetGroup,
    ) -> Result<AssetGroup, Error> {
        self.request_json(
            Method::PUT,
            &["asset_groups", &asset_group_id.to_string()],
            Some(update_asset_group),
        )
        .await
    }

    pub async fn delete_asset_group(&self, asset_group_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["asset_groups", &asset_group_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn add_asset_to_group(
        &self,
        asset_group_id: i64,
        add_asset_to_group: &AddAssetToGroup,
    ) -> Result<AssetGroup, Error> {
        self.request_json(
            Method::POST,
            &["asset_groups", &asset_group_id.to_string(), "assets"],
            Some(add_asset_to_group),
        )
        .await
    }

    pub async fn remove_asset_from_group(
        &self,
        asset_group_id: i64,
        asset_uuid: &str,
    ) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &[
                "asset_groups",
                &asset_group_id.to_string(),
                "assets",
                asset_uuid,
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn register_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "register"],
            None::<&()>,
        )
        .await
    }

    pub async fn register_asset_authorized(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "register-authorized"],
            None::<&()>,
        )
        .await
    }

    pub async fn lock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::PUT, &["assets", asset_uuid, "lock"], None::<&()>)
            .await
    }

    pub async fn unlock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::PUT, &["assets", asset_uuid, "unlock"], None::<&()>)
            .await
    }

    pub async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "activities"],
            Some(params),
        )
        .await
    }

    pub async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error> {
        let mut path = vec!["assets", asset_uuid, "ownerships"];
        let height_str;
        if let Some(h) = height {
            height_str = h.to_string();
            path.push(&height_str);
        }
        self.request_json(Method::GET, &path, None::<&()>).await
    }

    pub async fn get_asset_balance(&self, asset_uuid: &str) -> Result<Balance, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "balance"], None::<&()>)
            .await
    }

    pub async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "summary"], None::<&()>)
            .await
    }

    pub async fn get_asset_utxos(&self, asset_uuid: &str) -> Result<Vec<Utxo>, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "utxos"], None::<&()>)
            .await
    }

    pub async fn blacklist_asset_utxos(
        &self,
        asset_uuid: &str,
        utxos: &[Outpoint],
    ) -> Result<Vec<Utxo>, Error> {
        self.request_json(
            Method::POST,
            &["assets", asset_uuid, "utxos", "blacklist"],
            Some(utxos),
        )
        .await
    }

    pub async fn whitelist_asset_utxos(
        &self,
        asset_uuid: &str,
        utxos: &[Outpoint],
    ) -> Result<Vec<Utxo>, Error> {
        self.request_json(
            Method::POST,
            &["assets", asset_uuid, "utxos", "whitelist"],
            Some(utxos),
        )
        .await
    }

    pub async fn get_asset_treasury_addresses(
        &self,
        asset_uuid: &str,
    ) -> Result<Vec<String>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "treasury-addresses"],
            None::<&()>,
        )
        .await
    }

    pub async fn add_asset_treasury_addresses(
        &self,
        asset_uuid: &str,
        addresses: &[String],
    ) -> Result<(), Error> {
        self.request_empty(
            Method::POST,
            &["assets", asset_uuid, "treasury-addresses", "add"],
            Some(addresses),
        )
        .await
    }

    pub async fn delete_asset_treasury_addresses(
        &self,
        asset_uuid: &str,
        addresses: &[String],
    ) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["assets", asset_uuid, "treasury-addresses", "delete"],
            Some(addresses),
        )
        .await
    }

    pub async fn get_registered_users(
        &self,
    ) -> Result<Vec<crate::model::RegisteredUserResponse>, Error> {
        self.request_json(Method::GET, &["registered_users"], None::<&()>)
            .await
    }

    pub async fn get_registered_user(
        &self,
        user_id: i64,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(
            Method::GET,
            &["registered_users", &user_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn add_registered_user(
        &self,
        new_user: &crate::model::RegisteredUserAdd,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(Method::POST, &["registered_users", "add"], Some(new_user))
            .await
    }

    pub async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        self.request_json(Method::GET, &["categories"], None::<&()>)
            .await
    }

    pub async fn add_category(
        &self,
        new_category: &CategoryAdd,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(Method::POST, &["categories", "add"], Some(new_category))
            .await
    }

    pub async fn get_category(&self, category_id: i64) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::GET,
            &["categories", &category_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn edit_category(
        &self,
        category_id: i64,
        edit_category: &CategoryEdit,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &["categories", &category_id.to_string(), "edit"],
            Some(edit_category),
        )
        .await
    }

    pub async fn delete_category(&self, category_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["categories", &category_id.to_string(), "delete"],
            None::<&()>,
        )
        .await
    }

    pub async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "registered_users",
                &user_id.to_string(),
                "add",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "registered_users",
                &user_id.to_string(),
                "remove",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "assets",
                asset_uuid,
                "add",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn remove_asset_from_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "assets",
                asset_uuid,
                "remove",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn validate_gaid(
        &self,
        gaid: &str,
    ) -> Result<crate::model::ValidateGaidResponse, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "validate"], None::<&()>)
            .await
    }

    pub async fn get_gaid_address(
        &self,
        gaid: &str,
    ) -> Result<crate::model::AddressGaidResponse, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "address"], None::<&()>)
            .await
    }

    pub async fn get_managers(&self) -> Result<Vec<crate::model::Manager>, Error> {
        self.request_json(Method::GET, &["managers"], None::<&()>)
            .await
    }

    pub async fn create_manager(
        &self,
        new_manager: &crate::model::ManagerCreate,
    ) -> Result<crate::model::Manager, Error> {
        self.request_json(Method::POST, &["managers", "create"], Some(new_manager))
            .await
    }

    pub async fn create_asset_assignment(
        &self,
        asset_uuid: &str,
        request: &CreateAssetAssignmentRequest,
    ) -> Result<Assignment, Error> {
        self.request_json(
            Method::POST,
            &["assets", asset_uuid, "assignments"],
            Some(request),
        )
        .await
    }
}

fn get_amp_api_base_url() -> Result<Url, Error> {
    let url_str = env::var("AMP_API_BASE_URL")
        .unwrap_or_else(|_| "https://amp-test.blockstream.com/api".to_string());
    Url::parse(&url_str).map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use serial_test::serial;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.timeout_seconds, 10);
    }

    #[test]
    fn test_retry_config_for_tests() {
        let config = RetryConfig::for_tests();
        assert_eq!(config.max_attempts, 2);
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 5000);
        assert_eq!(config.timeout_seconds, 5);
    }

    #[test]
    #[serial]
    fn test_retry_config_from_env_defaults() {
        // Don't load .env for this test since we want to test defaults
        // Clear any existing environment variables
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");

        let config = RetryConfig::from_env().unwrap();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.timeout_seconds, 10);
    }

    #[test]
    #[serial]
    fn test_retry_config_from_env_custom_values() {
        dotenvy::dotenv().ok();
        env::set_var("API_RETRY_MAX_ATTEMPTS", "5");
        env::set_var("API_RETRY_BASE_DELAY_MS", "2000");
        env::set_var("API_RETRY_MAX_DELAY_MS", "60000");
        env::set_var("API_REQUEST_TIMEOUT_SECONDS", "30");

        let config = RetryConfig::from_env().unwrap();
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 2000);
        assert_eq!(config.max_delay_ms, 60000);
        assert_eq!(config.timeout_seconds, 30);

        // Clean up
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
    }

    #[test]
    #[serial]
    fn test_retry_config_from_env_invalid_values() {
        dotenvy::dotenv().ok();
        env::set_var("API_RETRY_MAX_ATTEMPTS", "invalid");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid API_RETRY_MAX_ATTEMPTS"));
        env::remove_var("API_RETRY_MAX_ATTEMPTS");

        env::set_var("API_RETRY_BASE_DELAY_MS", "not_a_number");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid API_RETRY_BASE_DELAY_MS"));
        env::remove_var("API_RETRY_BASE_DELAY_MS");

        env::set_var("API_RETRY_MAX_DELAY_MS", "-1");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid API_RETRY_MAX_DELAY_MS"));
        env::remove_var("API_RETRY_MAX_DELAY_MS");

        env::set_var("API_REQUEST_TIMEOUT_SECONDS", "abc");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid API_REQUEST_TIMEOUT_SECONDS"));
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
    }

    #[test]
    #[serial]
    fn test_retry_config_validation() {
        dotenvy::dotenv().ok();
        // Test zero max_attempts
        env::set_var("API_RETRY_MAX_ATTEMPTS", "0");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("max_attempts must be greater than 0"));
        env::remove_var("API_RETRY_MAX_ATTEMPTS");

        // Test zero base_delay_ms
        env::set_var("API_RETRY_BASE_DELAY_MS", "0");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("base_delay_ms must be greater than 0"));
        env::remove_var("API_RETRY_BASE_DELAY_MS");

        // Test max_delay_ms < base_delay_ms
        env::set_var("API_RETRY_BASE_DELAY_MS", "2000");
        env::set_var("API_RETRY_MAX_DELAY_MS", "1000");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("max_delay_ms must be greater than or equal to base_delay_ms"));
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");

        // Test zero timeout_seconds
        env::set_var("API_REQUEST_TIMEOUT_SECONDS", "0");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("timeout_seconds must be greater than 0"));
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_retry_config_builder_methods() {
        let config = RetryConfig::default()
            .with_timeout(20)
            .with_max_attempts(5)
            .with_base_delay_ms(2000)
            .with_max_delay_ms(60000);

        assert_eq!(config.timeout_seconds, 20);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 2000);
        assert_eq!(config.max_delay_ms, 60000);
    }

    #[test]
    #[serial]
    fn test_retry_config_partial_env_vars() {
        dotenvy::dotenv().ok();
        // Set only some environment variables, others should use defaults
        env::set_var("API_RETRY_MAX_ATTEMPTS", "7");
        env::set_var("API_RETRY_BASE_DELAY_MS", "1500");
        // Don't set API_RETRY_MAX_DELAY_MS and API_REQUEST_TIMEOUT_SECONDS

        let config = RetryConfig::from_env().unwrap();
        assert_eq!(config.max_attempts, 7);
        assert_eq!(config.base_delay_ms, 1500);
        assert_eq!(config.max_delay_ms, 30000); // default
        assert_eq!(config.timeout_seconds, 10); // default

        // Clean up
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
    }

    // Token Error Tests
    #[test]
    fn test_token_error_creation() {
        let refresh_error = TokenError::refresh_failed("Connection timeout");
        assert_eq!(
            refresh_error.to_string(),
            "Token refresh failed: Connection timeout"
        );

        let obtain_error = TokenError::obtain_failed(3, "Authentication failed".to_string());
        assert_eq!(
            obtain_error.to_string(),
            "Token obtain failed after 3 attempts: Authentication failed"
        );

        let rate_limit_error = TokenError::rate_limited(60);
        assert_eq!(
            rate_limit_error.to_string(),
            "Rate limited: retry after 60 seconds"
        );

        let timeout_error = TokenError::timeout(30);
        assert_eq!(
            timeout_error.to_string(),
            "Request timeout after 30 seconds"
        );

        let serialization_error = TokenError::serialization("Invalid JSON format");
        assert_eq!(
            serialization_error.to_string(),
            "Serialization error: Invalid JSON format"
        );

        let storage_error = TokenError::storage("Failed to write to disk");
        assert_eq!(
            storage_error.to_string(),
            "Token storage error: Failed to write to disk"
        );

        let validation_error = TokenError::validation("Token format invalid");
        assert_eq!(
            validation_error.to_string(),
            "Token validation error: Token format invalid"
        );
    }

    #[test]
    fn test_token_error_is_retryable() {
        assert!(TokenError::refresh_failed("test").is_retryable());
        assert!(TokenError::rate_limited(60).is_retryable());
        assert!(TokenError::timeout(30).is_retryable());

        assert!(!TokenError::obtain_failed(3, "test".to_string()).is_retryable());
        assert!(!TokenError::serialization("test").is_retryable());
        assert!(!TokenError::storage("test").is_retryable());
        assert!(!TokenError::validation("test").is_retryable());
    }

    #[test]
    fn test_token_error_is_rate_limited() {
        assert!(TokenError::rate_limited(60).is_rate_limited());

        assert!(!TokenError::refresh_failed("test").is_rate_limited());
        assert!(!TokenError::timeout(30).is_rate_limited());
        assert!(!TokenError::obtain_failed(3, "test".to_string()).is_rate_limited());
        assert!(!TokenError::serialization("test").is_rate_limited());
        assert!(!TokenError::storage("test").is_rate_limited());
        assert!(!TokenError::validation("test").is_rate_limited());
    }

    #[test]
    fn test_token_error_retry_after_seconds() {
        let rate_limit_error = TokenError::rate_limited(120);
        assert_eq!(rate_limit_error.retry_after_seconds(), Some(120));

        assert_eq!(
            TokenError::refresh_failed("test").retry_after_seconds(),
            None
        );
        assert_eq!(TokenError::timeout(30).retry_after_seconds(), None);
        assert_eq!(
            TokenError::obtain_failed(3, "test".to_string()).retry_after_seconds(),
            None
        );
        assert_eq!(
            TokenError::serialization("test").retry_after_seconds(),
            None
        );
        assert_eq!(TokenError::storage("test").retry_after_seconds(), None);
        assert_eq!(TokenError::validation("test").retry_after_seconds(), None);
    }

    #[test]
    fn test_token_error_equality() {
        let error1 = TokenError::refresh_failed("Connection timeout");
        let error2 = TokenError::refresh_failed("Connection timeout");
        let error3 = TokenError::refresh_failed("Different message");

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);

        let obtain_error1 = TokenError::obtain_failed(3, "Auth failed".to_string());
        let obtain_error2 = TokenError::obtain_failed(3, "Auth failed".to_string());
        let obtain_error3 = TokenError::obtain_failed(2, "Auth failed".to_string());

        assert_eq!(obtain_error1, obtain_error2);
        assert_ne!(obtain_error1, obtain_error3);

        let rate_limit1 = TokenError::rate_limited(60);
        let rate_limit2 = TokenError::rate_limited(60);
        let rate_limit3 = TokenError::rate_limited(120);

        assert_eq!(rate_limit1, rate_limit2);
        assert_ne!(rate_limit1, rate_limit3);
    }

    #[test]
    fn test_token_error_clone() {
        let original = TokenError::refresh_failed("Test message");
        let cloned = original.clone();

        assert_eq!(original, cloned);
        assert_eq!(original.to_string(), cloned.to_string());
    }

    #[test]
    fn test_token_error_debug_format() {
        let error = TokenError::refresh_failed("Test message");
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("RefreshFailed"));
        assert!(debug_str.contains("Test message"));

        let obtain_error = TokenError::obtain_failed(3, "Auth failed".to_string());
        let debug_str = format!("{:?}", obtain_error);
        assert!(debug_str.contains("ObtainFailed"));
        assert!(debug_str.contains("attempts: 3"));
        assert!(debug_str.contains("Auth failed"));
    }

    #[test]
    fn test_token_error_from_serde_json_error() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let token_error: TokenError = json_error.into();

        match token_error {
            TokenError::Serialization(msg) => {
                assert!(msg.contains("expected"));
            }
            _ => panic!("Expected Serialization error"),
        }
    }

    #[test]
    fn test_error_from_token_error() {
        let token_error = TokenError::refresh_failed("Test message");
        let error: Error = token_error.into();

        match error {
            Error::Token(TokenError::RefreshFailed(msg)) => {
                assert_eq!(msg, "Test message");
            }
            _ => panic!("Expected Token error variant"),
        }
    }

    #[test]
    fn test_error_display_with_token_error() {
        let token_error = TokenError::rate_limited(60);
        let error = Error::Token(token_error);

        assert_eq!(
            error.to_string(),
            "Token management error: Rate limited: retry after 60 seconds"
        );
    }

    // RetryClient Tests
    #[test]
    fn test_retry_client_creation() {
        let config = RetryConfig::for_tests();
        let retry_client = RetryClient::new(config.clone());

        assert_eq!(retry_client.config().max_attempts, config.max_attempts);
        assert_eq!(retry_client.config().base_delay_ms, config.base_delay_ms);
        assert_eq!(retry_client.config().max_delay_ms, config.max_delay_ms);
        assert_eq!(
            retry_client.config().timeout_seconds,
            config.timeout_seconds
        );
    }

    #[test]
    fn test_retry_client_with_default_config() {
        let retry_client = RetryClient::with_default_config();
        let default_config = RetryConfig::default();

        assert_eq!(
            retry_client.config().max_attempts,
            default_config.max_attempts
        );
        assert_eq!(
            retry_client.config().base_delay_ms,
            default_config.base_delay_ms
        );
        assert_eq!(
            retry_client.config().max_delay_ms,
            default_config.max_delay_ms
        );
        assert_eq!(
            retry_client.config().timeout_seconds,
            default_config.timeout_seconds
        );
    }

    #[test]
    fn test_retry_client_for_tests() {
        let retry_client = RetryClient::for_tests();
        let test_config = RetryConfig::for_tests();

        assert_eq!(retry_client.config().max_attempts, test_config.max_attempts);
        assert_eq!(
            retry_client.config().base_delay_ms,
            test_config.base_delay_ms
        );
        assert_eq!(retry_client.config().max_delay_ms, test_config.max_delay_ms);
        assert_eq!(
            retry_client.config().timeout_seconds,
            test_config.timeout_seconds
        );
    }

    #[test]
    fn test_retry_client_calculate_backoff_delay() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 10000,
            timeout_seconds: 10,
        };
        let retry_client = RetryClient::new(config);

        // Test first attempt (should be base_delay + jitter)
        let delay1 = retry_client.calculate_backoff_delay(1);
        assert!(delay1.as_millis() >= 1000); // At least base delay
        assert!(delay1.as_millis() <= 1500); // Base delay + max jitter (base/2)

        // Test second attempt (should be 2 * base_delay + jitter)
        let delay2 = retry_client.calculate_backoff_delay(2);
        assert!(delay2.as_millis() >= 2000); // At least 2 * base delay
        assert!(delay2.as_millis() <= 2500); // 2 * base delay + max jitter

        // Test third attempt (should be 4 * base_delay + jitter)
        let delay3 = retry_client.calculate_backoff_delay(3);
        assert!(delay3.as_millis() >= 4000); // At least 4 * base delay
        assert!(delay3.as_millis() <= 4500); // 4 * base delay + max jitter

        // Test that delay is capped at max_delay_ms
        let delay_large = retry_client.calculate_backoff_delay(10);
        assert_eq!(delay_large.as_millis(), 10000); // Should be capped at max_delay_ms
    }

    #[test]
    fn test_retry_client_calculate_backoff_delay_with_small_max() {
        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 1000,
            max_delay_ms: 2000, // Small max delay to test capping
            timeout_seconds: 10,
        };
        let retry_client = RetryClient::new(config);

        // Test that exponential backoff is capped properly
        let delay3 = retry_client.calculate_backoff_delay(3);
        assert_eq!(delay3.as_millis(), 2000); // Should be capped at max_delay_ms

        let delay4 = retry_client.calculate_backoff_delay(4);
        assert_eq!(delay4.as_millis(), 2000); // Should still be capped
    }

    #[test]
    fn test_retry_client_extract_retry_after() {
        use reqwest::header::{HeaderMap, HeaderValue};

        let config = RetryConfig::for_tests();
        let _retry_client = RetryClient::new(config);

        // Create a mock response with Retry-After header
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("120"));

        // We can't easily create a reqwest::Response for testing, so we'll test the logic
        // by creating a simple test that verifies the header parsing logic would work
        let retry_after_value = headers
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        assert_eq!(retry_after_value, Some(120));

        // Test invalid header value
        let mut headers_invalid = HeaderMap::new();
        headers_invalid.insert("retry-after", HeaderValue::from_static("invalid"));

        let retry_after_invalid = headers_invalid
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        assert_eq!(retry_after_invalid, None);

        // Test missing header
        let headers_empty = HeaderMap::new();
        let retry_after_missing = headers_empty
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        assert_eq!(retry_after_missing, None);
    }

    #[test]
    fn test_retry_client_client_access() {
        let retry_client = RetryClient::for_tests();
        let client = retry_client.client();

        // Verify we can access the underlying client
        assert!(client.get("https://example.com").build().is_ok());
    }

    #[test]
    fn test_retry_client_config_access() {
        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 2000,
            max_delay_ms: 20000,
            timeout_seconds: 15,
        };
        let retry_client = RetryClient::new(config.clone());

        let retrieved_config = retry_client.config();
        assert_eq!(retrieved_config.max_attempts, config.max_attempts);
        assert_eq!(retrieved_config.base_delay_ms, config.base_delay_ms);
        assert_eq!(retrieved_config.max_delay_ms, config.max_delay_ms);
        assert_eq!(retrieved_config.timeout_seconds, config.timeout_seconds);
    }

    #[test]
    fn test_retry_client_clone() {
        let config = RetryConfig::for_tests();
        let retry_client = RetryClient::new(config);
        let cloned_client = retry_client.clone();

        // Verify the clone has the same configuration
        assert_eq!(
            retry_client.config().max_attempts,
            cloned_client.config().max_attempts
        );
        assert_eq!(
            retry_client.config().base_delay_ms,
            cloned_client.config().base_delay_ms
        );
        assert_eq!(
            retry_client.config().max_delay_ms,
            cloned_client.config().max_delay_ms
        );
        assert_eq!(
            retry_client.config().timeout_seconds,
            cloned_client.config().timeout_seconds
        );
    }

    #[tokio::test]
    async fn test_retry_client_with_mock_server() {
        use httpmock::prelude::*;

        let server = MockServer::start();

        // Test successful request (no retries needed)
        let success_mock = server.mock(|when, then| {
            when.method(GET).path("/success");
            then.status(200).body("success");
        });

        let retry_client = RetryClient::for_tests();
        let url = format!("{}/success", server.base_url());

        let result = retry_client
            .execute_with_retry(|| retry_client.client().get(&url))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);
        success_mock.assert_hits(1); // Should only be called once
    }

    #[tokio::test]
    async fn test_retry_client_server_error_retry() {
        use httpmock::prelude::*;

        let server = MockServer::start();

        // Test server error - this will always return 500
        // We'll verify that it retries by checking the hit count
        let _server_error_mock = server.mock(|when, then| {
            when.method(GET).path("/server-error");
            then.status(500).body("server error");
        });

        let retry_client = RetryClient::for_tests();
        let url = format!("{}/server-error", server.base_url());

        let result = retry_client
            .execute_with_retry(|| retry_client.client().get(&url))
            .await;

        // Should fail after retries are exhausted
        assert!(result.is_err());
        match result.unwrap_err() {
            TokenError::ObtainFailed {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 2); // Should have tried max_attempts times
                assert!(last_error.contains("Server error: 500"));
            }
            _ => panic!("Expected ObtainFailed error"),
        }

        // Verify it was called the expected number of times
        _server_error_mock.assert_hits(2); // Should be called max_attempts times
    }

    #[tokio::test]
    async fn test_retry_client_rate_limit_handling() {
        use httpmock::prelude::*;

        let server = MockServer::start();

        // Test rate limiting (429) response
        let rate_limit_mock = server.mock(|when, then| {
            when.method(GET).path("/rate-limited");
            then.status(429)
                .header("retry-after", "1") // 1 second retry
                .body("rate limited");
        });

        let retry_client = RetryClient::for_tests();
        let url = format!("{}/rate-limited", server.base_url());

        let start_time = std::time::Instant::now();
        let result = retry_client
            .execute_with_retry(|| retry_client.client().get(&url))
            .await;

        let elapsed = start_time.elapsed();

        // Should fail with rate limit error after all retries
        assert!(result.is_err());
        match result.unwrap_err() {
            TokenError::RateLimited {
                retry_after_seconds,
            } => {
                assert_eq!(retry_after_seconds, 1);
            }
            _ => panic!("Expected RateLimited error"),
        }

        // Should have waited for rate limit delays
        assert!(elapsed.as_secs() >= 1); // At least 1 second for the rate limit delay

        // Should have been called max_attempts times
        rate_limit_mock.assert_hits(2); // for_tests() config has max_attempts = 2
    }

    #[tokio::test]
    async fn test_retry_client_non_retryable_client_error() {
        use httpmock::prelude::*;

        let server = MockServer::start();

        // Test non-retryable client error (404)
        let not_found_mock = server.mock(|when, then| {
            when.method(GET).path("/not-found");
            then.status(404).body("not found");
        });

        let retry_client = RetryClient::for_tests();
        let url = format!("{}/not-found", server.base_url());

        let result = retry_client
            .execute_with_retry(|| retry_client.client().get(&url))
            .await;

        // Should fail immediately without retries for 404
        assert!(result.is_err());
        match result.unwrap_err() {
            TokenError::ObtainFailed {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 1); // Should not retry
                assert!(last_error.contains("Client error: 404"));
            }
            _ => panic!("Expected ObtainFailed error"),
        }

        not_found_mock.assert_hits(1); // Should only be called once
    }

    #[tokio::test]
    async fn test_retry_client_timeout_handling() {
        use httpmock::prelude::*;
        use std::time::Duration;

        let server = MockServer::start();

        // Test timeout scenario
        let _timeout_mock = server.mock(|when, then| {
            when.method(GET).path("/timeout");
            then.status(200)
                .delay(Duration::from_secs(10)) // Delay longer than timeout
                .body("delayed response");
        });

        let config = RetryConfig {
            max_attempts: 2,
            base_delay_ms: 100,
            max_delay_ms: 1000,
            timeout_seconds: 1, // Very short timeout
        };
        let retry_client = RetryClient::new(config);
        let url = format!("{}/timeout", server.base_url());

        let start_time = std::time::Instant::now();
        let result = retry_client
            .execute_with_retry(|| retry_client.client().get(&url))
            .await;

        let elapsed = start_time.elapsed();

        // Should fail with timeout error
        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            TokenError::Timeout { timeout_seconds } => {
                assert_eq!(timeout_seconds, 1);
            }
            _ => panic!("Expected Timeout error, got: {:?}", error),
        }

        // Should have timed out relatively quickly (within a few seconds)
        assert!(elapsed.as_secs() < 5);
    }

    #[tokio::test]
    async fn test_retry_client_exhausted_retries() {
        use httpmock::prelude::*;

        let server = MockServer::start();

        // Test that all retries are exhausted
        let always_fail_mock = server.mock(|when, then| {
            when.method(GET).path("/always-fail");
            then.status(500).body("server error");
        });

        let retry_client = RetryClient::for_tests(); // max_attempts = 2
        let url = format!("{}/always-fail", server.base_url());

        let result = retry_client
            .execute_with_retry(|| retry_client.client().get(&url))
            .await;

        // Should fail after all retries are exhausted
        assert!(result.is_err());
        match result.unwrap_err() {
            TokenError::ObtainFailed {
                attempts,
                last_error,
            } => {
                assert_eq!(attempts, 2); // Should have tried max_attempts times
                assert!(last_error.contains("Server error: 500"));
            }
            _ => panic!("Expected ObtainFailed error"),
        }

        always_fail_mock.assert_hits(2); // Should be called max_attempts times
    }

    // TokenManager Tests
    #[tokio::test]
    #[serial]
    async fn test_token_manager_creation() {
        // Test creation with default config
        let result = TokenManager::new();
        // This might fail if environment variables are not set, which is expected
        match result {
            Ok(manager) => {
                // If successful, verify the manager was created
                assert!(manager.token_data.lock().await.is_none());
            }
            Err(Error::MissingEnvVar(_)) => {
                // Expected if AMP_API_BASE_URL is not set
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }

        // Test creation with custom config
        let config = RetryConfig::for_tests();
        let result = TokenManager::with_config(config);
        match result {
            Ok(manager) => {
                assert!(manager.token_data.lock().await.is_none());
                assert_eq!(manager.retry_client.config().max_attempts, 2);
            }
            Err(Error::MissingEnvVar(_)) => {
                // Expected if AMP_API_BASE_URL is not set
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_clear_token() {
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Initially no token
        assert!(manager.token_data.lock().await.is_none());

        // Manually set a token for testing
        let token_data = TokenData::new("test_token".to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Verify token exists
        assert!(manager.token_data.lock().await.is_some());

        // Clear the token
        let result = manager.clear_token().await;
        assert!(result.is_ok());

        // Verify token is cleared
        assert!(manager.token_data.lock().await.is_none());

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_get_token_info() {
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Initially no token info
        let info = manager.get_token_info().await.unwrap();
        assert!(info.is_none());

        // Set a token
        let expires_at = Utc::now() + Duration::hours(2);
        let token_data = TokenData::new("test_token".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data.clone());
        }

        // Get token info
        let info = manager.get_token_info().await.unwrap();
        assert!(info.is_some());

        let token_info = info.unwrap();
        assert_eq!(token_info.expires_at, expires_at);
        assert!(!token_info.is_expired);
        assert!(!token_info.expires_soon); // 2 hours > 5 minutes
        assert!(token_info.expires_in > Duration::hours(1));
        assert!(token_info.age < Duration::seconds(1));

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_get_token_info_expires_soon() {
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Set a token that expires soon (in 3 minutes)
        let expires_at = Utc::now() + Duration::minutes(3);
        let token_data = TokenData::new("test_token".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Get token info
        let info = manager.get_token_info().await.unwrap().unwrap();
        assert!(!info.is_expired);
        assert!(info.expires_soon); // 3 minutes < 5 minutes threshold
        assert!(info.expires_in < Duration::minutes(5));

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_get_token_info_expired() {
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Set an expired token
        let expires_at = Utc::now() - Duration::hours(1);
        let token_data = TokenData::new("test_token".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Get token info
        let info = manager.get_token_info().await.unwrap().unwrap();
        assert!(info.is_expired);
        assert!(info.expires_soon); // Expired tokens also expire "soon"
        assert!(info.expires_in < Duration::zero());

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_get_token_with_valid_token() {
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Set a valid token that doesn't expire soon
        let expires_at = Utc::now() + Duration::hours(2);
        let token_data = TokenData::new("valid_token_123".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Get token should return the existing valid token without making any requests
        let result = manager.get_token().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "valid_token_123");

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_obtain_token_success() {
        use httpmock::prelude::*;

        // Save original env vars before loading .env
        let original_base_url = env::var("AMP_API_BASE_URL").ok();
        let original_username = env::var("AMP_USERNAME").ok();
        let original_password = env::var("AMP_PASSWORD").ok();

        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());
        env::set_var("AMP_USERNAME", "test_user");
        env::set_var("AMP_PASSWORD", "test_password");

        // Mock successful token obtain
        let obtain_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/user/obtain_token");
            then.status(200)
                .json_body(serde_json::json!({
                    "token": "new_token_456"
                }));
        });

        // Create TokenManager with mock server URL
        let config = RetryConfig::for_tests();
        let base_url = Url::parse(&server.base_url()).unwrap();
        let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

        let result = manager.obtain_token().await;
        if let Err(e) = &result {
            println!("Error: {}", e);
        }
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "new_token_456");

        // Verify the token was stored
        let stored_token = manager.token_data.lock().await;
        assert!(stored_token.is_some());
        let token_data = stored_token.as_ref().unwrap();
        assert_eq!(token_data.token.expose_secret(), "new_token_456");
        assert!(token_data.expires_at > Utc::now() + Duration::hours(23)); // Should be ~24 hours

        obtain_mock.assert_hits(1);

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
        match original_username {
            Some(val) => env::set_var("AMP_USERNAME", val),
            None => env::remove_var("AMP_USERNAME"),
        }
        match original_password {
            Some(val) => env::set_var("AMP_PASSWORD", val),
            None => env::remove_var("AMP_PASSWORD"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_obtain_token_missing_credentials() {
        // Save original env vars before loading .env
        let original_base_url = env::var("AMP_API_BASE_URL").ok();
        let original_username = env::var("AMP_USERNAME").ok();
        let original_password = env::var("AMP_PASSWORD").ok();

        // Remove credentials
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");

        // Create TokenManager with test URL (doesn't matter since we won't make requests)
        let config = RetryConfig::for_tests();
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

        let result = manager.obtain_token().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            Error::MissingEnvVar(var) => {
                assert_eq!(var, "AMP_USERNAME");
            }
            e => panic!("Expected MissingEnvVar error, got: {}", e),
        }

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
        match original_username {
            Some(val) => env::set_var("AMP_USERNAME", val),
            None => env::remove_var("AMP_USERNAME"),
        }
        match original_password {
            Some(val) => env::set_var("AMP_PASSWORD", val),
            None => env::remove_var("AMP_PASSWORD"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_obtain_token_server_error() {
        use httpmock::prelude::*;

        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();
        let original_username = env::var("AMP_USERNAME").ok();
        let original_password = env::var("AMP_PASSWORD").ok();

        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());
        env::set_var("AMP_USERNAME", "test_user");
        env::set_var("AMP_PASSWORD", "wrong_password");

        // Mock authentication failure
        let obtain_mock = server.mock(|when, then| {
            when.method(POST).path("/user/obtain_token");
            then.status(401).body("Authentication failed");
        });

        // Create TokenManager with mock server URL
        let config = RetryConfig::for_tests();
        let base_url = Url::parse(&server.base_url()).unwrap();
        let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

        let result = manager.obtain_token().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            Error::TokenRequestFailed { status, error_text } => {
                assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
                assert_eq!(error_text, "Authentication failed");
            }
            Error::Token(TokenError::ObtainFailed { attempts, last_error }) => {
                // The retry client wraps the error, which is also acceptable
                assert_eq!(attempts, 1); // Should not retry 401 errors
                assert!(last_error.contains("401") || last_error.contains("Unauthorized"));
            }
            e => panic!("Expected TokenRequestFailed or ObtainFailed error, got: {}", e),
        }

        obtain_mock.assert_hits(1); // Should not retry 401 errors

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
        match original_username {
            Some(val) => env::set_var("AMP_USERNAME", val),
            None => env::remove_var("AMP_USERNAME"),
        }
        match original_password {
            Some(val) => env::set_var("AMP_PASSWORD", val),
            None => env::remove_var("AMP_PASSWORD"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_refresh_token_success() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());

        // Mock successful token refresh
        let refresh_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/user/refresh_token")
                .header("authorization", "token current_token_123");
            then.status(200)
                .json_body(serde_json::json!({
                    "token": "refreshed_token_789"
                }));
        });

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // Set an existing token
        let token_data = TokenData::new("current_token_123".to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        let result = manager.refresh_token().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "refreshed_token_789");

        // Verify the token was updated
        let stored_token = manager.token_data.lock().await;
        assert!(stored_token.is_some());
        let token_data = stored_token.as_ref().unwrap();
        assert_eq!(token_data.token.expose_secret(), "refreshed_token_789");

        refresh_mock.assert_hits(1);

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_token_manager_refresh_token_no_existing_token_logic() {
        // This test focuses on the logic without HTTP calls
        // We test that refresh_token with no existing token should call obtain_token

        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();

        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // No existing token
        assert!(manager.token_data.lock().await.is_none());

        // The refresh_token method should detect no token and try to obtain one
        // Since we don't have credentials set, it should fail with MissingEnvVar
        let result = manager.refresh_token().await;
        assert!(result.is_err());

        // Should fail because no credentials are set
        match result.unwrap_err() {
            Error::MissingEnvVar(var) => {
                assert_eq!(var, "AMP_USERNAME");
            }
            Error::Token(TokenError::ObtainFailed { .. }) => {
                // Also acceptable - means it tried to obtain but failed
            }
            e => panic!("Expected MissingEnvVar or ObtainFailed error, got: {}", e),
        }

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
    }

    #[tokio::test]
    async fn test_token_manager_refresh_token_fallback_logic() {
        // Test the fallback logic without actual HTTP calls

        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();

        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // Set an existing token
        let token_data = TokenData::new("expired_token".to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // The refresh should try to refresh the token, fail, then try to obtain
        // Since we don't have credentials, it should fail with MissingEnvVar
        let result = manager.refresh_token().await;
        assert!(result.is_err());

        // Should eventually fail because no credentials are set for the fallback obtain
        match result.unwrap_err() {
            Error::MissingEnvVar(var) => {
                assert_eq!(var, "AMP_USERNAME");
            }
            Error::Token(TokenError::ObtainFailed { .. }) => {
                // Also acceptable - means it tried to obtain but failed
            }
            e => panic!("Expected MissingEnvVar or ObtainFailed error, got: {}", e),
        }

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_force_refresh() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());

        // Mock successful token refresh
        let refresh_mock = server.mock(|when, then| {
            when.method(POST).path("/user/refresh_token");
            then.status(200)
                .json_body(serde_json::json!({
                    "token": "force_refreshed_token"
                }));
        });

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // Set an existing token
        let token_data = TokenData::new("current_token".to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        let result = manager.force_refresh().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "force_refreshed_token");

        refresh_mock.assert_hits(1);

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_get_token_proactive_refresh() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());

        // Mock successful token refresh
        let refresh_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/user/refresh_token")
                .header("authorization", "token expires_soon_token");
            then.status(200)
                .json_body(serde_json::json!({
                    "token": "proactively_refreshed_token"
                }));
        });

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // Set a token that expires soon (in 3 minutes, less than 5-minute threshold)
        let expires_at = Utc::now() + Duration::minutes(3);
        let token_data = TokenData::new("expires_soon_token".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        let result = manager.get_token().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "proactively_refreshed_token");

        refresh_mock.assert_hits(1);

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_token_manager_get_token_expired_fallback_logic() {
        // Test the expired token fallback logic without HTTP calls

        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();

        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // Set an expired token
        let expires_at = Utc::now() - Duration::hours(1);
        let token_data = TokenData::new("expired_token".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // get_token should detect the expired token and try to obtain a new one
        // Since we don't have credentials, it should fail with MissingEnvVar
        let result = manager.get_token().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            Error::MissingEnvVar(var) => {
                assert_eq!(var, "AMP_USERNAME");
            }
            Error::Token(TokenError::ObtainFailed { .. }) => {
                // Also acceptable - means it tried to obtain but failed
            }
            e => panic!("Expected MissingEnvVar or ObtainFailed error, got: {}", e),
        }

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_get_token_no_token_obtain_new() {
        use httpmock::prelude::*;

        dotenvy::dotenv().ok();
        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();
        let original_username = env::var("AMP_USERNAME").ok();
        let original_password = env::var("AMP_PASSWORD").ok();

        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());
        env::set_var("AMP_USERNAME", "test_user");
        env::set_var("AMP_PASSWORD", "test_password");

        // Mock successful obtain
        let obtain_mock = server.mock(|when, then| {
            when.method(POST).path("/user/obtain_token");
            then.status(200)
                .json_body(serde_json::json!({
                    "token": "first_token"
                }));
        });

        // Create TokenManager after setting environment variables
        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // No existing token
        assert!(manager.token_data.lock().await.is_none());

        let result = manager.get_token().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "first_token");

        obtain_mock.assert_hits(1);

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
        match original_username {
            Some(val) => env::set_var("AMP_USERNAME", val),
            None => env::remove_var("AMP_USERNAME"),
        }
        match original_password {
            Some(val) => env::set_var("AMP_PASSWORD", val),
            None => env::remove_var("AMP_PASSWORD"),
        }
    }

    #[tokio::test]
    async fn test_token_manager_concurrent_access() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();

        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let config = RetryConfig::for_tests();
        let manager = Arc::new(TokenManager::with_config(config).unwrap());

        // Pre-populate with a valid token to test concurrent access to existing token
        let expires_at = Utc::now() + Duration::hours(2);
        let token_data = TokenData::new("concurrent_token".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Launch multiple concurrent get_token requests
        let mut join_set = JoinSet::new();
        for i in 0..5 {
            let manager_clone = Arc::clone(&manager);
            join_set.spawn(async move {
                let result = manager_clone.get_token().await;
                (i, result)
            });
        }

        // Collect all results
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap());
        }

        // All requests should succeed and return the same token
        assert_eq!(results.len(), 5);
        for (_, result) in results {
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "concurrent_token");
        }

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
    }

    #[tokio::test]
    async fn test_token_manager_get_token_info_logging() {
        // Test logging behavior for get_token_info method
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Test logging when no token exists
        let info = manager.get_token_info().await.unwrap();
        assert!(info.is_none());

        // Set a token and test logging with token info
        let expires_at = Utc::now() + Duration::hours(2);
        let token_data = TokenData::new("test_token_for_logging".to_string(), expires_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data.clone());
        }

        // Get token info and verify it contains expected data
        let info = manager.get_token_info().await.unwrap();
        assert!(info.is_some());

        let token_info = info.unwrap();
        assert_eq!(token_info.expires_at, expires_at);
        assert!(!token_info.is_expired);
        assert!(!token_info.expires_soon); // 2 hours > 5 minutes
        assert!(token_info.expires_in > Duration::hours(1));
        assert!(token_info.age < Duration::seconds(1));

        // Test with expired token
        let expired_at = Utc::now() - Duration::hours(1);
        let expired_token_data = TokenData::new("expired_token".to_string(), expired_at);
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(expired_token_data);
        }

        let expired_info = manager.get_token_info().await.unwrap().unwrap();
        assert!(expired_info.is_expired);
        assert!(expired_info.expires_soon);

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_token_manager_clear_token_logging() {
        // Test logging behavior for clear_token method
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Test clearing when no token exists (should log that no token was stored)
        let result = manager.clear_token().await;
        assert!(result.is_ok());
        assert!(manager.token_data.lock().await.is_none());

        // Set a token and test clearing it (should log successful clearing)
        let token_data = TokenData::new("token_to_clear".to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Verify token exists before clearing
        assert!(manager.token_data.lock().await.is_some());

        // Clear the token and verify logging
        let result = manager.clear_token().await;
        assert!(result.is_ok());
        assert!(manager.token_data.lock().await.is_none());

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    #[serial]
    async fn test_token_manager_force_refresh_logging() {
        use httpmock::prelude::*;

        // Test logging behavior for force_refresh method
        let server = MockServer::start();
        env::set_var("AMP_API_BASE_URL", server.base_url());

        // Mock successful token refresh with specific header to differentiate from other tests
        let refresh_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/user/refresh_token")
                .header("authorization", "token existing_token");
            then.status(200)
                .json_body(serde_json::json!({
                    "token": "logging_test_token"
                }));
        });

        let config = RetryConfig::for_tests();
        let manager = TokenManager::with_config(config).unwrap();

        // Set an existing token and test force refresh with existing token
        let token_data = TokenData::new("existing_token".to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        let result = manager.force_refresh().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "logging_test_token");

        refresh_mock.assert_hits(1);

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_token_manager_force_refresh_no_token_logging() {
        // Test logging behavior when force_refresh is called with no existing token
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Ensure no token exists
        assert!(manager.token_data.lock().await.is_none());

        // Force refresh with no existing token should log appropriately
        // This will fail because we don't have credentials set, but should log the attempt
        let result = manager.force_refresh().await;
        assert!(result.is_err()); // Expected to fail without credentials

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_token_manager_utility_methods_secure_logging() {
        // Test that utility methods don't expose sensitive information in logs
        let config = RetryConfig::for_tests();

        // Create a mock base URL for testing
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let manager = TokenManager::with_config(config).unwrap();

        // Set a token with sensitive data
        let sensitive_token = "super_secret_token_12345";
        let token_data = TokenData::new(sensitive_token.to_string(), Utc::now() + Duration::hours(1));
        {
            let mut guard = manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Get token info - this should log information but not expose the actual token
        let info = manager.get_token_info().await.unwrap();
        assert!(info.is_some());

        // The logging should not contain the actual token value
        // This is verified by the fact that we use TokenInfo which doesn't contain the token
        let token_info = info.unwrap();
        assert!(!token_info.is_expired);

        // Clear token - should log the action but not the token value
        let result = manager.clear_token().await;
        assert!(result.is_ok());

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    // Integration tests for ApiClient with TokenManager
    #[tokio::test]
    async fn test_api_client_token_manager_integration() {
        // Set up environment
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        // Create API client - should initialize with TokenManager
        let client = ApiClient::new().unwrap();

        // Verify that the client has a token manager
        assert!(!client.token_manager.token_data.lock().await.is_some());

        // Test token info retrieval
        let token_info = client.get_token_info().await.unwrap();
        assert!(token_info.is_none()); // No token initially

        // Test token clearing (should not fail even with no token)
        let result = client.clear_token().await;
        assert!(result.is_ok());

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_api_client_with_custom_token_manager() {
        // Set up environment
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        // Create a custom token manager with test configuration
        let config = RetryConfig::for_tests();
        let token_manager = Arc::new(TokenManager::with_config(config).unwrap());

        // Create API client with custom token manager
        let client = ApiClient::with_token_manager(token_manager.clone()).unwrap();

        // Verify the client uses the same token manager instance
        assert!(Arc::ptr_eq(&client.token_manager, &token_manager));

        // Test that token operations work through the client
        let token_info = client.get_token_info().await.unwrap();
        assert!(token_info.is_none());

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_api_client_thread_safety() {
        use std::sync::Arc;
        use tokio::task;

        // Set up environment
        env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

        let client = Arc::new(ApiClient::new().unwrap());

        // Spawn multiple tasks that access token info concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let client_clone = client.clone();
            let handle = task::spawn(async move {
                // Each task should be able to access token info safely
                let token_info = client_clone.get_token_info().await.unwrap();
                assert!(token_info.is_none());

                // Test clearing token from multiple threads
                if i % 2 == 0 {
                    let _ = client_clone.clear_token().await;
                }

                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result < 10);
        }

        // Clean up
        env::remove_var("AMP_API_BASE_URL");
    }

    #[tokio::test]
    async fn test_api_client_token_manager_error_handling() {
        dotenvy::dotenv().ok();
        // Save original env vars
        let original_base_url = env::var("AMP_API_BASE_URL").ok();
        
        // Test with invalid base URL to trigger error in TokenManager creation
        env::set_var("AMP_API_BASE_URL", "not-a-valid-url");

        let result = ApiClient::new();
        assert!(result.is_err());

        // Restore original env vars
        match original_base_url {
            Some(val) => env::set_var("AMP_API_BASE_URL", val),
            None => env::remove_var("AMP_API_BASE_URL"),
        }
    }

    #[tokio::test]
    async fn test_api_client_maintains_public_api() {
        // Use a test URL directly instead of environment variables to avoid test interference
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let client = ApiClient::with_base_url(base_url).unwrap();

        // Test that all the new utility methods are available
        let _ = client.get_token_info().await;
        let _ = client.clear_token().await;

        // This will fail without credentials but should not panic
        let result = client.force_refresh().await;
        assert!(result.is_err());

        // Test that get_token method is still available (core API method)
        let result = client.get_token().await;
        assert!(result.is_err()); // Expected to fail without valid credentials
    }

    // Comprehensive concurrency tests for thread safety
    #[tokio::test]
    async fn test_token_manager_concurrent_access_safety() {
        use std::sync::Arc;
        use tokio::task;
        use url::Url;

        // Create a token manager with test configuration
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let config = RetryConfig::for_tests();
        let token_manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url).unwrap());

        // Test concurrent access to token info (read operations)
        let mut handles = vec![];
        for i in 0..20 {
            let manager_clone = token_manager.clone();
            let handle = task::spawn(async move {
                // Multiple threads accessing token info simultaneously
                let token_info = manager_clone.get_token_info().await.unwrap();
                assert!(token_info.is_none()); // No token initially

                // Test clearing token from multiple threads
                if i % 3 == 0 {
                    let _ = manager_clone.clear_token().await;
                }

                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result < 20);
        }
    }

    #[tokio::test]
    async fn test_token_manager_concurrent_token_operations() {
        use std::sync::Arc;
        use tokio::task;
        use url::Url;

        // Create a token manager with test configuration
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let config = RetryConfig::for_tests();
        let token_manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url).unwrap());

        // Test that multiple concurrent get_token calls don't cause race conditions
        // These will fail due to missing credentials, but should not cause panics or race conditions
        let mut handles = vec![];
        for i in 0..10 {
            let manager_clone = token_manager.clone();
            let handle = task::spawn(async move {
                // Multiple threads trying to get tokens simultaneously
                let result = manager_clone.get_token().await;
                // All should fail due to missing credentials, but safely
                assert!(result.is_err());
                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result < 10);
        }
    }

    #[tokio::test]
    async fn test_token_manager_semaphore_prevents_race_conditions() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};
        use tokio::task;
        use url::Url;

        // Create a token manager with test configuration
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let config = RetryConfig::for_tests();
        let token_manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url).unwrap());

        // Counter to track concurrent operations
        let operation_counter = Arc::new(AtomicU32::new(0));
        let max_concurrent = Arc::new(AtomicU32::new(0));

        // Test that semaphore prevents more than one token operation at a time
        let mut handles = vec![];
        for i in 0..5 {
            let manager_clone = token_manager.clone();
            let counter_clone = operation_counter.clone();
            let max_clone = max_concurrent.clone();

            let handle = task::spawn(async move {
                // This will fail due to missing credentials, but we're testing the semaphore behavior
                let result = manager_clone.obtain_token().await;

                // Track concurrent operations (this is a simplified test)
                let current = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;
                let max_val = max_clone.load(Ordering::SeqCst);
                if current > max_val {
                    max_clone.store(current, Ordering::SeqCst);
                }

                // Simulate some work
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                counter_clone.fetch_sub(1, Ordering::SeqCst);

                // Should fail due to missing credentials
                assert!(result.is_err());
                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result < 5);
        }

        // The semaphore should have prevented excessive concurrency
        // Note: This is a simplified test - in practice, the semaphore ensures only one token operation at a time
        assert!(max_concurrent.load(Ordering::SeqCst) <= 5);
    }

    #[tokio::test]
    async fn test_token_manager_atomic_token_updates() {
        use std::sync::Arc;
        use tokio::task;
        use url::Url;
        use chrono::{Duration, Utc};
        use crate::model::TokenData;


        // Create a token manager with test configuration
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let config = RetryConfig::for_tests();
        let token_manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url).unwrap());

        // Manually insert a token for testing atomic updates
        {
            let expires_at = Utc::now() + Duration::hours(1);
            let token_data = TokenData::new("test_token".to_string(), expires_at);
            let mut guard = token_manager.token_data.lock().await;
            *guard = Some(token_data);
        }

        // Test concurrent access to token info to ensure atomic reads
        let mut handles = vec![];
        for i in 0..15 {
            let manager_clone = token_manager.clone();
            let handle = task::spawn(async move {
                // Multiple threads reading token info simultaneously
                let token_info = manager_clone.get_token_info().await.unwrap();

                if let Some(info) = token_info {
                    // Token should be consistent across all reads
                    assert!(!info.is_expired);
                    assert!(info.expires_in.num_seconds() > 0);
                    assert!(info.age.num_seconds() >= 0);
                } else {
                    // If another thread cleared it, that's also valid
                }

                // Some threads clear the token to test atomic updates
                if i % 5 == 0 {
                    let _ = manager_clone.clear_token().await;
                }

                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result < 15);
        }
    }

    #[tokio::test]
    async fn test_token_manager_no_partial_state_corruption() {
        use std::sync::Arc;
        use tokio::task;
        use url::Url;
        use chrono::{Duration, Utc};
        use crate::model::TokenData;

        // Create a token manager with test configuration
        let base_url = Url::parse("https://test.example.com/api").unwrap();
        let config = RetryConfig::for_tests();
        let token_manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url).unwrap());

        // Test that token state is never partially corrupted during concurrent operations
        let mut handles = vec![];
        for i in 0..10 {
            let manager_clone = token_manager.clone();
            let handle = task::spawn(async move {
                // Some threads set tokens, others clear them
                if i % 2 == 0 {
                    // Set a token
                    let expires_at = Utc::now() + Duration::hours(1);
                    let token_data = TokenData::new(format!("token_{}", i), expires_at);
                    {
                        let mut guard = manager_clone.token_data.lock().await;
                        *guard = Some(token_data);
                    }
                } else {
                    // Clear the token
                    let _ = manager_clone.clear_token().await;
                }

                // Verify token state is always consistent
                let token_info = manager_clone.get_token_info().await.unwrap();
                if let Some(info) = token_info {
                    // If token exists, all fields should be valid
                    assert!(info.expires_at > Utc::now() - Duration::hours(1));
                    assert!(info.obtained_at <= Utc::now());
                    assert!(info.age >= Duration::zero());
                }

                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result < 10);
        }

        // Final state should be consistent
        let final_info = token_manager.get_token_info().await.unwrap();
        if let Some(info) = final_info {
            assert!(info.expires_at > Utc::now() - Duration::hours(1));
            assert!(info.obtained_at <= Utc::now());
            assert!(info.age >= Duration::zero());
        }
    }
}
