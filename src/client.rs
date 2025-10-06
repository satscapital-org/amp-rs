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
    Activity, Asset, AssetActivityParams, AssetSummary, Assignment, Balance, BroadcastResponse, 
    CategoryAdd, CategoryEdit, CategoryResponse, ChangePasswordRequest, ChangePasswordResponse, 
    CreateAssetAssignmentRequest, EditAssetRequest, IssuanceRequest, IssuanceResponse, Outpoint, 
    Ownership, Password, TokenData, TokenInfo, TokenRequest, TokenResponse, Utxo,
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
    pub fn calculate_backoff_delay(&self, attempt: u32) -> StdDuration {
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
    pub token_data: Arc<Mutex<Option<TokenData>>>,
    pub retry_client: RetryClient,
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



    pub async fn get_broadcast_status(&self, txid: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::GET, &["tx", "broadcast", txid], None::<&()>)
            .await
    }

    pub async fn broadcast_transaction(&self, tx_hex: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::POST, &["tx", "broadcast"], Some(tx_hex))
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

    /// Gets the treasury addresses for a specific asset
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to get treasury addresses for
    ///
    /// # Returns
    /// A vector of treasury addresses as strings
    ///
    /// # Errors
    /// Returns an error if:
    /// - The asset does not exist
    /// - The request fails
    /// - The response cannot be parsed
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

    /// Adds treasury addresses to a specific asset
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to add treasury addresses to
    /// * `addresses` - A slice of address strings to add as treasury addresses
    ///
    /// # Returns
    /// Returns `Ok(())` on success
    ///
    /// # Errors
    /// Returns an error if:
    /// - The asset does not exist
    /// - The addresses are invalid
    /// - The request fails
    /// - Insufficient permissions
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

    pub async fn delete_registered_user(&self, user_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["registered_users", &user_id.to_string(), "delete"],
            None::<&()>,
        )
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

    pub async fn get_asset_assignments(
        &self,
        asset_uuid: &str,
    ) -> Result<Vec<Assignment>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "assignments"],
            None::<&()>,
        )
        .await
    }

    pub async fn create_asset_assignments(
        &self,
        asset_uuid: &str,
        requests: &[CreateAssetAssignmentRequest],
    ) -> Result<Vec<Assignment>, Error> {
        use crate::model::CreateAssetAssignmentRequestWrapper;
        
        let mut all_assignments = Vec::new();
        
        // Submit each request separately since the API is restricted to a single element per call
        for request in requests {
            let wrapper = CreateAssetAssignmentRequestWrapper {
                assignments: vec![request.clone()],
            };
            
            let mut assignments: Vec<Assignment> = self.request_json(
                Method::POST,
                &["assets", asset_uuid, "assignments", "create"],
                Some(&wrapper),
            )
            .await?;
            
            all_assignments.append(&mut assignments);
        }
        
        Ok(all_assignments)
    }

    /// Gets a specific manager by ID.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager to retrieve
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed as JSON
    pub async fn get_manager(&self, manager_id: i64) -> Result<crate::model::Manager, Error> {
        self.request_json(Method::GET, &["managers", &manager_id.to_string()], None::<&()>)
            .await
    }

    /// Removes a manager's permissions to modify a specific asset.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager
    /// * `asset_uuid` - The UUID of the asset to remove permissions for
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    pub async fn manager_remove_asset(
        &self,
        manager_id: i64,
        asset_uuid: &str,
    ) -> Result<(), Error> {
        self.request_empty(
            Method::POST,
            &["managers", &manager_id.to_string(), "assets", asset_uuid, "remove"],
            None::<&()>,
        )
        .await
    }

    /// Revokes all asset permissions for a manager.
    ///
    /// This method first retrieves the manager's current asset permissions,
    /// then removes the manager's access to each asset they currently have access to.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager to revoke permissions for
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - Any individual asset removal fails
    pub async fn revoke_manager(&self, manager_id: i64) -> Result<(), Error> {
        // First, get the manager to see which assets they have access to
        let manager = self.get_manager(manager_id).await?;

        // Remove the manager's access to each asset
        for asset_uuid in &manager.assets {
            self.manager_remove_asset(manager_id, asset_uuid).await?;
        }

        Ok(())
    }

    /// Gets the current manager information as raw JSON.
    ///
    /// This method calls the `/managers/me` endpoint to retrieve information
    /// about the currently authenticated manager.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed as JSON
    pub async fn get_current_manager_raw(&self) -> Result<serde_json::Value, Error> {
        self.request_json(Method::GET, &["managers", "me"], None::<&()>)
            .await
    }

    /// Unlocks a manager account.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager to unlock
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    pub async fn unlock_manager(&self, manager_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::PUT,
            &["managers", &manager_id.to_string(), "unlock"],
            None::<&()>,
        )
        .await
    }

    /// Deletes a specific asset assignment.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `assignment_id` - The ID of the assignment to delete
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    pub async fn delete_asset_assignment(
        &self,
        asset_uuid: &str,
        assignment_id: &str,
    ) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["assets", asset_uuid, "assignments", assignment_id, "delete"],
            None::<&()>,
        )
        .await
    }

    /// Locks a specific asset assignment.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `assignment_id` - The ID of the assignment to lock
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    pub async fn lock_asset_assignment(
        &self,
        asset_uuid: &str,
        assignment_id: &str,
    ) -> Result<Assignment, Error> {
        self.request_json(
            Method::PUT,
            &["assets", asset_uuid, "assignments", assignment_id, "lock"],
            None::<&()>,
        )
        .await
    }

    /// Unlocks a specific asset assignment.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `assignment_id` - The ID of the assignment to unlock
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    pub async fn unlock_asset_assignment(
        &self,
        asset_uuid: &str,
        assignment_id: &str,
    ) -> Result<Assignment, Error> {
        self.request_json(
            Method::PUT,
            &["assets", asset_uuid, "assignments", assignment_id, "unlock"],
            None::<&()>,
        )
        .await
    }
}

fn get_amp_api_base_url() -> Result<Url, Error> {
    let url_str = env::var("AMP_API_BASE_URL")
        .unwrap_or_else(|_| "https://amp-test.blockstream.com/api".to_string());
    Url::parse(&url_str).map_err(Error::from)
}

