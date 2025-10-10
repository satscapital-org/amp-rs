use std::env;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use chrono::{Duration, Utc};

use reqwest::header::AUTHORIZATION;
use reqwest::{Client, Method, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::{Mutex, OnceCell, Semaphore};
use tokio::time::sleep;

use secrecy::ExposeSecret;
use secrecy::Secret;

use crate::model::{
    Activity, Asset, AssetActivityParams, AssetSummary, Assignment, Balance, BroadcastResponse,
    CategoriesRequest, CategoryAdd, CategoryEdit, CategoryResponse, ChangePasswordRequest,
    ChangePasswordResponse, CreateAssetAssignmentRequest, EditAssetRequest, GaidBalanceEntry,
    GaidRequest, IssuanceRequest, IssuanceResponse, Outpoint, Ownership, Password,
    TokenData, TokenInfo, TokenRequest, TokenResponse, Utxo,
};

/// Token environment detection for automatic strategy selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenEnvironment {
    /// Mock environment - use isolated token management without persistence
    Mock,
    /// Live environment - use full token management with persistence
    Live,
    /// Auto-detect environment based on credentials and settings
    Auto,
}

impl TokenEnvironment {
    /// Detects the current token environment based on environment variables and credential patterns
    ///
    /// Detection logic:
    /// 1. If `AMP_TESTS=live` is set, returns `Live`
    /// 2. If credentials contain "mock" string, returns `Mock`
    /// 3. If real credentials are present without live test flag, returns `Live`
    /// 4. Fallback to `Mock` for safety
    #[must_use]
    pub fn detect() -> Self {
        let username = env::var("AMP_USERNAME").unwrap_or_default();
        let password = env::var("AMP_PASSWORD").unwrap_or_default();
        let amp_tests = env::var("AMP_TESTS").unwrap_or_default();
        let base_url = env::var("AMP_API_BASE_URL").unwrap_or_default();

        tracing::debug!(
            "Detecting token environment - AMP_TESTS: '{}', username: '{}', base_url: '{}'",
            amp_tests,
            username,
            base_url
        );

        // Explicit live test environment
        if amp_tests == "live" {
            tracing::info!("Detected live environment via AMP_TESTS=live");
            return Self::Live;
        }

        // Mock credentials detected
        if Self::has_mock_credentials(&username, &password, &base_url) {
            tracing::info!("Detected mock environment via mock credentials");
            return Self::Mock;
        }

        // Real credentials without live test flag - default to live for production use
        if !username.is_empty() && !password.is_empty() {
            tracing::info!("Detected live environment via real credentials");
            return Self::Live;
        }

        // Fallback to mock for safety when no credentials are present
        tracing::info!("Detected mock environment via fallback (no credentials)");
        Self::Mock
    }

    /// Checks if the provided credentials indicate a mock environment
    ///
    /// Mock credentials are detected by:
    /// - Username containing "mock" (case-insensitive)
    /// - Password containing "mock" (case-insensitive)
    /// - Base URL containing localhost, 127.0.0.1, or "mock"
    #[must_use]
    pub fn has_mock_credentials(username: &str, password: &str, base_url: &str) -> bool {
        let username_lower = username.to_lowercase();
        let password_lower = password.to_lowercase();
        let base_url_lower = base_url.to_lowercase();

        let has_mock_username = username_lower.contains("mock");
        let has_mock_password = password_lower.contains("mock");
        let has_mock_url = base_url_lower.contains("localhost")
            || base_url_lower.contains("127.0.0.1")
            || base_url_lower.contains("mock");

        let is_mock = has_mock_username || has_mock_password || has_mock_url;

        tracing::debug!(
            "Mock credential check - username: {}, password: {}, url: {}, result: {}",
            has_mock_username,
            has_mock_password,
            has_mock_url,
            is_mock
        );

        is_mock
    }

    /// Creates a token strategy based on the environment type
    ///
    /// # Arguments
    /// * `mock_token` - Optional mock token to use for mock environments
    ///
    /// # Errors
    /// Returns an error if strategy creation fails
    pub async fn create_strategy(
        &self,
        mock_token: Option<String>,
    ) -> Result<Box<dyn TokenStrategy>, Error> {
        match self {
            Self::Mock => {
                let token = mock_token.unwrap_or_else(|| "default_mock_token".to_string());
                tracing::debug!("Creating mock token strategy with token");
                Ok(Box::new(MockTokenStrategy::new(token)))
            }
            Self::Live => {
                tracing::debug!("Creating live token strategy");
                let strategy = LiveTokenStrategy::new().await?;
                Ok(Box::new(strategy))
            }
            Self::Auto => {
                tracing::debug!("Auto-detecting environment for strategy creation");
                let detected = Self::detect();
                // Avoid recursion by directly matching the detected environment
                match detected {
                    Self::Mock => {
                        let token = mock_token.unwrap_or_else(|| "default_mock_token".to_string());
                        tracing::debug!("Auto-detected mock environment, creating mock strategy");
                        Ok(Box::new(MockTokenStrategy::new(token)))
                    }
                    Self::Live => {
                        tracing::debug!("Auto-detected live environment, creating live strategy");
                        let strategy = LiveTokenStrategy::new().await?;
                        Ok(Box::new(strategy))
                    }
                    Self::Auto => {
                        // This should never happen since detect() never returns Auto
                        tracing::error!("Unexpected Auto environment from detect()");
                        Err(Error::Token(TokenError::validation(
                            "Environment detection returned Auto, which should not happen"
                                .to_string(),
                        )))
                    }
                }
            }
        }
    }

    /// Creates a token strategy with automatic environment detection
    ///
    /// This is a convenience method that combines environment detection with strategy creation.
    ///
    /// # Arguments
    /// * `mock_token` - Optional mock token to use if mock environment is detected
    ///
    /// # Errors
    /// Returns an error if strategy creation fails
    pub async fn create_auto_strategy(
        mock_token: Option<String>,
    ) -> Result<Box<dyn TokenStrategy>, Error> {
        let environment = Self::detect();
        environment.create_strategy(mock_token).await
    }

    /// Determines if token persistence should be enabled for this environment
    #[must_use]
    pub fn should_persist_tokens(&self) -> bool {
        match self {
            Self::Mock => false,
            Self::Live => true,
            Self::Auto => Self::detect().should_persist_tokens(),
        }
    }

    /// Returns true if this is a mock environment
    #[must_use]
    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock) || (matches!(self, Self::Auto) && Self::detect().is_mock())
    }

    /// Returns true if this is a live environment
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live) || (matches!(self, Self::Auto) && Self::detect().is_live())
    }
}

/// Token management strategy trait for different token handling approaches
#[async_trait]
pub trait TokenStrategy: Send + Sync + std::fmt::Debug {
    /// Gets a valid authentication token
    async fn get_token(&self) -> Result<String, Error>;

    /// Clears stored token (for testing)
    async fn clear_token(&self) -> Result<(), Error>;

    /// Returns whether this strategy should persist tokens
    fn should_persist(&self) -> bool;

    /// Returns the strategy type for debugging
    fn strategy_type(&self) -> &'static str;

    /// Returns self as Any for downcasting (used internally)
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Mock token strategy for isolated testing without persistence
#[derive(Debug, Clone)]
pub struct MockTokenStrategy {
    token: String,
}

impl MockTokenStrategy {
    /// Creates a new mock token strategy with the provided token
    #[must_use]
    pub const fn new(token: String) -> Self {
        Self { token }
    }

    /// Creates a mock token strategy with a default test token
    #[must_use]
    pub fn with_default_token() -> Self {
        Self::new("mock_token_default".to_string())
    }

    /// Creates a mock token strategy for a specific test case
    #[must_use]
    pub fn for_test(test_name: &str) -> Self {
        Self::new(format!("mock_token_{}", test_name))
    }
}

#[async_trait]
impl TokenStrategy for MockTokenStrategy {
    async fn get_token(&self) -> Result<String, Error> {
        tracing::debug!("Using mock token strategy - returning pre-set token");
        Ok(self.token.clone())
    }

    async fn clear_token(&self) -> Result<(), Error> {
        tracing::debug!("Mock token strategy - clear_token is a no-op");
        Ok(())
    }

    fn should_persist(&self) -> bool {
        false
    }

    fn strategy_type(&self) -> &'static str {
        "mock"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Live token strategy that wraps the existing `TokenManager` for full token management
#[derive(Debug)]
pub struct LiveTokenStrategy {
    token_manager: Arc<TokenManager>,
}

impl LiveTokenStrategy {
    /// Creates a new live token strategy using the global `TokenManager` instance
    ///
    /// # Errors
    /// Returns an error if the `TokenManager` cannot be initialized
    pub async fn new() -> Result<Self, Error> {
        let token_manager = TokenManager::get_global_instance().await?;
        Ok(Self { token_manager })
    }

    /// Creates a new live token strategy with a custom `TokenManager`
    #[must_use]
    pub const fn with_token_manager(token_manager: Arc<TokenManager>) -> Self {
        Self { token_manager }
    }

    /// Creates a live token strategy with custom retry configuration
    ///
    /// # Errors
    /// Returns an error if the `TokenManager` cannot be initialized
    pub async fn with_config(config: RetryConfig) -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        let token_manager =
            Arc::new(TokenManager::with_config_and_base_url(config, base_url).await?);
        Ok(Self { token_manager })
    }

    /// Creates a live token strategy optimized for testing
    ///
    /// # Errors
    /// Returns an error if the `TokenManager` cannot be initialized
    pub async fn for_testing() -> Result<Self, Error> {
        let config = RetryConfig::for_tests();
        Self::with_config(config).await
    }

    /// Gets current token information for debugging and monitoring
    ///
    /// # Errors
    /// Returns an error if token information retrieval fails
    pub async fn get_token_info(&self) -> Result<Option<TokenInfo>, Error> {
        self.token_manager.get_token_info().await
    }
}

#[async_trait]
impl TokenStrategy for LiveTokenStrategy {
    async fn get_token(&self) -> Result<String, Error> {
        tracing::debug!("Using live token strategy - full token management");
        self.token_manager.get_token().await
    }

    async fn clear_token(&self) -> Result<(), Error> {
        self.token_manager.clear_token().await
    }

    fn should_persist(&self) -> bool {
        true
    }

    fn strategy_type(&self) -> &'static str {
        "live"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

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
#[derive(Error, Debug, Clone, PartialEq, Eq)]
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
    /// Creates a new `RefreshFailed` error
    #[must_use]
    pub fn refresh_failed<S: Into<String>>(message: S) -> Self {
        Self::RefreshFailed(message.into())
    }

    /// Creates a new `ObtainFailed` error
    #[must_use]
    pub const fn obtain_failed(attempts: u32, last_error: String) -> Self {
        Self::ObtainFailed {
            attempts,
            last_error,
        }
    }

    /// Creates a new `RateLimited` error
    #[must_use]
    pub const fn rate_limited(retry_after_seconds: u64) -> Self {
        Self::RateLimited {
            retry_after_seconds,
        }
    }

    /// Creates a new Timeout error
    #[must_use]
    pub const fn timeout(timeout_seconds: u64) -> Self {
        Self::Timeout { timeout_seconds }
    }

    /// Creates a new Serialization error
    #[must_use]
    pub fn serialization<S: Into<String>>(message: S) -> Self {
        Self::Serialization(message.into())
    }

    /// Creates a new Storage error
    #[must_use]
    pub fn storage<S: Into<String>>(message: S) -> Self {
        Self::Storage(message.into())
    }

    /// Creates a new Validation error
    #[must_use]
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation(message.into())
    }

    /// Returns true if this error indicates a retryable condition
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RefreshFailed(_) | Self::RateLimited { .. } | Self::Timeout { .. }
        )
    }

    /// Returns true if this error indicates a rate limiting condition
    #[must_use]
    pub const fn is_rate_limited(&self) -> bool {
        matches!(self, Self::RateLimited { .. })
    }

    /// Returns the retry delay in seconds if this is a rate limited error
    #[must_use]
    pub const fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            Self::RateLimited {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            _ => None,
        }
    }
}

// Conversion from serde_json::Error for serialization errors
impl From<serde_json::Error> for TokenError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
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
    /// Creates a `RetryConfig` from environment variables with default fallbacks
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
                Error::InvalidRetryConfig(format!("Invalid API_RETRY_MAX_ATTEMPTS: {e}"))
            })?,
            Err(_) => 3,
        };

        let base_delay_ms = match env::var("API_RETRY_BASE_DELAY_MS") {
            Ok(val) => val.parse::<u64>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_RETRY_BASE_DELAY_MS: {e}"))
            })?,
            Err(_) => 1000,
        };

        let max_delay_ms = match env::var("API_RETRY_MAX_DELAY_MS") {
            Ok(val) => val.parse::<u64>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_RETRY_MAX_DELAY_MS: {e}"))
            })?,
            Err(_) => 30000,
        };

        let timeout_seconds = match env::var("API_REQUEST_TIMEOUT_SECONDS") {
            Ok(val) => val.parse::<u64>().map_err(|e| {
                Error::InvalidRetryConfig(format!("Invalid API_REQUEST_TIMEOUT_SECONDS: {e}"))
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

    /// Creates a `RetryConfig` optimized for test environments
    ///
    /// Uses reduced values for faster test execution:
    /// - 2 retry attempts
    /// - 500ms base delay
    /// - 5000ms max delay
    /// - 5 second timeout
    #[must_use]
    pub const fn for_tests() -> Self {
        Self {
            max_attempts: 2,
            base_delay_ms: 500,
            max_delay_ms: 5000,
            timeout_seconds: 5,
        }
    }

    /// Sets a custom timeout value
    #[must_use]
    pub const fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    /// Sets custom max attempts
    #[must_use]
    pub const fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Sets custom base delay
    #[must_use]
    pub const fn with_base_delay_ms(mut self, base_delay_ms: u64) -> Self {
        self.base_delay_ms = base_delay_ms;
        self
    }

    /// Sets custom max delay
    #[must_use]
    pub const fn with_max_delay_ms(mut self, max_delay_ms: u64) -> Self {
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
    /// Creates a new `RetryClient` with the given configuration
    #[must_use]
    pub fn new(config: RetryConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Creates a new `RetryClient` with default configuration
    #[must_use]
    pub fn with_default_config() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Creates a new `RetryClient` with test-optimized configuration
    #[must_use]
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
    #[allow(clippy::cognitive_complexity)]
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
                        let retry_after = Self::extract_retry_after(&response).unwrap_or(60);

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
                        last_error = format!("Client error: {status}");
                        tracing::error!("Non-retryable client error: {}", status);
                        break;
                    }

                    // Handle server errors (5xx) - these are retryable
                    if status.is_server_error() {
                        last_error = format!("Server error: {status}");
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
    /// Uses the formula: `min(base_delay * 2^(attempt-1) + jitter, max_delay)`
    /// where jitter is a random value between 0 and `base_delay/2`
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
    fn extract_retry_after(response: &reqwest::Response) -> Option<u64> {
        response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    }

    /// Gets the underlying reqwest client
    #[must_use]
    pub const fn client(&self) -> &Client {
        &self.client
    }

    /// Gets the retry configuration
    #[must_use]
    pub const fn config(&self) -> &RetryConfig {
        &self.config
    }
}

/// Singleton instance of the TokenManager for shared token storage across all ApiClient instances
static GLOBAL_TOKEN_MANAGER: OnceCell<Arc<TokenManager>> = OnceCell::const_new();

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
    /// Gets the global singleton instance of TokenManager
    ///
    /// This ensures all ApiClient instances share the same token storage,
    /// preventing multiple token acquisition attempts in concurrent tests.
    ///
    /// # Errors
    /// Returns an error if the TokenManager cannot be initialized
    pub async fn get_global_instance() -> Result<Arc<TokenManager>, Error> {
        let manager = GLOBAL_TOKEN_MANAGER
            .get_or_try_init(|| async {
                let config = RetryConfig::from_env()?;
                let base_url = get_amp_api_base_url()?;
                let manager = Self::with_config_and_base_url(config, base_url).await?;
                Ok::<Arc<TokenManager>, Error>(Arc::new(manager))
            })
            .await?;

        Ok(manager.clone())
    }

    /// Creates a new `TokenManager` with default configuration
    ///
    /// # Errors
    /// Returns an error if the base URL cannot be obtained from environment variables
    pub async fn new() -> Result<Self, Error> {
        let config = RetryConfig::from_env()?;
        Self::with_config(config).await
    }

    /// Creates a new `TokenManager` with the specified retry configuration
    ///
    /// # Errors
    /// Returns an error if the base URL cannot be obtained from environment variables
    pub async fn with_config(config: RetryConfig) -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        Self::with_config_and_base_url(config, base_url).await
    }

    /// Creates a new `TokenManager` with the specified configuration and base URL (for testing)
    ///
    /// # Errors
    /// This method is infallible but returns Result for API consistency
    pub async fn with_config_and_base_url(
        config: RetryConfig,
        base_url: Url,
    ) -> Result<Self, Error> {
        let manager = Self {
            token_data: Arc::new(Mutex::new(None)),
            retry_client: RetryClient::new(config),
            base_url,
            token_operation_semaphore: Arc::new(Semaphore::new(1)),
        };

        // Load token from disk if persistence is enabled
        if Self::should_persist_tokens() {
            if let Ok(Some(token_data)) = manager.load_token_from_disk().await {
                *manager.token_data.lock().await = Some(token_data);
                tracing::info!("Token loaded from disk during initialization");
            }
        }

        Ok(manager)
    }

    /// Creates a new `TokenManager` with a pre-set mock token (for testing)
    ///
    /// # Errors
    /// This method is infallible but returns Result for API consistency
    pub async fn with_mock_token(
        config: RetryConfig,
        base_url: Url,
        mock_token: String,
    ) -> Result<Self, Error> {
        let expires_at = Utc::now() + Duration::hours(24); // Mock token valid for 24 hours
        let token_data = TokenData::new(mock_token, expires_at);

        let manager = Self {
            token_data: Arc::new(Mutex::new(Some(token_data))),
            retry_client: RetryClient::new(config),
            base_url,
            token_operation_semaphore: Arc::new(Semaphore::new(1)),
        };

        Ok(manager)
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
        if let Some(token) = self.check_existing_token().await? {
            return Ok(token);
        }

        // Slow path: token needs refresh/obtain, acquire semaphore for thread safety
        let _permit = self.acquire_token_semaphore().await?;

        // Double-check token state after acquiring semaphore - another thread may have updated it
        if let Some(token) = self.check_existing_token().await? {
            tracing::debug!("Token was updated by another thread, using existing valid token");
            return Ok(token);
        }

        // At this point, we need to refresh or obtain a new token
        self.handle_token_refresh_or_obtain().await
    }

    /// Checks if we have a valid existing token that doesn't expire soon
    async fn check_existing_token(&self) -> Result<Option<String>, Error> {
        let token_guard = self.token_data.lock().await;
        if let Some(ref token_data) = *token_guard {
            if !token_data.expires_soon(Duration::minutes(5)) {
                tracing::debug!("Using existing valid token");
                let token = token_data.token.expose_secret().clone();
                drop(token_guard);
                return Ok(Some(token));
            }
        }
        drop(token_guard);
        Ok(None)
    }

    /// Acquires the token operation semaphore for thread-safe operations
    async fn acquire_token_semaphore(&self) -> Result<tokio::sync::SemaphorePermit<'_>, Error> {
        let permit = self
            .token_operation_semaphore
            .acquire()
            .await
            .map_err(|e| {
                Error::Token(TokenError::storage(format!(
                    "Failed to acquire token operation semaphore: {e}"
                )))
            })?;

        tracing::debug!("Acquired token operation semaphore for thread-safe token management");
        Ok(permit)
    }

    /// Handles the token refresh or obtain logic
    async fn handle_token_refresh_or_obtain(&self) -> Result<String, Error> {
        let needs_refresh = self.determine_token_operation().await;

        if needs_refresh {
            match self.refresh_token_internal().await {
                Ok(token) => {
                    tracing::info!("Token refreshed successfully");
                    return Ok(token);
                }
                Err(e) => {
                    tracing::warn!("Token refresh failed, falling back to obtain: {e}");
                    // Fall through to obtain new token
                }
            }
        }

        // Either we needed to obtain from the start, or refresh failed
        self.obtain_token_internal().await
    }

    /// Determines whether we need to refresh or obtain a new token
    async fn determine_token_operation(&self) -> bool {
        let token_guard = self.token_data.lock().await;
        token_guard.as_ref().map_or_else(
            || {
                tracing::info!("No token exists, will obtain new token");
                false
            },
            |token_data| {
                if token_data.is_expired() {
                    tracing::info!("Token is expired, will obtain new token");
                    false
                } else {
                    tracing::info!("Token expires soon, will attempt refresh");
                    true
                }
            },
        )
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
        let _permit = self
            .token_operation_semaphore
            .acquire()
            .await
            .map_err(|e| {
                Error::Token(TokenError::storage(format!(
                    "Failed to acquire token operation semaphore: {e}"
                )))
            })?;

        self.obtain_token_internal().await
    }

    /// Internal method to obtain a new authentication token without acquiring semaphore
    ///
    /// This method should only be called from contexts where the token operation semaphore
    /// has already been acquired (e.g., from within `get_token()`).
    ///
    /// # Errors
    /// Returns an error if:
    /// - Environment variables are missing
    /// - All retry attempts fail
    /// - Response parsing fails
    async fn obtain_token_internal(&self) -> Result<String, Error> {
        tracing::debug!("Obtaining new authentication token");

        let request_payload = Self::get_credentials_from_env()?;
        let url = self.build_obtain_token_url();
        let response = self.execute_token_request(&url, &request_payload).await?;
        let token_response = self.parse_token_response(response).await?;

        self.store_token_data(&token_response.token).await;

        tracing::info!("New authentication token obtained successfully");
        Ok(token_response.token)
    }

    /// Gets credentials from environment variables
    fn get_credentials_from_env() -> Result<TokenRequest, Error> {
        let username = env::var("AMP_USERNAME")
            .map_err(|_| Error::MissingEnvVar("AMP_USERNAME".to_string()))?;
        let password = env::var("AMP_PASSWORD")
            .map_err(|_| Error::MissingEnvVar("AMP_PASSWORD".to_string()))?;

        Ok(TokenRequest { username, password })
    }

    /// Builds the URL for token obtain endpoint
    fn build_obtain_token_url(&self) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("user")
            .push("obtain_token");
        url
    }

    /// Executes the token request with retry logic
    async fn execute_token_request(
        &self,
        url: &Url,
        request_payload: &TokenRequest,
    ) -> Result<reqwest::Response, Error> {
        let response = self
            .retry_client
            .execute_with_retry(|| {
                self.retry_client
                    .client()
                    .post(url.clone())
                    .json(request_payload)
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

        Ok(response)
    }

    /// Parses the token response from the API
    async fn parse_token_response(
        &self,
        response: reqwest::Response,
    ) -> Result<TokenResponse, Error> {
        response
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))
    }

    /// Stores the token data with 24-hour expiry and optional disk persistence
    async fn store_token_data(&self, token: &str) {
        let expires_at = Utc::now() + Duration::days(1);
        let token_data = TokenData::new(token.to_string(), expires_at);

        // Atomic token update - hold the lock for the minimal time needed
        *self.token_data.lock().await = Some(token_data.clone());
        tracing::debug!("Token data updated atomically in storage");

        // Save to disk if persistence is enabled
        if Self::should_persist_tokens() {
            if let Err(e) = self.save_token_to_disk(&token_data).await {
                tracing::warn!("Failed to save token to disk: {e}");
            }
        }
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
        let _permit = self
            .token_operation_semaphore
            .acquire()
            .await
            .map_err(|e| {
                Error::Token(TokenError::storage(format!(
                    "Failed to acquire token operation semaphore: {e}"
                )))
            })?;

        self.refresh_token_internal().await
    }

    /// Internal method to refresh the current authentication token without acquiring semaphore
    ///
    /// This method should only be called from contexts where the token operation semaphore
    /// has already been acquired (e.g., from within `get_token()`).
    ///
    /// # Errors
    /// Returns an error if both refresh and obtain operations fail
    #[allow(clippy::cognitive_complexity)]
    async fn refresh_token_internal(&self) -> Result<String, Error> {
        tracing::debug!("Refreshing authentication token");

        let Some(current_token) = self.get_current_token_for_refresh().await else {
            tracing::warn!("No token available for refresh, obtaining new token");
            return self.obtain_token_internal().await;
        };

        let url = self.build_refresh_token_url();
        let response = self.execute_refresh_request(&url, &current_token).await;

        match response {
            Ok(resp) => self.handle_refresh_response(resp).await,
            Err(e) => {
                tracing::warn!("Token refresh request failed: {e}, falling back to obtain");
                self.obtain_token_internal().await
            }
        }
    }

    /// Gets the current token for refresh operations
    async fn get_current_token_for_refresh(&self) -> Option<String> {
        let token_guard = self.token_data.lock().await;
        token_guard
            .as_ref()
            .map(|token_data| token_data.token.expose_secret().clone())
    }

    /// Builds the URL for token refresh endpoint
    fn build_refresh_token_url(&self) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("user")
            .push("refresh_token");
        url
    }

    /// Executes the refresh request with retry logic
    async fn execute_refresh_request(
        &self,
        url: &Url,
        current_token: &str,
    ) -> Result<reqwest::Response, TokenError> {
        self.retry_client
            .execute_with_retry(|| {
                self.retry_client
                    .client()
                    .post(url.clone())
                    .header(AUTHORIZATION, format!("token {current_token}"))
            })
            .await
    }

    /// Handles the refresh response, either storing the new token or falling back to obtain
    async fn handle_refresh_response(&self, resp: reqwest::Response) -> Result<String, Error> {
        if !resp.status().is_success() {
            let status = resp.status();
            let error_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            tracing::warn!("Token refresh failed with status {status}: {error_text}");
            return self.obtain_token_internal().await;
        }

        let token_response: TokenResponse = resp
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))?;

        self.store_token_data(&token_response.token).await;
        tracing::info!("Authentication token refreshed successfully");
        Ok(token_response.token)
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
    ///
    /// # Errors
    /// Returns an error if token information retrieval fails
    pub async fn get_token_info(&self) -> Result<Option<TokenInfo>, Error> {
        tracing::debug!("Retrieving token information for debugging");

        let token_info = self.token_data.lock().await.as_ref().map(TokenInfo::from);

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
    ///
    /// # Errors
    /// Returns an error if token clearing fails
    pub async fn clear_token(&self) -> Result<(), Error> {
        tracing::debug!("Clearing stored token from memory and disk");

        let mut token_guard = self.token_data.lock().await;
        let had_token = token_guard.is_some();
        *token_guard = None;
        drop(token_guard);

        // Remove from disk if persistence is enabled
        if Self::should_persist_tokens() {
            if let Err(e) = self.remove_token_from_disk().await {
                tracing::warn!("Failed to remove token from disk: {e}");
            }
        }

        if had_token {
            tracing::info!("Token successfully cleared from memory and disk - next get_token() will obtain fresh token");
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

        let _permit = self.acquire_token_semaphore().await?;
        self.log_token_status_for_refresh().await;
        self.execute_forced_refresh().await
    }

    /// Logs the current token status for forced refresh operation
    async fn log_token_status_for_refresh(&self) {
        let has_token = {
            let token_guard = self.token_data.lock().await;
            token_guard.is_some()
        };

        if has_token {
            tracing::debug!("Existing token found, attempting forced refresh");
        } else {
            tracing::debug!("No existing token found, will obtain new token");
        }
    }

    /// Executes the forced refresh operation
    async fn execute_forced_refresh(&self) -> Result<String, Error> {
        match self.refresh_token_internal().await {
            Ok(token) => {
                tracing::info!("Forced token refresh completed successfully");
                Ok(token)
            }
            Err(e) => {
                tracing::error!("Forced token refresh failed: {e}");
                Err(e)
            }
        }
    }

    /// Determines if token persistence is enabled based on environment variables
    ///
    /// Token persistence is enabled when:
    /// - `AMP_TESTS=live` (for live API testing)
    /// - `AMP_TOKEN_PERSISTENCE=true` is set
    /// - NOT in mock test environments (to prevent test pollution)
    fn should_persist_tokens() -> bool {
        // Use the new environment detection logic
        let environment = TokenEnvironment::detect();

        // Never persist tokens in mock environments to prevent test pollution
        if environment.is_mock() {
            tracing::debug!("Token persistence disabled - mock environment detected");
            return false;
        }

        // Check if explicitly enabled
        if env::var("AMP_TOKEN_PERSISTENCE").unwrap_or_default() == "true" {
            tracing::debug!("Token persistence enabled - AMP_TOKEN_PERSISTENCE=true");
            return true;
        }

        // Use environment-based persistence setting
        let should_persist = environment.should_persist_tokens();
        tracing::debug!(
            "Token persistence setting from environment: {}",
            should_persist
        );
        should_persist
    }

    /// Loads token data from disk if it exists and is valid
    async fn load_token_from_disk(&self) -> Result<Option<TokenData>, Error> {
        use tokio::fs;

        let token_file = "token.json";

        // Check if file exists
        if !tokio::fs::try_exists(token_file).await.unwrap_or(false) {
            tracing::debug!("Token file does not exist: {}", token_file);
            return Ok(None);
        }

        // Read and parse the token file
        match fs::read_to_string(token_file).await {
            Ok(content) => match serde_json::from_str::<TokenData>(&content) {
                Ok(token_data) => {
                    if token_data.is_expired() {
                        tracing::info!("Token loaded from disk is expired, removing file");
                        let _ = fs::remove_file(token_file).await;
                        Ok(None)
                    } else {
                        tracing::info!("Valid token loaded from disk");
                        Ok(Some(token_data))
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse token file, removing: {e}");
                    let _ = fs::remove_file(token_file).await;
                    Err(Error::Token(TokenError::serialization(format!(
                        "Failed to parse token file: {e}"
                    ))))
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read token file: {e}");
                Err(Error::Token(TokenError::storage(format!(
                    "Failed to read token file: {e}"
                ))))
            }
        }
    }

    /// Saves token data to disk
    async fn save_token_to_disk(&self, token_data: &TokenData) -> Result<(), Error> {
        use tokio::fs;

        let token_file = "token.json";

        match serde_json::to_string_pretty(token_data) {
            Ok(json) => match fs::write(token_file, json).await {
                Ok(()) => {
                    tracing::debug!("Token saved to disk: {}", token_file);
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Failed to write token file: {e}");
                    Err(Error::Token(TokenError::storage(format!(
                        "Failed to write token file: {e}"
                    ))))
                }
            },
            Err(e) => {
                tracing::error!("Failed to serialize token data: {e}");
                Err(Error::Token(TokenError::serialization(format!(
                    "Failed to serialize token data: {e}"
                ))))
            }
        }
    }

    /// Removes the token file from disk
    async fn remove_token_from_disk(&self) -> Result<(), Error> {
        use tokio::fs;

        let token_file = "token.json";

        match fs::remove_file(token_file).await {
            Ok(()) => {
                tracing::debug!("Token file removed from disk: {}", token_file);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!("Token file does not exist, nothing to remove");
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to remove token file: {e}");
                Err(Error::Token(TokenError::storage(format!(
                    "Failed to remove token file: {e}"
                ))))
            }
        }
    }

    /// Forces cleanup of token persistence files (useful for testing)
    /// This method removes token files regardless of persistence settings
    pub async fn force_cleanup_token_files() -> Result<(), Error> {
        use tokio::fs;

        let token_file = "token.json";

        match fs::remove_file(token_file).await {
            Ok(()) => {
                tracing::debug!("Token file forcefully removed: {}", token_file);
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!("No token file to clean up");
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to force cleanup token file: {e}");
                Err(Error::Token(TokenError::storage(format!(
                    "Failed to force cleanup token file: {e}"
                ))))
            }
        }
    }

    /// Resets the global TokenManager singleton (useful for testing)
    ///
    /// This method clears the global singleton instance, forcing the next
    /// call to get_global_instance() to create a fresh TokenManager.
    /// Primarily intended for test scenarios where a clean state is needed.
    pub async fn reset_global_instance() -> Result<(), Error> {
        // Clear any existing token from the current global instance
        if let Some(manager) = GLOBAL_TOKEN_MANAGER.get() {
            let _ = manager.clear_token().await;
        }

        // Reset the OnceCell to allow a new instance to be created
        // Note: OnceCell doesn't have a reset method, so we can't actually reset it
        // The best we can do is clear the token from the existing instance
        tracing::debug!("Global TokenManager instance token cleared for testing");
        Ok(())
    }
}

#[derive(Debug)]
pub struct ApiClient {
    client: Client,
    base_url: Url,
    token_strategy: Box<dyn TokenStrategy>,
}

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
impl ApiClient {
    /// Creates a new API client with the base URL from environment variables.
    ///
    /// Automatically selects the appropriate token strategy based on environment detection:
    /// - Mock strategy for mock environments (no persistence, isolated tokens)
    /// - Live strategy for live environments (full token management with persistence)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `AMP_API_BASE_URL` environment variable contains an invalid URL
    /// - Token strategy initialization fails
    pub async fn new() -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        let client = Client::new();

        // Automatic strategy selection based on environment
        let token_strategy = TokenEnvironment::create_auto_strategy(None).await?;

        tracing::info!(
            "Created ApiClient with {} strategy for base URL: {}",
            token_strategy.strategy_type(),
            base_url
        );

        Ok(Self {
            client,
            base_url,
            token_strategy,
        })
    }

    /// Creates a new API client with the specified base URL.
    ///
    /// Automatically selects the appropriate token strategy based on environment detection.
    ///
    /// # Errors
    ///
    /// Returns an error if token strategy initialization fails.
    pub async fn with_base_url(base_url: Url) -> Result<Self, Error> {
        let client = Client::new();

        // Automatic strategy selection based on environment
        let token_strategy = TokenEnvironment::create_auto_strategy(None).await?;

        tracing::info!(
            "Created ApiClient with {} strategy for base URL: {}",
            token_strategy.strategy_type(),
            base_url
        );

        Ok(Self {
            client,
            base_url,
            token_strategy,
        })
    }

    /// Creates a new API client with a custom token strategy (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL cannot be obtained from environment variables.
    pub fn with_token_strategy(token_strategy: Box<dyn TokenStrategy>) -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;

        tracing::info!(
            "Created ApiClient with explicit {} strategy for base URL: {}",
            token_strategy.strategy_type(),
            base_url
        );

        Ok(Self {
            client: Client::new(),
            base_url,
            token_strategy,
        })
    }

    /// Creates a new API client with a custom token manager (useful for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL cannot be obtained from environment variables.
    pub fn with_token_manager(token_manager: Arc<TokenManager>) -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        let token_strategy: Box<dyn TokenStrategy> =
            Box::new(LiveTokenStrategy::with_token_manager(token_manager));

        tracing::info!(
            "Created ApiClient with custom token manager for base URL: {}",
            base_url
        );

        Ok(Self {
            client: Client::new(),
            base_url,
            token_strategy,
        })
    }

    /// Creates a new API client for testing with a mock token strategy that always returns a fixed token.
    /// This bypasses all token acquisition and management logic and uses complete isolation.
    ///
    /// # Errors
    ///
    /// This method is infallible but returns Result for API consistency.
    pub async fn with_mock_token(base_url: Url, mock_token: String) -> Result<Self, Error> {
        let client = Client::new();
        let token_strategy: Box<dyn TokenStrategy> =
            Box::new(MockTokenStrategy::new(mock_token.clone()));

        tracing::info!(
            "Created ApiClient with explicit mock token strategy for base URL: {}",
            base_url
        );

        Ok(Self {
            client,
            base_url,
            token_strategy,
        })
    }

    /// Obtains a new authentication token from the AMP API.
    ///
    /// **Note**: This method is deprecated in favor of the automatic token management
    /// provided by `get_token()`. The `TokenManager` handles token acquisition internally
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
        // Delegate to get_token for backward compatibility
        self.get_token().await
    }

    /// Gets current token information for debugging and monitoring.
    ///
    /// Returns detailed information about the current token including:
    /// - Expiry time and remaining duration
    /// - Token age since acquisition
    /// - Expiry status flags
    ///
    /// Note: Mock strategies may return limited or no token information.
    ///
    /// # Returns
    /// `Some(TokenInfo)` if a token exists, `None` if no token is stored or strategy doesn't support info
    ///
    /// # Errors
    /// Returns an error if token information retrieval fails
    pub async fn get_token_info(&self) -> Result<Option<TokenInfo>, Error> {
        // Only live strategies support detailed token information
        if let Some(live_strategy) = self
            .token_strategy
            .as_any()
            .downcast_ref::<LiveTokenStrategy>()
        {
            live_strategy.get_token_info().await
        } else {
            // Mock strategies don't provide detailed token information
            tracing::debug!(
                "Token info not available for {} strategy",
                self.token_strategy.strategy_type()
            );
            Ok(None)
        }
    }

    /// Clears the stored token (useful for testing scenarios).
    ///
    /// This method removes the current token from storage, forcing the next
    /// `get_token()` call to obtain a fresh token.
    ///
    /// # Errors
    /// Returns an error if token clearing fails
    pub async fn clear_token(&self) -> Result<(), Error> {
        self.token_strategy.clear_token().await
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
        // Clear current token and get a fresh one
        self.token_strategy.clear_token().await?;
        self.token_strategy.get_token().await
    }

    /// Resets the global TokenManager singleton (useful for testing).
    ///
    /// This method clears the token from the global TokenManager instance.
    /// Primarily intended for test scenarios where a clean token state is needed.
    ///
    /// # Errors
    /// Returns an error if the reset operation fails
    pub async fn reset_global_token_manager() -> Result<(), Error> {
        TokenManager::reset_global_instance().await
    }

    /// Gets a valid authentication token with automatic token management.
    ///
    /// This method uses the integrated `TokenManager` to handle:
    /// - Proactive token refresh (5 minutes before expiry)
    /// - Automatic fallback from refresh to obtain on failure
    /// - Retry logic with exponential backoff
    /// - Thread-safe token storage
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition or refresh fails after all retries.
    pub async fn get_token(&self) -> Result<String, Error> {
        self.token_strategy.get_token().await
    }

    /// Returns the type of token strategy currently in use
    ///
    /// This is useful for debugging and testing to verify the correct strategy is selected.
    ///
    /// # Returns
    /// A string indicating the strategy type: "mock" or "live"
    pub fn get_strategy_type(&self) -> &'static str {
        self.token_strategy.strategy_type()
    }

    /// Returns whether the current strategy persists tokens
    ///
    /// This is useful for understanding the token management behavior.
    ///
    /// # Returns
    /// `true` if tokens are persisted to disk, `false` for in-memory only
    pub fn should_persist_tokens(&self) -> bool {
        self.token_strategy.should_persist()
    }

    /// Force cleanup of token files (for test cleanup)
    ///
    /// This is a static method that can be used to cleanup token files
    /// without needing an ApiClient instance. Useful for test teardown.
    ///
    /// # Errors
    /// Returns an error if token file cleanup fails
    pub async fn force_cleanup_token_files() -> Result<(), Error> {
        // Only cleanup if we're not in a live test environment
        let environment = TokenEnvironment::detect();
        if !environment.is_live() || environment.is_mock() {
            TokenManager::force_cleanup_token_files().await?;
            tracing::debug!("Token files cleaned up for non-live environment");
        } else {
            tracing::debug!("Skipping token file cleanup in live environment");
        }
        Ok(())
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

    /// Gets the memo for a specific asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to retrieve the memo for
    ///
    /// # Returns
    /// The memo string associated with the asset
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The asset does not exist
    /// - The response cannot be parsed
    pub async fn get_asset_memo(&self, asset_uuid: &str) -> Result<String, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "memo"], None::<&()>)
            .await
    }

    /// Sets a memo for the specified asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to set the memo for
    /// * `memo` - The memo string to associate with the asset
    ///
    /// # Returns
    /// Returns `Ok(())` on success.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The asset does not exist
    /// - The memo cannot be set due to validation errors
    ///
    /// # Example
    /// ```rust
    /// # use amp_rs::ApiClient;
    /// # async fn example(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    /// client.set_asset_memo("asset-uuid-123", "This is a memo for the asset").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_asset_memo(&self, asset_uuid: &str, memo: &str) -> Result<(), Error> {
        let token = self.get_token().await?;
        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().extend(&["assets", asset_uuid, "memo", "set"]);

        let response = self
            .client
            .request(Method::POST, url)
            .header(AUTHORIZATION, format!("token {token}"))
            .header("content-type", "application/json")
            .body(format!("\"{}\"", memo.replace("\"", "\\\"")))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Request to [\"assets\", \"{}\", \"memo\", \"set\"] failed with status {}: {}",
                asset_uuid, status, error_text
            )));
        }

        Ok(())
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

    pub async fn edit_registered_user(
        &self,
        registered_user_id: i64,
        edit_data: &crate::model::RegisteredUserEdit,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(
            Method::PUT,
            &["registered_users", &registered_user_id.to_string(), "edit"],
            Some(edit_data),
        )
        .await
    }

    pub async fn get_registered_user_summary(
        &self,
        registered_user_id: i64,
    ) -> Result<crate::model::RegisteredUserSummary, Error> {
        self.request_json(
            Method::GET,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "summary",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn get_registered_user_gaids(
        &self,
        registered_user_id: i64,
    ) -> Result<Vec<String>, Error> {
        self.request_json(
            Method::GET,
            &["registered_users", &registered_user_id.to_string(), "gaids"],
            None::<&()>,
        )
        .await
    }

    /// Associates a GAID with a registered user.
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user
    /// * `gaid` - The GAID to associate with the user
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The registered user ID is invalid
    /// - The GAID is invalid or already associated
    pub async fn add_gaid_to_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error> {
        let request = GaidRequest {
            gaid: gaid.to_string(),
        };

        self.request_empty(
            Method::POST,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "gaids",
                "add",
            ],
            Some(request),
        )
        .await
    }

    /// Sets an existing GAID as the default for a registered user.
    ///
    /// This method allows you to designate a specific GAID as the primary/default
    /// GAID for a registered user. The GAID must already be associated with the user.
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user
    /// * `gaid` - The GAID to set as default
    ///
    /// # Returns
    /// Returns `Ok(())` if the operation is successful.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The registered user ID is invalid
    /// - The GAID is not associated with the user
    pub async fn set_default_gaid_for_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error> {
        let request = GaidRequest {
            gaid: gaid.to_string(),
        };

        self.request_empty(
            Method::POST,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "gaids",
                "set-default",
            ],
            Some(request),
        )
        .await
    }

    /// Retrieves the registered user associated with a GAID
    ///
    /// # Arguments
    /// * `gaid` - The GAID to look up
    ///
    /// # Returns
    /// Returns the registered user data if the GAID is associated with a user
    ///
    /// # Errors
    /// This function will return an error if:
    /// - The GAID has no associated user
    /// - The GAID is invalid
    /// - Network or authentication errors occur
    pub async fn get_gaid_registered_user(
        &self,
        gaid: &str,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(
            Method::GET,
            &["gaids", gaid, "registered_user"],
            None::<&()>,
        )
        .await
    }

    /// Gets the balance information for a specific GAID.
    ///
    /// This method retrieves all asset balances associated with the given GAID,
    /// including confirmed balances and any lost outputs.
    ///
    /// # Arguments
    /// * `gaid` - The GAID to query balance for
    ///
    /// # Returns
    /// Returns a `Balance` struct containing confirmed balances and lost outputs
    ///
    /// # Errors
    /// Returns an error if:
    /// - The GAID is invalid
    /// - Network or authentication errors occur
    /// - The response cannot be parsed
    pub async fn get_gaid_balance(&self, gaid: &str) -> Result<Balance, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "balance"], None::<&()>)
            .await
    }

    /// Retrieves the specific asset balance for a GAID
    ///
    /// # Arguments
    /// * `gaid` - The GAID to query
    /// * `asset_uuid` - The UUID of the asset to query
    ///
    /// # Returns
    /// Returns the specific asset balance information
    ///
    /// # Errors
    /// Returns an error if:
    /// - The GAID is invalid
    /// - The asset UUID is invalid
    /// - Network or authentication errors occur
    /// - The response cannot be parsed
    pub async fn get_gaid_asset_balance(
        &self,
        gaid: &str,
        asset_uuid: &str,
    ) -> Result<Ownership, Error> {
        // Try to get the response as a GaidBalanceEntry first, then convert to Ownership
        let balance_entry: GaidBalanceEntry = self
            .request_json(
                Method::GET,
                &["gaids", gaid, "balance", asset_uuid],
                None::<&()>,
            )
            .await?;

        // Convert GaidBalanceEntry to Ownership format
        Ok(Ownership {
            owner: gaid.to_string(),
            amount: balance_entry.balance,
            gaid: Some(gaid.to_string()),
        })
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

    pub async fn get_asset_assignments(&self, asset_uuid: &str) -> Result<Vec<Assignment>, Error> {
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

        // The API only supports maximum length 1 per request, so we need to break
        // multiple assignments into separate CreateAssetAssignmentRequestWrapper instances
        let mut all_assignments = Vec::new();

        for request in requests {
            let wrapper = CreateAssetAssignmentRequestWrapper {
                assignments: vec![request.clone()],
            };

            let assignments: Vec<Assignment> = self
                .request_json(
                    Method::POST,
                    &["assets", asset_uuid, "assignments", "create"],
                    Some(&wrapper),
                )
                .await?;

            all_assignments.extend(assignments);
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
        self.request_json(
            Method::GET,
            &["managers", &manager_id.to_string()],
            None::<&()>,
        )
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
            &[
                "managers",
                &manager_id.to_string(),
                "assets",
                asset_uuid,
                "remove",
            ],
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

    /// Adds categories to a registered user.
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user
    /// * `categories` - A slice of category IDs to add to the user
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The registered user ID is invalid
    /// - Any category ID is invalid
    pub async fn add_categories_to_registered_user(
        &self,
        registered_user_id: i64,
        categories: &[i64],
    ) -> Result<(), Error> {
        let request_body = CategoriesRequest {
            categories: categories.to_vec(),
        };

        self.request_empty(
            Method::PUT,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "categories",
                "add",
            ],
            Some(request_body),
        )
        .await
    }

    /// Removes categories from a registered user
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user
    /// * `categories` - A slice of category IDs to remove from the user
    ///
    /// # Returns
    /// Returns `Ok(())` if the categories are successfully removed, or an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The registered user ID is invalid
    /// - Any category ID is not associated with the user
    pub async fn remove_categories_from_registered_user(
        &self,
        registered_user_id: i64,
        categories: &[i64],
    ) -> Result<(), Error> {
        let request_body = CategoriesRequest {
            categories: categories.to_vec(),
        };

        self.request_empty(
            Method::PUT,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "categories",
                "delete",
            ],
            Some(request_body),
        )
        .await
    }
}

fn get_amp_api_base_url() -> Result<Url, Error> {
    let url_str = env::var("AMP_API_BASE_URL")
        .unwrap_or_else(|_| "https://amp-test.blockstream.com/api".to_string());
    Url::parse(&url_str).map_err(Error::from)
}

/// Creates a token strategy based on automatic environment detection
///
/// This function detects the current environment and creates the appropriate strategy:
/// - Mock strategy for mock environments (isolated, no persistence)
/// - Live strategy for live environments (full token management)
///
/// # Arguments
/// * `mock_token` - Optional token to use for mock environments
///
/// # Errors
/// Returns an error if strategy creation fails
pub async fn create_auto_token_strategy(
    mock_token: Option<String>,
) -> Result<Box<dyn TokenStrategy>, Error> {
    TokenEnvironment::create_auto_strategy(mock_token).await
}

/// Creates a mock token strategy with the specified token
///
/// # Arguments
/// * `token` - The mock token to use
#[must_use]
pub fn create_mock_token_strategy(token: String) -> Box<dyn TokenStrategy> {
    Box::new(MockTokenStrategy::new(token))
}

/// Creates a live token strategy with default configuration
///
/// # Errors
/// Returns an error if the TokenManager cannot be initialized
pub async fn create_live_token_strategy() -> Result<Box<dyn TokenStrategy>, Error> {
    let strategy = LiveTokenStrategy::new().await?;
    Ok(Box::new(strategy))
}

/// Creates a token strategy for the specified environment
///
/// # Arguments
/// * `environment` - The target environment
/// * `mock_token` - Optional token to use for mock environments
///
/// # Errors
/// Returns an error if strategy creation fails
pub async fn create_token_strategy_for_environment(
    environment: TokenEnvironment,
    mock_token: Option<String>,
) -> Result<Box<dyn TokenStrategy>, Error> {
    environment.create_strategy(mock_token).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_mock_token_strategy_basic_functionality() {
        let mock_token = "mock_token_12345".to_string();
        let strategy = MockTokenStrategy::new(mock_token.clone());

        // Test get_token returns the mock token
        let result = strategy.get_token().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_token);

        // Test strategy type identification
        assert_eq!(strategy.strategy_type(), "mock");

        // Test persistence is disabled
        assert!(!strategy.should_persist());

        // Test clear_token is a no-op (should not fail)
        let clear_result = strategy.clear_token().await;
        assert!(clear_result.is_ok());

        // Verify token is still available after clear (since it's a no-op for mock)
        let token_after_clear = strategy.get_token().await;
        assert!(token_after_clear.is_ok());
        assert_eq!(token_after_clear.unwrap(), mock_token);
    }

    #[tokio::test]
    async fn test_mock_token_strategy_isolation() {
        let token1 = "token_instance_1".to_string();
        let token2 = "token_instance_2".to_string();

        let strategy1 = MockTokenStrategy::new(token1.clone());
        let strategy2 = MockTokenStrategy::new(token2.clone());

        // Test that different instances are isolated
        let result1 = strategy1.get_token().await.unwrap();
        let result2 = strategy2.get_token().await.unwrap();

        assert_eq!(result1, token1);
        assert_eq!(result2, token2);
        assert_ne!(result1, result2);

        // Test that operations on one don't affect the other
        let _ = strategy1.clear_token().await;
        let result2_after_clear = strategy2.get_token().await.unwrap();
        assert_eq!(result2_after_clear, token2);
    }

    #[tokio::test]
    async fn test_live_token_strategy_creation() {
        // Test creating a live strategy with global instance
        let strategy_result = LiveTokenStrategy::new().await;
        assert!(strategy_result.is_ok());

        let strategy = strategy_result.unwrap();
        assert_eq!(strategy.strategy_type(), "live");
        assert!(strategy.should_persist());
    }

    #[tokio::test]
    async fn test_live_token_strategy_with_custom_manager() {
        // Create a custom token manager for testing
        let config = RetryConfig::for_tests();
        let base_url = Url::parse("http://localhost:8080").unwrap();
        let mock_token = "test_live_token".to_string();

        let token_manager = Arc::new(
            TokenManager::with_mock_token(config, base_url, mock_token.clone())
                .await
                .unwrap(),
        );

        let strategy = LiveTokenStrategy::with_token_manager(token_manager);

        // Test strategy properties
        assert_eq!(strategy.strategy_type(), "live");
        assert!(strategy.should_persist());

        // Test token retrieval
        let token_result = strategy.get_token().await;
        assert!(token_result.is_ok());
        assert_eq!(token_result.unwrap(), mock_token);
    }

    #[tokio::test]
    async fn test_live_token_strategy_clear_token() {
        // Create a live strategy with a mock token manager
        let config = RetryConfig::for_tests();
        let base_url = Url::parse("http://localhost:8080").unwrap();
        let mock_token = "test_clear_token".to_string();

        let token_manager = Arc::new(
            TokenManager::with_mock_token(config, base_url, mock_token.clone())
                .await
                .unwrap(),
        );

        let strategy = LiveTokenStrategy::with_token_manager(token_manager);

        // Verify token is available initially
        let initial_token = strategy.get_token().await;
        assert!(initial_token.is_ok());
        assert_eq!(initial_token.unwrap(), mock_token);

        // Clear the token
        let clear_result = strategy.clear_token().await;
        assert!(clear_result.is_ok());

        // Note: After clearing, the TokenManager would try to obtain a new token
        // In a real scenario, this would fail without proper credentials
        // But our mock token manager will still return the same token
    }

    #[tokio::test]
    async fn test_strategy_type_identification() {
        let mock_strategy = MockTokenStrategy::new("test_token".to_string());
        let live_strategy = LiveTokenStrategy::new().await.unwrap();

        // Test that we can identify strategy types for debugging
        assert_eq!(mock_strategy.strategy_type(), "mock");
        assert_eq!(live_strategy.strategy_type(), "live");

        // Test persistence settings
        assert!(!mock_strategy.should_persist());
        assert!(live_strategy.should_persist());
    }

    #[tokio::test]
    async fn test_strategy_debug_formatting() {
        let mock_strategy = MockTokenStrategy::new("debug_test_token".to_string());
        let debug_output = format!("{:?}", mock_strategy);

        // Verify debug output contains expected information
        assert!(debug_output.contains("MockTokenStrategy"));
        assert!(debug_output.contains("debug_test_token"));
    }

    // Environment Detection Tests

    #[test]
    fn test_token_environment_detect_live_via_amp_tests() {
        // Set up environment for live test detection
        env::set_var("AMP_TESTS", "live");
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "real_pass");
        env::remove_var("AMP_API_BASE_URL");

        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Live);

        // Clean up
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
    }

    #[test]
    fn test_token_environment_detect_mock_via_credentials() {
        // Set up environment for mock detection via username
        env::remove_var("AMP_TESTS");
        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "real_pass");
        env::remove_var("AMP_API_BASE_URL");

        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Mock);

        // Test mock detection via password
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "mock_pass");

        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Mock);

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
    }

    #[test]
    fn test_token_environment_detect_mock_via_base_url() {
        // Set up environment for mock detection via localhost URL
        env::remove_var("AMP_TESTS");
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "real_pass");
        env::set_var("AMP_API_BASE_URL", "http://localhost:8080/api");

        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Mock);

        // Test with 127.0.0.1
        env::set_var("AMP_API_BASE_URL", "http://127.0.0.1:3000/api");
        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Mock);

        // Test with mock in URL
        env::set_var("AMP_API_BASE_URL", "http://mock-server.example.com/api");
        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Mock);

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
        env::remove_var("AMP_API_BASE_URL");
    }

    #[test]
    fn test_token_environment_detect_live_via_real_credentials() {
        // Set up environment for live detection via real credentials
        env::remove_var("AMP_TESTS");
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "real_pass");
        env::set_var("AMP_API_BASE_URL", "https://amp-test.blockstream.com/api");

        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Live);

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
        env::remove_var("AMP_API_BASE_URL");
    }

    #[test]
    fn test_token_environment_detect_mock_fallback() {
        // Set up environment with no credentials (fallback to mock)
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
        env::remove_var("AMP_API_BASE_URL");

        let environment = TokenEnvironment::detect();
        assert_eq!(environment, TokenEnvironment::Mock);
    }

    #[test]
    fn test_has_mock_credentials() {
        // Test mock username detection
        assert!(TokenEnvironment::has_mock_credentials(
            "mock_user",
            "real_pass",
            ""
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "Mock_User",
            "real_pass",
            ""
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "user_mock",
            "real_pass",
            ""
        ));

        // Test mock password detection
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "mock_pass",
            ""
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "Mock_Pass",
            ""
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "pass_mock",
            ""
        ));

        // Test mock URL detection
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "real_pass",
            "http://localhost:8080"
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "real_pass",
            "http://127.0.0.1:3000"
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "real_pass",
            "http://mock-server.com"
        ));
        assert!(TokenEnvironment::has_mock_credentials(
            "real_user",
            "real_pass",
            "http://Mock-Server.com"
        ));

        // Test non-mock credentials
        assert!(!TokenEnvironment::has_mock_credentials(
            "real_user",
            "real_pass",
            "https://amp-test.blockstream.com"
        ));
        assert!(!TokenEnvironment::has_mock_credentials("", "", ""));
    }

    #[test]
    fn test_token_environment_should_persist_tokens() {
        assert!(!TokenEnvironment::Mock.should_persist_tokens());
        assert!(TokenEnvironment::Live.should_persist_tokens());

        // Auto should delegate to detect()
        env::set_var("AMP_TESTS", "live");
        assert!(TokenEnvironment::Auto.should_persist_tokens());

        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "some_password");
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_API_BASE_URL");
        assert!(!TokenEnvironment::Auto.should_persist_tokens());

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
    }

    #[test]
    fn test_token_environment_is_mock_and_is_live() {
        assert!(TokenEnvironment::Mock.is_mock());
        assert!(!TokenEnvironment::Mock.is_live());

        assert!(!TokenEnvironment::Live.is_mock());
        assert!(TokenEnvironment::Live.is_live());

        // Auto should delegate to detect()
        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "some_password");
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_API_BASE_URL");
        assert!(TokenEnvironment::Auto.is_mock());
        assert!(!TokenEnvironment::Auto.is_live());

        env::set_var("AMP_TESTS", "live");
        assert!(!TokenEnvironment::Auto.is_mock());
        assert!(TokenEnvironment::Auto.is_live());

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
        env::remove_var("AMP_TESTS");
    }

    #[tokio::test]
    async fn test_token_environment_create_strategy_mock() {
        let mock_token = "test_mock_token".to_string();
        let strategy = TokenEnvironment::Mock
            .create_strategy(Some(mock_token.clone()))
            .await
            .unwrap();

        assert_eq!(strategy.strategy_type(), "mock");
        assert!(!strategy.should_persist());

        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, mock_token);
    }

    #[tokio::test]
    async fn test_token_environment_create_strategy_live() {
        let strategy = TokenEnvironment::Live.create_strategy(None).await.unwrap();

        assert_eq!(strategy.strategy_type(), "live");
        assert!(strategy.should_persist());
    }

    #[tokio::test]
    async fn test_token_environment_create_auto_strategy() {
        // Test with mock environment - need both username and password for proper detection
        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "some_password");
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_API_BASE_URL");

        let mock_token = "auto_mock_token".to_string();
        let strategy = TokenEnvironment::create_auto_strategy(Some(mock_token.clone()))
            .await
            .unwrap();

        assert_eq!(strategy.strategy_type(), "mock");
        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, mock_token);

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
    }

    #[tokio::test]
    async fn test_mock_token_strategy_factory_methods() {
        // Test with_default_token
        let strategy = MockTokenStrategy::with_default_token();
        assert_eq!(strategy.strategy_type(), "mock");
        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, "mock_token_default");

        // Test for_test
        let strategy = MockTokenStrategy::for_test("my_test");
        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, "mock_token_my_test");
    }

    #[tokio::test]
    async fn test_live_token_strategy_factory_methods() {
        // Test for_testing
        let strategy = LiveTokenStrategy::for_testing().await.unwrap();
        assert_eq!(strategy.strategy_type(), "live");
        assert!(strategy.should_persist());
    }

    #[tokio::test]
    async fn test_standalone_factory_functions() {
        // Test create_mock_token_strategy
        let mock_token = "standalone_mock".to_string();
        let strategy = create_mock_token_strategy(mock_token.clone());
        assert_eq!(strategy.strategy_type(), "mock");
        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, mock_token);

        // Test create_live_token_strategy
        let strategy = create_live_token_strategy().await.unwrap();
        assert_eq!(strategy.strategy_type(), "live");

        // Test create_auto_token_strategy with mock environment
        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "some_password");
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_API_BASE_URL");

        let auto_mock_token = "auto_standalone_mock".to_string();
        let strategy = create_auto_token_strategy(Some(auto_mock_token.clone()))
            .await
            .unwrap();
        assert_eq!(strategy.strategy_type(), "mock");
        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, auto_mock_token);

        // Test create_token_strategy_for_environment
        let env_mock_token = "env_mock".to_string();
        let strategy = create_token_strategy_for_environment(
            TokenEnvironment::Mock,
            Some(env_mock_token.clone()),
        )
        .await
        .unwrap();
        assert_eq!(strategy.strategy_type(), "mock");
        let token = strategy.get_token().await.unwrap();
        assert_eq!(token, env_mock_token);

        // Clean up
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
    }

    #[test]
    fn test_environment_detection_with_various_credential_combinations() {
        // Test case 1: AMP_TESTS=live overrides everything
        env::set_var("AMP_TESTS", "live");
        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "mock_pass");
        env::set_var("AMP_API_BASE_URL", "http://localhost:8080");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Live);

        // Test case 2: Mock username with real password and URL
        env::remove_var("AMP_TESTS");
        env::set_var("AMP_USERNAME", "mock_user");
        env::set_var("AMP_PASSWORD", "real_password");
        env::set_var("AMP_API_BASE_URL", "https://amp-test.blockstream.com/api");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Mock);

        // Test case 3: Real username with mock password
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "mock_password");
        env::set_var("AMP_API_BASE_URL", "https://amp-test.blockstream.com/api");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Mock);

        // Test case 4: Real credentials with localhost URL
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "real_password");
        env::set_var("AMP_API_BASE_URL", "http://localhost:3000/api");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Mock);

        // Test case 5: All real credentials
        env::set_var("AMP_USERNAME", "real_user");
        env::set_var("AMP_PASSWORD", "real_password");
        env::set_var("AMP_API_BASE_URL", "https://amp-test.blockstream.com/api");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Live);

        // Test case 6: Empty credentials
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
        env::remove_var("AMP_API_BASE_URL");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Mock);

        // Test case 7: Only username set
        env::set_var("AMP_USERNAME", "real_user");
        env::remove_var("AMP_PASSWORD");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Mock);

        // Test case 8: Only password set
        env::remove_var("AMP_USERNAME");
        env::set_var("AMP_PASSWORD", "real_password");
        assert_eq!(TokenEnvironment::detect(), TokenEnvironment::Mock);

        // Clean up all environment variables
        env::remove_var("AMP_TESTS");
        env::remove_var("AMP_USERNAME");
        env::remove_var("AMP_PASSWORD");
        env::remove_var("AMP_API_BASE_URL");
    }
}
