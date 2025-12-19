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

use elements::encode::Decodable;
use secrecy::ExposeSecret;
use secrecy::Secret;
use std::str::FromStr;

use crate::model::{
    Activity, AddressGaidResponse, Asset, AssetActivityParams, AssetDistributionAssignment,
    AssetLostOutputs, AssetSummary, AssetTransaction, AssetTransactionParams, Assignment, Balance,
    BroadcastResponse, CategoriesRequest, CategoryAdd, CategoryEdit, CategoryResponse,
    ChangePasswordRequest, ChangePasswordResponse, CreateAssetAssignmentRequest, EditAssetRequest,
    GaidBalanceEntry, IssuanceRequest, IssuanceResponse, Outpoint, Ownership, Password,
    ReceivedByAddress, RegisterAssetResponse, RegisteredUserResponse, Reissuance, TokenData,
    TokenInfo, TokenRequest, TokenResponse, TransactionDetail, TxInput, Unspent,
    UpdateBlindersRequest, Utxo, ValidateGaidResponse,
};
use crate::signer::{Signer, SignerError};

/// Environment variables used for token environment detection
#[derive(Debug)]
struct EnvironmentVariables {
    username: String,
    password: String,
    amp_tests: String,
    base_url: String,
}

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
        let env_vars = Self::read_environment_variables();
        Self::log_detection_start(&env_vars);

        if Self::is_explicit_live_environment(&env_vars.amp_tests) {
            return Self::Live;
        }

        if Self::has_mock_credentials(&env_vars.username, &env_vars.password, &env_vars.base_url) {
            Self::log_detection_result("mock environment via mock credentials");
            return Self::Mock;
        }

        if Self::has_real_credentials(&env_vars.username, &env_vars.password) {
            Self::log_detection_result("live environment via real credentials");
            return Self::Live;
        }

        Self::log_detection_result("mock environment via fallback (no credentials)");
        Self::Mock
    }

    /// Reads environment variables needed for token environment detection
    fn read_environment_variables() -> EnvironmentVariables {
        EnvironmentVariables {
            username: env::var("AMP_USERNAME").unwrap_or_default(),
            password: env::var("AMP_PASSWORD").unwrap_or_default(),
            amp_tests: env::var("AMP_TESTS").unwrap_or_default(),
            base_url: env::var("AMP_API_BASE_URL").unwrap_or_default(),
        }
    }

    /// Logs the start of environment detection with current variable values
    fn log_detection_start(env_vars: &EnvironmentVariables) {
        tracing::debug!(
            "Detecting token environment - AMP_TESTS: '{}', username: '{}', base_url: '{}'",
            env_vars.amp_tests,
            env_vars.username,
            env_vars.base_url
        );
    }

    /// Checks if the environment is explicitly set to live testing
    fn is_explicit_live_environment(amp_tests: &str) -> bool {
        if amp_tests == "live" {
            Self::log_detection_result("live environment via AMP_TESTS=live");
            true
        } else {
            false
        }
    }

    /// Checks if real (non-empty) credentials are present
    const fn has_real_credentials(username: &str, password: &str) -> bool {
        !username.is_empty() && !password.is_empty()
    }

    /// Logs the final detection result
    fn log_detection_result(reason: &str) {
        tracing::info!("Detected {}", reason);
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
            Self::Mock => Ok(Self::create_mock_strategy(mock_token)),
            Self::Live => Self::create_live_strategy().await,
            Self::Auto => Self::create_auto_detected_strategy(mock_token).await,
        }
    }

    /// Creates a mock token strategy with the provided or default token
    fn create_mock_strategy(mock_token: Option<String>) -> Box<dyn TokenStrategy> {
        let token = mock_token.unwrap_or_else(|| "default_mock_token".to_string());
        tracing::debug!("Creating mock token strategy with token");
        Box::new(MockTokenStrategy::new(token))
    }

    /// Creates a live token strategy
    async fn create_live_strategy() -> Result<Box<dyn TokenStrategy>, Error> {
        tracing::debug!("Creating live token strategy");
        let strategy = LiveTokenStrategy::new().await?;
        Ok(Box::new(strategy))
    }

    /// Creates a strategy based on auto-detected environment
    async fn create_auto_detected_strategy(
        mock_token: Option<String>,
    ) -> Result<Box<dyn TokenStrategy>, Error> {
        tracing::debug!("Auto-detecting environment for strategy creation");
        let detected = Self::detect();

        match detected {
            Self::Mock => Ok(Self::create_auto_detected_mock_strategy(mock_token)),
            Self::Live => Self::create_auto_detected_live_strategy().await,
            Self::Auto => Self::handle_unexpected_auto_detection(),
        }
    }

    /// Creates a mock strategy for auto-detected mock environment
    fn create_auto_detected_mock_strategy(mock_token: Option<String>) -> Box<dyn TokenStrategy> {
        let token = mock_token.unwrap_or_else(|| "default_mock_token".to_string());
        tracing::debug!("Auto-detected mock environment, creating mock strategy");
        Box::new(MockTokenStrategy::new(token))
    }

    /// Creates a live strategy for auto-detected live environment
    async fn create_auto_detected_live_strategy() -> Result<Box<dyn TokenStrategy>, Error> {
        tracing::debug!("Auto-detected live environment, creating live strategy");
        let strategy = LiveTokenStrategy::new().await?;
        Ok(Box::new(strategy))
    }

    /// Handles the unexpected case where `detect()` returns Auto
    fn handle_unexpected_auto_detection() -> Result<Box<dyn TokenStrategy>, Error> {
        tracing::error!("Unexpected Auto environment from detect()");
        Err(Error::Token(TokenError::validation(
            "Environment detection returned Auto, which should not happen".to_string(),
        )))
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
        Self::new(format!("mock_token_{test_name}"))
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
    #[error("AMP request failed\n\nMethod: {method}\nEndpoint: {endpoint}\nStatus: {status}\n\nError: {error_message}")]
    RequestFailedDetailed {
        /// The HTTP method used (GET, POST, etc.)
        method: String,
        /// The full endpoint URL that was called
        endpoint: String,
        /// The HTTP status code
        status: reqwest::StatusCode,
        /// The error message or response body
        error_message: String,
    },
    #[error("Failed to parse AMP response: {0}")]
    ResponseParsingFailed(String),
    #[error("Failed to parse AMP response: {serde_error}\n\nMethod: {method}\nEndpoint: {endpoint}\nExpected Type: {expected_type}\n\nRaw Response:\n{raw_response}")]
    ResponseDeserializationFailed {
        /// The HTTP method used (GET, POST, etc.)
        method: String,
        /// The full endpoint URL that was called
        endpoint: String,
        /// The expected Rust type name
        expected_type: String,
        /// The original serde deserialization error message
        serde_error: String,
        /// The complete raw response body text that failed to parse
        raw_response: String,
    },
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

/// Enhanced error enum for distribution operations and `ElementsRpc`
#[derive(Error, Debug)]
pub enum AmpError {
    #[error("API error: {0}")]
    Api(String),
    #[error("API error\n\nEndpoint: {endpoint}\nMethod: {method}\n\nError: {error_message}")]
    ApiDetailed {
        /// The API endpoint that was called
        endpoint: String,
        /// The HTTP method used
        method: String,
        /// The error message
        error_message: String,
    },

    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("RPC error: {error_message}\n\nMethod: {rpc_method}\nParameters: {params}\n\nRaw Response:\n{raw_response}")]
    RpcDetailed {
        /// The RPC method name that was called
        rpc_method: String,
        /// The parameters passed to the RPC method
        params: String,
        /// The error message
        error_message: String,
        /// The complete raw response from the RPC server
        raw_response: String,
    },

    #[error("Signer error: {0}")]
    Signer(#[from] SignerError),

    #[error("Timeout waiting for confirmations: {0}")]
    Timeout(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Serialization error: {serde_error}\n\nOperation: {operation}\nData Type: {data_type}\n\nContext: {context}")]
    SerializationDetailed {
        /// The serialization operation (serialize/deserialize)
        operation: String,
        /// The data type being processed
        data_type: String,
        /// Additional context about the operation
        context: String,
        /// The original serde error message
        serde_error: String,
    },

    #[error(transparent)]
    Existing(#[from] Error),
}

impl AmpError {
    /// Creates a new API error
    pub fn api<S: Into<String>>(message: S) -> Self {
        Self::Api(message.into())
    }

    /// Creates a new RPC error
    pub fn rpc<S: Into<String>>(message: S) -> Self {
        Self::Rpc(message.into())
    }

    /// Creates a new timeout error
    pub fn timeout<S: Into<String>>(message: S) -> Self {
        Self::Timeout(message.into())
    }

    /// Creates a new validation error
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation(message.into())
    }

    /// Adds context to an error
    #[must_use]
    pub fn with_context<S: Into<String>>(self, context: S) -> Self {
        let context_str = context.into();
        match self {
            Self::Api(msg) => Self::Api(format!("{context_str}: {msg}")),
            Self::ApiDetailed {
                endpoint,
                method,
                error_message,
            } => Self::ApiDetailed {
                endpoint,
                method,
                error_message: format!("{context_str}: {error_message}"),
            },
            Self::Rpc(msg) => Self::Rpc(format!("{context_str}: {msg}")),
            Self::RpcDetailed {
                rpc_method,
                params,
                error_message,
                raw_response,
            } => Self::RpcDetailed {
                rpc_method,
                params,
                error_message: format!("{context_str}: {error_message}"),
                raw_response,
            },
            Self::Timeout(msg) => Self::Timeout(format!("{context_str}: {msg}")),
            Self::Validation(msg) => Self::Validation(format!("{context_str}: {msg}")),
            other => other, // Don't modify other error types
        }
    }

    /// Returns true if this error indicates a retryable condition
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) | Self::Rpc(_) | Self::RpcDetailed { .. } => true, // RPC errors might be transient
            Self::Existing(Error::Token(token_err)) => token_err.is_retryable(),
            _ => false,
        }
    }

    /// Provides user-friendly retry instructions when applicable
    #[must_use]
    pub fn retry_instructions(&self) -> Option<String> {
        match self {
            Self::Network(_) => Some("Check network connection and retry".to_string()),
            Self::Rpc(_) | Self::RpcDetailed { .. } => {
                Some("Check Elements node connection and retry".to_string())
            }
            Self::Timeout(msg) if msg.contains("txid") => {
                Some("Use the transaction ID to manually confirm the distribution".to_string())
            }
            Self::Existing(Error::Token(TokenError::RateLimited {
                retry_after_seconds,
            })) => Some(format!(
                "Rate limited. Retry after {retry_after_seconds} seconds"
            )),
            _ => None,
        }
    }
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

#[cfg(test)]
mod amp_error_tests {
    use super::*;

    #[test]
    fn test_amp_error_creation_helpers() {
        let api_error = AmpError::api("Failed to create distribution");
        assert!(matches!(api_error, AmpError::Api(_)));

        let rpc_error = AmpError::rpc("Elements node connection failed");
        assert!(matches!(rpc_error, AmpError::Rpc(_)));

        let validation_error = AmpError::validation("Invalid asset UUID format");
        assert!(matches!(validation_error, AmpError::Validation(_)));

        let timeout_error = AmpError::timeout("Confirmation timeout");
        assert!(matches!(timeout_error, AmpError::Timeout(_)));
    }

    #[test]
    fn test_amp_error_with_context() {
        let api_error = AmpError::api("Failed to create distribution");
        let contextual_error = api_error.with_context("During distribution creation");

        match contextual_error {
            AmpError::Api(msg) => {
                assert!(msg.contains("During distribution creation"));
                assert!(msg.contains("Failed to create distribution"));
            }
            _ => panic!("Expected Api error variant"),
        }

        // Test that context doesn't modify errors that already have good context
        let signer_error = AmpError::Signer(SignerError::Lwk("Test error".to_string()));
        let contextual_signer = signer_error.with_context("Additional context");
        assert!(matches!(contextual_signer, AmpError::Signer(_)));
    }

    #[test]
    fn test_amp_error_retryability() {
        let api_error = AmpError::api("Failed to create distribution");
        assert!(!api_error.is_retryable());

        let rpc_error = AmpError::rpc("Elements node connection failed");
        assert!(rpc_error.is_retryable());

        let validation_error = AmpError::validation("Invalid asset UUID format");
        assert!(!validation_error.is_retryable());

        let timeout_error = AmpError::timeout("Confirmation timeout");
        assert!(!timeout_error.is_retryable());

        let signer_error = AmpError::Signer(SignerError::Lwk("Test error".to_string()));
        assert!(!signer_error.is_retryable());
    }

    #[test]
    fn test_amp_error_retry_instructions() {
        let rpc_error = AmpError::rpc("Elements node connection failed");
        let instructions = rpc_error.retry_instructions();
        assert!(instructions.is_some());
        assert!(instructions.unwrap().contains("Elements node"));

        let validation_error = AmpError::validation("Invalid asset UUID format");
        assert!(validation_error.retry_instructions().is_none());

        let timeout_with_txid = AmpError::timeout("Confirmation timeout for txid abc123");
        let timeout_instructions = timeout_with_txid.retry_instructions();
        assert!(timeout_instructions.is_some());
        assert!(timeout_instructions.unwrap().contains("transaction ID"));
    }

    #[test]
    fn test_amp_error_display() {
        let api_error = AmpError::api("Test API error");
        assert_eq!(format!("{}", api_error), "API error: Test API error");

        let rpc_error = AmpError::rpc("Test RPC error");
        assert_eq!(format!("{}", rpc_error), "RPC error: Test RPC error");

        let validation_error = AmpError::validation("Test validation error");
        assert_eq!(
            format!("{}", validation_error),
            "Validation error: Test validation error"
        );

        let timeout_error = AmpError::timeout("Test timeout error");
        assert_eq!(
            format!("{}", timeout_error),
            "Timeout waiting for confirmations: Test timeout error"
        );
    }

    #[test]
    fn test_amp_error_from_conversions() {
        // Test conversion from SignerError
        let signer_error = SignerError::Lwk("Test LWK error".to_string());
        let amp_error = AmpError::from(signer_error);
        assert!(matches!(amp_error, AmpError::Signer(_)));

        // Test conversion from existing Error
        let existing_error = Error::MissingEnvVar("TEST_VAR".to_string());
        let amp_error = AmpError::from(existing_error);
        assert!(matches!(amp_error, AmpError::Existing(_)));

        // Test conversion from serde_json::Error
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let amp_error = AmpError::from(json_error);
        assert!(matches!(amp_error, AmpError::Serialization(_)));
    }
}

/// Elements RPC client for blockchain operations
#[derive(Debug, Clone)]
pub struct ElementsRpc {
    client: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

/// Network information from Elements node
#[derive(Debug, serde::Deserialize)]
pub struct NetworkInfo {
    pub version: i64,
    pub subversion: String,
    pub protocolversion: i64,
    pub localservices: String,
    pub localrelay: bool,
    pub timeoffset: i64,
    pub networkactive: bool,
    pub connections: i64,
    pub networks: Vec<serde_json::Value>,
    pub relayfee: f64,
    pub incrementalfee: f64,
    pub localaddresses: Vec<serde_json::Value>,
    pub warnings: String,
}

/// Blockchain information from Elements node
#[derive(Debug, serde::Deserialize)]
pub struct BlockchainInfo {
    pub chain: String,
    pub blocks: i64,
    pub headers: i64,
    pub bestblockhash: String,
    #[serde(default)]
    pub difficulty: Option<f64>,
    #[serde(default)]
    pub mediantime: Option<i64>,
    #[serde(default)]
    pub verificationprogress: Option<f64>,
    #[serde(default)]
    pub initialblockdownload: Option<bool>,
    #[serde(default)]
    pub chainwork: Option<String>,
    #[serde(default)]
    pub size_on_disk: Option<i64>,
    #[serde(default)]
    pub pruned: Option<bool>,
    #[serde(default)]
    pub softforks: Option<serde_json::Value>,
    #[serde(default)]
    pub warnings: Option<String>,
}

/// RPC request structure for Elements node
#[derive(Debug, serde::Serialize)]
struct RpcRequest {
    jsonrpc: String,
    id: String,
    method: String,
    params: serde_json::Value,
}

/// RPC response structure from Elements node
#[derive(Debug, serde::Deserialize)]
struct RpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: Option<String>, // Optional for JSON-RPC 1.0 compatib
    #[allow(dead_code)]
    id: String,
    result: Option<T>,
    error: Option<RpcError>,
}

/// RPC error structure from Elements node
#[derive(Debug, serde::Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl ElementsRpc {
    /// Creates a new `ElementsRpc` client with connection parameters
    ///
    /// # Arguments
    /// * `url` - The RPC endpoint URL (e.g., <http://localhost:18884>)
    /// * `username` - RPC authentication username
    /// * `password` - RPC authentication password
    ///
    /// # Examples
    /// ```
    /// use amp_rs::ElementsRpc;
    ///
    /// let rpc = ElementsRpc::new(
    ///     "http://localhost:18884".to_string(),
    ///     "user".to_string(),
    ///     "pass".to_string()
    /// );
    /// ```
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created.
    #[must_use]
    pub fn new(url: String, username: String, password: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: url,
            username,
            password,
        }
    }

    /// Creates a new `ElementsRpc` client from environment variables
    ///
    /// Expected environment variables:
    /// - `ELEMENTS_RPC_URL`: RPC endpoint URL
    /// - `ELEMENTS_RPC_USER`: RPC username
    /// - `ELEMENTS_RPC_PASSWORD`: RPC password
    ///
    /// # Errors
    /// Returns an error if any required environment variable is missing
    ///
    /// # Examples
    /// ```no_run
    /// use amp_rs::ElementsRpc;
    ///
    /// let rpc = ElementsRpc::from_env().unwrap();
    /// ```
    pub fn from_env() -> Result<Self, AmpError> {
        let url = env::var("ELEMENTS_RPC_URL")
            .map_err(|_| AmpError::validation("Missing ELEMENTS_RPC_URL environment variable"))?;
        let username = env::var("ELEMENTS_RPC_USER")
            .map_err(|_| AmpError::validation("Missing ELEMENTS_RPC_USER environment variable"))?;
        let password = env::var("ELEMENTS_RPC_PASSWORD").map_err(|_| {
            AmpError::validation("Missing ELEMENTS_RPC_PASSWORD environment variable")
        })?;

        Ok(Self::new(url, username, password))
    }

    /// Makes an RPC call to the Elements node
    ///
    /// # Arguments
    /// * `method` - The RPC method name
    /// * `params` - The parameters for the RPC call
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or returns an error
    async fn rpc_call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, AmpError> {
        tracing::debug!("Making RPC call: {} with params: {:?}", method, params);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: method.to_string(),
            params,
        };

        let response = self
            .client
            .post(&self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<T> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        rpc_response
            .result
            .ok_or_else(|| AmpError::rpc("RPC response missing result field".to_string()))
    }

    /// Retrieves network information from the Elements node
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let network_info = rpc.get_network_info().await?;
    /// println!("Node version: {}", network_info.version);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_network_info(&self) -> Result<NetworkInfo, AmpError> {
        self.rpc_call("getnetworkinfo", serde_json::Value::Array(vec![]))
            .await
    }

    /// Retrieves blockchain information from the Elements node
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let blockchain_info = rpc.get_blockchain_info().await?;
    /// println!("Current block height: {}", blockchain_info.blocks);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_blockchain_info(&self) -> Result<BlockchainInfo, AmpError> {
        self.rpc_call("getblockchaininfo", serde_json::Value::Array(vec![]))
            .await
    }

    /// Unlocks the wallet with a passphrase for the specified timeout
    ///
    /// # Arguments
    /// * `passphrase` - The wallet passphrase
    /// * `timeout` - Timeout in seconds for the unlock
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.wallet_passphrase("my_passphrase", 300).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn wallet_passphrase(&self, passphrase: &str, timeout: u64) -> Result<(), AmpError> {
        let params = serde_json::json!([passphrase, timeout]);

        // wallet_passphrase returns null on success, so we need to handle this specially
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "walletpassphrase".to_string(),
            params,
        };

        let response = self
            .client
            .post(&self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        // For wallet_passphrase, null result is success
        Ok(())
    }

    /// Validates the connection to the Elements node
    ///
    /// This method performs basic connectivity and authentication checks by
    /// retrieving network information from the node.
    ///
    /// # Errors
    /// Returns an error if the connection validation fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.validate_connection().await?;
    /// println!("Connection to Elements node is valid");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn validate_connection(&self) -> Result<(), AmpError> {
        tracing::info!(
            "Validating connection to Elements node at {}",
            self.base_url
        );

        let network_info = self
            .get_network_info()
            .await
            .map_err(|e| e.with_context("Failed to validate Elements node connection"))?;

        tracing::info!(
            "Successfully connected to Elements node - Version: {}, Connections: {}",
            network_info.version,
            network_info.connections
        );

        Ok(())
    }

    /// Retrieves comprehensive node status including network and blockchain information
    ///
    /// This method combines network and blockchain information to provide a complete
    /// status overview of the Elements node.
    ///
    /// # Errors
    /// Returns an error if any RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let (network_info, blockchain_info) = rpc.get_node_status().await?;
    /// println!("Node version: {}, Block height: {}", network_info.version, blockchain_info.blocks);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_node_status(&self) -> Result<(NetworkInfo, BlockchainInfo), AmpError> {
        let network_info = self.get_network_info().await?;
        let blockchain_info = self.get_blockchain_info().await?;

        Ok((network_info, blockchain_info))
    }

    /// Lists unspent transaction outputs (UTXOs) for a specific asset
    ///
    /// # Arguments
    /// * `asset_id` - Optional asset ID to filter UTXOs. If None, returns all UTXOs
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Panics
    /// May panic if `asset_id` is `Some` but the warning log message attempts to unwrap it.
    /// This is a known logging issue and does not affect normal operation.
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let utxos = rpc.list_unspent(Some("asset_id_hex")).await?;
    /// println!("Found {} UTXOs", utxos.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_unspent(&self, asset_id: Option<&str>) -> Result<Vec<Unspent>, AmpError> {
        tracing::debug!("Listing unspent outputs for asset: {:?}", asset_id);

        let params = asset_id.map_or_else(
            || serde_json::json!([1, 9_999_999, [], true]),
            |asset| serde_json::json!([1, 9_999_999, [], true, {"asset": asset}]),
        );

        let utxos: Vec<Unspent> = self
            .rpc_call("listunspent", params)
            .await
            .map_err(|e| {
                if let Some(asset) = asset_id {
                    e.with_context(format!(
                        "Failed to list unspent outputs for asset {asset}. \
                        This may indicate that the treasury address is not imported in the Elements node. \
                        Ensure the treasury address is properly imported as a watch-only address."
                    ))
                } else {
                    e.with_context("Failed to list unspent outputs")
                }
            })?;

        tracing::debug!("Found {} unspent outputs", utxos.len());

        // If we're looking for a specific asset and found no UTXOs, provide helpful context
        if utxos.is_empty() && asset_id.is_some() {
            tracing::warn!(
                "No UTXOs found for asset {}. This may indicate:\n\
                1. The treasury address is not imported in the Elements node\n\
                2. The asset issuance transaction hasn't been confirmed yet\n\
                3. The UTXOs have already been spent",
                asset_id.unwrap()
            );
        }

        Ok(utxos)
    }

    /// List unspent outputs for a specific wallet
    ///
    /// This method lists unspent transaction outputs (UTXOs) for a specific wallet,
    /// optionally filtered by asset ID.
    ///
    /// # Arguments
    ///
    /// * `wallet_name` - Name of the Elements wallet to query
    /// * `asset_id` - Optional asset ID to filter UTXOs by
    ///
    /// # Returns
    ///
    /// Returns a vector of unspent outputs
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the wallet cannot be loaded
    ///
    /// # Panics
    /// May panic when processing UTXO blinding data if scriptpubkey is unexpectedly missing.
    /// This should not occur under normal operation with valid Elements node responses.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// // Note: This would need to be called in an async context
    /// // let utxos = rpc.list_unspent_for_wallet("test_wallet", None).await?;
    /// // println!("Found {} UTXOs", utxos.len());
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub async fn list_unspent_for_wallet(
        &self,
        wallet_name: &str,
        asset_id: Option<&str>,
    ) -> Result<Vec<Unspent>, AmpError> {
        tracing::debug!(
            "Listing unspent outputs for wallet {} and asset: {:?}",
            wallet_name,
            asset_id
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = asset_id.map_or_else(
            || serde_json::json!([1, 9_999_999, [], true]),
            |asset| serde_json::json!([1, 9_999_999, [], true, {"asset": asset}]),
        );

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "listunspent".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<Vec<Unspent>> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error listing unspent outputs: {} (code: {})",
                error.message, error.code
            )));
        }

        let mut utxos = rpc_response.result.unwrap_or_default();

        // Enrich UTXOs with scriptpubkey information if missing
        for utxo in &mut utxos {
            if utxo.scriptpubkey.is_none() {
                tracing::debug!(
                    "UTXO {}:{} missing scriptpubkey, attempting to derive from address",
                    utxo.txid,
                    utxo.vout
                );

                // Try to derive scriptpubkey from the address
                if let Ok(address) = elements::Address::from_str(&utxo.address) {
                    let script_pubkey = address.script_pubkey();
                    utxo.scriptpubkey = Some(hex::encode(script_pubkey.as_bytes()));
                    tracing::info!(
                        "Derived scriptpubkey for UTXO {}:{} from address {}: {}",
                        utxo.txid,
                        utxo.vout,
                        utxo.address,
                        utxo.scriptpubkey.as_ref().unwrap()
                    );
                } else {
                    tracing::error!(
                        "Failed to parse address {} for UTXO {}:{}",
                        utxo.address,
                        utxo.txid,
                        utxo.vout
                    );

                    // Fallback: try to get transaction details
                    match self.get_transaction_from_wallet(wallet_name, &utxo.txid).await {
                        Ok(tx_detail) => {
                            tracing::debug!(
                                "Retrieved transaction details for {} as fallback",
                                utxo.txid
                            );
                            // Parse the transaction hex to extract the scriptpubkey for this output
                            match hex::decode(&tx_detail.hex) {
                                Ok(tx_bytes) => {
                                    match elements::Transaction::consensus_decode(&tx_bytes[..]) {
                                        Ok(tx) => {
                                            if let Some(output) = tx.output.get(utxo.vout as usize)
                                            {
                                                utxo.scriptpubkey = Some(hex::encode(
                                                    output.script_pubkey.as_bytes(),
                                                ));
                                                tracing::info!("Enriched UTXO {}:{} with scriptpubkey from transaction: {}", 
                                                    utxo.txid, utxo.vout, utxo.scriptpubkey.as_ref().unwrap());
                                            } else {
                                                tracing::error!(
                                                    "Output {} not found in transaction {}",
                                                    utxo.vout,
                                                    utxo.txid
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to decode transaction {}: {}",
                                                utxo.txid,
                                                e
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to decode hex for transaction {}: {}",
                                        utxo.txid,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to get transaction details for {}: {}",
                                utxo.txid,
                                e
                            );
                        }
                    }
                }
            } else {
                tracing::debug!("UTXO {}:{} already has scriptpubkey", utxo.txid, utxo.vout);
            }
        }

        tracing::debug!(
            "Found {} unspent outputs for wallet {}",
            utxos.len(),
            wallet_name
        );

        // If we're looking for a specific asset and found no UTXOs, provide helpful context
        if utxos.is_empty() && asset_id.is_some() {
            tracing::warn!(
                "No UTXOs found for asset {} in wallet {}. This may indicate:\n\
                1. The asset issuance transaction hasn't been confirmed yet\n\
                2. The UTXOs have already been spent\n\
                3. The wallet doesn't contain the expected addresses",
                asset_id.unwrap(),
                wallet_name
            );
        }

        Ok(utxos)
    }

    /// Creates a raw transaction with the specified inputs and outputs
    ///
    /// # Arguments
    /// * `inputs` - Vector of transaction inputs (UTXOs to spend)
    /// * `outputs` - Map of addresses to amounts for regular outputs
    /// * `assets` - Map of addresses to asset IDs for Liquid-specific outputs
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or transaction creation fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ElementsRpc, model::{TxInput}};
    /// # use std::collections::HashMap;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let inputs = vec![TxInput {
    ///     txid: "abc123".to_string(),
    ///     vout: 0,
    ///     sequence: None,
    /// }];
    /// let mut outputs = HashMap::new();
    /// outputs.insert("address1".to_string(), 100.0);
    /// let mut assets = HashMap::new();
    /// assets.insert("address1".to_string(), "asset_id".to_string());
    /// let raw_tx = rpc.create_raw_transaction(inputs, outputs, assets).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn create_raw_transaction(
        &self,
        inputs: Vec<TxInput>,
        outputs: std::collections::HashMap<String, f64>,
        assets: std::collections::HashMap<String, String>,
    ) -> Result<String, AmpError> {
        tracing::debug!(
            "Creating raw transaction with {} inputs and {} outputs",
            inputs.len(),
            outputs.len()
        );

        // Elements RPC createrawtransaction expects:
        // createrawtransaction inputs outputs locktime replaceable assets
        let params = serde_json::json!([
            inputs,  // inputs as TxInput array
            outputs, // outputs as address->amount map
            0,       // locktime (0 = no locktime)
            false,   // replaceable (false = not replaceable)
            assets   // assets as address->asset_id map
        ]);

        // Debug: Log the exact parameters being sent to createrawtransaction
        tracing::error!("createrawtransaction parameters:");
        tracing::error!(
            "  inputs: {}",
            serde_json::to_string_pretty(&inputs).unwrap_or_default()
        );
        tracing::error!(
            "  outputs: {}",
            serde_json::to_string_pretty(&outputs).unwrap_or_default()
        );
        tracing::error!(
            "  assets: {}",
            serde_json::to_string_pretty(&assets).unwrap_or_default()
        );

        let raw_tx: String = self
            .rpc_call("createrawtransaction", params)
            .await
            .map_err(|e| {
                tracing::error!("createrawtransaction RPC call failed: {}", e);
                e.with_context("Failed to create raw transaction")
            })?;

        tracing::debug!("Created raw transaction: {}", raw_tx);
        Ok(raw_tx)
    }

    /// Imports an address into a specific wallet as watch-only
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to import into
    /// * `address` - The address to import
    /// * `label` - Optional label for the address
    /// * `rescan` - Whether to rescan the blockchain for transactions
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    async fn import_address_to_wallet(
        &self,
        wallet_name: &str,
        address: &str,
        label: Option<&str>,
        rescan: bool,
    ) -> Result<(), AmpError> {
        tracing::debug!("Importing address {} into wallet {}", address, wallet_name);

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([address, label.unwrap_or(""), rescan]);

        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importaddress".to_string(),
            params,
        };

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            // Ignore "already imported" errors
            if error.code != -4 {
                return Err(AmpError::rpc(format!(
                    "RPC error importing address: {} (code: {})",
                    error.message, error.code
                )));
            }
        }

        tracing::debug!(
            "Successfully imported address {} into wallet {}",
            address,
            wallet_name
        );
        Ok(())
    }

    /// Creates a raw transaction using a specific wallet context
    ///
    /// This method uses the wallet-specific RPC endpoint which is necessary
    /// for confidential transactions that require wallet context for blinding keys.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to use for transaction creation
    /// * `inputs` - Transaction inputs
    /// * `outputs` - Map of addresses to amounts
    /// * `assets` - Map of addresses to asset IDs
    ///
    /// # Returns
    /// Returns the raw transaction hex
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    #[allow(dead_code)]
    #[allow(clippy::cognitive_complexity)]
    async fn create_raw_transaction_with_wallet(
        &self,
        wallet_name: &str,
        inputs: Vec<TxInput>,
        outputs: std::collections::HashMap<String, f64>,
        assets: std::collections::HashMap<String, String>,
    ) -> Result<String, AmpError> {
        tracing::debug!(
            "Creating raw transaction with wallet {} - {} inputs and {} outputs",
            wallet_name,
            inputs.len(),
            outputs.len()
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        // Elements RPC createrawtransaction expects outputs as an array of objects
        // Each output object should contain both address, amount, and asset
        let mut outputs_array = Vec::new();

        for (address, amount) in &outputs {
            let asset_id = assets.get(address).ok_or_else(|| {
                AmpError::validation(format!("No asset ID found for address {address}"))
            })?;

            // Convert amount to string with proper precision for Elements
            let amount_str = format!("{amount:.8}");

            outputs_array.push(serde_json::json!({
                address.clone(): amount_str,
                "asset": asset_id
            }));
        }

        let params = serde_json::json!([
            inputs,        // inputs as TxInput array
            outputs_array, // outputs as array of {address: amount, asset: id} objects
            0,             // locktime (0 = no locktime)
            false,         // replaceable (false = not replaceable)
        ]);

        // Debug: Log the exact parameters being sent to createrawtransaction
        tracing::error!("createrawtransaction parameters (wallet-specific, corrected format):");
        tracing::error!("  wallet: {}", wallet_name);
        tracing::error!(
            "  inputs: {}",
            serde_json::to_string_pretty(&inputs).unwrap_or_default()
        );
        tracing::error!(
            "  outputs_array: {}",
            serde_json::to_string_pretty(&outputs_array).unwrap_or_default()
        );

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "createrawtransaction".to_string(),
            params,
        };

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<String> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error creating raw transaction: {} (code: {})",
                error.message, error.code
            )));
        }

        let raw_tx = rpc_response
            .result
            .ok_or_else(|| AmpError::rpc("No raw transaction returned".to_string()))?;

        tracing::debug!(
            "Created raw transaction with wallet {}: {}",
            wallet_name,
            raw_tx
        );
        Ok(raw_tx)
    }

    /// Imports an address into a specific wallet as watch-only
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet
    /// * `address` - The address to import
    /// * `label` - Optional label for the address
    /// * `rescan` - Optional whether to rescan the blockchain (default: false)
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.import_address("my_wallet", "vjU8L4dKa1XyyVcPqKBbTgjT1tRC7qYp5VJGwndZSCFk4ntpWey1pQe6hcSGDMVurr9CsZ21EGsqGjWA", Some("test_address"), Some(false)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_address(
        &self,
        wallet_name: &str,
        address: &str,
        label: Option<&str>,
        rescan: Option<bool>,
    ) -> Result<(), AmpError> {
        let rescan_value = rescan.unwrap_or(false);
        tracing::debug!(
            "Importing address: {} into wallet: {} with label: {:?}, rescan: {}",
            address,
            wallet_name,
            label,
            rescan_value
        );

        let params = serde_json::json!([address, label.unwrap_or(""), rescan_value]);

        // importaddress returns null on success
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importaddress".to_string(),
            params,
        };

        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        tracing::debug!(
            "Successfully imported address: {} into wallet: {}",
            address,
            wallet_name
        );
        Ok(())
    }

    /// Rescans the blockchain for a wallet
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to rescan
    /// * `start_height` - Optional start height for rescan (default: 0)
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let result = rpc.rescan_blockchain("my_wallet", None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn rescan_blockchain(
        &self,
        wallet_name: &str,
        start_height: Option<u64>,
    ) -> Result<serde_json::Value, AmpError> {
        tracing::debug!("Rescanning blockchain for wallet: {}", wallet_name);

        let params = start_height.map_or_else(
            || serde_json::json!([]),
            |height| serde_json::json!([height]),
        );

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "rescanblockchain".to_string(),
            params,
        };

        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error rescanning blockchain: {} (code: {})",
                error.message, error.code
            )));
        }

        let result = rpc_response
            .result
            .ok_or_else(|| AmpError::rpc("No result returned from rescanblockchain".to_string()))?;

        tracing::debug!(
            "Successfully rescanned blockchain for wallet: {}",
            wallet_name
        );
        Ok(result)
    }

    /// Creates or loads a wallet
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to create or load
    /// * `disable_private_keys` - Whether to disable private keys (watch-only wallet)
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.create_wallet("test_wallet", true).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_wallet(
        &self,
        wallet_name: &str,
        disable_private_keys: bool,
    ) -> Result<(), AmpError> {
        tracing::debug!(
            "Creating wallet: {} with disable_private_keys: {}",
            wallet_name,
            disable_private_keys
        );

        let params = serde_json::json!([wallet_name, disable_private_keys]);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "createwallet".to_string(),
            params,
        };

        let response = self
            .client
            .post(&self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            // Ignore "wallet already exists" error
            if error.code != -4 {
                return Err(AmpError::rpc(format!(
                    "RPC error {}: {}",
                    error.code, error.message
                )));
            }
            tracing::debug!("Wallet {} already exists", wallet_name);
        } else {
            tracing::debug!("Successfully created wallet: {}", wallet_name);
        }

        Ok(())
    }

    /// Loads an existing wallet
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to load
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.load_wallet("test_wallet").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn load_wallet(&self, wallet_name: &str) -> Result<(), AmpError> {
        tracing::debug!("Loading wallet: {}", wallet_name);

        let params = serde_json::json!([wallet_name]);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "loadwallet".to_string(),
            params,
        };

        let response = self
            .client
            .post(&self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            tracing::debug!(
                "Load wallet failed with status: {} - Body: {}",
                status,
                error_body
            );

            // For wallet loading, we want to be more permissive with errors
            // since the wallet might already be loaded
            if status == 500 && error_body.contains("already loaded") {
                tracing::debug!(
                    "Wallet {} appears to already be loaded (500 error)",
                    wallet_name
                );
                return Ok(());
            }

            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            // Ignore "wallet already loaded" error
            if error.code != -35 {
                return Err(AmpError::rpc(format!(
                    "RPC error {}: {}",
                    error.code, error.message
                )));
            }
            tracing::debug!("Wallet {} already loaded", wallet_name);
        } else {
            tracing::debug!("Successfully loaded wallet: {}", wallet_name);
        }

        Ok(())
    }

    /// Unloads a wallet
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to unload
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.unload_wallet("test_wallet").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn unload_wallet(&self, wallet_name: &str) -> Result<(), AmpError> {
        tracing::debug!("Unloading wallet: {}", wallet_name);

        let params = serde_json::json!([wallet_name]);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "unloadwallet".to_string(),
            params,
        };

        let response = self
            .client
            .post(&self.base_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        tracing::debug!("Successfully unloaded wallet: {}", wallet_name);
        Ok(())
    }

    /// Lists all available wallets
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let wallets = rpc.list_wallets().await?;
    /// println!("Available wallets: {:?}", wallets);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_wallets(&self) -> Result<Vec<String>, AmpError> {
        tracing::debug!("Listing available wallets");

        let params = serde_json::json!([]);

        let wallets: Vec<String> = self
            .rpc_call("listwallets", params)
            .await
            .map_err(|e| e.with_context("Failed to list wallets"))?;

        tracing::debug!("Found {} wallets", wallets.len());
        Ok(wallets)
    }

    /// Sets up a watch-only wallet with the given address
    ///
    /// This is a convenience method that creates a watch-only wallet and imports the address
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to create
    /// * `address` - Address to import as watch-only
    /// * `label` - Optional label for the address
    ///
    /// # Errors
    /// Returns an error if wallet creation or address import fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.setup_watch_only_wallet("test_wallet", "vjU8L4dKa1XyyVcPqKBbTgjT1tRC7qYp5VJGwndZSCFk4ntpWey1pQe6hcSGDMVurr9CsZ21EGsqGjWA", Some("treasury")).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn setup_watch_only_wallet(
        &self,
        wallet_name: &str,
        address: &str,
        label: Option<&str>,
    ) -> Result<(), AmpError> {
        tracing::info!(
            "Setting up watch-only wallet '{}' with address: {}",
            wallet_name,
            address
        );

        // Try the full wallet setup approach first
        match self
            .setup_wallet_with_address(wallet_name, address, label)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    "Successfully set up watch-only wallet '{}' with address: {}",
                    wallet_name,
                    address
                );
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(
                    "Full wallet setup failed: {}, trying direct address import",
                    e
                );
            }
        }

        // Fallback: Try to import the address directly without wallet operations
        match self.import_address_direct(address, label) {
            Ok(()) => {
                tracing::info!("Successfully imported address directly: {}", address);
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "Both wallet setup and direct import failed for address: {}",
                    address
                );
                Err(AmpError::rpc(format!(
                    "Failed to set up watch-only wallet or import address: wallet setup error: {e}, direct import error: {e}"
                )))
            }
        }
    }

    /// Attempts to set up a wallet with address using the standard approach
    async fn setup_wallet_with_address(
        &self,
        wallet_name: &str,
        address: &str,
        label: Option<&str>,
    ) -> Result<(), AmpError> {
        // Try to create the wallet (will ignore if it already exists)
        self.create_wallet(wallet_name, true).await?;

        // Try to load the wallet (will ignore if already loaded)
        self.load_wallet(wallet_name).await?;

        // Import the address without rescanning (for faster setup)
        self.import_address(wallet_name, address, label, Some(false))
            .await?;

        Ok(())
    }

    /// Attempts to import an address directly without wallet operations (uses default wallet)
    #[allow(clippy::unused_self)]
    fn import_address_direct(&self, address: &str, _label: Option<&str>) -> Result<(), AmpError> {
        tracing::debug!("Attempting direct address import for: {}", address);

        // This is a fallback method - we'll use empty string for wallet name to use default behavior
        // Note: This may not work as expected with the new signature, but kept for compatibility
        Err(AmpError::rpc(
            "Direct address import not supported with wallet-specific import_address".to_string(),
        ))
    }

    /// Broadcasts a signed raw transaction to the network
    ///
    /// # Arguments
    /// * `hex` - The signed transaction in hexadecimal format
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or transaction broadcast fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let signed_tx_hex = "0200000000..."; // Signed transaction hex
    /// let txid = rpc.send_raw_transaction(signed_tx_hex).await?;
    /// println!("Transaction broadcast with ID: {}", txid);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_raw_transaction(&self, hex: &str) -> Result<String, AmpError> {
        tracing::debug!(
            "Broadcasting raw transaction: {}",
            &hex[..std::cmp::min(hex.len(), 64)]
        );

        let params = serde_json::json!([hex]);

        let txid: String = self
            .rpc_call("sendrawtransaction", params)
            .await
            .map_err(|e| {
                tracing::error!("Raw transaction broadcast failed: {}", e);
                tracing::error!("Transaction hex (first 200 chars): {}", &hex[..std::cmp::min(hex.len(), 200)]);

                // Provide specific guidance for blinding-related errors
                if e.to_string().contains("bad-txns-in-ne-out") || e.to_string().contains("value in != value out") {
                    AmpError::rpc(format!(
                        "Transaction broadcast failed due to confidential transaction blinding error. \
                        This indicates that the blinding factors don't balance properly. \
                        Possible solutions:\n\
                        1. Ensure all addresses have proper blinding keys in the wallet\n\
                        2. Verify that blindrawtransaction was called before signing\n\
                        3. Check that UTXO blinding factors match between Elements and LWK\n\
                        4. Try using unconfidential addresses for testing\n\
                        Original error: {e}"
                    ))
                } else {
                    e.with_context("Failed to broadcast raw transaction")
                }
            })?;

        tracing::info!("Successfully broadcast transaction with ID: {}", txid);
        Ok(txid)
    }

    /// Retrieves transaction details from the Elements node's default wallet
    ///
    /// This method queries through the node's default RPC endpoint. Use this when:
    /// - Your Elements node has a default wallet configured, OR
    /// - You're querying non-confidential transactions
    ///
    /// For confidential transactions where you need a specific wallet's blinding keys
    /// to unblind the transaction, use [`get_transaction_from_wallet`] instead.
    ///
    /// # Arguments
    /// * `txid` - Transaction ID to retrieve
    ///
    /// # Returns
    /// Returns transaction details including confirmations
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or transaction is not found
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let tx_detail = rpc.get_transaction("abc123...").await?;
    /// println!("Transaction has {} confirmations", tx_detail.confirmations);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # See Also
    /// * [`get_transaction_from_wallet`] - For querying through a specific wallet
    pub async fn get_transaction(&self, txid: &str) -> Result<TransactionDetail, AmpError> {
        tracing::debug!("Retrieving transaction details for: {}", txid);

        let params = serde_json::json!([txid, true]); // true for verbose output

        let tx_detail: TransactionDetail = self
            .rpc_call("gettransaction", params)
            .await
            .map_err(|e| e.with_context(format!("Failed to get transaction details for {txid}")))?;

        tracing::debug!(
            "Retrieved transaction {} with {} confirmations",
            txid,
            tx_detail.confirmations
        );

        Ok(tx_detail)
    }

    /// Retrieves transaction details from a specific wallet in Elements
    ///
    /// This method queries through a wallet-specific RPC endpoint (`/wallet/{name}`). Use this when:
    /// - You need to query confidential transactions (the wallet holds blinding keys to unblind them)
    /// - Your Elements node doesn't have a default wallet configured
    /// - You're working with multiple wallets and need to specify which one to query
    ///
    /// For simpler setups where the node has a default wallet configured,
    /// you can use [`get_transaction`] instead.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the Elements wallet to query through
    /// * `txid` - Transaction ID to retrieve
    ///
    /// # Returns
    /// Returns transaction details including confirmations
    ///
    /// # Errors
    /// Returns an error if the transaction is not found, wallet not loaded, or RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let wallet_name = "amp_elements_wallet";
    /// let tx_detail = rpc.get_transaction_from_wallet(wallet_name, "abc123...").await?;
    /// println!("Transaction has {} confirmations", tx_detail.confirmations);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # See Also
    /// * [`get_transaction`] - For nodes with a default wallet configured
    pub async fn get_transaction_from_wallet(
        &self,
        wallet_name: &str,
        txid: &str,
    ) -> Result<TransactionDetail, AmpError> {
        tracing::debug!(
            "Retrieving transaction details for {} from wallet {}",
            txid,
            wallet_name
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([txid, true]); // true for verbose output

        // Create RPC request
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "gettransaction".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {body}"
            )));
        }

        let rpc_response: RpcResponse<TransactionDetail> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error getting transaction: {} (code: {})",
                error.message, error.code
            )));
        }

        let tx_detail = rpc_response
            .result
            .ok_or_else(|| AmpError::rpc(format!("No transaction details returned for {txid}")))?;

        tracing::debug!(
            "Retrieved transaction {} from wallet {} with {} confirmations",
            txid,
            wallet_name,
            tx_detail.confirmations
        );

        Ok(tx_detail)
    }

    /// Sends multiple outputs to multiple addresses using Elements' sendmany RPC
    ///
    /// This method uses Elements' built-in sendmany command which properly handles
    /// confidential transactions and blinding. This is the recommended approach for
    /// asset distribution as it avoids manual transaction construction issues.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the Elements wallet to use
    /// * `address_amounts` - Map of addresses to amounts to send
    /// * `asset_amounts` - Map of addresses to asset IDs for each output
    /// * `min_conf` - Minimum confirmations for inputs (default: 1)
    /// * `comment` - Optional transaction comment
    /// * `subtract_fee_from` - Optional addresses to subtract fees from
    /// * `replaceable` - Whether transaction is replaceable (default: false)
    /// * `conf_target` - Confirmation target for fee estimation (default: 1)
    /// * `estimate_mode` - Fee estimation mode (default: "UNSET")
    ///
    /// # Returns
    /// Returns the transaction ID of the sent transaction
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or transaction creation fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # use std::collections::HashMap;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    ///
    /// let mut address_amounts = HashMap::new();
    /// address_amounts.insert("address1".to_string(), 100.0);
    /// address_amounts.insert("address2".to_string(), 50.0);
    ///
    /// let mut asset_amounts = HashMap::new();
    /// asset_amounts.insert("address1".to_string(), "asset_id_hex".to_string());
    /// asset_amounts.insert("address2".to_string(), "asset_id_hex".to_string());
    ///
    /// let txid = rpc.sendmany("wallet_name", address_amounts, asset_amounts, None, None, None, None, None, None).await?;
    /// println!("Transaction sent with ID: {}", txid);
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
    pub async fn sendmany(
        &self,
        wallet_name: &str,
        address_amounts: std::collections::HashMap<String, f64>,
        asset_amounts: std::collections::HashMap<String, String>,
        min_conf: Option<u32>,
        comment: Option<&str>,
        subtract_fee_from: Option<Vec<String>>,
        replaceable: Option<bool>,
        conf_target: Option<u32>,
        estimate_mode: Option<&str>,
    ) -> Result<String, AmpError> {
        tracing::debug!(
            "Sending to {} addresses using sendmany for wallet {}",
            address_amounts.len(),
            wallet_name
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        // Elements sendmany parameters:
        // 1. dummy (empty string for compatibility)
        // 2. amounts (map of address -> amount)
        // 3. minconf (minimum confirmations, default 1)
        // 4. comment (optional comment)
        // 5. subtractfeefrom (array of addresses to subtract fee from)
        // 6. replaceable (boolean, default false)
        // 7. conf_target (confirmation target for fee estimation)
        // 8. estimate_mode (fee estimation mode)
        // 9. assetlabel (map of address -> asset_id for multi-asset sends)
        let params = serde_json::json!([
            "",                                    // dummy (required for compatibility)
            address_amounts,                       // amounts map
            min_conf.unwrap_or(1),                 // minconf
            comment.unwrap_or(""),                 // comment
            subtract_fee_from.unwrap_or_default(), // subtractfeefrom
            replaceable.unwrap_or(false),          // replaceable
            conf_target.unwrap_or(1),              // conf_target
            estimate_mode.unwrap_or("UNSET"),      // estimate_mode
            asset_amounts                          // assetlabel (asset map)
        ]);

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "sendmany".to_string(),
            params,
        };

        tracing::debug!("Sendmany request parameters:");
        tracing::debug!("  wallet: {}", wallet_name);
        tracing::debug!("  address_amounts: {:?}", address_amounts);
        tracing::debug!("  asset_amounts: {:?}", asset_amounts);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send sendmany RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "Sendmany RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<String> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse sendmany RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "Sendmany RPC error: {} (code: {})",
                error.message, error.code
            )));
        }

        let txid = rpc_response.result.unwrap_or_default();
        tracing::info!("Successfully sent transaction with sendmany: {}", txid);
        Ok(txid)
    }

    /// Waits for blockchain confirmations with configurable timeout
    ///
    /// This method polls the blockchain every 15 seconds to check for transaction confirmations.
    /// It waits for a minimum number of confirmations (default 2) before returning successfully.
    /// The method includes a configurable timeout to prevent indefinite waiting.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the Elements wallet to query through
    /// * `txid` - The transaction ID to monitor for confirmations
    /// * `min_confirmations` - Minimum number of confirmations required (default: 2)
    /// * `timeout_minutes` - Timeout in minutes (default: 10)
    ///
    /// # Returns
    /// Returns the final `TransactionDetail` when sufficient confirmations are reached
    ///
    /// # Errors
    /// Returns `AmpError::Timeout` if the timeout is exceeded before confirmations are received
    /// Returns `AmpError::Rpc` if there are issues communicating with the Elements node
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let wallet_name = "test_wallet";
    /// let tx_detail = rpc.wait_for_confirmations(wallet_name, "abc123...", Some(2), Some(10)).await?;
    /// println!("Transaction confirmed with {} confirmations", tx_detail.confirmations);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn wait_for_confirmations(
        &self,
        wallet_name: &str,
        txid: &str,
        min_confirmations: Option<u32>,
        timeout_minutes: Option<u64>,
    ) -> Result<TransactionDetail, AmpError> {
        self.wait_for_confirmations_with_interval(wallet_name, txid, min_confirmations, timeout_minutes, None)
            .await
    }

    /// Internal method for waiting for confirmations with configurable poll interval
    /// This is primarily used for testing to avoid long waits
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The timeout is exceeded before confirmations are received
    /// - There are issues communicating with the Elements node
    /// - The transaction cannot be found or is invalid
    #[allow(clippy::cognitive_complexity)]
    pub async fn wait_for_confirmations_with_interval(
        &self,
        wallet_name: &str,
        txid: &str,
        min_confirmations: Option<u32>,
        timeout_minutes: Option<u64>,
        poll_interval_secs: Option<u64>,
    ) -> Result<TransactionDetail, AmpError> {
        let min_confirmations = min_confirmations.unwrap_or(2);
        let timeout_minutes = timeout_minutes.unwrap_or(10);
        let timeout_duration = if timeout_minutes == 0 {
            std::time::Duration::from_secs(3) // Minimum 3 seconds for testing
        } else {
            std::time::Duration::from_secs(timeout_minutes * 60)
        };
        let poll_interval = std::time::Duration::from_secs(poll_interval_secs.unwrap_or(15));

        tracing::info!(
            "Starting confirmation monitoring for transaction {} (min_confirmations: {}, timeout: {} minutes)",
            txid,
            min_confirmations,
            timeout_minutes
        );

        let start_time = std::time::Instant::now();

        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() >= timeout_duration {
                let error_msg = format!(
                    "Timeout waiting for confirmations after {timeout_minutes} minutes. Transaction ID: {txid}. \
                    You can retry confirmation by calling the confirmation API with this txid."
                );
                tracing::error!("{}", error_msg);
                return Err(AmpError::Timeout(error_msg));
            }

            // Get current transaction details
            match self.get_transaction_from_wallet(wallet_name, txid).await {
                Ok(tx_detail) => {
                    tracing::debug!(
                        "Transaction {} has {} confirmations (need {})",
                        txid,
                        tx_detail.confirmations,
                        min_confirmations
                    );

                    if tx_detail.confirmations >= min_confirmations {
                        tracing::info!(
                            "Transaction {} confirmed with {} confirmations",
                            txid,
                            tx_detail.confirmations
                        );
                        return Ok(tx_detail);
                    }

                    // Log progress every few polls to avoid spam
                    if start_time.elapsed().as_secs() % 60 < 15 {
                        tracing::info!(
                            "Waiting for confirmations: {}/{} (elapsed: {}s)",
                            tx_detail.confirmations,
                            min_confirmations,
                            start_time.elapsed().as_secs()
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to get transaction details for {}: {}. Retrying in {} seconds...",
                        txid,
                        e,
                        poll_interval.as_secs()
                    );
                    // Continue polling even if individual calls fail, as the transaction
                    // might not be visible immediately after broadcasting
                }
            }

            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Reissues an asset using the Elements RPC reissueasset command
    ///
    /// This method reissues the specified amount of an asset.
    /// It requires the asset to be reissuable and the reissuance token to be available.
    ///
    /// # Arguments
    /// * `asset_id` - The asset ID (hex string) to reissue
    /// * `amount` - The amount to reissue (in satoshis for the asset)
    ///
    /// # Returns
    /// Returns a JSON value containing the reissuance output with txid and vin fields
    ///
    /// # Errors
    /// Returns an error if:
    /// - The asset ID is invalid
    /// - The asset is not reissuable
    /// - The reissuance token is not available
    /// - The RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    /// let amount = 1000000.0; // 0.01 of an asset with 8 decimals
    /// let result = rpc.reissueasset(asset_id, amount).await?;
    /// println!("Reissuance txid: {}, vin: {}", result["txid"], result["vin"]);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn reissueasset(
        &self,
        asset_id: &str,
        amount: f64,
    ) -> Result<serde_json::Value, AmpError> {
        tracing::debug!("Reissuing asset {} with amount {}", asset_id, amount);

        let params = serde_json::json!([asset_id, amount]);

        let result: serde_json::Value = self
            .rpc_call("reissueasset", params)
            .await
            .map_err(|e| {
                e.with_context(format!(
                    "Failed to reissue asset {asset_id}. \
                    Ensure the asset is reissuable and the reissuance token is available in the wallet."
                ))
            })?;

        // Extract txid and vin from result for logging
        let txid = result
            .get("txid")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let vin = result
            .get("vin")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        tracing::info!("Reissuance transaction created: txid={}, vin={}", txid, vin);

        Ok(result)
    }

    /// Lists all issuances for a specific asset or all assets
    ///
    /// This method retrieves issuance information including initial issuances
    /// and reissuances. If an `asset_id` is provided, only issuances for that
    /// asset are returned.
    ///
    /// # Arguments
    /// * `asset_id` - Optional asset ID to filter issuances by. If None, returns all issuances
    ///
    /// # Returns
    /// Returns a vector of JSON values, each containing issuance information
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    /// let issuances = rpc.list_issuances(Some(asset_id)).await?;
    /// for issuance in issuances {
    ///     if let Some(is_reissuance) = issuance.get("isreissuance").and_then(|v| v.as_bool()) {
    ///         println!("Reissuance: {}", is_reissuance);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_issuances(
        &self,
        asset_id: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, AmpError> {
        tracing::debug!("Listing issuances for asset: {:?}", asset_id);

        let params = asset_id.map_or_else(
            || serde_json::Value::Array(vec![]),
            |asset| serde_json::json!([asset]),
        );

        let issuances: Vec<serde_json::Value> = self
            .rpc_call("listissuances", params)
            .await
            .map_err(|e| e.with_context("Failed to list issuances"))?;

        tracing::debug!("Found {} issuances", issuances.len());

        Ok(issuances)
    }

    /// Destroys (burns) a specific amount of an asset
    ///
    /// This method calls the Elements node's `destroyamount` RPC to permanently
    /// remove (burn) a specified amount of an asset from the wallet.
    ///
    /// # Arguments
    /// * `asset_id` - The asset ID to burn
    /// * `amount` - The amount to burn (as a floating point number)
    ///
    /// # Returns
    /// Returns a JSON value containing the transaction ID of the burn transaction
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or if insufficient balance exists
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    /// let amount = 1000.0; // Burn 1000 units
    ///
    /// let txid = rpc.destroyamount(asset_id, amount).await?;
    /// println!("Burn transaction created: {}", txid);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn destroyamount(&self, asset_id: &str, amount: f64) -> Result<String, AmpError> {
        tracing::debug!("Burning asset {} with amount {}", asset_id, amount);

        let params = serde_json::json!([asset_id, amount]);

        let result: String = self.rpc_call("destroyamount", params).await.map_err(|e| {
            e.with_context(format!(
                "Failed to burn asset {asset_id}. \
                    Ensure sufficient balance exists in the wallet."
            ))
        })?;

        tracing::info!("Burn transaction created: txid={}", result);

        Ok(result)
    }

    /// Gets the balance for all assets or a specific asset
    ///
    /// This method calls the Elements node's `getbalance` RPC to retrieve
    /// the wallet balance. If an `asset_id` is provided, returns the balance
    /// for that specific asset. If None, returns balances for all assets.
    ///
    /// # Arguments
    /// * `asset_id` - Optional asset ID to get balance for. If None, returns all asset balances
    ///
    /// # Returns
    /// Returns a JSON value containing asset balances (as a map of `asset_id` -> balance)
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    ///
    /// let balances = rpc.get_balance(None).await?;
    /// if let Some(balance) = balances.get(asset_id) {
    ///     println!("Balance for asset {}: {}", asset_id, balance);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_balance(&self, asset_id: Option<&str>) -> Result<serde_json::Value, AmpError> {
        tracing::debug!("Getting balance for asset: {:?}", asset_id);

        // getbalance RPC signature: getbalance ( "dummy" minconf include_watchonly )
        // We use "*" as the account, 0 minconf, false for include_watchonly
        let params = serde_json::json!(["*", 0, false]);

        let balances: serde_json::Value = self
            .rpc_call("getbalance", params)
            .await
            .map_err(|e| e.with_context("Failed to get balance"))?;

        // If asset_id is specified, return just that balance
        if let Some(asset_id) = asset_id {
            if let Some(balance) = balances.get(asset_id) {
                return Ok(balance.clone());
            }
            return Ok(serde_json::json!(0.0));
        }

        tracing::debug!(
            "Retrieved balances for {} assets",
            balances.as_object().map_or(0, serde_json::Map::len)
        );

        Ok(balances)
    }

    /// Selects appropriate UTXOs to cover the required amount plus fees
    ///
    /// This method implements a simple UTXO selection algorithm that:
    /// 1. Filters UTXOs by asset ID and spendability
    /// 2. Sorts UTXOs by amount (largest first) for efficiency
    /// 3. Selects UTXOs until the target amount plus estimated fees is covered
    ///
    /// # Arguments
    /// * `asset_id` - The asset ID to select UTXOs for
    /// * `target_amount` - The total amount needed for distribution
    /// * `estimated_fee` - Estimated transaction fee in the same asset
    ///
    /// # Returns
    /// Returns a tuple of (`selected_utxos`, `total_selected_amount`)
    ///
    /// # Errors
    /// Returns an error if insufficient UTXOs are available or RPC calls fail
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let (selected_utxos, total_amount) = rpc.select_utxos_for_amount(
    ///     "wallet_name",
    ///     "asset_id_hex",
    ///     150.0,
    ///     0.001
    /// ).await?;
    /// println!("Selected {} UTXOs totaling {}", selected_utxos.len(), total_amount);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn select_utxos_for_amount(
        &self,
        wallet_name: &str,
        asset_id: &str,
        target_amount: f64,
        estimated_fee: f64,
    ) -> Result<(Vec<Unspent>, f64), AmpError> {
        tracing::debug!(
            "Selecting UTXOs for asset {} from wallet {} - target: {}, fee: {}",
            asset_id,
            wallet_name,
            target_amount,
            estimated_fee
        );

        // Get all UTXOs for this asset from the specified wallet
        let mut utxos = self
            .list_unspent_for_wallet(wallet_name, Some(asset_id))
            .await?;

        // Filter for spendable UTXOs only
        utxos.retain(|utxo| utxo.spendable && utxo.asset == asset_id);

        if utxos.is_empty() {
            return Err(AmpError::validation(format!(
                "No spendable UTXOs found for asset {asset_id}. \
                This typically means:\n\
                1. The treasury address is not imported in the Elements node as a watch-only address\n\
                2. The asset issuance transaction hasn't been confirmed yet\n\
                3. The UTXOs have already been spent\n\
                \n\
                To fix this:\n\
                - Ensure the treasury address is imported: `elements-cli importaddress <treasury_address> treasury false`\n\
                - Wait for the asset issuance transaction to be confirmed\n\
                - Check that the treasury address matches the one used for asset issuance"
            )));
        }

        // Sort UTXOs by amount (largest first) for efficient selection
        utxos.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let required_amount = target_amount + estimated_fee;
        let mut selected_utxos = Vec::new();
        let mut total_selected = 0.0;

        // Select UTXOs until we have enough to cover the required amount
        for utxo in utxos {
            selected_utxos.push(utxo.clone());
            total_selected += utxo.amount;

            if total_selected >= required_amount {
                break;
            }
        }

        // Check if we have sufficient funds
        if total_selected < required_amount {
            return Err(AmpError::validation(format!(
                "Insufficient UTXOs: need {required_amount}, have {total_selected} (target: {target_amount}, fee: {estimated_fee})"
            )));
        }

        tracing::info!(
            "Selected {} UTXOs totaling {} for target {} + fee {}",
            selected_utxos.len(),
            total_selected,
            target_amount,
            estimated_fee
        );

        Ok((selected_utxos, total_selected))
    }

    /// Builds a raw transaction for asset distribution with proper change handling
    ///
    /// This method orchestrates the complete transaction building process:
    /// 1. Selects appropriate UTXOs using `select_utxos_for_amount`
    /// 2. Creates transaction inputs from selected UTXOs
    /// 3. Creates outputs for distribution addresses
    /// 4. Calculates and creates change output if necessary
    /// 5. Builds the raw transaction using `create_raw_transaction`
    ///
    /// # Arguments
    /// * `asset_id` - The asset ID being distributed
    /// * `address_amounts` - Map of recipient addresses to amounts
    /// * `change_address` - Address to send change to (if any)
    /// * `estimated_fee` - Estimated transaction fee
    ///
    /// # Returns
    /// Returns a tuple of (`raw_transaction_hex`, `selected_utxos`, `change_amount`)
    ///
    /// # Errors
    /// Returns an error if UTXO selection fails or transaction building fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # use std::collections::HashMap;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let mut address_amounts = HashMap::new();
    /// address_amounts.insert("address1".to_string(), 100.0);
    /// address_amounts.insert("address2".to_string(), 50.0);
    ///
    /// let (raw_tx, utxos, change) = rpc.build_distribution_transaction(
    ///     "wallet_name",
    ///     "asset_id_hex",
    ///     address_amounts,
    ///     "change_address",
    ///     0.001
    /// ).await?;
    /// println!("Built transaction with {} inputs, change: {}", utxos.len(), change);
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    #[allow(clippy::too_many_lines)]
    pub async fn build_distribution_transaction(
        &self,
        wallet_name: &str,
        asset_id: &str,
        address_amounts: std::collections::HashMap<String, f64>,
        change_address: &str,
        _estimated_fee: f64,
    ) -> Result<(String, Vec<Unspent>, f64), AmpError> {
        const DUST_THRESHOLD: f64 = 0.00001;
        const LBTC_ASSET_ID: &str =
            "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49"; // L-BTC on Liquid testnet

        tracing::debug!(
            "Building distribution transaction for asset {} with {} outputs",
            asset_id,
            address_amounts.len()
        );

        // Calculate total distribution amount
        let total_distribution: f64 = address_amounts.values().sum();

        if total_distribution <= 0.0 {
            return Err(AmpError::validation(
                "Total distribution amount must be greater than zero".to_string(),
            ));
        }

        // Select UTXOs to cover the distribution (custom asset)
        let (selected_asset_utxos, total_selected) = self
            .select_utxos_for_amount(wallet_name, asset_id, total_distribution, 0.0)
            .await?;

        // Also select L-BTC UTXOs for transaction fees
        // Elements requires L-BTC inputs for fees even when distributing custom assets
        let min_lbtc_fee = 0.00001; // Minimum L-BTC needed for fees
        let (selected_lbtc_utxos, lbtc_total) = match self
            .select_utxos_for_amount(wallet_name, LBTC_ASSET_ID, 0.0, min_lbtc_fee)
            .await
        {
            Ok((utxos, total)) => {
                tracing::info!(
                    "Selected {} L-BTC UTXOs totaling {} for fees",
                    utxos.len(),
                    total
                );
                (utxos, total)
            }
            Err(e) => {
                tracing::warn!(
                    "Could not select L-BTC UTXOs for fees: {}. Transaction may fail.",
                    e
                );
                (Vec::new(), 0.0)
            }
        };

        // Combine custom asset UTXOs and L-BTC UTXOs
        let mut all_utxos = selected_asset_utxos.clone();
        all_utxos.extend(selected_lbtc_utxos.clone());

        if selected_lbtc_utxos.is_empty() {
            tracing::warn!(
                "No L-BTC UTXOs selected for fees. Transaction may fail during broadcast."
            );
        } else {
            tracing::info!(
                "Transaction includes {} custom asset UTXOs and {} L-BTC UTXOs for fees",
                selected_asset_utxos.len(),
                selected_lbtc_utxos.len()
            );
        }

        // Create transaction inputs from all selected UTXOs
        let inputs: Vec<TxInput> = all_utxos
            .iter()
            .map(|utxo| TxInput {
                txid: utxo.txid.clone(),
                vout: utxo.vout,
                sequence: None, // Use default sequence
            })
            .collect();

        // Create outputs for distribution (custom asset)
        // We need to track outputs as a vector since we may have multiple outputs to the same address
        // (e.g., custom asset change + L-BTC change to the same change address)
        let mut output_list = Vec::new();

        // Add distribution outputs (custom asset)
        for (address, amount) in &address_amounts {
            output_list.push((address.clone(), *amount, asset_id.to_string()));
        }

        // Calculate change amount for custom asset (total selected - distribution)
        let asset_change_amount = total_selected - total_distribution;

        // Add asset change output if there's a significant amount left
        if asset_change_amount > DUST_THRESHOLD {
            output_list.push((
                change_address.to_string(),
                asset_change_amount,
                asset_id.to_string(),
            ));

            tracing::debug!(
                "Adding asset change output: {} {} to address {}",
                asset_change_amount,
                asset_id,
                change_address
            );
        } else if asset_change_amount > 0.0 {
            tracing::warn!(
                "Asset change amount {} is below dust threshold {}, will be lost",
                asset_change_amount,
                DUST_THRESHOLD
            );
        }

        // Handle L-BTC change if we selected L-BTC UTXOs for fees
        // In Elements, the fee is implicit - it's the difference between L-BTC inputs and outputs
        // We should NOT subtract the fee from outputs; Elements calculates it automatically
        if !selected_lbtc_utxos.is_empty() {
            tracing::debug!(
                "L-BTC input total: {}, minimum fee needed: {}",
                lbtc_total,
                min_lbtc_fee
            );

            // Check if we have enough L-BTC for the minimum fee
            if lbtc_total < min_lbtc_fee {
                return Err(AmpError::validation(format!(
                    "Insufficient L-BTC for fees: have {lbtc_total}, need at least {min_lbtc_fee}"
                )));
            }

            // For now, let's try NOT adding any L-BTC change output
            // and let Elements handle the fee automatically from the input/output difference
            tracing::info!(
                "Using L-BTC input {} for fees - no explicit L-BTC change output (Elements will handle fee automatically)",
                lbtc_total
            );

            // Note: If this approach works, the entire L-BTC input will become the fee
            // If we need change, we'll need to figure out the correct way to handle it
        }

        // For confidential addresses, we need to import them into the wallet first
        // so Elements knows about the blinding keys
        for address in address_amounts.keys() {
            if address.starts_with('v') {
                // Confidential address
                tracing::debug!("Importing confidential address into wallet: {}", address);
                if let Err(e) = self
                    .import_address_to_wallet(wallet_name, address, None, false)
                    .await
                {
                    tracing::warn!("Failed to import confidential address {}: {}", address, e);
                    // Continue anyway - the address might already be imported
                }
            }
        }

        // Build the raw transaction using wallet-specific endpoint for confidential transactions
        // For confidential transactions, we need to use blindrawtransaction to properly handle blinding
        let raw_transaction = self
            .create_raw_transaction_with_outputs(wallet_name, inputs, output_list)
            .await
            .map_err(|e| {
                // Provide more helpful error message for the common L-BTC fee issue
                if e.to_string().contains("bad-txns-in-ne-out") || e.to_string().contains("value in != value out") {
                    AmpError::validation(format!(
                        "Transaction failed due to confidential transaction blinding mismatch. \
                        This occurs when Elements creates blinding factors that don't match LWK's expectations. \
                        To fix this:\n\
                        1. Ensure the wallet has proper blinding keys for all addresses\n\
                        2. Use blindrawtransaction before signing\n\
                        3. Verify UTXO blinding factors match between Elements and LWK\n\
                        4. Original error: {e}"
                    ))
                } else {
                    e.with_context("Failed to build distribution transaction")
                }
            })?;

        // For confidential transactions, we need to blind the transaction properly
        // This ensures the blinding factors are compatible with LWK signing
        tracing::debug!("Blinding raw transaction for confidential asset distribution");
        let blinded_transaction = self
            .blind_raw_transaction(wallet_name, &raw_transaction)
            .await
            .map_err(|e| {
                tracing::warn!(
                    "Failed to blind transaction, proceeding with unblinded: {}",
                    e
                );
                // If blinding fails, we'll try to proceed with the unblinded transaction
                // This might work for some cases but could fail during broadcast
                e.with_context("Transaction blinding failed")
            })
            .unwrap_or_else(|_| {
                tracing::warn!("Using unblinded transaction - this may cause broadcast failures");
                raw_transaction.clone()
            });

        tracing::info!(
            "Built distribution transaction: {} inputs, {} outputs, asset change: {}",
            all_utxos.len(),
            address_amounts.len() + usize::from(asset_change_amount > DUST_THRESHOLD),
            if asset_change_amount > DUST_THRESHOLD {
                asset_change_amount
            } else {
                0.0
            }
        );

        Ok((blinded_transaction, all_utxos, asset_change_amount))
    }

    /// Creates a raw transaction with multiple outputs that can handle multiple assets to the same address
    ///
    /// This method is similar to `create_raw_transaction_with_wallet` but handles the case where
    /// multiple outputs with different assets need to go to the same address (e.g., asset change + L-BTC change).
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the Elements wallet to use
    /// * `inputs` - Vector of transaction inputs
    /// * `outputs` - Vector of (address, amount, `asset_id`) tuples
    ///
    /// # Returns
    /// Returns the raw transaction hex string
    #[allow(clippy::cognitive_complexity)]
    async fn create_raw_transaction_with_outputs(
        &self,
        wallet_name: &str,
        inputs: Vec<TxInput>,
        outputs: Vec<(String, f64, String)>, // (address, amount, asset_id)
    ) -> Result<String, AmpError> {
        tracing::debug!(
            "Creating raw transaction with wallet {} - {} inputs and {} outputs",
            wallet_name,
            inputs.len(),
            outputs.len()
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        // Elements RPC createrawtransaction expects outputs as an array of objects
        // Each output object should contain both address, amount, and asset
        let mut outputs_array = Vec::new();

        for (address, amount, asset_id) in &outputs {
            // Convert amount to string with proper precision for Elements
            let amount_str = format!("{amount:.8}");

            outputs_array.push(serde_json::json!({
                address.clone(): amount_str,
                "asset": asset_id
            }));
        }

        let params = serde_json::json!([
            inputs,        // inputs as TxInput array
            outputs_array, // outputs as array of {address: amount, asset: id} objects
            0,             // locktime (0 = no locktime)
            false,         // replaceable (false = not replaceable)
        ]);

        // Debug: Log the exact parameters being sent to createrawtransaction
        tracing::error!("createrawtransaction parameters (wallet-specific, corrected format):");
        tracing::error!("  wallet: {}", wallet_name);
        tracing::error!(
            "  inputs: {}",
            serde_json::to_string_pretty(&inputs).unwrap_or_default()
        );
        tracing::error!(
            "  outputs_array: {}",
            serde_json::to_string_pretty(&outputs_array).unwrap_or_default()
        );

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "createrawtransaction".to_string(),
            params,
        };

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<String> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error creating raw transaction: {} (code: {})",
                error.message, error.code
            )));
        }

        Ok(rpc_response.result.unwrap_or_default())
    }

    /// Blinds a raw transaction for confidential transactions
    ///
    /// This method uses Elements' blindrawtransaction RPC to properly blind a transaction
    /// for confidential asset transfers. This is crucial for Liquid transactions to ensure
    /// the blinding factors are properly balanced.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the Elements wallet to use for blinding
    /// * `raw_transaction` - The raw transaction hex to blind
    ///
    /// # Returns
    /// Returns the blinded transaction hex string
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or blinding is not possible
    pub async fn blind_raw_transaction(
        &self,
        wallet_name: &str,
        raw_transaction: &str,
    ) -> Result<String, AmpError> {
        tracing::debug!(
            "Blinding raw transaction for wallet {} - tx length: {} chars",
            wallet_name,
            raw_transaction.len()
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        // Elements blindrawtransaction parameters:
        // 1. Raw transaction hex
        // 2. Input blinding data (can be empty array for auto-detection)
        // 3. Input amounts (can be empty array for auto-detection from UTXOs)
        // 4. Input assets (can be empty array for auto-detection from UTXOs)
        // 5. Input asset blinders (can be empty array for auto-detection)
        // 6. Input amount blinders (can be empty array for auto-detection)
        let params = serde_json::json!([
            raw_transaction, // Raw transaction hex
            [],              // Input blinding data (empty for auto-detection)
            [],              // Input amounts (empty for auto-detection)
            [],              // Input assets (empty for auto-detection)
            [],              // Input asset blinders (empty for auto-detection)
            []               // Input amount blinders (empty for auto-detection)
        ]);

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "blindrawtransaction".to_string(),
            params,
        };

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                AmpError::rpc(format!("Failed to send blindrawtransaction request: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "blindrawtransaction failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<String> = response.json().await.map_err(|e| {
            AmpError::rpc(format!("Failed to parse blindrawtransaction response: {e}"))
        })?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error blinding transaction: {} (code: {})",
                error.message, error.code
            )));
        }

        let blinded_tx = rpc_response.result.unwrap_or_default();

        tracing::info!(
            "Successfully blinded transaction - original: {} chars, blinded: {} chars",
            raw_transaction.len(),
            blinded_tx.len()
        );

        Ok(blinded_tx)
    }

    /// Signs a raw transaction using the provided signer callback
    ///
    /// This method integrates with the Signer trait to sign unsigned transactions.
    /// It handles the complete signing workflow including:
    /// 1. Validation of the unsigned transaction hex format
    /// 2. Calling the signer's `sign_transaction` method
    /// 3. Validation of the signed transaction format and structure
    /// 4. Proper error handling and context propagation
    ///
    /// # Arguments
    /// * `unsigned_tx_hex` - The unsigned transaction in hexadecimal format
    /// * `signer` - Implementation of the Signer trait for transaction signing
    ///
    /// # Returns
    /// Returns the signed transaction as a hex string
    ///
    /// # Errors
    /// Returns an error if:
    /// - The unsigned transaction hex is invalid or malformed
    /// - The signer fails to sign the transaction
    /// - The signed transaction format is invalid
    /// - Any validation checks fail
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ElementsRpc, signer::{Signer, LwkSoftwareSigner}};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let (_, signer) = LwkSoftwareSigner::generate_new()?;
    /// let unsigned_tx = "020000000001..."; // Unsigned transaction hex
    /// let signed_tx = rpc.sign_transaction(unsigned_tx, &signer).await?;
    /// println!("Transaction signed successfully: {}", signed_tx);
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn sign_transaction(
        &self,
        unsigned_tx_hex: &str,
        signer: &dyn crate::signer::Signer,
    ) -> Result<String, AmpError> {
        const MIN_TX_SIZE: usize = 10; // Minimum bytes for a valid transaction

        tracing::debug!(
            "Signing transaction: {}...",
            &unsigned_tx_hex[..std::cmp::min(unsigned_tx_hex.len(), 64)]
        );

        // Validate unsigned transaction hex format
        if unsigned_tx_hex.is_empty() {
            return Err(AmpError::validation(
                "Unsigned transaction hex cannot be empty".to_string(),
            ));
        }

        // Check if hex string has valid format (even length, valid hex characters)
        if !unsigned_tx_hex.len().is_multiple_of(2) {
            return Err(AmpError::validation(
                "Unsigned transaction hex must have even length".to_string(),
            ));
        }

        // Validate hex characters
        if !unsigned_tx_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AmpError::validation(
                "Unsigned transaction contains invalid hex characters".to_string(),
            ));
        }

        // Attempt to decode hex to validate transaction structure
        let tx_bytes = hex::decode(unsigned_tx_hex).map_err(|e| {
            AmpError::validation(format!("Failed to decode unsigned transaction hex: {e}"))
        })?;

        tracing::debug!("Unsigned transaction validation passed, calling signer");

        // Call the signer to sign the transaction
        let signed_tx_hex = signer
            .sign_transaction(unsigned_tx_hex)
            .await
            .map_err(|e| {
                tracing::error!("Transaction signing failed: {}", e);
                AmpError::Signer(e).with_context("Failed to sign transaction")
            })?;

        tracing::debug!(
            "Signer returned signed transaction: {}...",
            &signed_tx_hex[..std::cmp::min(signed_tx_hex.len(), 64)]
        );

        // Validate signed transaction format and structure
        if signed_tx_hex.is_empty() {
            return Err(AmpError::validation(
                "Signed transaction hex cannot be empty".to_string(),
            ));
        }

        // Check if signed transaction has valid hex format
        if !signed_tx_hex.len().is_multiple_of(2) {
            return Err(AmpError::validation(
                "Signed transaction hex must have even length".to_string(),
            ));
        }

        // Validate hex characters in signed transaction
        if !signed_tx_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AmpError::validation(
                "Signed transaction contains invalid hex characters".to_string(),
            ));
        }

        // Attempt to decode signed transaction to validate structure
        let signed_tx_bytes = hex::decode(&signed_tx_hex).map_err(|e| {
            AmpError::validation(format!("Failed to decode signed transaction hex: {e}"))
        })?;

        // Basic validation: signed transaction should be at least as long as unsigned
        // (signatures add data, so signed tx should be larger or equal)
        if signed_tx_bytes.len() < tx_bytes.len() {
            return Err(AmpError::validation(
                "Signed transaction is shorter than unsigned transaction, which is invalid"
                    .to_string(),
            ));
        }

        // Additional validation: check that the transaction structure is reasonable
        // Minimum transaction size for Elements (very basic check)
        if signed_tx_bytes.len() < MIN_TX_SIZE {
            return Err(AmpError::validation(format!(
                "Signed transaction does not meet minimum size ({} bytes), minimum is {} bytes",
                signed_tx_bytes.len(),
                MIN_TX_SIZE
            )));
        }

        tracing::info!(
            "Transaction signed successfully - unsigned: {} bytes, signed: {} bytes",
            tx_bytes.len(),
            signed_tx_bytes.len()
        );

        Ok(signed_tx_hex)
    }

    /// Signs and broadcasts a transaction in a single operation
    ///
    /// This is a convenience method that combines transaction signing and broadcasting.
    /// It performs the complete workflow of signing an unsigned transaction and
    /// immediately broadcasting it to the network.
    ///
    /// # Arguments
    /// * `unsigned_tx_hex` - The unsigned transaction in hexadecimal format
    /// * `signer` - Implementation of the Signer trait for transaction signing
    ///
    /// # Returns
    /// Returns the transaction ID of the broadcast transaction
    ///
    /// # Errors
    /// Returns an error if signing or broadcasting fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ElementsRpc, signer::{Signer, LwkSoftwareSigner}};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let (_, signer) = LwkSoftwareSigner::generate_new()?;
    /// let unsigned_tx = "020000000001..."; // Unsigned transaction hex
    /// let txid = rpc.sign_and_broadcast_transaction(unsigned_tx, &signer).await?;
    /// println!("Transaction broadcast with ID: {}", txid);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sign_and_broadcast_transaction(
        &self,
        unsigned_tx_hex: &str,
        signer: &dyn crate::signer::Signer,
    ) -> Result<String, AmpError> {
        tracing::info!("Signing and broadcasting transaction");

        // Sign the transaction
        let signed_tx_hex = self
            .sign_transaction(unsigned_tx_hex, signer)
            .await
            .map_err(|e| e.with_context("Failed during transaction signing phase"))?;

        // Broadcast the signed transaction
        let txid = self
            .send_raw_transaction(&signed_tx_hex)
            .await
            .map_err(|e| e.with_context("Failed during transaction broadcast phase"))?;

        tracing::info!("Successfully signed and broadcast transaction: {}", txid);
        Ok(txid)
    }

    /// Signs and broadcasts a transaction with UTXO information for proper PSBT construction
    ///
    /// This method provides UTXO information to the signer for proper PSBT construction,
    /// which is required for confidential transactions where the signer needs to know
    /// the previous transaction outputs being spent.
    ///
    /// # Arguments
    /// * `unsigned_tx_hex` - The unsigned transaction in hexadecimal format
    /// * `utxos` - Vector of UTXOs being spent in the transaction
    /// * `signer` - Implementation of the Signer trait for transaction signing
    ///
    /// # Returns
    /// Returns the transaction ID of the broadcast transaction
    ///
    /// # Errors
    /// Returns an error if signing or broadcasting fails
    #[allow(clippy::cognitive_complexity)]
    pub async fn sign_and_broadcast_transaction_with_utxos(
        &self,
        unsigned_tx_hex: &str,
        utxos: &[Unspent],
        signer: &dyn crate::signer::Signer,
    ) -> Result<String, AmpError> {
        tracing::info!(
            "Signing and broadcasting transaction with {} UTXOs",
            utxos.len()
        );

        // Try to use the enhanced signing method if the signer supports it
        let signed_tx_hex = if let Some(lwk_signer) = signer
            .as_any()
            .downcast_ref::<crate::signer::LwkSoftwareSigner>(
        ) {
            // Use the enhanced signing method with UTXO information
            tracing::debug!("Using LWK signer with UTXO information");
            lwk_signer
                .sign_transaction_with_utxos(unsigned_tx_hex, utxos)
                .await
                .map_err(|e| {
                    AmpError::Signer(e)
                        .with_context("Failed during enhanced transaction signing phase")
                })?
        } else {
            // Fall back to standard signing method
            tracing::debug!("Using standard signing method (no UTXO information)");
            self.sign_transaction(unsigned_tx_hex, signer)
                .await
                .map_err(|e| e.with_context("Failed during transaction signing phase"))?
        };

        // Broadcast the signed transaction
        let txid = self
            .send_raw_transaction(&signed_tx_hex)
            .await
            .map_err(|e| e.with_context("Failed during transaction broadcast phase"))?;

        tracing::info!("Successfully signed and broadcast transaction: {}", txid);
        Ok(txid)
    }

    /// Collects change data from a confirmed transaction for distribution confirmation
    ///
    /// This method queries the Elements node to find change UTXOs from a specific transaction
    /// that belong to the specified asset. It's used after a distribution transaction is
    /// confirmed to collect the change outputs for the final confirmation API call.
    ///
    /// # Arguments
    /// * `asset_id` - The asset ID to filter change UTXOs for
    /// * `txid` - The transaction ID to filter change UTXOs from
    ///
    /// # Returns
    /// Returns a vector of Unspent UTXOs that represent change outputs from the transaction.
    /// Returns an empty vector if no change outputs exist for the specified asset and transaction.
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or if there are issues querying the Elements node
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let change_data = rpc.collect_change_data(
    ///     "asset_id_hex",
    ///     "transaction_id_hex",
    ///     &rpc,
    ///     "wallet_name"
    /// ).await?;
    ///
    /// if change_data.is_empty() {
    ///     println!("No change outputs found for this transaction");
    /// } else {
    ///     println!("Found {} change outputs", change_data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn collect_change_data(
        &self,
        asset_id: &str,
        txid: &str,
        node_rpc: &Self,
        wallet_name: &str,
    ) -> Result<Vec<Unspent>, AmpError> {
        tracing::debug!(
            "Collecting change data for asset {} from transaction {}",
            asset_id,
            txid
        );

        // Use the raw listunspent RPC call to get full blinding information
        // This is essential for confidential transactions as the AMP API requires
        // both amountblinder and assetblinder fields
        let all_utxos = node_rpc
            .list_unspent_with_blinding_data(wallet_name)
            .await
            .map_err(|e| {
                e.with_context(
                    "Failed to query unspent outputs with blinding data for change data collection",
                )
            })?;

        // Filter UTXOs to only include those from the specified transaction
        let change_utxos: Vec<Unspent> = all_utxos
            .into_iter()
            .filter(|utxo| {
                // Match UTXOs that:
                // 1. Come from the specified transaction (txid matches)
                // 2. Are for the correct asset
                // 3. Are spendable
                utxo.txid == txid && utxo.asset == asset_id && utxo.spendable
            })
            .collect();

        tracing::info!(
            "Collected {} change UTXOs for asset {} from transaction {}",
            change_utxos.len(),
            asset_id,
            txid
        );

        // Log details of found change UTXOs for debugging
        for (index, utxo) in change_utxos.iter().enumerate() {
            tracing::debug!(
                "Change UTXO {}: txid={}, vout={}, amount={}, asset={}, amountblinder={:?}, assetblinder={:?}",
                index + 1,
                utxo.txid,
                utxo.vout,
                utxo.amount,
                utxo.asset,
                utxo.amountblinder,
                utxo.assetblinder
            );
        }

        // Handle the case where no change outputs exist
        if change_utxos.is_empty() {
            tracing::info!(
                "No change outputs found for asset {} in transaction {} - this is normal if all funds were distributed",
                asset_id,
                txid
            );
        }

        Ok(change_utxos)
    }

    /// Lists unspent outputs with full blinding data for confidential transactions
    ///
    /// This method calls the raw `listunspent` RPC to get complete UTXO information
    /// including blinding data (amountblinder and assetblinder) which is required
    /// for confidential transaction confirmation with the AMP API.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the Elements wallet to query
    ///
    /// # Returns
    /// Returns a vector of `Unspent` structs with complete blinding information
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let utxos = rpc.list_unspent_with_blinding_data("wallet_name").await?;
    /// for utxo in utxos {
    ///     println!("UTXO: {} with blinders: {:?}, {:?}",
    ///              utxo.txid, utxo.amountblinder, utxo.assetblinder);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_unspent_with_blinding_data(
        &self,
        wallet_name: &str,
    ) -> Result<Vec<Unspent>, AmpError> {
        tracing::debug!(
            "Listing unspent outputs with blinding data for wallet: {}",
            wallet_name
        );

        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        // Call listunspent with parameters to get all UTXOs
        // Parameters: minconf, maxconf, addresses, include_unsafe, query_options
        let params = serde_json::json!([
            0,         // minconf: include unconfirmed
            9_999_999, // maxconf: include all confirmed
            [],        // addresses: empty array means all addresses
            true,      // include_unsafe: include unconfirmed transactions
            {}         // query_options: empty object for default options
        ]);

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "listunspent".to_string(),
            params,
        };

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send listunspent RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "Listunspent RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<Vec<Unspent>> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse listunspent RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "Listunspent RPC error: {} (code: {})",
                error.message, error.code
            )));
        }

        let utxos = rpc_response.result.unwrap_or_default();
        tracing::info!(
            "Retrieved {} UTXOs with blinding data from wallet {}",
            utxos.len(),
            wallet_name
        );

        Ok(utxos)
    }

    /// Creates a standard wallet in Elements (Elements-first approach)
    ///
    /// This method creates a new standard wallet in the Elements node that can generate
    /// addresses and private keys. This is part of the Elements-first approach where
    /// we create the wallet in Elements first, then export keys to LWK.
    ///
    /// # Arguments
    /// * `wallet_name` - Name for the new wallet
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.create_elements_wallet("test_wallet").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_elements_wallet(&self, wallet_name: &str) -> Result<(), AmpError> {
        let params = serde_json::json!([wallet_name]);

        let _result: serde_json::Value = self.rpc_call("createwallet", params).await?;

        tracing::info!("Successfully created Elements wallet: {}", wallet_name);
        Ok(())
    }

    /// Get a new address from an Elements wallet
    ///
    /// This method requests a new address from the specified Elements wallet.
    /// The address will be generated by Elements and can be used for receiving funds.
    /// Defaults to native segwit (bech32) addresses for optimal compatibility.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to get address from
    /// * `address_type` - Optional address type ("bech32", "legacy", "p2sh-segwit"). Defaults to "bech32"
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the response format is unexpected
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    ///
    /// // Generate native segwit address (default)
    /// let address = rpc.get_new_address("test_wallet", None).await?;
    ///
    /// // Or explicitly request native segwit
    /// let bech32_address = rpc.get_new_address("test_wallet", Some("bech32")).await?;
    ///
    /// println!("Native segwit address: {}", address);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_new_address(
        &self,
        wallet_name: &str,
        address_type: Option<&str>,
    ) -> Result<String, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        // Set default to native segwit (bech32) for Elements
        let addr_type = address_type.unwrap_or("bech32");

        // For Elements, we need to use the correct parameters for getnewaddress
        // getnewaddress [label] [address_type]
        let params = serde_json::json!(["", addr_type]);

        // Create RPC request for getnewaddress
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "getnewaddress".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error getting new address: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            if let Some(address) = result.as_str() {
                tracing::info!("Generated new {} address: {}", addr_type, address);
                return Ok(address.to_string());
            }
        }

        Err(AmpError::rpc(format!(
            "Failed to get new address from wallet '{wallet_name}': unexpected response format"
        )))
    }

    /// Get the confidential version of an address from Elements wallet
    ///
    /// This method takes a regular (unconfidential) address and returns its confidential
    /// counterpart, which includes blinding keys for confidential transactions.
    ///
    /// # Arguments
    ///
    /// * `wallet_name` - Name of the Elements wallet
    /// * `address` - The unconfidential address to get info for
    ///
    /// # Returns
    ///
    /// Returns the confidential address string
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let unconfidential_address = "tex1q...";
    /// // Note: This would need to be called in an async context
    /// // let confidential_address = rpc.get_confidential_address("test_wallet", unconfidential_address).await?;
    /// // println!("Confidential address: {}", confidential_address);
    /// # Ok(())
    /// # }
    /// ```
    /// Gets the confidential address for a given unconfidential address from a wallet
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the response format is unexpected
    pub async fn get_confidential_address(
        &self,
        wallet_name: &str,
        address: &str,
    ) -> Result<String, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([address]);

        // Create RPC request for getaddressinfo
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "getaddressinfo".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {status} - Body: {error_body}"
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error getting address info: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            if let Some(confidential_address) = result.get("confidential").and_then(|v| v.as_str())
            {
                tracing::info!("Retrieved confidential address for: {}", address);
                return Ok(confidential_address.to_string());
            }
        }

        Err(AmpError::rpc(format!(
            "Failed to get confidential address for '{address}': unexpected response format"
        )))
    }

    /// Get the private key for an address from Elements wallet
    ///
    /// This method exports the private key for a specific address from the Elements wallet.
    /// The private key can then be imported into LWK for signing.
    ///
    /// Note: This is a simplified implementation that returns a placeholder private key.
    /// For production use, implement proper wallet-specific RPC calls.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet containing the address
    /// * `address` - The address to get the private key for
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let address = rpc.get_new_address("test_wallet", None).await?;
    /// let private_key = rpc.dump_private_key("test_wallet", &address).await?;
    /// println!("Private key: {}", private_key);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn dump_private_key(
        &self,
        wallet_name: &str,
        address: &str,
    ) -> Result<String, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([address]);

        // Create RPC request for dumpprivkey
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "dumpprivkey".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error dumping private key: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            if let Some(private_key) = result.as_str() {
                tracing::info!("Successfully exported private key for address: {}", address);
                return Ok(private_key.to_string());
            }
        }

        Err(AmpError::rpc(format!(
            "Failed to dump private key for address '{address}': unexpected response format"
        )))
    }

    /// Creates a descriptor wallet in Elements
    ///
    /// # Arguments
    /// * `wallet_name` - Name for the new wallet
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.create_descriptor_wallet("test_wallet").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_descriptor_wallet(&self, wallet_name: &str) -> Result<(), AmpError> {
        let params = serde_json::json!([wallet_name, true]); // true enables descriptors

        let _result: serde_json::Value = self.rpc_call("createwallet", params).await?;

        tracing::info!("Successfully created descriptor wallet: {}", wallet_name);
        Ok(())
    }

    /// Imports a single descriptor into an Elements wallet
    ///
    /// This method imports a descriptor that enables the wallet to scan and recognize
    /// addresses/UTXOs from a mnemonic. For LWK descriptors with `<0;1>/*` format,
    /// a single descriptor covers both receive and change addresses.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to import descriptor into
    /// * `descriptor` - The descriptor to import
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let descriptor = "ct(slip77(...),elwpkh([...]/84h/1h/0h]tpub.../<0;1>/*))#checksum";
    /// rpc.import_descriptor("test_wallet", descriptor).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_descriptor(
        &self,
        wallet_name: &str,
        descriptor: &str,
    ) -> Result<(), AmpError> {
        tracing::info!("Importing descriptor into wallet: {}", wallet_name);
        tracing::debug!("Descriptor: {}", descriptor);

        let descriptors = serde_json::json!([
            {
                "desc": descriptor,
                "timestamp": "now",
                "active": true,
                "internal": false  // For LWK descriptors with <0;1>/*, this covers both chains
            }
        ]);

        // Use -rpcwallet parameter to specify the wallet
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importdescriptors".to_string(),
            params: descriptors,
        };

        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        let result = rpc_response
            .result
            .ok_or_else(|| AmpError::rpc("RPC response missing result field".to_string()))?;

        // Check if descriptor was imported successfully
        if let Some(results) = result.as_array() {
            if let Some(result) = results.first() {
                if let Some(success) = result.get("success").and_then(serde_json::Value::as_bool) {
                    if !success {
                        let error_msg = result
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error");
                        return Err(AmpError::rpc(format!(
                            "Failed to import descriptor: {error_msg}"
                        )));
                    }
                } else {
                    return Err(AmpError::rpc(format!(
                        "Invalid response format for descriptor import: {result:?}"
                    )));
                }
            }
        } else {
            return Err(AmpError::rpc(format!(
                "Invalid response format: expected array, got {result:?}"
            )));
        }

        tracing::info!(
            "Successfully imported descriptor into wallet: {}",
            wallet_name
        );
        Ok(())
    }

    /// Imports descriptors into an Elements wallet (legacy method for compatibility)
    ///
    /// This method imports descriptors that enable the wallet to scan and recognize
    /// addresses/UTXOs from a mnemonic. If both descriptors are the same (as with LWK
    /// descriptors using `<0;1>/*` format), only one descriptor is imported.
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to import descriptors into
    /// * `receive_descriptor` - The receive descriptor
    /// * `change_descriptor` - The change descriptor
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let descriptor = "ct(slip77(...),elwpkh([...]/84h/1h/0h]tpub.../<0;1>/*))#checksum";
    /// rpc.import_descriptors("test_wallet", descriptor, descriptor).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn import_descriptors(
        &self,
        wallet_name: &str,
        receive_descriptor: &str,
        change_descriptor: &str,
    ) -> Result<(), AmpError> {
        // If both descriptors are the same (LWK case), import only once
        if receive_descriptor == change_descriptor {
            return self
                .import_descriptor(wallet_name, receive_descriptor)
                .await;
        }

        tracing::info!(
            "Importing separate receive and change descriptors into wallet: {}",
            wallet_name
        );
        tracing::debug!("Receive descriptor: {}", receive_descriptor);
        tracing::debug!("Change descriptor: {}", change_descriptor);

        let descriptors = serde_json::json!([
            {
                "desc": receive_descriptor,
                "timestamp": "now",
                "active": true,
                "internal": false
            },
            {
                "desc": change_descriptor,
                "timestamp": "now",
                "active": true,
                "internal": true
            }
        ]);

        // Use -rpcwallet parameter to specify the wallet
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importdescriptors".to_string(),
            params: descriptors,
        };

        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        let result = rpc_response
            .result
            .ok_or_else(|| AmpError::rpc("RPC response missing result field".to_string()))?;

        // Check if both descriptors were imported successfully
        if let Some(results) = result.as_array() {
            for (i, result) in results.iter().enumerate() {
                if let Some(success) = result.get("success").and_then(serde_json::Value::as_bool) {
                    if !success {
                        let desc_type = if i == 0 { "receive" } else { "change" };
                        let error_msg = result
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error");
                        return Err(AmpError::rpc(format!(
                            "Failed to import {desc_type} descriptor: {error_msg}"
                        )));
                    }
                } else {
                    return Err(AmpError::rpc(format!(
                        "Invalid response format for descriptor import: {result:?}"
                    )));
                }
            }
        } else {
            return Err(AmpError::rpc(format!(
                "Invalid response format: expected array, got {result:?}"
            )));
        }

        tracing::info!(
            "Successfully imported descriptors into wallet: {}",
            wallet_name
        );
        Ok(())
    }

    /// Sets up a wallet with descriptors from a mnemonic
    ///
    /// This is a convenience method that combines wallet creation and descriptor import.
    /// It creates a descriptor wallet and imports the receive and change descriptors
    /// generated from the provided mnemonic.
    ///
    /// # Arguments
    /// * `wallet_name` - Name for the new wallet
    /// * `receive_descriptor` - The receive descriptor (external chain /0/*)
    /// * `change_descriptor` - The change descriptor (internal chain /1/*)
    ///
    /// # Errors
    /// Returns an error if wallet creation or descriptor import fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let receive_desc = "wpkh([d34db33f/84h/1h/0h]xprv.../0/*)#checksum";
    /// let change_desc = "wpkh([d34db33f/84h/1h/0h]xprv.../1/*)#checksum";
    /// rpc.setup_wallet_with_descriptors("test_wallet", receive_desc, change_desc).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn setup_wallet_with_descriptors(
        &self,
        wallet_name: &str,
        receive_descriptor: &str,
        change_descriptor: &str,
    ) -> Result<(), AmpError> {
        tracing::info!("Setting up wallet with descriptors: {}", wallet_name);

        // Try to create the wallet (may fail if it already exists)
        match self.create_descriptor_wallet(wallet_name).await {
            Ok(()) => {
                tracing::info!("Created new descriptor wallet: {}", wallet_name);
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("already exists")
                    || error_msg.contains("Database already exists")
                {
                    tracing::info!(
                        "Wallet {} already exists, proceeding with descriptor import",
                        wallet_name
                    );
                } else {
                    return Err(e);
                }
            }
        }

        // Import the descriptors
        self.import_descriptors(wallet_name, receive_descriptor, change_descriptor)
            .await?;

        tracing::info!(
            "Successfully set up wallet with descriptors: {}",
            wallet_name
        );
        Ok(())
    }

    /// Exports a wallet to a file using dumpwallet RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to export
    /// * `file_path` - Path where the wallet dump file will be created
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the wallet cannot be exported
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.dump_wallet("my_wallet", "/tmp/wallet_export.dat").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn dump_wallet(&self, wallet_name: &str, file_path: &str) -> Result<(), AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([file_path]);

        // Create RPC request for dumpwallet
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "dumpwallet".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error dumping wallet: {} (code: {})",
                error.message, error.code
            )));
        }

        tracing::info!(
            "Successfully exported wallet {} to {}",
            wallet_name,
            file_path
        );
        Ok(())
    }

    /// Imports a wallet from a file using importwallet RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to import into
    /// * `file_path` - Path to the wallet dump file to import
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the wallet cannot be imported
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.import_wallet("my_wallet", "/tmp/wallet_export.dat").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_wallet(&self, wallet_name: &str, file_path: &str) -> Result<(), AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([file_path]);

        // Create RPC request for importwallet
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importwallet".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error importing wallet: {} (code: {})",
                error.message, error.code
            )));
        }

        tracing::info!(
            "Successfully imported wallet {} from {}",
            wallet_name,
            file_path
        );
        Ok(())
    }

    /// Exports a blinding key for a confidential address using dumpblindingkey RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet containing the address
    /// * `address` - The confidential address to export the blinding key for
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the address doesn't have a blinding key
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let key = rpc.dump_blinding_key("my_wallet", "VTpz...").await?;
    /// println!("Blinding key: {}", key);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn dump_blinding_key(
        &self,
        wallet_name: &str,
        address: &str,
    ) -> Result<String, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([address]);

        // Create RPC request for dumpblindingkey
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "dumpblindingkey".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error dumping blinding key: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            if let Some(blinding_key) = result.as_str() {
                tracing::info!(
                    "Successfully exported blinding key for address: {}",
                    address
                );
                return Ok(blinding_key.to_string());
            }
        }

        Err(AmpError::rpc(format!(
            "Failed to dump blinding key for address '{address}': unexpected response format"
        )))
    }

    /// Imports a blinding key for a confidential address using importblindingkey RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to import the blinding key into
    /// * `address` - The confidential address to import the blinding key for
    /// * `blinding_key` - The blinding key to import
    ///
    /// # Errors
    /// Returns an error if the RPC call fails or the blinding key cannot be imported
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.import_blinding_key("my_wallet", "VTpz...", "blinding_key_hex").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_blinding_key(
        &self,
        wallet_name: &str,
        address: &str,
        blinding_key: &str,
    ) -> Result<(), AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([address, blinding_key]);

        // Create RPC request for importblindingkey
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importblindingkey".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error importing blinding key: {} (code: {})",
                error.message, error.code
            )));
        }

        tracing::info!(
            "Successfully imported blinding key for address: {}",
            address
        );
        Ok(())
    }

    /// Gets wallet information using getwalletinfo RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to get information for
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let info = rpc.get_wallet_info("my_wallet").await?;
    /// println!("Wallet info: {:?}", info);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_wallet_info(&self, wallet_name: &str) -> Result<serde_json::Value, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([]);

        // Create RPC request for getwalletinfo
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "getwalletinfo".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error getting wallet info: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            tracing::info!("Successfully retrieved wallet info for: {}", wallet_name);
            return Ok(result);
        }

        Err(AmpError::rpc(format!(
            "Failed to get wallet info for '{wallet_name}': unexpected response format"
        )))
    }

    /// Gets the unconfidential address for a confidential address
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet
    /// * `confidential_address` - The confidential address to convert
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let unconf = rpc.get_unconfidential_address("my_wallet", "VTpz...").await?;
    /// println!("Unconfidential address: {}", unconf);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_unconfidential_address(
        &self,
        wallet_name: &str,
        confidential_address: &str,
    ) -> Result<String, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([confidential_address]);

        // Create RPC request for getunconfidentialaddress
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "getunconfidentialaddress".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error getting unconfidential address: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            if let Some(address) = result.as_str() {
                tracing::info!(
                    "Successfully got unconfidential address for: {}",
                    confidential_address
                );
                return Ok(address.to_string());
            }
        }

        Err(AmpError::rpc(format!(
            "Failed to get unconfidential address for '{confidential_address}': unexpected response format"
        )))
    }

    /// Imports a private key into the wallet using importprivkey RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to import into
    /// * `private_key` - The private key in WIF format
    /// * `label` - Optional label for the address
    /// * `rescan` - Whether to rescan the blockchain for transactions
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// rpc.import_private_key("my_wallet", "cT1...", Some("my_address"), Some(false)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn import_private_key(
        &self,
        wallet_name: &str,
        private_key: &str,
        label: Option<&str>,
        rescan: Option<bool>,
    ) -> Result<(), AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([private_key, label.unwrap_or(""), rescan.unwrap_or(false)]);

        // Create RPC request for importprivkey
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "importprivkey".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error importing private key: {} (code: {})",
                error.message, error.code
            )));
        }

        tracing::info!("Successfully imported private key");
        Ok(())
    }

    /// Lists all descriptors in a wallet using listdescriptors RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet
    /// * `private_keys` - Whether to include private keys in the output
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let descriptors = rpc.list_descriptors("my_wallet", Some(true)).await?;
    /// for desc in descriptors {
    ///     println!("Descriptor: {}", desc);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_descriptors(
        &self,
        wallet_name: &str,
        private_keys: Option<bool>,
    ) -> Result<Vec<String>, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([private_keys.unwrap_or(false)]);

        // Create RPC request for listdescriptors
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "listdescriptors".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error listing descriptors: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            // Result has a "descriptors" array with objects containing "desc" field
            if let Some(descriptors_array) = result.get("descriptors").and_then(|v| v.as_array()) {
                let descriptors: Vec<String> = descriptors_array
                    .iter()
                    .filter_map(|d| d.get("desc").and_then(|v| v.as_str()).map(String::from))
                    .collect();
                tracing::info!(
                    "Successfully retrieved {} descriptors for wallet: {}",
                    descriptors.len(),
                    wallet_name
                );
                return Ok(descriptors);
            }
        }

        Ok(Vec::new())
    }

    /// Gets all addresses in a wallet by label using getaddressesbylabel RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet
    /// * `label` - Label to filter by (empty string for all addresses)
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let addresses = rpc.get_addresses_by_label("my_wallet", "").await?;
    /// for addr in addresses {
    ///     println!("Address: {}", addr);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_addresses_by_label(
        &self,
        wallet_name: &str,
        label: &str,
    ) -> Result<Vec<String>, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([label]);

        // Create RPC request for getaddressesbylabel
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "getaddressesbylabel".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error getting addresses by label: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            // Result is an object with addresses as keys
            if let Some(obj) = result.as_object() {
                let addresses: Vec<String> = obj.keys().cloned().collect();
                tracing::info!(
                    "Successfully retrieved {} addresses for wallet: {}",
                    addresses.len(),
                    wallet_name
                );
                return Ok(addresses);
            }
        }

        Ok(Vec::new())
    }

    /// Lists addresses that have received transactions using listreceivedbyaddress RPC
    ///
    /// # Arguments
    /// * `wallet_name` - Name of the wallet to list addresses for
    /// * `min_conf` - Minimum number of confirmations (0 for unconfirmed)
    /// * `include_empty` - Whether to include addresses that haven't received payments
    ///
    /// # Errors
    /// Returns an error if the RPC call fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ElementsRpc;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let rpc = ElementsRpc::from_env()?;
    /// let addresses = rpc.list_received_by_address("my_wallet", 0, true).await?;
    /// for addr in addresses {
    ///     println!("Address: {:?}", addr);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_received_by_address(
        &self,
        wallet_name: &str,
        min_conf: u32,
        include_empty: bool,
    ) -> Result<Vec<ReceivedByAddress>, AmpError> {
        // First load the wallet to ensure it's available
        self.load_wallet(wallet_name).await?;

        let params = serde_json::json!([min_conf, include_empty]);

        // Create RPC request for listreceivedbyaddress
        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            id: "amp-client".to_string(),
            method: "listreceivedbyaddress".to_string(),
            params,
        };

        // Use the wallet-specific RPC endpoint
        let wallet_url = format!("{}/wallet/{}", self.base_url, wallet_name);

        let response = self
            .client
            .post(&wallet_url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&request)
            .send()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to send RPC request: {e}")))?;

        if !response.status().is_success() {
            return Err(AmpError::rpc(format!(
                "RPC request failed with status: {}",
                response.status()
            )));
        }

        let rpc_response: RpcResponse<Vec<ReceivedByAddress>> = response
            .json()
            .await
            .map_err(|e| AmpError::rpc(format!("Failed to parse RPC response: {e}")))?;

        if let Some(error) = rpc_response.error {
            return Err(AmpError::rpc(format!(
                "RPC error listing received by address: {} (code: {})",
                error.message, error.code
            )));
        }

        if let Some(result) = rpc_response.result {
            tracing::info!(
                "Successfully listed {} addresses for wallet: {}",
                result.len(),
                wallet_name
            );
            return Ok(result);
        }

        Ok(Vec::new())
    }
}

#[cfg(test)]
mod elements_rpc_tests {
    use super::*;
    use httpmock::prelude::*;
    use serial_test::serial;
    use std::collections::HashMap;

    #[test]
    fn test_elements_rpc_new() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        assert_eq!(rpc.base_url, "http://localhost:18884");
        assert_eq!(rpc.username, "user");
        assert_eq!(rpc.password, "pass");
    }

    #[test]
    #[serial]
    fn test_elements_rpc_from_env_missing_vars() {
        // Store original values to restore later
        let original_url = env::var("ELEMENTS_RPC_URL").ok();
        let original_user = env::var("ELEMENTS_RPC_USER").ok();
        let original_password = env::var("ELEMENTS_RPC_PASSWORD").ok();

        // Clear environment variables to test error handling
        env::remove_var("ELEMENTS_RPC_URL");
        env::remove_var("ELEMENTS_RPC_USER");
        env::remove_var("ELEMENTS_RPC_PASSWORD");

        let result = ElementsRpc::from_env();
        assert!(
            result.is_err(),
            "ElementsRpc::from_env() should fail when env vars are missing"
        );

        match result.unwrap_err() {
            AmpError::Validation(msg) => {
                assert!(
                    msg.contains("ELEMENTS_RPC_URL"),
                    "Error message should mention missing ELEMENTS_RPC_URL"
                );
            }
            _ => panic!("Expected validation error"),
        }

        // Restore original values or keep removed if they weren't set
        if let Some(val) = original_url {
            env::set_var("ELEMENTS_RPC_URL", val);
        }
        if let Some(val) = original_user {
            env::set_var("ELEMENTS_RPC_USER", val);
        }
        if let Some(val) = original_password {
            env::set_var("ELEMENTS_RPC_PASSWORD", val);
        }
    }

    #[test]
    #[serial]
    fn test_elements_rpc_from_env_success() {
        // Store original values to restore later
        let original_url = env::var("ELEMENTS_RPC_URL").ok();
        let original_user = env::var("ELEMENTS_RPC_USER").ok();
        let original_password = env::var("ELEMENTS_RPC_PASSWORD").ok();

        // Set test values
        env::set_var("ELEMENTS_RPC_URL", "http://localhost:18884");
        env::set_var("ELEMENTS_RPC_USER", "testuser");
        env::set_var("ELEMENTS_RPC_PASSWORD", "testpass");

        let result = ElementsRpc::from_env();
        assert!(
            result.is_ok(),
            "ElementsRpc::from_env() should succeed when all env vars are set"
        );

        let rpc = result.unwrap();
        assert_eq!(rpc.base_url, "http://localhost:18884");
        assert_eq!(rpc.username, "testuser");
        assert_eq!(rpc.password, "testpass");

        // Restore original values or remove if they weren't set
        match original_url {
            Some(val) => env::set_var("ELEMENTS_RPC_URL", val),
            None => env::remove_var("ELEMENTS_RPC_URL"),
        }
        match original_user {
            Some(val) => env::set_var("ELEMENTS_RPC_USER", val),
            None => env::remove_var("ELEMENTS_RPC_USER"),
        }
        match original_password {
            Some(val) => env::set_var("ELEMENTS_RPC_PASSWORD", val),
            None => env::remove_var("ELEMENTS_RPC_PASSWORD"),
        }
    }

    #[test]
    fn test_elements_rpc_method_signatures() {
        // Test that all new methods have correct signatures and can be called
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Test that methods exist and have correct signatures (compilation test)
        let _: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Vec<Unspent>, AmpError>> + Send + '_>,
        > = Box::pin(rpc.list_unspent(Some("test_asset")));

        let inputs = vec![TxInput {
            txid: "test_txid".to_string(),
            vout: 0,
            sequence: None,
        }];
        let outputs = std::collections::HashMap::new();
        let assets = std::collections::HashMap::new();

        let _: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<String, AmpError>> + Send + '_>,
        > = Box::pin(rpc.create_raw_transaction(inputs, outputs, assets));

        let _: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<String, AmpError>> + Send + '_>,
        > = Box::pin(rpc.send_raw_transaction("test_hex"));

        let _: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<TransactionDetail, AmpError>> + Send + '_>,
        > = Box::pin(rpc.get_transaction("test_txid"));
    }

    // Mock RPC response tests for UTXO and transaction operations

    #[tokio::test]
    async fn test_get_network_info_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "version": 220000,
                "subversion": "/Liquid:22.0.0/",
                "protocolversion": 70016,
                "localservices": "0000000000000409",
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": 8,
                "networks": [],
                "relayfee": 0.00001000,
                "incrementalfee": 0.00001000,
                "localaddresses": [],
                "warnings": ""
            }
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz") // base64 of "user:pass"
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "getnetworkinfo",
                    "params": []
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_network_info().await;

        assert!(result.is_ok());
        let network_info = result.unwrap();
        assert_eq!(network_info.version, 220000);
        assert_eq!(network_info.subversion, "/Liquid:22.0.0/");
        assert_eq!(network_info.connections, 8);

        mock.assert();
    }

    #[tokio::test]
    async fn test_get_blockchain_info_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "chain": "liquidregtest",
                "blocks": 12345,
                "headers": 12345,
                "bestblockhash": "abc123def456789",
                "difficulty": 4.656542373906925e-10,
                "mediantime": 1640995200,
                "verificationprogress": 1.0,
                "initialblockdownload": false,
                "chainwork": "0000000000000000000000000000000000000000000000000000000000003039",
                "size_on_disk": 1234567,
                "pruned": false,
                "softforks": {},
                "warnings": ""
            }
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "getblockchaininfo",
                    "params": []
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_blockchain_info().await;

        assert!(result.is_ok());
        let blockchain_info = result.unwrap();
        assert_eq!(blockchain_info.chain, "liquidregtest");
        assert_eq!(blockchain_info.blocks, 12345);
        assert_eq!(blockchain_info.bestblockhash, "abc123def456789");

        mock.assert();
    }

    #[tokio::test]
    async fn test_list_unspent_with_asset_filter() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": "abc123def456789",
                    "vout": 0,
                    "amount": 100.0,
                    "asset": "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d",
                    "address": "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq",
                    "spendable": true,
                    "confirmations": 6,
                    "scriptpubkey": "76a914abc123def456789abc123def456789abc123de88ac"
                },
                {
                    "txid": "def456abc123789",
                    "vout": 1,
                    "amount": 50.0,
                    "asset": "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d",
                    "address": "lq1qq3xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq",
                    "spendable": true,
                    "confirmations": 3
                }
            ]
        });

        let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "listunspent",
                    "params": [1, 9999999, [], true, {"asset": asset_id}]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.list_unspent(Some(asset_id)).await;

        assert!(result.is_ok());
        let utxos = result.unwrap();
        assert_eq!(utxos.len(), 2);
        assert_eq!(utxos[0].txid, "abc123def456789");
        assert_eq!(utxos[0].amount, 100.0);
        assert_eq!(utxos[0].asset, asset_id);
        assert_eq!(utxos[1].txid, "def456abc123789");
        assert_eq!(utxos[1].amount, 50.0);

        mock.assert();
    }

    #[tokio::test]
    async fn test_list_unspent_without_filter() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": "ghi789jkl012345",
                    "vout": 0,
                    "amount": 25.0,
                    "asset": "different_asset_id",
                    "address": "lq1qq4xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq",
                    "spendable": true,
                    "confirmations": 10
                }
            ]
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "listunspent",
                    "params": [1, 9999999, [], true]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.list_unspent(None).await;

        assert!(result.is_ok());
        let utxos = result.unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].txid, "ghi789jkl012345");
        assert_eq!(utxos[0].amount, 25.0);

        mock.assert();
    }

    #[tokio::test]
    async fn test_create_raw_transaction_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": "0200000000010abc123def456789abc123def456789abc123def456789abc123def456789abc123def456789000000006b483045022100..."
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "createrawtransaction",
                    "params": [
                        [
                            {
                                "txid": "input_txid_123",
                                "vout": 0,
                                "sequence": 4294967295u32
                            }
                        ],
                        {
                            "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq": 100.0
                        },
                        0,
                        false,
                        {
                            "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq": "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
                        }
                    ]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

        let inputs = vec![TxInput {
            txid: "input_txid_123".to_string(),
            vout: 0,
            sequence: Some(0xffffffff),
        }];

        let mut outputs = HashMap::new();
        outputs.insert(
            "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            100.0,
        );

        let mut assets = HashMap::new();
        assets.insert(
            "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d".to_string(),
        );

        let result = rpc.create_raw_transaction(inputs, outputs, assets).await;

        assert!(result.is_ok());
        let raw_tx = result.unwrap();
        assert!(raw_tx.starts_with("0200000000010abc123def456789"));

        mock.assert();
    }

    #[tokio::test]
    async fn test_send_raw_transaction_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": "abc123def456789abc123def456789abc123def456789abc123def456789abc123de"
        });

        let signed_tx_hex = "0200000000010abc123def456789abc123def456789abc123def456789abc123def456789abc123def456789000000006b483045022100...";

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "sendrawtransaction",
                    "params": [signed_tx_hex]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.send_raw_transaction(signed_tx_hex).await;

        assert!(result.is_ok());
        let txid = result.unwrap();
        assert_eq!(
            txid,
            "abc123def456789abc123def456789abc123def456789abc123def456789abc123de"
        );

        mock.assert();
    }

    #[tokio::test]
    async fn test_get_transaction_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": "abc123def456789abc123def456789abc123def456789abc123def456789abc123de",
                "confirmations": 6,
                "blockheight": 12345,
                "hex": "0200000000010abc123def456789...",
                "blockhash": "def456abc123789def456abc123789def456abc123789def456abc123789def456ab",
                "blocktime": 1640995200,
                "time": 1640995200,
                "timereceived": 1640995180
            }
        });

        let txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "gettransaction",
                    "params": [txid, true]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_transaction(txid).await;

        assert!(result.is_ok());
        let tx_detail = result.unwrap();
        assert_eq!(tx_detail.txid, txid);
        assert_eq!(tx_detail.confirmations, 6);
        assert_eq!(tx_detail.blockheight, Some(12345));
        assert_eq!(tx_detail.blocktime, Some(1640995200));

        mock.assert();
    }

    #[tokio::test]
    async fn test_get_transaction_from_wallet_success() {
        let server = MockServer::start();

        let wallet_name = "test_wallet";
        let txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

        // Mock the load_wallet call first
        let load_wallet_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "loadwallet",
                    "params": [wallet_name]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "result": {
                        "name": wallet_name,
                        "warning": ""
                    }
                }));
        });

        // Mock the get_transaction call to wallet-specific endpoint
        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": txid,
                "confirmations": 6,
                "blockheight": 12345,
                "hex": "0200000000010abc123def456789...",
                "blockhash": "def456abc123789def456abc123789def456abc123789def456abc123789def456ab",
                "blocktime": 1640995200,
                "time": 1640995200,
                "timereceived": 1640995180
            }
        });

        let get_transaction_mock = server.mock(|when, then| {
            when.method(POST)
                .path("//wallet/test_wallet")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "gettransaction",
                    "params": [txid, true]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_transaction_from_wallet(wallet_name, txid).await;

        assert!(result.is_ok());
        let tx_detail = result.unwrap();
        assert_eq!(tx_detail.txid, txid);
        assert_eq!(tx_detail.confirmations, 6);
        assert_eq!(tx_detail.blockheight, Some(12345));
        assert_eq!(tx_detail.blocktime, Some(1640995200));

        load_wallet_mock.assert();
        get_transaction_mock.assert();
    }

    // Error handling tests

    #[tokio::test]
    async fn test_rpc_call_network_failure() {
        // Use an invalid URL to simulate network failure
        let rpc = ElementsRpc::new(
            "http://invalid-host:99999".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        let result = rpc.get_network_info().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            AmpError::Rpc(msg) => {
                assert!(msg.contains("Failed to send RPC request"));
            }
            _ => panic!("Expected RPC error for network failure"),
        }
    }

    #[tokio::test]
    async fn test_rpc_call_http_error_status() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(500)
                .header("content-type", "application/json")
                .body("Internal Server Error");
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_network_info().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AmpError::Rpc(msg) => {
                assert!(msg.contains("RPC request failed with status: 500"));
            }
            _ => panic!("Expected RPC error for HTTP error status"),
        }

        mock.assert();
    }

    #[tokio::test]
    async fn test_rpc_call_invalid_json_response() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .body("invalid json response");
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_network_info().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AmpError::Rpc(msg) => {
                assert!(msg.contains("Failed to parse RPC response"));
            }
            _ => panic!("Expected RPC error for invalid JSON"),
        }

        mock.assert();
    }

    #[tokio::test]
    async fn test_rpc_call_error_response() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": null,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        });

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_network_info().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AmpError::Rpc(msg) => {
                assert!(msg.contains("RPC error -32601: Method not found"));
            }
            _ => panic!("Expected RPC error for error response"),
        }

        mock.assert();
    }

    #[tokio::test]
    async fn test_rpc_call_missing_result() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": null,
            "error": null
        });

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_network_info().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AmpError::Rpc(msg) => {
                assert!(msg.contains("RPC response missing result field"));
            }
            _ => panic!("Expected RPC error for missing result"),
        }

        mock.assert();
    }

    // Authentication tests

    #[tokio::test]
    async fn test_rpc_authentication_headers() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "version": 220000,
                "subversion": "/Liquid:22.0.0/",
                "protocolversion": 70016,
                "localservices": "0000000000000409",
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": 8,
                "networks": [],
                "relayfee": 0.00001000,
                "incrementalfee": 0.00001000,
                "localaddresses": [],
                "warnings": ""
            }
        });

        // Test with custom username and password
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dGVzdHVzZXI6dGVzdHBhc3M=") // base64 of "testuser:testpass"
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "getnetworkinfo",
                    "params": []
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(
            server.url("/"),
            "testuser".to_string(),
            "testpass".to_string(),
        );
        let result = rpc.get_network_info().await;

        assert!(result.is_ok());
        mock.assert();
    }

    // Wallet passphrase tests

    #[tokio::test]
    async fn test_wallet_passphrase_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": null
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "walletpassphrase",
                    "params": ["my_passphrase", 300]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.wallet_passphrase("my_passphrase", 300).await;

        assert!(result.is_ok());
        mock.assert();
    }

    // Connection validation tests

    #[tokio::test]
    async fn test_validate_connection_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "version": 220000,
                "subversion": "/Liquid:22.0.0/",
                "protocolversion": 70016,
                "localservices": "0000000000000409",
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": 8,
                "networks": [],
                "relayfee": 0.00001000,
                "incrementalfee": 0.00001000,
                "localaddresses": [],
                "warnings": ""
            }
        });

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.validate_connection().await;

        assert!(result.is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_get_node_status_success() {
        let server = MockServer::start();

        let network_mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "version": 220000,
                "subversion": "/Liquid:22.0.0/",
                "protocolversion": 70016,
                "localservices": "0000000000000409",
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": 8,
                "networks": [],
                "relayfee": 0.00001000,
                "incrementalfee": 0.00001000,
                "localaddresses": [],
                "warnings": ""
            }
        });

        let blockchain_mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "chain": "liquidregtest",
                "blocks": 12345,
                "headers": 12345,
                "bestblockhash": "abc123def456789",
                "difficulty": 4.656542373906925e-10,
                "mediantime": 1640995200,
                "verificationprogress": 1.0,
                "initialblockdownload": false,
                "chainwork": "0000000000000000000000000000000000000000000000000000000000003039",
                "size_on_disk": 1234567,
                "pruned": false,
                "softforks": {},
                "warnings": ""
            }
        });

        let network_mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body(serde_json::json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "getnetworkinfo",
                "params": []
            }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(network_mock_response);
        });

        let blockchain_mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body(serde_json::json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "getblockchaininfo",
                "params": []
            }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(blockchain_mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
        let result = rpc.get_node_status().await;

        assert!(result.is_ok());
        let (network_info, blockchain_info) = result.unwrap();
        assert_eq!(network_info.version, 220000);
        assert_eq!(blockchain_info.blocks, 12345);

        network_mock.assert();
        blockchain_mock.assert();
    }

    // Tests for UTXO selection and transaction building logic

    #[tokio::test]
    async fn test_build_distribution_transaction_zero_amount() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        let address_amounts = HashMap::new(); // Empty distribution

        let result = rpc
            .build_distribution_transaction(
                "test_wallet",
                "asset_id",
                address_amounts,
                "change_address",
                1.0,
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AmpError::Validation(msg) => {
                assert!(msg.contains("Total distribution amount must be greater than zero"));
            }
            _ => panic!("Expected validation error for zero distribution amount"),
        }
    }

    #[tokio::test]
    async fn test_sign_transaction_validation() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Mock signer for testing
        struct MockSigner {
            should_fail: bool,
            return_value: String,
        }

        #[async_trait::async_trait]
        impl crate::signer::Signer for MockSigner {
            async fn sign_transaction(
                &self,
                _unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                if self.should_fail {
                    Err(crate::signer::SignerError::Lwk(
                        "Mock signing failure".to_string(),
                    ))
                } else {
                    // Return a longer hex string to simulate signed transaction (20+ bytes when decoded)
                    Ok(format!(
                        "{}deadbeefcafebabe1234567890abcdef",
                        self.return_value
                    ))
                }
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        // Test empty transaction hex
        let mock_signer = MockSigner {
            should_fail: false,
            return_value: "".to_string(),
        };
        let result = rpc.sign_transaction("", &mock_signer).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        // Test odd length hex
        let result = rpc.sign_transaction("abc", &mock_signer).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("even length"));

        // Test invalid hex characters
        let result = rpc.sign_transaction("abcg", &mock_signer).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid hex characters"));

        // Test signer failure
        let mock_signer = MockSigner {
            should_fail: true,
            return_value: "".to_string(),
        };
        let result = rpc.sign_transaction("abcd", &mock_signer).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mock signing failure"));

        // Test successful signing
        let mock_signer = MockSigner {
            should_fail: false,
            return_value: "abcd".to_string(),
        };
        let result = rpc.sign_transaction("abcd", &mock_signer).await;
        if result.is_err() {
            println!("Error: {}", result.as_ref().unwrap_err());
        }
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "abcddeadbeefcafebabe1234567890abcdef");
    }

    #[tokio::test]
    async fn test_sign_transaction_validation_edge_cases() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Mock signer that returns invalid responses
        struct BadMockSigner {
            return_empty: bool,
            return_odd_length: bool,
            return_invalid_hex: bool,
            return_shorter: bool,
        }

        #[async_trait::async_trait]
        impl crate::signer::Signer for BadMockSigner {
            async fn sign_transaction(
                &self,
                unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                if self.return_empty {
                    Ok("".to_string())
                } else if self.return_odd_length {
                    Ok("abc".to_string())
                } else if self.return_invalid_hex {
                    Ok("abcg".to_string())
                } else if self.return_shorter {
                    Ok("ab".to_string()) // Shorter than input "abcd"
                } else {
                    Ok(format!("{}deadbeef", unsigned_tx))
                }
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        // Test signer returning empty string
        let bad_signer = BadMockSigner {
            return_empty: true,
            return_odd_length: false,
            return_invalid_hex: false,
            return_shorter: false,
        };
        let result = rpc.sign_transaction("abcd", &bad_signer).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));

        // Test signer returning odd length hex
        let bad_signer = BadMockSigner {
            return_empty: false,
            return_odd_length: true,
            return_invalid_hex: false,
            return_shorter: false,
        };
        let result = rpc.sign_transaction("abcd", &bad_signer).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("even length"));

        // Test signer returning invalid hex
        let bad_signer = BadMockSigner {
            return_empty: false,
            return_odd_length: false,
            return_invalid_hex: true,
            return_shorter: false,
        };
        let result = rpc.sign_transaction("abcd", &bad_signer).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid hex characters"));

        // Test signer returning shorter transaction (invalid)
        let bad_signer = BadMockSigner {
            return_empty: false,
            return_odd_length: false,
            return_invalid_hex: false,
            return_shorter: true,
        };
        let result = rpc.sign_transaction("abcd", &bad_signer).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("shorter than unsigned transaction"));
    }

    #[tokio::test]
    async fn test_sign_transaction_minimum_size_validation() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Mock signer that returns very small transactions
        struct TinyMockSigner;

        #[async_trait::async_trait]
        impl crate::signer::Signer for TinyMockSigner {
            async fn sign_transaction(
                &self,
                _unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                Ok("abcd".to_string()) // Only 2 bytes when decoded
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let tiny_signer = TinyMockSigner;
        let result = rpc.sign_transaction("abcd", &tiny_signer).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("minimum size"));
        assert!(error_msg.contains("minimum is 10 bytes"));
    }

    #[tokio::test]
    async fn test_sign_transaction_success_case() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Mock signer that returns a valid signed transaction
        struct GoodMockSigner;

        #[async_trait::async_trait]
        impl crate::signer::Signer for GoodMockSigner {
            async fn sign_transaction(
                &self,
                unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                // Return a longer valid hex string (20+ bytes when decoded)
                Ok(format!("{}deadbeefcafebabe1234567890abcdef", unsigned_tx))
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let good_signer = GoodMockSigner;

        // Test with a reasonable sized unsigned transaction
        let unsigned_tx = "0200000000010123456789abcdef"; // 14 bytes when decoded
        let result = rpc.sign_transaction(unsigned_tx, &good_signer).await;

        assert!(result.is_ok());
        let signed_tx = result.unwrap();
        assert!(signed_tx.starts_with(unsigned_tx));
        assert!(signed_tx.len() > unsigned_tx.len());
        assert!(signed_tx.contains("deadbeefcafebabe"));
    }

    #[tokio::test]
    async fn test_sign_and_broadcast_transaction_mock() {
        // Create a mock server for testing the broadcast part
        let server = MockServer::start();

        // Mock the RPC response for sendrawtransaction
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body(serde_json::json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "sendrawtransaction",
                "params": ["0200000000010123456789abcdefdeadbeefcafebabe1234567890abcdef"]
            }));
            then.status(200).json_body(serde_json::json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "abc123def456789",
                "error": null
            }));
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

        // Mock signer for testing
        struct TestMockSigner;

        #[async_trait::async_trait]
        impl crate::signer::Signer for TestMockSigner {
            async fn sign_transaction(
                &self,
                unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                Ok(format!("{}deadbeefcafebabe1234567890abcdef", unsigned_tx))
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let signer = TestMockSigner;
        let unsigned_tx = "0200000000010123456789abcdef";

        let result = rpc
            .sign_and_broadcast_transaction(unsigned_tx, &signer)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "abc123def456789");

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_sign_and_broadcast_transaction_signing_failure() {
        let rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Mock signer that fails
        struct FailingSigner;

        #[async_trait::async_trait]
        impl crate::signer::Signer for FailingSigner {
            async fn sign_transaction(
                &self,
                _unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                Err(crate::signer::SignerError::Lwk(
                    "Signing failed".to_string(),
                ))
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let failing_signer = FailingSigner;
        let result = rpc
            .sign_and_broadcast_transaction("abcd", &failing_signer)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // The error should be a Signer error containing the original failure message
        assert!(error_msg.contains("Signer error"));
        assert!(error_msg.contains("Signing failed"));
    }

    #[tokio::test]
    async fn test_sign_and_broadcast_transaction_broadcast_failure() {
        // Create a mock server that returns an error for broadcast
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).json_body(serde_json::json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": null,
                "error": {
                    "code": -26,
                    "message": "Transaction rejected"
                }
            }));
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

        // Mock signer that succeeds
        struct WorkingSigner;

        #[async_trait::async_trait]
        impl crate::signer::Signer for WorkingSigner {
            async fn sign_transaction(
                &self,
                unsigned_tx: &str,
            ) -> Result<String, crate::signer::SignerError> {
                Ok(format!("{}deadbeefcafebabe1234567890abcdef", unsigned_tx))
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let working_signer = WorkingSigner;
        let unsigned_tx = "0200000000010123456789abcdef";

        let result = rpc
            .sign_and_broadcast_transaction(unsigned_tx, &working_signer)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Failed during transaction broadcast phase"));
        assert!(error_msg.contains("Transaction rejected"));

        mock.assert();
    }

    #[tokio::test]
    async fn test_wait_for_confirmations_success() {
        let server = MockServer::start();

        let wallet_name = "test_wallet";
        let txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

        // Mock load_wallet first
        let load_wallet_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "loadwallet",
                    "params": [wallet_name]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "result": {
                        "name": wallet_name,
                        "warning": ""
                    }
                }));
        });

        // Second call returns 2 confirmations (sufficient)
        let mock_response_2 = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": txid,
                "confirmations": 2,
                "blockheight": 12345,
                "hex": "0200000000010abc123def456789...",
                "blockhash": "def456abc123789def456abc123789def456abc123789def456abc123789def456ab",
                "blocktime": 1640995200,
                "time": 1640995200,
                "timereceived": 1640995180
            }
        });

        // Create a mock that returns 2 confirmations immediately (simpler test)
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("//wallet/test_wallet")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "gettransaction",
                    "params": [txid, true]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response_2); // Return sufficient confirmations immediately
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

        // Use fast polling (1 second) for testing
        let result = rpc
            .wait_for_confirmations_with_interval(wallet_name, txid, Some(2), Some(1), Some(1))
            .await;

        assert!(result.is_ok());
        let tx_detail = result.unwrap();
        assert_eq!(tx_detail.confirmations, 2);
        assert_eq!(tx_detail.txid, txid);

        // Mocks should have been called
        load_wallet_mock.assert();
        mock.assert();
    }

    #[tokio::test]
    async fn test_wait_for_confirmations_timeout() {
        let server = MockServer::start();

        let wallet_name = "test_wallet";
        let txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

        // Mock load_wallet first
        let _load_wallet_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "loadwallet",
                    "params": [wallet_name]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "result": {
                        "name": wallet_name,
                        "warning": ""
                    }
                }));
        });

        // Always return insufficient confirmations
        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": txid,
                "confirmations": 1,
                "blockheight": 12345,
                "hex": "0200000000010abc123def456789...",
                "blockhash": null,
                "blocktime": null,
                "time": null,
                "timereceived": null
            }
        });

        let _mock = server.mock(|when, then| {
            when.method(POST)
                .path("//wallet/test_wallet")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "gettransaction",
                    "params": [txid, true]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

        // Use a very short timeout for testing (0 = 3 seconds) and fast polling (1 second)
        let result = rpc
            .wait_for_confirmations_with_interval(wallet_name, txid, Some(2), Some(0), Some(1))
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AmpError::Timeout(msg) => {
                assert!(msg.contains("Timeout waiting for confirmations"));
                assert!(msg.contains(txid));
                assert!(msg.contains("retry confirmation"));
            }
            _ => panic!("Expected timeout error"),
        }

        // Mock will be called multiple times during the timeout period
        // We don't assert on the exact number since it depends on timing
    }

    #[tokio::test]
    async fn test_wait_for_confirmations_immediate_success() {
        let server = MockServer::start();

        let wallet_name = "test_wallet";
        let txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

        // Mock load_wallet first
        let _load_wallet_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "loadwallet",
                    "params": [wallet_name]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "result": {
                        "name": wallet_name,
                        "warning": ""
                    }
                }));
        });

        // Transaction already has sufficient confirmations
        let mock_response = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": txid,
                "confirmations": 5,
                "blockheight": 12345,
                "hex": "0200000000010abc123def456789...",
                "blockhash": "def456abc123789def456abc123789def456abc123789def456abc123789def456ab",
                "blocktime": 1640995200,
                "time": 1640995200,
                "timereceived": 1640995180
            }
        });

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("//wallet/test_wallet")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .json_body(serde_json::json!({
                    "jsonrpc": "1.0",
                    "id": "amp-client",
                    "method": "gettransaction",
                    "params": [txid, true]
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(mock_response);
        });

        let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

        let result = rpc.wait_for_confirmations(wallet_name, txid, Some(2), Some(10)).await;

        assert!(result.is_ok());
        let tx_detail = result.unwrap();
        assert_eq!(tx_detail.confirmations, 5);
        assert_eq!(tx_detail.txid, txid);

        // Should only need one call since confirmations are already sufficient
        mock.assert();
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
            max_delay_ms: 30_000,
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
            Err(_) => 30_000,
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

/// Singleton instance of the `TokenManager` for shared token storage across all `ApiClient` instances
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
    /// Gets the global singleton instance of `TokenManager`
    ///
    /// This ensures all `ApiClient` instances share the same token storage,
    /// preventing multiple token acquisition attempts in concurrent tests.
    ///
    /// # Errors
    /// Returns an error if the `TokenManager` cannot be initialized
    pub async fn get_global_instance() -> Result<Arc<Self>, Error> {
        let manager = GLOBAL_TOKEN_MANAGER
            .get_or_try_init(|| async {
                let config = RetryConfig::from_env()?;
                let base_url = get_amp_api_base_url()?;
                let manager = Self::with_config_and_base_url(config, base_url).await?;
                Ok::<Arc<Self>, Error>(Arc::new(manager))
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
    pub fn with_mock_token(
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

        let had_token = self.clear_token_from_memory().await;
        self.clear_token_from_disk_if_enabled().await;
        Self::log_token_clear_result(had_token);

        Ok(())
    }

    /// Clears the token from memory and returns whether a token was present
    async fn clear_token_from_memory(&self) -> bool {
        let mut token_guard = self.token_data.lock().await;
        let had_token = token_guard.is_some();
        *token_guard = None;
        drop(token_guard);
        had_token
    }

    /// Clears the token from disk if persistence is enabled
    async fn clear_token_from_disk_if_enabled(&self) {
        if Self::should_persist_tokens() {
            if let Err(e) = self.remove_token_from_disk().await {
                tracing::warn!("Failed to remove token from disk: {e}");
            }
        }
    }

    /// Logs the result of the token clearing operation
    fn log_token_clear_result(had_token: bool) {
        if had_token {
            tracing::info!("Token successfully cleared from memory and disk - next get_token() will obtain fresh token");
        } else {
            tracing::debug!("No token was stored to clear");
        }
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
        let token_file = "token.json";

        if !self.token_file_exists(token_file).await {
            return Ok(None);
        }

        let content = self.read_token_file(token_file).await?;
        self.parse_and_validate_token(token_file, &content).await
    }

    /// Checks if the token file exists on disk
    async fn token_file_exists(&self, token_file: &str) -> bool {
        tokio::fs::try_exists(token_file).await.map_or_else(
            |_| {
                tracing::debug!("Error checking token file existence: {}", token_file);
                false
            },
            |exists| {
                if !exists {
                    tracing::debug!("Token file does not exist: {}", token_file);
                }
                exists
            },
        )
    }

    /// Reads the token file content from disk
    async fn read_token_file(&self, token_file: &str) -> Result<String, Error> {
        use tokio::fs;

        match fs::read_to_string(token_file).await {
            Ok(content) => Ok(content),
            Err(e) => {
                tracing::warn!("Failed to read token file: {e}");
                Err(Error::Token(TokenError::storage(format!(
                    "Failed to read token file: {e}"
                ))))
            }
        }
    }

    /// Parses token content and validates expiration
    async fn parse_and_validate_token(
        &self,
        token_file: &str,
        content: &str,
    ) -> Result<Option<TokenData>, Error> {
        match serde_json::from_str::<TokenData>(content) {
            Ok(token_data) => self.handle_parsed_token(token_file, token_data).await,
            Err(e) => self.handle_parse_error(token_file, e).await,
        }
    }

    /// Handles successfully parsed token data, checking expiration
    async fn handle_parsed_token(
        &self,
        token_file: &str,
        token_data: TokenData,
    ) -> Result<Option<TokenData>, Error> {
        if token_data.is_expired() {
            tracing::info!("Token loaded from disk is expired, removing file");
            let _ = tokio::fs::remove_file(token_file).await;
            Ok(None)
        } else {
            tracing::info!("Valid token loaded from disk");
            Ok(Some(token_data))
        }
    }

    /// Handles token parsing errors by cleaning up the invalid file
    async fn handle_parse_error(
        &self,
        token_file: &str,
        e: serde_json::Error,
    ) -> Result<Option<TokenData>, Error> {
        tracing::warn!("Failed to parse token file, removing: {e}");
        let _ = tokio::fs::remove_file(token_file).await;
        Err(Error::Token(TokenError::serialization(format!(
            "Failed to parse token file: {e}"
        ))))
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
    ///
    /// # Errors
    /// Returns an error if:
    /// - File system permissions prevent deletion of the token file
    /// - I/O errors occur during file deletion operations
    /// - The token file is locked by another process
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

    /// Resets the global `TokenManager` singleton (useful for testing)
    ///
    /// This method clears the global singleton instance, forcing the next
    /// call to `get_global_instance()` to create a fresh `TokenManager`.
    /// Primarily intended for test scenarios where a clean state is needed.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Token clearing operations fail during the reset process
    /// - File system errors occur when clearing persistent token data
    /// - The global instance is in an invalid state that prevents cleanup
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

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: Url,
    token_strategy: Arc<Box<dyn TokenStrategy>>,
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a new client - automatically detects environment
    /// let client = ApiClient::new().await?;
    ///
    /// // Client is ready to use
    /// let assets = client.get_assets().await?;
    /// println!("Found {} assets", assets.len());
    /// # Ok(())
    /// # }
    /// ```
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
            token_strategy: Arc::new(token_strategy),
        })
    }

    /// Creates a new API client with the specified base URL.
    ///
    /// Automatically selects the appropriate token strategy based on environment detection.
    ///
    /// # Errors
    ///
    /// Returns an error if token strategy initialization fails.
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # use reqwest::Url;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let base_url = Url::parse("https://amp-test.blockstream.com/api")?;
    /// let client = ApiClient::with_base_url(base_url).await?;
    ///
    /// // Client is ready to use with the specified URL
    /// let assets = client.get_assets().await?;
    /// # Ok(())
    /// # }
    /// ```
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
            token_strategy: Arc::new(token_strategy),
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
            token_strategy: Arc::new(token_strategy),
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
            token_strategy: Arc::new(token_strategy),
        })
    }

    /// Creates a new API client for testing with a mock token strategy that always returns a fixed token.
    /// This bypasses all token acquisition and management logic and uses complete isolation.
    ///
    /// # Errors
    ///
    /// This method is infallible but returns Result for API consistency.
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # use reqwest::Url;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let base_url = Url::parse("http://localhost:8080/api")?;
    /// let client = ApiClient::with_mock_token(base_url, "test_token".to_string())?;
    ///
    /// // Client will always use "test_token" for authentication
    /// let token = client.get_token().await?;
    /// assert_eq!(token, "test_token");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_mock_token(base_url: Url, mock_token: String) -> Result<Self, Error> {
        let client = Client::new();
        let token_strategy: Box<dyn TokenStrategy> = Box::new(MockTokenStrategy::new(mock_token));

        tracing::info!(
            "Created ApiClient with explicit mock token strategy for base URL: {}",
            base_url
        );

        Ok(Self {
            client,
            base_url,
            token_strategy: Arc::new(token_strategy),
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// if let Some(token_info) = client.get_token_info().await? {
    ///     println!("Token expires at: {}", token_info.expires_at);
    ///     println!("Token is expired: {}", token_info.is_expired);
    /// } else {
    ///     println!("No token stored or mock strategy in use");
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// // Clear any existing token
    /// client.clear_token().await?;
    ///
    /// // Next get_token() call will obtain a fresh token
    /// let token = client.get_token().await?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// Resets the global `TokenManager` singleton (useful for testing).
    ///
    /// This method clears the token from the global `TokenManager` instance.
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// // Get a valid token - automatically handles refresh if needed
    /// let token = client.get_token().await?;
    /// println!("Got token: {}", &token[..10]); // Print first 10 chars
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_token(&self) -> Result<String, Error> {
        self.token_strategy.get_token().await
    }

    /// Returns the type of token strategy currently in use
    ///
    /// This is useful for debugging and testing to verify the correct strategy is selected.
    ///
    /// # Returns
    /// A string indicating the strategy type: "mock" or "live"
    #[must_use]
    pub fn get_strategy_type(&self) -> &'static str {
        self.token_strategy.strategy_type()
    }

    /// Returns whether the current strategy persists tokens
    ///
    /// This is useful for understanding the token management behavior.
    ///
    /// # Returns
    /// `true` if tokens are persisted to disk, `false` for in-memory only
    #[must_use]
    pub fn should_persist_tokens(&self) -> bool {
        self.token_strategy.should_persist()
    }

    /// Force cleanup of token files (for test cleanup)
    ///
    /// This is a static method that can be used to cleanup token files
    /// without needing an `ApiClient` instance. Useful for test teardown.
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
        let debug_logging = std::env::var("AMP_DEBUG").is_ok();

        if debug_logging {
            eprintln!("🌐 HTTP Request: {} /{}", method, path.join("/"));
        }

        let token = self.get_token().await?;
        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().extend(path);

        if debug_logging {
            eprintln!("🔗 Full URL: {url}");
        }

        // Retry logic for network issues
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            if debug_logging && attempt > 1 {
                eprintln!("🔄 Retry attempt {attempt} of {max_retries}");
            }

            let mut request_builder = self
                .client
                .request(method.clone(), url.clone())
                .header(AUTHORIZATION, format!("token {token}"))
                .timeout(std::time::Duration::from_secs(60)); // Increase timeout to 60 seconds

            if let Some(ref body) = body {
                if debug_logging && attempt == 1 {
                    if let Ok(json_body) = serde_json::to_string_pretty(&body) {
                        eprintln!(
                            "📤 Request body ({} bytes):\n{}",
                            json_body.len(),
                            json_body
                        );
                    } else {
                        eprintln!("📤 Request body: [serialization failed]");
                    }
                }
                request_builder = request_builder.json(&body);
            } else if debug_logging && attempt == 1 {
                eprintln!("📤 Request body: [empty]");
            }

            if debug_logging {
                eprintln!("🚀 Sending HTTP request (attempt {attempt})...");
            }

            match request_builder.send().await {
                Ok(response) => {
                    let status = response.status();

                    if debug_logging {
                        eprintln!("📥 Response status: {status}");
                    }

                    if !status.is_success() {
                        let error_text = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());

                        if debug_logging {
                            eprintln!("❌ Error response body: {error_text}");
                        }

                        return Err(Error::RequestFailed(format!(
                            "Request to {path:?} failed with status {status}: {error_text}"
                        )));
                    }

                    if debug_logging {
                        eprintln!("✅ HTTP request successful");
                    }

                    return Ok(response);
                }
                Err(e) => {
                    if debug_logging {
                        eprintln!("❌ HTTP request failed (attempt {attempt}): {e:?}");
                        eprintln!("   Error kind: {:?}", e.is_timeout());
                        eprintln!("   Is connect error: {}", e.is_connect());
                        eprintln!("   Is request error: {}", e.is_request());
                    }

                    last_error = Some(e);

                    // Only retry on network/connection errors, not on client errors
                    if attempt < max_retries {
                        #[allow(clippy::cast_sign_loss)] // attempt is always positive (1-3)
                        let delay = std::time::Duration::from_millis((attempt as u64) * 1000);
                        if debug_logging {
                            eprintln!("⏳ Waiting {}ms before retry...", delay.as_millis());
                        }
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // If we get here, all retries failed
        if debug_logging {
            eprintln!("❌ All {max_retries} retry attempts failed");
        }

        Err(Error::Reqwest(last_error.unwrap()))
    }

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<T, Error> {
        // Capture request context for better error messages
        let method_str = method.to_string();
        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().extend(path);
        let endpoint = url.to_string();
        let expected_type = std::any::type_name::<T>().to_string();

        let response = self.request_raw(method, path, body).await?;

        // Try to deserialize, capturing raw response on failure
        match response.text().await {
            Ok(raw_response) => serde_json::from_str(&raw_response).map_err(|e| {
                Error::ResponseDeserializationFailed {
                    method: method_str,
                    endpoint,
                    expected_type,
                    serde_error: e.to_string(),
                    raw_response,
                }
            }),
            Err(e) => Err(Error::ResponseParsingFailed(format!(
                "Failed to read response body: {e}"
            ))),
        }
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let assets = client.get_assets().await?;
    /// for asset in assets {
    ///     println!("Asset: {} ({})", asset.name, asset.ticker.unwrap_or_default());
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let asset = client.get_asset(asset_uuid).await?;
    ///
    /// println!("Asset: {} ({})", asset.name, asset.ticker.unwrap_or_default());
    /// println!("Registered: {}, Locked: {}", asset.is_registered, asset.is_locked);
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::IssuanceRequest};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let issuance_request = IssuanceRequest {
    ///     name: "My Token".to_string(),
    ///     amount: 1000000,
    ///     destination_address: "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26".to_string(),
    ///     domain: "example.com".to_string(),
    ///     ticker: "MYTKN".to_string(),
    ///     pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
    ///     precision: Some(8),
    ///     is_confidential: Some(true),
    ///     is_reissuable: Some(false),
    ///     reissuance_amount: None,
    ///     reissuance_address: None,
    ///     transfer_restricted: Some(false),
    /// };
    ///
    /// let response = client.issue_asset(&issuance_request).await?;
    /// println!("Issued asset with UUID: {}", response.asset_uuid);
    /// # Ok(())
    /// # }
    /// ```
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

    /// Registers an asset with the Blockstream Asset Registry.
    ///
    /// This method publishes an asset to the public registry, making it discoverable
    /// and verifiable by other users and applications. The asset must already exist
    /// in the AMP system before it can be registered.
    ///
    /// # Arguments
    ///
    /// * `asset_uuid` - The unique identifier of the asset to register
    ///
    /// # Returns
    ///
    /// Returns a `RegisterAssetResponse` containing:
    /// - `success`: Boolean indicating whether the registration was successful
    /// - `message`: Optional status message from the API
    /// - `asset_id`: The registered asset identifier (hex string)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The asset does not exist or cannot be found (404)
    /// - Authentication fails or token is invalid (401)
    /// - The asset is already registered (returns success with appropriate message)
    /// - Network connectivity issues occur
    /// - The server returns an error status (5xx)
    /// - The response cannot be parsed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    ///
    /// let response = client.register_asset(asset_uuid).await?;
    /// if response.success {
    ///     println!("Asset registered successfully!");
    ///     if let Some(asset) = response.asset_data {
    ///         println!("Asset ID: {}", asset.asset_id);
    ///     }
    ///     if let Some(message) = response.message {
    ///         println!("Message: {}", message);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error> {
        // Make HTTP request directly to handle both success and error responses
        let token = self.get_token().await?;
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .extend(&["assets", asset_uuid, "register"]);

        let response = self
            .client
            .request(Method::GET, url)
            .header(AUTHORIZATION, format!("token {token}"))
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| Error::RequestFailed(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            Error::ResponseParsingFailed(format!("Failed to read response body: {e}"))
        })?;

        // Handle HTTP 200 - success case
        if status == reqwest::StatusCode::OK {
            // Try to parse as Asset (full registration response)
            if let Ok(asset) = serde_json::from_str::<Asset>(&response_text) {
                return Ok(RegisterAssetResponse {
                    success: true,
                    message: Some("Asset registered successfully".to_string()),
                    asset_data: Some(asset),
                });
            }

            // If parsing as Asset fails, return success with raw message
            return Ok(RegisterAssetResponse {
                success: true,
                message: Some(response_text),
                asset_data: None,
            });
        }

        // Handle error responses
        // Try to parse error response as JSON
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            // Check for "already registered" error
            if let Some(error_msg) = error_json.get("Error").and_then(|e| e.as_str()) {
                let error_msg_lower = error_msg.to_lowercase();
                if error_msg_lower.contains("already registered") {
                    return Ok(RegisterAssetResponse {
                        success: true,
                        message: Some("Asset is already registered".to_string()),
                        asset_data: None,
                    });
                }

                // Other errors - return as error
                return Err(Error::RequestFailed(format!(
                    "Request to [\"assets\", \"{asset_uuid}\", \"register\"] failed with status {status}: {error_msg}"
                )));
            }
        }

        // Fallback error for non-JSON or unexpected responses
        Err(Error::RequestFailed(format!(
            "Request to [\"assets\", \"{asset_uuid}\", \"register\"] failed with status {status}: {response_text}"
        )))
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset does not exist or cannot be found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    pub async fn delete_asset(&self, asset_uuid: &str) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["assets", asset_uuid, "delete"],
            None::<&()>,
        )
        .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The transaction ID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn get_broadcast_status(&self, txid: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::GET, &["tx", "broadcast", txid], None::<&()>)
            .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The transaction hex is invalid or malformed
    /// - The transaction is rejected by the network
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn broadcast_transaction(&self, tx_hex: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::POST, &["tx", "broadcast"], Some(tx_hex))
            .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The user lacks authorization to register the asset
    /// - The asset is already registered
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn register_asset_authorized(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "register-authorized"],
            None::<&()>,
        )
        .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The asset is already locked
    /// - The user lacks permission to lock the asset
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn lock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::PUT, &["assets", asset_uuid, "lock"], None::<&()>)
            .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The asset is not currently locked
    /// - The user lacks permission to unlock the asset
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn unlock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::PUT, &["assets", asset_uuid, "unlock"], None::<&()>)
            .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The activity parameters are invalid
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
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

    /// Gets all transactions for a specific asset.
    ///
    /// This method retrieves a list of all transactions associated with the specified asset,
    /// including transfers, issuances, reissuances, and burns. The results can be filtered
    /// and paginated using the provided query parameters.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to retrieve transactions for
    /// * `params` - Optional query parameters for filtering and pagination
    ///
    /// # Returns
    /// A vector of `AssetTransaction` objects containing transaction details including:
    /// - Transaction ID and type
    /// - Amount and confirmation status
    /// - Block height and datetime
    /// - Associated user and address information
    ///
    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The query parameters are invalid
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # use amp_rs::model::AssetTransactionParams;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// // Get all transactions for an asset
    /// let params = AssetTransactionParams::default();
    /// let txs = client.get_asset_transactions("asset-uuid-123", &params).await?;
    ///
    /// for tx in txs {
    ///     println!("Transaction: {} - Type: {} - Outputs: {}",
    ///              tx.txid, tx.transaction_type(), tx.outputs.len());
    /// }
    ///
    /// // Get transactions with filtering by block height
    /// let params = AssetTransactionParams {
    ///     count: Some(10),
    ///     sortorder: Some("desc".to_string()),
    ///     height_start: Some(1000),
    ///     ..Default::default()
    /// };
    /// let recent_txs = client.get_asset_transactions("asset-uuid-123", &params).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # See Also
    /// - [`get_asset_transaction`](Self::get_asset_transaction) - Get a specific transaction by txid
    /// - [`get_asset_activities`](Self::get_asset_activities) - Get asset activities
    pub async fn get_asset_transactions(
        &self,
        asset_uuid: &str,
        params: &AssetTransactionParams,
    ) -> Result<Vec<AssetTransaction>, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "txs"], Some(params))
            .await
    }

    /// Gets a specific transaction for an asset by transaction ID.
    ///
    /// This method retrieves detailed information about a specific transaction
    /// associated with an asset, identified by its transaction ID (txid).
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `txid` - The transaction ID to retrieve
    ///
    /// # Returns
    /// An `AssetTransaction` object containing detailed transaction information including:
    /// - Transaction type and amount
    /// - Confirmation status and block height
    /// - Associated addresses and user information
    /// - Blinding factors for confidential transactions
    ///
    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The transaction ID is invalid or not found
    /// - The transaction is not associated with the specified asset
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let txid = "abc123def456789012345678901234567890123456789012345678901234abcd";
    ///
    /// let tx = client.get_asset_transaction(asset_uuid, txid).await?;
    ///
    /// println!("Transaction: {}", tx.txid);
    /// println!("Type: {}", tx.transaction_type());
    /// println!("Block height: {}", tx.blockheight);
    /// println!("Outputs: {}", tx.outputs.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # See Also
    /// - [`get_asset_transactions`](Self::get_asset_transactions) - List all transactions for an asset
    /// - [`get_asset_activities`](Self::get_asset_activities) - Get asset activities
    pub async fn get_asset_transaction(
        &self,
        asset_uuid: &str,
        txid: &str,
    ) -> Result<AssetTransaction, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "txs", txid],
            None::<&()>,
        )
        .await
    }

    /// Gets the lost outputs for a specific asset.
    ///
    /// Lost outputs are outputs that the AMP API is unable to track, typically due to
    /// missing blinder information or other tracking issues. This endpoint returns both
    /// regular lost outputs and reissuance token lost outputs.
    ///
    /// # Arguments
    ///
    /// * `asset_uuid` - The UUID of the asset to query
    ///
    /// # Returns
    ///
    /// Returns an `AssetLostOutputs` struct containing:
    /// - `lost_outputs`: Regular outputs that cannot be tracked
    /// - `reissuance_lost_outputs`: Reissuance token outputs that cannot be tracked
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::ApiClient;
    /// # use reqwest::Url;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let base_url = Url::parse("https://amp.blockstream.com/api")?;
    /// let client = ApiClient::with_mock_token(base_url, "test_token".to_string())?;
    /// let lost_outputs = client.get_asset_lost_outputs("bc2d31af-60d0-4346-bfba-11b045f92dff").await?;
    /// println!("Lost outputs: {}", lost_outputs.lost_outputs.len());
    /// println!("Reissuance lost outputs: {}", lost_outputs.reissuance_lost_outputs.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_lost_outputs(
        &self,
        asset_uuid: &str,
    ) -> Result<AssetLostOutputs, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "lost-outputs"],
            None::<&()>,
        )
        .await
    }

    /// Updates blinder keys for a specific asset output.
    ///
    /// This endpoint is used to provide missing blinder information for outputs,
    /// typically for issuance outputs where the blinders were initially set to zeros.
    /// The blinders are cryptographic secrets used in Confidential Transactions to
    /// hide asset amounts and types.
    ///
    /// # Arguments
    ///
    /// * `asset_uuid` - The UUID of the asset
    /// * `request` - The blinder update request containing:
    ///   - `txid`: Transaction ID of the output
    ///   - `vout`: Output index
    ///   - `asset_blinder`: Asset blinding factor (32-byte hex string)
    ///   - `amount_blinder`: Amount blinding factor (32-byte hex string)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The transaction or output does not exist
    /// - The blinder values are invalid
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::{ApiClient, UpdateBlindersRequest};
    /// # use reqwest::Url;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let base_url = Url::parse("https://amp.blockstream.com/api")?;
    /// let client = ApiClient::with_mock_token(base_url, "test_token".to_string())?;
    ///
    /// let request = UpdateBlindersRequest {
    ///     txid: "abcd1234...".to_string(),
    ///     vout: 0,
    ///     asset_blinder: "00112233...".to_string(),
    ///     amount_blinder: "44556677...".to_string(),
    /// };
    ///
    /// client.update_asset_blinders("bc2d31af-60d0-4346-bfba-11b045f92dff", &request).await?;
    /// println!("Blinders updated successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_asset_blinders(
        &self,
        asset_uuid: &str,
        request: &UpdateBlindersRequest,
    ) -> Result<(), Error> {
        self.request_empty(
            Method::POST,
            &["assets", asset_uuid, "update-blinders"],
            Some(request),
        )
        .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - The specified height is invalid or out of range
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
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

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn get_asset_balance(&self, asset_uuid: &str) -> Result<Balance, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "balance"], None::<&()>)
            .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "summary"], None::<&()>)
            .await
    }

    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn get_asset_utxos(&self, asset_uuid: &str) -> Result<Vec<Utxo>, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "utxos"], None::<&()>)
            .await
    }

    /// Gets the reissuances for a specific asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to retrieve reissuances for
    ///
    /// # Returns
    /// A vector of `Reissuance` objects containing information about all reissuances
    /// performed for the specified asset, including:
    /// - Transaction ID and output index
    /// - Destination address
    /// - Reissuance amount
    /// - Block confirmation
    /// - Creation timestamp
    ///
    /// # Errors
    /// Returns an error if:
    /// - The asset UUID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Example
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    /// let reissuances = client.get_asset_reissuances("asset-uuid-123").await?;
    /// for reissuance in reissuances {
    ///     println!("Reissuance txid: {}, amount: {}", reissuance.txid, reissuance.reissuance_amount);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_reissuances(&self, asset_uuid: &str) -> Result<Vec<Reissuance>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "reissuances"],
            None::<&()>,
        )
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
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    /// client.set_asset_memo("asset-uuid-123", "This is a memo for the asset").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_asset_memo(&self, asset_uuid: &str, memo: &str) -> Result<(), Error> {
        let token = self.get_token().await?;
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .extend(&["assets", asset_uuid, "memo", "set"]);

        let response = self
            .client
            .request(Method::POST, url)
            .header(AUTHORIZATION, format!("token {token}"))
            .header("content-type", "application/json")
            .body(format!("\"{}\"", memo.replace('"', "\\\"")))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Request to [\"assets\", \"{asset_uuid}\", \"memo\", \"set\"] failed with status {status}: {error_text}"
            )));
        }

        Ok(())
    }

    /// Blacklists specific UTXOs for an asset to prevent them from being used in transactions.
    ///
    /// This method adds the specified UTXOs to the asset's blacklist, preventing them from being
    /// used in future transactions. This is typically used for security purposes when UTXOs are
    /// suspected to be compromised or need to be temporarily disabled.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to blacklist UTXOs for
    /// * `utxos` - A slice of `Outpoint` structs representing the UTXOs to blacklist
    ///
    /// # Returns
    /// Returns a vector of `Utxo` structs representing the blacklisted UTXOs with their updated status.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - One or more UTXOs are invalid or already blacklisted
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::Outpoint};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let utxos = vec![
    ///     Outpoint {
    ///         txid: "abc123...".to_string(),
    ///         vout: 0,
    ///     },
    ///     Outpoint {
    ///         txid: "def456...".to_string(),
    ///         vout: 1,
    ///     },
    /// ];
    ///
    /// let blacklisted_utxos = client.blacklist_asset_utxos(asset_uuid, &utxos).await?;
    /// println!("Blacklisted {} UTXOs", blacklisted_utxos.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`whitelist_asset_utxos`](Self::whitelist_asset_utxos) - Remove UTXOs from blacklist
    /// - [`get_asset`](Self::get_asset) - Get asset information including UTXO status
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

    /// Removes UTXOs from the asset's blacklist, allowing them to be used in transactions again.
    ///
    /// This method removes the specified UTXOs from the asset's blacklist, restoring their ability
    /// to be used in transactions. This is the reverse operation of blacklisting UTXOs.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to whitelist UTXOs for
    /// * `utxos` - A slice of `Outpoint` structs representing the UTXOs to remove from blacklist
    ///
    /// # Returns
    /// Returns a vector of `Utxo` structs representing the whitelisted UTXOs with their updated status.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - One or more UTXOs are invalid or not currently blacklisted
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::Outpoint};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let utxos = vec![
    ///     Outpoint {
    ///         txid: "abc123...".to_string(),
    ///         vout: 0,
    ///     },
    /// ];
    ///
    /// let whitelisted_utxos = client.whitelist_asset_utxos(asset_uuid, &utxos).await?;
    /// println!("Whitelisted {} UTXOs", whitelisted_utxos.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`blacklist_asset_utxos`](Self::blacklist_asset_utxos) - Add UTXOs to blacklist
    /// - [`get_asset`](Self::get_asset) - Get asset information including UTXO status
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

    /// Removes treasury addresses from a specific asset.
    ///
    /// This method removes the specified addresses from the asset's treasury address list.
    /// Treasury addresses are special addresses that can be used for asset management operations
    /// such as reissuance and burning.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to remove treasury addresses from
    /// * `addresses` - A slice of address strings to remove from the treasury addresses
    ///
    /// # Returns
    /// Returns `Ok(())` on successful removal.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - One or more addresses are invalid or not currently treasury addresses
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - Attempting to remove the last treasury address (if not allowed)
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let addresses = vec![
    ///     "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
    ///     "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string(),
    /// ];
    ///
    /// client.delete_asset_treasury_addresses(asset_uuid, &addresses).await?;
    /// println!("Removed {} treasury addresses", addresses.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`add_asset_treasury_addresses`](Self::add_asset_treasury_addresses) - Add treasury addresses
    /// - [`get_asset_treasury_addresses`](Self::get_asset_treasury_addresses) - Get current treasury addresses
    /// - [`reissue_asset`](Self::reissue_asset) - Reissue assets using treasury addresses
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

    /// Gets a list of all registered users.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let users = client.get_registered_users().await?;
    /// for user in users {
    ///     println!("User: {} (ID: {})", user.name, user.id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_registered_users(
        &self,
    ) -> Result<Vec<crate::model::RegisteredUserResponse>, Error> {
        self.request_json(Method::GET, &["registered_users"], None::<&()>)
            .await
    }

    /// Gets a specific registered user by ID.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the registered user to retrieve
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The user ID does not exist
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let user = client.get_registered_user(1).await?;
    /// println!("User: {} (ID: {})", user.name, user.id);
    /// # Ok(())
    /// # }
    /// ```
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

    /// Creates a new registered user in the AMP system.
    ///
    /// This method creates a new registered user with the provided information. Registered users
    /// can be associated with GAIDs, assigned to categories, and receive asset assignments.
    ///
    /// # Arguments
    /// * `new_user` - A `RegisteredUserAdd` struct containing the user information to create
    ///
    /// # Returns
    /// Returns a `RegisteredUserResponse` containing the created user's information including
    /// the assigned user ID.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The user data is invalid (e.g., missing required fields, invalid email format)
    /// - A user with the same identifier already exists
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::RegisteredUserAdd};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let new_user = RegisteredUserAdd {
    ///     name: "John Doe".to_string(),
    ///     gaid: Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string()),
    ///     is_company: false,
    /// };
    ///
    /// let created_user = client.add_registered_user(&new_user).await?;
    /// println!("Created user: {} with ID {}", created_user.name, created_user.id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_registered_users`](Self::get_registered_users) - List all registered users
    /// - [`edit_registered_user`](Self::edit_registered_user) - Update user information
    /// - [`delete_registered_user`](Self::delete_registered_user) - Remove a user
    pub async fn add_registered_user(
        &self,
        new_user: &crate::model::RegisteredUserAdd,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(Method::POST, &["registered_users", "add"], Some(new_user))
            .await
    }

    /// Removes a registered user from the AMP system.
    ///
    /// This method permanently deletes a registered user and all associated data. This operation
    /// cannot be undone. Any GAIDs associated with the user will be disassociated, and any
    /// pending assignments may be affected.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the registered user to delete
    ///
    /// # Returns
    /// Returns `Ok(())` on successful deletion.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The user ID is invalid or does not exist
    /// - The user has active assignments that prevent deletion
    /// - The HTTP request fails
    /// - The server returns an error status
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let user_id = 123;
    /// client.delete_registered_user(user_id).await?;
    /// println!("Successfully deleted user with ID {}", user_id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_registered_user`](Self::get_registered_user) - Get user information before deletion
    /// - [`add_registered_user`](Self::add_registered_user) - Create a new user
    /// - [`get_registered_user_summary`](Self::get_registered_user_summary) - Check user's assignments
    pub async fn delete_registered_user(&self, user_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["registered_users", &user_id.to_string(), "delete"],
            None::<&()>,
        )
        .await
    }

    /// Updates registered user information.
    ///
    /// This method allows you to modify the information of an existing registered user.
    /// Only the fields provided in the edit data will be updated; other fields remain unchanged.
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user to update
    /// * `edit_data` - A `RegisteredUserEdit` struct containing the fields to update
    ///
    /// # Returns
    /// Returns a `RegisteredUserResponse` containing the updated user information.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The user ID is invalid or does not exist
    /// - The edit data contains invalid values (e.g., invalid email format)
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::RegisteredUserEdit};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let user_id = 123;
    /// let edit_data = RegisteredUserEdit {
    ///     name: Some("Jane Doe".to_string()),
    /// };
    ///
    /// let updated_user = client.edit_registered_user(user_id, &edit_data).await?;
    /// println!("Updated user: {}", updated_user.name);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_registered_user`](Self::get_registered_user) - Get current user information
    /// - [`add_registered_user`](Self::add_registered_user) - Create a new user
    /// - [`delete_registered_user`](Self::delete_registered_user) - Remove a user
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

    /// Gets comprehensive summary information for a registered user including assets and distributions.
    ///
    /// This method retrieves detailed summary information about a registered user, including
    /// their basic information, associated assets, assignment history, and distribution records.
    /// This provides a complete overview of the user's activity and holdings in the system.
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user to get summary for
    ///
    /// # Returns
    /// Returns a `RegisteredUserSummary` containing:
    /// - Basic user information (name, email, etc.)
    /// - List of associated GAIDs
    /// - Asset assignments and their status
    /// - Distribution history
    /// - Balance information
    /// - Activity timestamps
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The user ID is invalid or does not exist
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let user_id = 123;
    /// let summary = client.get_registered_user_summary(user_id).await?;
    ///
    /// println!("Asset UUID: {}", summary.asset_uuid);
    /// println!("Asset ID: {}", summary.asset_id);
    /// println!("Asset assignments: {}", summary.assignments.len());
    /// println!("Distributions received: {}", summary.distributions.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_registered_user`](Self::get_registered_user) - Get basic user information
    /// - [`get_registered_user_gaids`](Self::get_registered_user_gaids) - Get only GAIDs
    /// - [`get_asset_assignments`](Self::get_asset_assignments) - Get assignments for specific asset
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

    /// Gets all GAIDs (Green Address IDs) associated with a registered user.
    ///
    /// This method retrieves a list of all GAIDs that are currently associated with the specified
    /// registered user. GAIDs are unique identifiers that can be used to receive assets and
    /// track ownership.
    ///
    /// # Arguments
    /// * `registered_user_id` - The ID of the registered user to get GAIDs for
    ///
    /// # Returns
    /// Returns a vector of GAID strings associated with the user.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The user ID is invalid or does not exist
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let user_id = 123;
    /// let gaids = client.get_registered_user_gaids(user_id).await?;
    ///
    /// println!("User {} has {} associated GAIDs:", user_id, gaids.len());
    /// for gaid in gaids {
    ///     println!("  - {}", gaid);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`add_gaid_to_registered_user`](Self::add_gaid_to_registered_user) - Associate a GAID with user
    /// - [`set_default_gaid_for_registered_user`](Self::set_default_gaid_for_registered_user) - Set default GAID
    /// - [`get_gaid_registered_user`](Self::get_gaid_registered_user) - Find user by GAID
    /// - [`validate_gaid`](Self::validate_gaid) - Validate GAID format
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
        // Send GAID as a plain string, not wrapped in an object
        self.request_empty(
            Method::POST,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "gaids",
                "add",
            ],
            Some(gaid),
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
        // Send GAID as a plain string, not wrapped in an object
        self.request_empty(
            Method::POST,
            &[
                "registered_users",
                &registered_user_id.to_string(),
                "gaids",
                "set-default",
            ],
            Some(gaid),
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
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    /// let balance = client.get_gaid_balance(gaid).await?;
    ///
    /// println!("GAID {} has {} balance entries", gaid, balance.len());
    /// for entry in balance {
    ///     println!("Asset {}: {} units", entry.asset_id, entry.balance);
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
            owner: Some(gaid.to_string()),
            amount: balance_entry.balance,
            gaid: Some(gaid.to_string()),
        })
    }

    /// Gets a list of all categories.
    ///
    /// # Returns
    /// Returns a vector of `CategoryResponse` objects
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let categories = client.get_categories().await?;
    /// for category in categories {
    ///     println!("Category: {} (ID: {})", category.name, category.id);
    ///     if let Some(desc) = category.description {
    ///         println!("  Description: {}", desc);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        self.request_json(Method::GET, &["categories"], None::<&()>)
            .await
    }

    /// Creates a new category for organizing users and assets.
    ///
    /// This method creates a new category that can be used to group registered users and assets
    /// for organizational purposes. Categories help manage permissions and provide logical
    /// groupings for assets and users.
    ///
    /// # Arguments
    /// * `new_category` - A `CategoryAdd` struct containing the category information to create
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the created category information including
    /// the assigned category ID.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category data is invalid (e.g., missing name, invalid characters)
    /// - A category with the same name already exists
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::CategoryAdd};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let new_category = CategoryAdd {
    ///     name: "Premium Users".to_string(),
    ///     description: Some("High-value users with special privileges".to_string()),
    /// };
    ///
    /// let created_category = client.add_category(&new_category).await?;
    /// println!("Created category: {} with ID {}", created_category.name, created_category.id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_categories`](Self::get_categories) - List all categories
    /// - [`edit_category`](Self::edit_category) - Update category information
    /// - [`delete_category`](Self::delete_category) - Remove a category
    /// - [`add_registered_user_to_category`](Self::add_registered_user_to_category) - Add users to category
    pub async fn add_category(
        &self,
        new_category: &CategoryAdd,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(Method::POST, &["categories", "add"], Some(new_category))
            .await
    }

    /// Gets a specific category by ID.
    ///
    /// This method retrieves detailed information about a specific category, including
    /// its name, description, and associated users and assets.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to retrieve
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the category information including:
    /// - Category ID, name, and description
    /// - List of associated registered users
    /// - List of associated assets
    /// - Creation and modification timestamps
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// let category = client.get_category(category_id).await?;
    ///
    /// println!("Category: {} (ID: {})", category.name, category.id);
    /// if let Some(desc) = category.description {
    ///     println!("Description: {}", desc);
    /// }
    /// println!("Users: {}, Assets: {}", category.registered_users.len(), category.assets.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_categories`](Self::get_categories) - List all categories
    /// - [`add_category`](Self::add_category) - Create a new category
    /// - [`edit_category`](Self::edit_category) - Update category information
    /// - [`delete_category`](Self::delete_category) - Remove a category
    pub async fn get_category(&self, category_id: i64) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::GET,
            &["categories", &category_id.to_string()],
            None::<&()>,
        )
        .await
    }

    /// Updates category information.
    ///
    /// This method allows you to modify the information of an existing category.
    /// Only the fields provided in the edit data will be updated; other fields remain unchanged.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to update
    /// * `edit_category` - A `CategoryEdit` struct containing the fields to update
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the updated category information.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The edit data contains invalid values (e.g., empty name, invalid characters)
    /// - A category with the new name already exists (if name is being changed)
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::CategoryEdit};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// let edit_data = CategoryEdit {
    ///     name: Some("VIP Users".to_string()),
    ///     description: Some("Very important users with premium access".to_string()),
    /// };
    ///
    /// let updated_category = client.edit_category(category_id, &edit_data).await?;
    /// println!("Updated category: {}", updated_category.name);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_category`](Self::get_category) - Get current category information
    /// - [`add_category`](Self::add_category) - Create a new category
    /// - [`delete_category`](Self::delete_category) - Remove a category
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

    /// Removes a category from the system.
    ///
    /// This method permanently deletes a category. All users and assets associated with the
    /// category will be disassociated, but the users and assets themselves are not deleted.
    /// This operation cannot be undone.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to delete
    ///
    /// # Returns
    /// Returns `Ok(())` on successful deletion.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The category is still in use and cannot be deleted (depending on system configuration)
    /// - The HTTP request fails
    /// - The server returns an error status
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// client.delete_category(category_id).await?;
    /// println!("Successfully deleted category with ID {}", category_id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_category`](Self::get_category) - Get category information before deletion
    /// - [`add_category`](Self::add_category) - Create a new category
    /// - [`remove_registered_user_from_category`](Self::remove_registered_user_from_category) - Remove users first
    /// - [`remove_asset_from_category`](Self::remove_asset_from_category) - Remove assets first
    pub async fn delete_category(&self, category_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["categories", &category_id.to_string(), "delete"],
            None::<&()>,
        )
        .await
    }

    /// Associates a registered user with a category.
    ///
    /// This method adds a registered user to a category, allowing for organized grouping
    /// of users. Users can belong to multiple categories, and categories can contain
    /// multiple users.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to add the user to
    /// * `user_id` - The ID of the registered user to add to the category
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the updated category information including
    /// the newly added user.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The user ID is invalid or does not exist
    /// - The user is already associated with the category
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// let user_id = 123;
    ///
    /// let updated_category = client.add_registered_user_to_category(category_id, user_id).await?;
    /// println!("Added user {} to category '{}'", user_id, updated_category.name);
    /// println!("Category now has {} users", updated_category.registered_users.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`remove_registered_user_from_category`](Self::remove_registered_user_from_category) - Remove user from category
    /// - [`get_category`](Self::get_category) - Get category information including users
    /// - [`get_registered_user`](Self::get_registered_user) - Get user information
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

    /// Removes a registered user from a category.
    ///
    /// This method disassociates a registered user from a category. The user remains in the
    /// system but is no longer part of the specified category. This does not affect the user's
    /// association with other categories.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to remove the user from
    /// * `user_id` - The ID of the registered user to remove from the category
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the updated category information without
    /// the removed user.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The user ID is invalid or does not exist
    /// - The user is not currently associated with the category
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// let user_id = 123;
    ///
    /// let updated_category = client.remove_registered_user_from_category(category_id, user_id).await?;
    /// println!("Removed user {} from category '{}'", user_id, updated_category.name);
    /// println!("Category now has {} users", updated_category.registered_users.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`add_registered_user_to_category`](Self::add_registered_user_to_category) - Add user to category
    /// - [`get_category`](Self::get_category) - Get category information including users
    /// - [`get_registered_user`](Self::get_registered_user) - Get user information
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

    /// Associates an asset with a category.
    ///
    /// This method adds an asset to a category, allowing for organized grouping of assets.
    /// Assets can belong to multiple categories, and categories can contain multiple assets.
    /// This helps with asset management and permission organization.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to add the asset to
    /// * `asset_uuid` - The UUID of the asset to add to the category
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the updated category information including
    /// the newly added asset.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The asset UUID is invalid or does not exist
    /// - The asset is already associated with the category
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    ///
    /// let updated_category = client.add_asset_to_category(category_id, asset_uuid).await?;
    /// println!("Added asset {} to category '{}'", asset_uuid, updated_category.name);
    /// println!("Category now has {} assets", updated_category.assets.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`remove_asset_from_category`](Self::remove_asset_from_category) - Remove asset from category
    /// - [`get_category`](Self::get_category) - Get category information including assets
    /// - [`get_asset`](Self::get_asset) - Get asset information
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

    /// Removes an asset from a category.
    ///
    /// This method disassociates an asset from a category. The asset remains in the system
    /// but is no longer part of the specified category. This does not affect the asset's
    /// association with other categories.
    ///
    /// # Arguments
    /// * `category_id` - The ID of the category to remove the asset from
    /// * `asset_uuid` - The UUID of the asset to remove from the category
    ///
    /// # Returns
    /// Returns a `CategoryResponse` containing the updated category information without
    /// the removed asset.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The category ID is invalid or does not exist
    /// - The asset UUID is invalid or does not exist
    /// - The asset is not currently associated with the category
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let category_id = 1;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    ///
    /// let updated_category = client.remove_asset_from_category(category_id, asset_uuid).await?;
    /// println!("Removed asset {} from category '{}'", asset_uuid, updated_category.name);
    /// println!("Category now has {} assets", updated_category.assets.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`add_asset_to_category`](Self::add_asset_to_category) - Add asset to category
    /// - [`get_category`](Self::get_category) - Get category information including assets
    /// - [`get_asset`](Self::get_asset) - Get asset information
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

    /// Validates a GAID (Green Address ID).
    ///
    /// # Arguments
    /// * `gaid` - The GAID string to validate
    ///
    /// # Returns
    /// Returns a `ValidateGaidResponse` indicating whether the GAID is valid
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    /// let validation = client.validate_gaid(gaid).await?;
    ///
    /// if validation.is_valid {
    ///     println!("GAID {} is valid", gaid);
    /// } else {
    ///     println!("GAID {} is invalid: {:?}", gaid, validation.error);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn validate_gaid(
        &self,
        gaid: &str,
    ) -> Result<crate::model::ValidateGaidResponse, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "validate"], None::<&()>)
            .await
    }

    /// Gets the address associated with a GAID.
    ///
    /// # Arguments
    /// * `gaid` - The GAID to get the address for
    ///
    /// # Returns
    /// Returns an `AddressGaidResponse` containing the address
    ///
    /// # Errors
    /// Returns an error if:
    /// - The GAID is invalid
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    /// let address_response = client.get_gaid_address(gaid).await?;
    ///
    /// println!("Address for GAID {}: {}", gaid, address_response.address);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_gaid_address(
        &self,
        gaid: &str,
    ) -> Result<crate::model::AddressGaidResponse, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "address"], None::<&()>)
            .await
    }

    /// Gets a list of all managers.
    ///
    /// # Returns
    /// Returns a vector of `Manager` objects
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let managers = client.get_managers().await?;
    /// for manager in managers {
    ///     println!("Manager: {} (ID: {})", manager.username, manager.id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_managers(&self) -> Result<Vec<crate::model::Manager>, Error> {
        self.request_json(Method::GET, &["managers"], None::<&()>)
            .await
    }

    /// Creates a new manager.
    ///
    /// # Arguments
    /// * `new_manager` - The manager creation request containing username and password
    ///
    /// # Returns
    /// Returns the created `Manager` object
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The manager creation request is invalid
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::ManagerCreate};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let new_manager = ManagerCreate {
    ///     username: "new_manager".to_string(),
    ///     password: "secure_password".to_string(),
    /// };
    ///
    /// let manager = client.create_manager(&new_manager).await?;
    /// println!("Created manager: {} (ID: {})", manager.username, manager.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_manager(
        &self,
        new_manager: &crate::model::ManagerCreate,
    ) -> Result<crate::model::Manager, Error> {
        self.request_json(Method::POST, &["managers", "create"], Some(new_manager))
            .await
    }

    /// Gets all assignments for a specific asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to get assignments for
    ///
    /// # Returns
    /// Returns a vector of `Assignment` objects
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The asset UUID is invalid
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let assignments = client.get_asset_assignments(asset_uuid).await?;
    ///
    /// for assignment in assignments {
    ///     println!("Assignment ID: {}, Amount: {}", assignment.id, assignment.amount);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_assignments(&self, asset_uuid: &str) -> Result<Vec<Assignment>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "assignments"],
            None::<&()>,
        )
        .await
    }

    /// Creates multiple asset assignments in batch.
    ///
    /// This method creates multiple asset assignments for the specified asset. Each assignment
    /// allocates a specific amount of the asset to a registered user. The assignments are
    /// created individually due to API limitations, but this method handles the batch processing
    /// automatically.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to create assignments for
    /// * `requests` - A slice of `CreateAssetAssignmentRequest` structs containing assignment details
    ///
    /// # Returns
    /// Returns a vector of `Assignment` structs representing the created assignments with their
    /// assigned IDs and status information.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - Any assignment request contains invalid data (e.g., invalid user ID, negative amount)
    /// - Insufficient asset balance for the total requested assignments
    /// - Any individual assignment creation fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::CreateAssetAssignmentRequest};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let requests = vec![
    ///     CreateAssetAssignmentRequest {
    ///         registered_user: 123,
    ///         amount: 1000,
    ///         vesting_timestamp: None,
    ///         ready_for_distribution: false,
    ///     },
    ///     CreateAssetAssignmentRequest {
    ///         registered_user: 456,
    ///         amount: 500,
    ///         vesting_timestamp: None,
    ///         ready_for_distribution: true,
    ///     },
    /// ];
    ///
    /// let assignments = client.create_asset_assignments(asset_uuid, &requests).await?;
    /// println!("Created {} assignments", assignments.len());
    /// for assignment in assignments {
    ///     println!("Assignment {}: {} units to user {}",
    ///              assignment.id, assignment.amount, assignment.registered_user);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_asset_assignments`](Self::get_asset_assignments) - List all assignments for an asset
    /// - [`delete_asset_assignment`](Self::delete_asset_assignment) - Remove an assignment
    /// - [`edit_asset_assignment`](Self::edit_asset_assignment) - Update assignment details
    /// - [`set_assignment_ready_for_distribution`](Self::set_assignment_ready_for_distribution) - Mark for distribution
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

    /// Gets a specific asset assignment by asset UUID and assignment ID.
    ///
    /// This method sends a GET request to retrieve detailed information about a specific asset
    /// assignment. Asset assignments represent the allocation of assets to users or entities,
    /// including information such as the assigned amount, recipient details, and assignment status.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset for which to retrieve the assignment
    /// * `assignment_id` - The ID of the specific assignment to retrieve
    ///
    /// # Returns
    /// Returns an `Assignment` struct containing the assignment details including:
    /// - Assignment ID and amount
    /// - Recipient information
    /// - Assignment status and metadata
    /// - Creation and modification timestamps
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The asset UUID is invalid or does not exist
    /// - The assignment ID is invalid or does not exist
    /// - The assignment is not accessible to the current user
    /// - The response cannot be parsed as a valid Assignment
    ///
    /// # Example
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// // Retrieve assignment with ID "123" for asset "550e8400-e29b-41d4-a716-446655440000"
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let assignment_id = "123";
    ///
    /// let assignment = client.get_asset_assignment(asset_uuid, assignment_id).await?;
    ///
    /// println!("Assignment ID: {}", assignment.id);
    /// println!("Assigned amount: {}", assignment.amount);
    /// println!("Registered user: {}", assignment.registered_user);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_assignment(
        &self,
        asset_uuid: &str,
        assignment_id: &str,
    ) -> Result<Assignment, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "assignments", assignment_id],
            None::<&()>,
        )
        .await
    }

    /// Creates a distribution for an asset with the specified assignments.
    ///
    /// This method initiates the distribution creation process by sending assignment details
    /// to the AMP API. The API will return a distribution UUID and address mappings that
    /// can be used for subsequent transaction creation and confirmation steps.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to distribute
    /// * `assignments` - A vector of `AssetDistributionAssignment` structs containing user IDs, addresses, and amounts
    ///
    /// # Returns
    /// Returns a `DistributionResponse` containing:
    /// - `distribution_uuid` - Unique identifier for the created distribution
    /// - `map_address_amount` - Mapping of addresses to amounts to be distributed
    /// - `map_address_asset` - Mapping of addresses to asset IDs
    /// - `asset_id` - The asset ID for the distribution
    ///
    /// # Errors
    /// Returns an `AmpError` if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - Assignment data is invalid (e.g., invalid user IDs, negative amounts, invalid addresses)
    /// - Insufficient asset balance for the requested distribution
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::AssetDistributionAssignment, AmpError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await.map_err(AmpError::from)?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let assignments = vec![
    ///     AssetDistributionAssignment {
    ///         user_id: "user123".to_string(),
    ///         address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
    ///         amount: 100.0,
    ///     },
    ///     AssetDistributionAssignment {
    ///         user_id: "user456".to_string(),
    ///         address: "lq1qq3xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
    ///         amount: 50.0,
    ///     },
    /// ];
    ///
    /// let distribution_response = client.create_distribution(asset_uuid, assignments).await?;
    /// println!("Created distribution: {}", distribution_response.distribution_uuid);
    /// println!("Asset ID: {}", distribution_response.asset_id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_asset_assignments`](Self::get_asset_assignments) - List assignments for an asset
    /// - [`create_asset_assignments`](Self::create_asset_assignments) - Create new assignments
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub async fn create_distribution(
        &self,
        asset_uuid: &str,
        assignments: Vec<crate::model::AssetDistributionAssignment>,
    ) -> Result<crate::model::DistributionResponse, AmpError> {
        use crate::model::{CreateDistributionRequest, DistributionAssignmentRequest};

        let create_span = tracing::debug_span!(
            "create_distribution",
            asset_uuid = %asset_uuid,
            assignment_count = assignments.len()
        );
        let _enter = create_span.enter();

        tracing::debug!(
            "Creating distribution for asset {} with {} assignments",
            asset_uuid,
            assignments.len()
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Distribution creation failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        if assignments.is_empty() {
            tracing::error!("Distribution creation failed: empty assignments");
            return Err(AmpError::validation("Assignments cannot be empty"));
        }

        // Convert AssetDistributionAssignment to DistributionAssignmentRequest
        // The API expects user_uuid field, but our input uses user_id
        tracing::trace!("Converting {} assignments to API format", assignments.len());
        let mut total_amount = 0.0;
        let api_assignments: Vec<DistributionAssignmentRequest> = assignments
            .into_iter()
            .enumerate()
            .map(
                #[allow(clippy::cognitive_complexity)]
                |(index, assignment)| {
                    tracing::trace!(
                        "Converting assignment {}: user_id={}, address={}, amount={}",
                        index,
                        assignment.user_id,
                        assignment.address,
                        assignment.amount
                    );

                    // Validate assignment data
                    if assignment.user_id.is_empty() {
                        tracing::error!("Assignment {} has empty user_id", index);
                        return Err(AmpError::validation(format!(
                            "Assignment {index} has empty user_id"
                        )));
                    }
                    if assignment.address.is_empty() {
                        tracing::error!("Assignment {} has empty address", index);
                        return Err(AmpError::validation(format!(
                            "Assignment {index} has empty address"
                        )));
                    }
                    if assignment.amount <= 0.0 {
                        tracing::error!(
                            "Assignment {} has non-positive amount: {}",
                            index,
                            assignment.amount
                        );
                        return Err(AmpError::validation(format!(
                            "Assignment {} has non-positive amount: {}",
                            index, assignment.amount
                        )));
                    }

                    total_amount += assignment.amount;

                    Ok(DistributionAssignmentRequest {
                        user_uuid: assignment.user_id, // Map user_id to user_uuid for API
                        amount: assignment.amount,
                        address: assignment.address,
                    })
                },
            )
            .collect::<Result<Vec<_>, AmpError>>()?;

        tracing::debug!(
            "Converted {} assignments successfully, total amount: {}",
            api_assignments.len(),
            total_amount
        );

        let request = CreateDistributionRequest {
            assignments: api_assignments,
        };

        tracing::debug!("Sending distribution creation request to AMP API");
        let api_call_start = std::time::Instant::now();

        // Make the API call
        let response: crate::model::DistributionResponse = self
            .request_json(
                Method::GET,
                &["assets", asset_uuid, "distributions", "create"],
                Some(&request),
            )
            .await
            .map_err(
                #[allow(clippy::cognitive_complexity)]
                |e| {
                    let api_call_duration = api_call_start.elapsed();
                    let error_msg =
                        format!("Failed to create distribution after {api_call_duration:?}: {e}");
                    tracing::error!("{}", error_msg);

                    // Check for specific API error patterns
                    let error_str = e.to_string();
                    if error_str.contains("404") || error_str.contains("not found") {
                        tracing::error!(
                            "Asset {} not found - verify asset UUID is correct",
                            asset_uuid
                        );
                    } else if error_str.contains("400") || error_str.contains("bad request") {
                        tracing::error!("Bad request - check assignment data format and values");
                    } else if error_str.contains("401") || error_str.contains("unauthorized") {
                        tracing::error!("Unauthorized - check API credentials and token validity");
                    } else if error_str.contains("403") || error_str.contains("forbidden") {
                        tracing::error!("Forbidden - check permissions for asset distribution");
                    } else if error_str.contains("429") || error_str.contains("rate limit") {
                        tracing::error!("Rate limited - wait before retrying");
                    } else if error_str.contains("500") || error_str.contains("internal server") {
                        tracing::error!(
                            "Server error - this may be a temporary issue, retry may help"
                        );
                    }

                    AmpError::api(error_msg)
                },
            )?;

        let api_call_duration = api_call_start.elapsed();
        tracing::info!(
            "Successfully created distribution: {} (took {:?})",
            response.distribution_uuid,
            api_call_duration
        );

        // Validate response data
        if response.distribution_uuid.is_empty() {
            tracing::error!("API returned empty distribution UUID");
            return Err(AmpError::api("API returned empty distribution UUID"));
        }

        if response.asset_id.is_empty() {
            tracing::error!("API returned empty asset ID");
            return Err(AmpError::api("API returned empty asset ID"));
        }

        if response.map_address_amount.is_empty() {
            tracing::error!("API returned empty address mapping");
            return Err(AmpError::api("API returned empty address mapping"));
        }

        tracing::debug!(
            "Distribution response validated - {} addresses mapped, asset_id: {}",
            response.map_address_amount.len(),
            response.asset_id
        );

        Ok(response)
    }

    /// Confirms a distribution with transaction and change data.
    ///
    /// This method submits the final confirmation for a distribution by providing
    /// the transaction details and any change UTXOs to the AMP API. This completes
    /// the distribution workflow after the transaction has been broadcast and confirmed
    /// on the blockchain.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset being distributed
    /// * `distribution_uuid` - The UUID of the distribution to confirm (from `create_distribution` response)
    /// * `tx_data` - Transaction data containing details and txid from the blockchain
    /// * `change_data` - Vector of change UTXOs from the transaction
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The asset UUID or distribution UUID is invalid
    /// - The transaction data is invalid or incomplete
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, model::{AmpTxData, Unspent}, AmpError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// # let client = ApiClient::new().await?;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let distribution_uuid = "dist-550e8400-e29b-41d4-a716-446655440000";
    ///
    /// // Transaction data for AMP API confirmation
    /// let tx_data = AmpTxData {
    ///     details: serde_json::json!([{
    ///         "account": "",
    ///         "address": "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq",
    ///         "category": "send",
    ///         "amount": -100.0,
    ///         "vout": 0,
    ///         "fee": -0.001
    ///     }]),
    ///     txid: "abc123def456...".to_string(),
    /// };
    ///
    /// // Change UTXOs from Elements node listunspent call
    /// let change_data = vec![
    ///     Unspent {
    ///         txid: "abc123def456...".to_string(),
    ///         vout: 1,
    ///         amount: 25.0,
    ///         asset: "asset_id_hex".to_string(),
    ///         address: "change_address".to_string(),
    ///         spendable: true,
    ///         confirmations: Some(2),
    ///         scriptpubkey: Some("76a914...88ac".to_string()),
    ///         redeemscript: None,
    ///         witnessscript: None,
    ///         amountblinder: None,
    ///         assetblinder: None,
    ///     }
    /// ];
    ///
    /// client.confirm_distribution(asset_uuid, distribution_uuid, tx_data, change_data).await?;
    /// println!("Distribution confirmed successfully");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`create_distribution`](Self::create_distribution) - Create a new distribution
    /// - [`get_asset_assignments`](Self::get_asset_assignments) - List assignments for an asset
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub async fn confirm_distribution(
        &self,
        asset_uuid: &str,
        distribution_uuid: &str,
        tx_data: crate::model::AmpTxData,
        change_data: Vec<crate::model::Unspent>,
    ) -> Result<(), AmpError> {
        use crate::model::ConfirmDistributionRequest;

        let confirm_span = tracing::debug_span!(
            "confirm_distribution",
            asset_uuid = %asset_uuid,
            distribution_uuid = %distribution_uuid,
            txid = %tx_data.txid,
            change_count = change_data.len()
        );
        let _enter = confirm_span.enter();

        tracing::debug!(
            "Confirming distribution {} for asset {} with txid {} ({} change UTXOs)",
            distribution_uuid,
            asset_uuid,
            tx_data.txid,
            change_data.len()
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Distribution confirmation failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        if distribution_uuid.is_empty() {
            tracing::error!("Distribution confirmation failed: empty distribution UUID");
            return Err(AmpError::validation("Distribution UUID cannot be empty"));
        }

        if tx_data.txid.is_empty() {
            tracing::error!("Distribution confirmation failed: empty transaction ID");
            return Err(AmpError::validation("Transaction ID cannot be empty"));
        }

        // Log transaction details for debugging
        tracing::debug!("Transaction details array: {:?}", tx_data.details);

        // Log change data details
        if change_data.is_empty() {
            tracing::debug!("No change UTXOs to include in confirmation");
        } else {
            let total_change: f64 = change_data.iter().map(|utxo| utxo.amount).sum();
            tracing::debug!(
                "Change data - {} UTXOs, total amount: {}",
                change_data.len(),
                total_change
            );

            for (i, utxo) in change_data.iter().enumerate() {
                tracing::trace!(
                    "Change UTXO {}: txid={}, vout={}, amount={}, spendable={}",
                    i,
                    utxo.txid,
                    utxo.vout,
                    utxo.amount,
                    utxo.spendable
                );
            }
        }

        let request = ConfirmDistributionRequest {
            tx_data: tx_data.clone(),
            change_data: change_data.clone(),
        };

        tracing::debug!("Sending distribution confirmation request to AMP API");
        let api_call_start = std::time::Instant::now();

        // Make the API call
        self.request_empty(
            Method::POST,
            &["assets", asset_uuid, "distributions", distribution_uuid, "confirm"],
            Some(&request),
        )
        .await
        .map_err(#[allow(clippy::cognitive_complexity)] |e| {
            let api_call_duration = api_call_start.elapsed();
            let error_msg = format!(
                "Failed to confirm distribution {} after {:?}: {}. IMPORTANT: Transaction {} was successful on blockchain. Use this txid to manually retry confirmation.",
                distribution_uuid, api_call_duration, e, tx_data.txid
            );
            tracing::error!("{}", error_msg);

            // Check for specific API error patterns
            let error_str = e.to_string();
            if error_str.contains("404") || error_str.contains("not found") {
                tracing::error!("Distribution {} not found - verify distribution UUID is correct", distribution_uuid);
            } else if error_str.contains("400") || error_str.contains("bad request") {
                tracing::error!("Bad request - check transaction data format and change data");
            } else if error_str.contains("409") || error_str.contains("conflict") {
                tracing::error!("Conflict - distribution may already be confirmed");
            } else if error_str.contains("422") || error_str.contains("unprocessable") {
                tracing::error!("Unprocessable entity - check transaction confirmations and data validity");
            } else if error_str.contains("500") || error_str.contains("internal server") {
                tracing::error!("Server error - this may be a temporary issue, retry with txid: {}", tx_data.txid);
            }

            AmpError::api(error_msg)
        })?;

        let api_call_duration = api_call_start.elapsed();
        tracing::info!(
            "Successfully confirmed distribution: {} for asset: {} with txid: {} (took {:?})",
            distribution_uuid,
            asset_uuid,
            tx_data.txid,
            api_call_duration
        );

        Ok(())
    }

    /// Cancels an in-progress distribution for an asset.
    ///
    /// This method cancels a distribution that is currently in progress (unconfirmed status).
    /// Once a distribution is cancelled, it cannot be confirmed and the assigned amounts
    /// become available for new distributions.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `distribution_uuid` - The UUID of the distribution to cancel
    ///
    /// # Returns
    /// Returns `Ok(())` if the distribution was successfully cancelled.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The distribution is not found
    /// - The distribution is already confirmed and cannot be cancelled
    ///
    /// # Examples
    /// ```no_run
    /// use amp_rs::ApiClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = ApiClient::new().await?;
    ///     
    ///     client.cancel_distribution(
    ///         "asset-uuid-123",
    ///         "distribution-uuid-456"
    ///     ).await?;
    ///     
    ///     println!("Distribution cancelled successfully");
    ///     Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub async fn cancel_distribution(
        &self,
        asset_uuid: &str,
        distribution_uuid: &str,
    ) -> Result<(), AmpError> {
        let cancel_span = tracing::debug_span!(
            "cancel_distribution",
            asset_uuid = %asset_uuid,
            distribution_uuid = %distribution_uuid
        );
        let _enter = cancel_span.enter();

        tracing::debug!(
            "Cancelling distribution {} for asset {}",
            distribution_uuid,
            asset_uuid
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Distribution cancellation failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        if distribution_uuid.is_empty() {
            tracing::error!("Distribution cancellation failed: empty distribution UUID");
            return Err(AmpError::validation("Distribution UUID cannot be empty"));
        }

        let api_call_start = std::time::Instant::now();

        self.request_empty(
            Method::DELETE,
            &[
                "assets",
                asset_uuid,
                "distributions",
                distribution_uuid,
                "cancel",
            ],
            None::<&()>,
        )
        .await
        .map_err(|e| {
            let api_call_duration = api_call_start.elapsed();
            let error_msg = format!(
                "Failed to cancel distribution {distribution_uuid} for asset {asset_uuid} after {api_call_duration:?}: {e}"
            );
            tracing::error!("{}", error_msg);

            // Check for specific API error patterns
            let error_str = e.to_string();
            if error_str.contains("404") || error_str.contains("not found") {
                tracing::error!(
                    "Distribution {} not found - verify distribution UUID is correct",
                    distribution_uuid
                );
            } else if error_str.contains("400") || error_str.contains("bad request") {
                tracing::error!("Bad request - distribution may already be confirmed or invalid");
            } else if error_str.contains("409") || error_str.contains("conflict") {
                tracing::error!(
                    "Conflict - distribution may already be confirmed and cannot be cancelled"
                );
            } else if error_str.contains("422") || error_str.contains("unprocessable") {
                tracing::error!(
                    "Unprocessable entity - distribution is in a state that cannot be cancelled"
                );
            }

            AmpError::api(error_msg)
        })?;

        let api_call_duration = api_call_start.elapsed();
        tracing::info!(
            "Successfully cancelled distribution: {} for asset: {} (took {:?})",
            distribution_uuid,
            asset_uuid,
            api_call_duration
        );

        Ok(())
    }

    /// Gets all distributions for a specific asset.
    ///
    /// This method retrieves all distributions (both confirmed and unconfirmed) for the specified asset.
    /// This is useful for checking if there are any in-progress distributions before deleting an asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to get distributions for
    ///
    /// # Returns
    /// Returns a vector of `Distribution` objects for the asset.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// use amp_rs::ApiClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = ApiClient::new().await?;
    ///     
    ///     let distributions = client.get_asset_distributions("asset-uuid-123").await?;
    ///     
    ///     for distribution in distributions {
    ///         println!("Distribution: {} - Status: {:?}",
    ///                  distribution.distribution_uuid,
    ///                  distribution.distribution_status);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_asset_distributions(
        &self,
        asset_uuid: &str,
    ) -> Result<Vec<crate::model::Distribution>, Error> {
        let distributions_span = tracing::debug_span!(
            "get_asset_distributions",
            asset_uuid = %asset_uuid
        );
        let _enter = distributions_span.enter();

        tracing::debug!("Getting distributions for asset {}", asset_uuid);

        // Validate input
        if asset_uuid.is_empty() {
            tracing::error!("Get distributions failed: empty asset UUID");
            return Err(Error::RequestFailed(
                "Asset UUID cannot be empty".to_string(),
            ));
        }

        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "distributions"],
            None::<&()>,
        )
        .await
    }

    /// Gets a specific distribution by UUID for an asset.
    ///
    /// This method retrieves detailed information about a specific distribution,
    /// including its status, UUID, and associated transactions.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `distribution_uuid` - The UUID of the distribution to retrieve
    ///
    /// # Returns
    /// Returns a `Distribution` struct containing:
    /// - `distribution_uuid` - The unique identifier for the distribution
    /// - `distribution_status` - Current status of the distribution
    /// - `transactions` - List of transactions associated with the distribution
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed as JSON
    /// - The asset UUID or distribution UUID is empty
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let distribution = client.get_asset_distribution(
    ///     "asset-uuid-123",
    ///     "distribution-uuid-456"
    /// ).await?;
    ///
    /// println!("Distribution: {} - Status: {:?}",
    ///          distribution.distribution_uuid,
    ///          distribution.distribution_status);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_asset_distributions`](Self::get_asset_distributions) - List all distributions for an asset
    /// - [`create_distribution`](Self::create_distribution) - Create a new distribution
    /// - [`confirm_distribution`](Self::confirm_distribution) - Confirm a distribution
    /// - [`cancel_distribution`](Self::cancel_distribution) - Cancel a distribution
    #[allow(clippy::cognitive_complexity)]
    pub async fn get_asset_distribution(
        &self,
        asset_uuid: &str,
        distribution_uuid: &str,
    ) -> Result<crate::model::Distribution, Error> {
        let distribution_span = tracing::debug_span!(
            "get_asset_distribution",
            asset_uuid = %asset_uuid,
            distribution_uuid = %distribution_uuid
        );
        let _enter = distribution_span.enter();

        tracing::debug!(
            "Getting distribution {} for asset {}",
            distribution_uuid,
            asset_uuid
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Get distribution failed: empty asset UUID");
            return Err(Error::RequestFailed(
                "Asset UUID cannot be empty".to_string(),
            ));
        }

        if distribution_uuid.is_empty() {
            tracing::error!("Get distribution failed: empty distribution UUID");
            return Err(Error::RequestFailed(
                "Distribution UUID cannot be empty".to_string(),
            ));
        }

        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "distributions", distribution_uuid],
            None::<&()>,
        )
        .await
    }

    /// Requests reissuance data for an asset
    ///
    /// This method creates a reissuance request with the AMP API and returns
    /// the necessary data to execute the reissuance transaction, including
    /// asset information, amount, and required UTXOs.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to reissue
    /// * `amount_to_reissue` - The amount to reissue (in satoshis for the asset)
    ///
    /// # Returns
    /// Returns a `ReissueRequestResponse` containing:
    /// - `command` - The command type ("reissue")
    /// - `min_supported_client_script_version` - Minimum script version required
    /// - `base_url` - Base URL for the AMP API
    /// - `asset_uuid` - The asset UUID
    /// - `asset_id` - The asset ID (hex string)
    /// - `amount` - The amount to reissue
    /// - `reissuance_utxos` - List of required reissuance token UTXOs
    ///
    /// # Errors
    /// Returns an `AmpError` if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - The asset is not reissuable
    /// - Insufficient reissuance tokens are available
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, AmpError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await.map_err(AmpError::from)?;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let amount = 1000000; // 0.01 of an asset with 8 decimals
    ///
    /// let response = client.reissue_request(asset_uuid, amount).await?;
    /// println!("Reissuance request created for asset: {}", response.asset_id);
    /// println!("Amount to reissue: {}", response.amount);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`reissue_confirm`](Self::reissue_confirm) - Confirm a completed reissuance
    /// - [`reissue_asset`](Self::reissue_asset) - Complete reissuance workflow
    #[allow(clippy::cognitive_complexity)]
    pub async fn reissue_request(
        &self,
        asset_uuid: &str,
        amount_to_reissue: i64,
    ) -> Result<crate::model::ReissueRequestResponse, AmpError> {
        use crate::model::ReissueRequest;

        let request_span = tracing::debug_span!("reissue_request", asset_uuid = %asset_uuid);
        let _enter = request_span.enter();

        tracing::debug!(
            "Creating reissuance request for asset {} with amount {}",
            asset_uuid,
            amount_to_reissue
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Reissuance request failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        if amount_to_reissue <= 0 {
            tracing::error!(
                "Reissuance request failed: invalid amount {}",
                amount_to_reissue
            );
            return Err(AmpError::validation("Amount to reissue must be positive"));
        }

        let request = ReissueRequest { amount_to_reissue };

        let response: crate::model::ReissueRequestResponse = self
            .request_json(
                Method::POST,
                &["assets", asset_uuid, "reissue-request"],
                Some(&request),
            )
            .await
            .map_err(|e| {
                tracing::error!("Reissuance request failed: {}", e);
                AmpError::api(format!("Failed to create reissuance request: {e}"))
                    .with_context("Reissuance request creation")
            })?;

        tracing::info!(
            "Reissuance request created successfully: asset_id={}, amount={}",
            response.asset_id,
            response.amount
        );

        Ok(response)
    }

    /// Confirms a completed reissuance transaction
    ///
    /// This method confirms a reissuance transaction that has been broadcast
    /// to the Elements network. It provides the transaction details and issuance
    /// information to the AMP API to register the reissuance.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset that was reissued
    /// * `details` - Transaction details from `gettransaction` RPC call (as JSON Value)
    /// * `listissuances` - List of issuances from `listissuances` RPC call for this transaction
    /// * `reissuance_output` - Reissuance output containing txid and vin (as JSON Value)
    ///
    /// # Returns
    /// Returns a `ReissueResponse` containing:
    /// - `txid` - The transaction ID
    /// - `vin` - The input index of the reissuance
    /// - `reissuance_amount` - The amount that was reissued
    ///
    /// # Errors
    /// Returns an `AmpError` if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - The transaction data is invalid or incomplete
    /// - The reissuance transaction is not valid
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, AmpError, ElementsRpc};
    /// # use serde_json::json;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await.map_err(AmpError::from)?;
    /// let rpc = ElementsRpc::from_env()?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let txid = "abc123...";
    ///
    /// // Get transaction details
    /// let tx_detail = rpc.get_transaction(txid).await?;
    /// let details = serde_json::to_value(&tx_detail.details).unwrap();
    ///
    /// // Get issuances for this transaction
    /// let issuances = rpc.list_issuances(None).await?;
    /// let listissuances: Vec<_> = issuances
    ///     .into_iter()
    ///     .filter(|i| i.get("txid").and_then(|v| v.as_str()) == Some(txid))
    ///     .collect();
    ///
    /// let reissuance_output = json!({"txid": txid, "vin": 0});
    ///
    /// let response = client.reissue_confirm(
    ///     asset_uuid,
    ///     details,
    ///     listissuances,
    ///     reissuance_output,
    /// ).await?;
    ///
    /// println!("Reissuance confirmed: txid={}, vin={}", response.txid, response.vin);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`reissue_request`](Self::reissue_request) - Create a reissuance request
    /// - [`reissue_asset`](Self::reissue_asset) - Complete reissuance workflow
    #[allow(clippy::cognitive_complexity)]
    pub async fn reissue_confirm(
        &self,
        asset_uuid: &str,
        details: serde_json::Value,
        listissuances: Vec<serde_json::Value>,
        reissuance_output: serde_json::Value,
    ) -> Result<crate::model::ReissueResponse, AmpError> {
        use crate::model::ReissueConfirmRequest;

        let confirm_span = tracing::debug_span!("reissue_confirm", asset_uuid = %asset_uuid);
        let _enter = confirm_span.enter();

        // Extract txid for logging (clone to avoid borrow checker issue)
        let txid = reissuance_output
            .get("txid")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);
        let vin = reissuance_output
            .get("vin")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        tracing::debug!(
            "Confirming reissuance for asset {} with txid {} vin {} ({} issuances)",
            asset_uuid,
            txid,
            vin,
            listissuances.len()
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Reissuance confirmation failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        let request = ReissueConfirmRequest {
            details,
            listissuances,
            reissuance_output,
        };

        let response: crate::model::ReissueResponse = self
            .request_json(
                Method::POST,
                &["assets", asset_uuid, "reissue-confirm"],
                Some(&request),
            )
            .await
            .map_err(|e| {
                tracing::error!("Reissuance confirmation failed: {}", e);
                AmpError::api(format!(
                    "Failed to confirm reissuance for txid {}: {}. \
                    IMPORTANT: Transaction {} was successful on blockchain. \
                    You may need to retry confirmation with this txid.",
                    &txid, e, &txid
                ))
                .with_context("Reissuance confirmation")
            })?;

        tracing::info!(
            "Reissuance confirmed successfully: txid={}, vin={}, amount={}",
            response.txid,
            response.vin,
            response.reissuance_amount
        );

        Ok(response)
    }

    /// Creates a burn request for an asset
    ///
    /// This method requests the data needed to burn (destroy) a specific amount of an asset.
    /// The response contains UTXOs that need to be available in the wallet for the burn operation.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to burn
    /// * `amount` - The amount to burn (in satoshis for the asset)
    ///
    /// # Returns
    /// Returns a `BurnCreate` containing:
    /// - Asset information (UUID, asset ID)
    /// - Amount to burn
    /// - Required UTXOs that must be available in the wallet
    ///
    /// # Errors
    /// Returns an `AmpError` if:
    /// - The asset UUID is invalid or empty
    /// - The amount is invalid (non-positive)
    /// - Authentication fails or insufficient permissions
    /// - The asset does not exist
    /// - The HTTP request fails
    /// - The server returns an error status
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, AmpError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await?;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let amount = 1000000; // 0.01 of an asset with 8 decimals
    ///
    /// let response = client.burn_request(asset_uuid, amount).await?;
    /// println!("Burn request created for asset: {}", response.asset_id);
    /// println!("Amount to burn: {}", response.amount);
    /// println!("Required UTXOs: {:?}", response.utxos);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`burn_confirm`](Self::burn_confirm) - Confirm a burn transaction
    /// - [`burn_asset`](Self::burn_asset) - Complete burn workflow
    #[allow(clippy::cognitive_complexity)]
    pub async fn burn_request(
        &self,
        asset_uuid: &str,
        amount: i64,
    ) -> Result<crate::model::BurnCreate, AmpError> {
        use crate::model::BurnRequest;

        let request_span = tracing::debug_span!("burn_request", asset_uuid = %asset_uuid);
        let _enter = request_span.enter();

        tracing::debug!(
            "Creating burn request for asset {} with amount {}",
            asset_uuid,
            amount
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Burn request failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        if amount <= 0 {
            tracing::error!("Burn request failed: invalid amount {}", amount);
            return Err(AmpError::validation("Amount to burn must be positive"));
        }

        let request = BurnRequest { amount };

        let response: crate::model::BurnCreate = self
            .request_json(
                Method::POST,
                &["assets", asset_uuid, "burn-request"],
                Some(&request),
            )
            .await
            .map_err(|e| {
                tracing::error!("Burn request failed: {}", e);
                AmpError::api(format!("Failed to create burn request: {e}"))
                    .with_context("Burn request creation")
            })?;

        tracing::info!(
            "Burn request created successfully: asset_id={}, amount={}",
            response.asset_id,
            response.amount
        );

        Ok(response)
    }

    /// Confirms a completed burn transaction
    ///
    /// This method confirms a burn transaction that has been broadcast
    /// to the Elements network. It provides the transaction details and
    /// change data to complete the burn registration with the AMP API.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset that was burned
    /// * `tx_data` - Transaction data from `gettransaction` RPC call (as JSON Value, containing at least txid)
    /// * `change_data` - Change data from `listunspent` RPC call filtered by `asset_id` and txid (as JSON Values)
    ///
    /// # Returns
    /// Returns `Ok(())` on success (the API returns an empty response with status 200)
    ///
    /// # Errors
    /// Returns an `AmpError` if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - The transaction data is invalid or incomplete
    /// - The burn transaction is not valid
    /// - The HTTP request fails
    /// - The server returns an error status
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, AmpError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await?;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    ///
    /// let tx_data = serde_json::json!({
    ///     "txid": "abc123def456..."
    /// });
    ///
    /// let change_data = vec![serde_json::json!({
    ///     "txid": "abc123def456...",
    ///     "vout": 0,
    ///     "address": "tlq1qq...",
    ///     "amount": 100.0,
    ///     "asset": "asset_id_here",
    ///     "spendable": true
    /// })];
    ///
    /// client.burn_confirm(asset_uuid, tx_data, change_data).await?;
    /// println!("Burn confirmed successfully");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`burn_request`](Self::burn_request) - Create a burn request
    /// - [`burn_asset`](Self::burn_asset) - Complete burn workflow
    #[allow(clippy::cognitive_complexity)]
    pub async fn burn_confirm(
        &self,
        asset_uuid: &str,
        tx_data: serde_json::Value,
        change_data: Vec<serde_json::Value>,
    ) -> Result<(), AmpError> {
        use crate::model::BurnConfirmRequest;

        let confirm_span = tracing::debug_span!("burn_confirm", asset_uuid = %asset_uuid);
        let _enter = confirm_span.enter();

        // Extract txid for logging
        let txid = tx_data
            .get("txid")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);

        tracing::debug!(
            "Confirming burn for asset {} with txid {} ({} change outputs)",
            asset_uuid,
            txid,
            change_data.len()
        );

        // Validate inputs
        if asset_uuid.is_empty() {
            tracing::error!("Burn confirmation failed: empty asset UUID");
            return Err(AmpError::validation("Asset UUID cannot be empty"));
        }

        let request = BurnConfirmRequest {
            tx_data,
            change_data,
        };

        // The burn-confirm endpoint returns 200 with empty body (no JSON response)
        self.request_empty(
            Method::POST,
            &["assets", asset_uuid, "burn-confirm"],
            Some(&request),
        )
        .await
        .map_err(|e| {
            tracing::error!("Burn confirmation failed: {}", e);
            AmpError::api(format!(
                "Failed to confirm burn for txid {}: {}. \
                IMPORTANT: Transaction {} was successful on blockchain. \
                You may need to retry confirmation with this txid.",
                &txid, e, &txid
            ))
            .with_context("Burn confirmation")
        })?;

        tracing::info!("Burn confirmed successfully: txid={}", txid);

        Ok(())
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
    /// This method revokes a manager's access to a specific asset, preventing them from
    /// performing asset management operations such as creating assignments, managing ownership,
    /// or modifying asset properties. The manager will no longer be able to access this asset
    /// through their management interface.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager to remove permissions from
    /// * `asset_uuid` - The UUID of the asset to remove permissions for
    ///
    /// # Returns
    /// Returns `Ok(())` on successful permission removal.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The manager ID is invalid or does not exist
    /// - The asset UUID is invalid or does not exist
    /// - The manager does not currently have permissions for this asset
    /// - The HTTP request fails
    /// - The server returns an error status
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let manager_id = 123;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    ///
    /// client.manager_remove_asset(manager_id, asset_uuid).await?;
    /// println!("Removed asset {} from manager {}", asset_uuid, manager_id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`add_asset_to_manager`](Self::add_asset_to_manager) - Grant manager permissions for an asset
    /// - [`get_manager`](Self::get_manager) - Get manager information including current assets
    /// - [`revoke_manager`](Self::revoke_manager) - Remove all asset permissions from manager
    /// - [`lock_manager`](Self::lock_manager) - Lock manager account
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

    /// Locks a manager account to prevent further operations.
    ///
    /// This method sends a PUT request to lock the specified manager, preventing any further
    /// operations on that manager account. This is typically used for security purposes or
    /// when a manager needs to be temporarily disabled.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager to lock
    ///
    /// # Returns
    /// Returns `Ok(())` if the manager was successfully locked.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The manager ID is invalid or does not exist
    /// - The manager is already locked
    ///
    /// # Example
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// // Lock manager with ID 123
    /// client.lock_manager(123).await?;
    /// println!("Manager 123 has been locked successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lock_manager(&self, manager_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::PUT,
            &["managers", &manager_id.to_string(), "lock"],
            None::<&()>,
        )
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

    /// Changes the password for a specific manager.
    ///
    /// This method updates the password for the specified manager and returns new credentials
    /// including a new authentication token. The manager's username and new password are
    /// returned in the response along with a new token that should be used for subsequent
    /// API requests.
    ///
    /// # Arguments
    ///
    /// * `manager_id` - The ID of the manager whose password should be changed
    /// * `password` - The new password to set for the manager
    ///
    /// # Returns
    ///
    /// Returns a `ChangePasswordResponse` containing:
    /// - `username`: The manager's username
    /// - `password`: The new password (wrapped in `Secret`)
    /// - `token`: A new authentication token (wrapped in `Secret`)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The manager ID is invalid or not found
    /// - Authentication fails or token is invalid
    /// - The caller lacks permissions to change this manager's password
    /// - Network connectivity issues occur
    /// - The server returns an error status
    /// - The response cannot be parsed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::ApiClient;
    /// # use reqwest::Url;
    /// # use secrecy::Secret;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let base_url = Url::parse("https://amp.blockstream.com/api")?;
    /// let client = ApiClient::with_mock_token(base_url, "test_token".to_string())?;
    ///
    /// let new_password = Secret::new("new_secure_password".to_string());
    /// let response = client.change_manager_password(123, new_password).await?;
    ///
    /// println!("Password changed for manager: {}", response.username);
    /// // The response.token can be used for subsequent API calls
    /// # Ok(())
    /// # }
    /// ```
    pub async fn change_manager_password(
        &self,
        manager_id: i64,
        password: Secret<String>,
    ) -> Result<ChangePasswordResponse, Error> {
        let request = ChangePasswordRequest {
            password: Secret::new(Password(password.expose_secret().clone())),
        };
        self.request_json(
            Method::POST,
            &["managers", &manager_id.to_string(), "change-password"],
            Some(request),
        )
        .await
    }

    /// Authorizes a manager to manage a specific asset.
    ///
    /// This method sends a PUT request to authorize the specified manager to manage the given asset.
    /// Once authorized, the manager will have permissions to perform operations on the asset such as
    /// creating assignments, managing ownership, and other asset-related operations.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager to authorize
    /// * `asset_uuid` - The UUID of the asset to add to the manager's authorized assets
    ///
    /// # Returns
    /// Returns `Ok(())` if the manager was successfully authorized for the asset.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The manager ID is invalid or does not exist
    /// - The asset UUID is invalid or does not exist
    /// - The manager is already authorized for this asset
    /// - The manager is locked and cannot be modified
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// // Authorize manager 123 to manage asset with UUID "550e8400-e29b-41d4-a716-446655440000"
    /// let manager_id = 123;
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    ///
    /// client.add_asset_to_manager(manager_id, asset_uuid).await?;
    /// println!("Manager {} is now authorized to manage asset {}", manager_id, asset_uuid);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`manager_remove_asset`](Self::manager_remove_asset) - Remove manager permissions for an asset
    /// - [`get_manager`](Self::get_manager) - Get manager information including current assets
    /// - [`get_manager_permissions`](Self::get_manager_permissions) - Get manager's current permissions
    /// - [`lock_manager`](Self::lock_manager) - Lock manager account
    pub async fn add_asset_to_manager(
        &self,
        manager_id: i64,
        asset_uuid: &str,
    ) -> Result<(), Error> {
        self.request_empty(
            Method::PUT,
            &[
                "managers",
                &manager_id.to_string(),
                "assets",
                asset_uuid,
                "add",
            ],
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
    ///   Removes an asset assignment.
    ///
    /// This method permanently deletes an asset assignment, returning the allocated assets
    /// back to the available pool. This operation cannot be undone. If the assignment has
    /// already been distributed, this operation may fail.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset containing the assignment
    /// * `assignment_id` - The ID of the assignment to delete
    ///
    /// # Returns
    /// Returns `Ok(())` on successful deletion.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Authentication fails or insufficient permissions
    /// - The asset UUID is invalid or does not exist
    /// - The assignment ID is invalid or does not exist
    /// - The assignment has already been distributed and cannot be deleted
    /// - The assignment is locked and cannot be modified
    /// - The HTTP request fails
    /// - The server returns an error status
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::ApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = ApiClient::new().await?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let assignment_id = "123";
    ///
    /// client.delete_asset_assignment(asset_uuid, assignment_id).await?;
    /// println!("Successfully deleted assignment {}", assignment_id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`get_asset_assignment`](Self::get_asset_assignment) - Get assignment details before deletion
    /// - [`create_asset_assignments`](Self::create_asset_assignments) - Create new assignments
    /// - [`edit_asset_assignment`](Self::edit_asset_assignment) - Update assignment instead of deleting
    /// - [`lock_asset_assignment`](Self::lock_asset_assignment) - Lock assignment to prevent changes
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

    /// Distributes assets to multiple users through a comprehensive workflow
    ///
    /// This method orchestrates the complete asset distribution process:
    /// 1. Validates input parameters (asset UUID format, assignments structure)
    /// 2. Verifies `ElementsRpc` connection and signer interface availability
    /// 3. Authenticates with the AMP API using the client's token
    /// 4. Creates a distribution request via the AMP API
    /// 5. Constructs and signs the blockchain transaction using the provided signer
    /// 6. Broadcasts the transaction to the Elements network
    /// 7. Waits for blockchain confirmations (2 confirmations minimum)
    /// 8. Confirms the distribution with the AMP API
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to distribute (must be valid UUID format)
    /// * `assignments` - Vector of assignments specifying `user_id`, address, and amount
    /// * `node_rpc` - `ElementsRpc` client for blockchain operations
    /// * `signer` - Signer implementation for transaction signing
    ///
    /// # Returns
    /// Returns `Ok(())` if the distribution completes successfully, or an `AmpError` if:
    /// - Input validation fails (invalid UUID format, empty assignments, etc.)
    /// - `ElementsRpc` connection cannot be established
    /// - Signer interface is not available
    /// - Authentication with AMP API fails
    /// - Distribution creation fails
    /// - Transaction construction or signing fails
    /// - Blockchain broadcasting fails
    /// - Confirmation timeout occurs
    /// - Distribution confirmation with AMP API fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, ElementsRpc, AmpError};
    /// # use amp_rs::model::AssetDistributionAssignment;
    /// # use amp_rs::signer::{Signer, LwkSoftwareSigner};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await?;
    /// let elements_rpc = ElementsRpc::from_env()?;
    /// let (_, signer) = LwkSoftwareSigner::generate_new()?;
    ///
    /// let assignments = vec![
    ///     AssetDistributionAssignment {
    ///         user_id: "user123".to_string(),
    ///         address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
    ///         amount: 100.0,
    ///     },
    /// ];
    ///
    /// client.distribute_asset(
    ///     "550e8400-e29b-41d4-a716-446655440000",
    ///     assignments,
    ///     &elements_rpc,
    ///     "wallet_name",
    ///     &signer
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Requirements
    /// This method implements requirements:
    /// - 1.1: Single method for complete distribution workflow
    /// - 2.2: Assignment details validation
    /// - 2.4: Input validation for all parameters
    /// - 5.1: Comprehensive error handling with context
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub async fn distribute_asset(
        &self,
        asset_uuid: &str,
        assignments: Vec<AssetDistributionAssignment>,
        node_rpc: &ElementsRpc,
        wallet_name: &str,
        signer: &dyn Signer,
    ) -> Result<(), AmpError> {
        let distribution_span = tracing::info_span!(
            "distribute_asset",
            asset_uuid = %asset_uuid,
            assignment_count = assignments.len()
        );
        let _enter = distribution_span.enter();

        tracing::info!(
            "Starting asset distribution workflow for asset: {} with {} assignments",
            asset_uuid,
            assignments.len()
        );

        // Step 1: Input validation - asset_uuid format
        tracing::debug!("Step 1: Validating asset UUID format");
        Self::validate_asset_uuid(asset_uuid).map_err(|e| {
            let error = AmpError::validation(format!("Invalid asset UUID: {e}"));
            tracing::error!("Asset UUID validation failed: {}", e);
            error.with_context("Step 1: Asset UUID validation")
        })?;
        tracing::debug!("Asset UUID validation passed");

        // Step 2: Input validation - assignments data structure
        tracing::debug!("Step 2: Validating {} assignments", assignments.len());
        Self::validate_assignments(&assignments).map_err(|e| {
            let error = AmpError::validation(format!("Invalid assignments: {e}"));
            tracing::error!("Assignments validation failed: {}", e);
            error.with_context("Step 2: Assignments validation")
        })?;
        tracing::debug!("Assignments validation passed");

        // Step 3: Check ElementsRpc connection availability
        tracing::debug!("Step 3: Validating Elements RPC connection");
        self.validate_elements_rpc_connection(node_rpc)
            .await
            .map_err(|e| {
                let error = AmpError::rpc(format!("ElementsRpc connection validation failed: {e}"));
                tracing::error!("Elements RPC connection validation failed: {}", e);
                error.with_context("Step 3: Elements RPC connection validation")
            })?;
        tracing::debug!("Elements RPC connection validation passed");

        // Step 4: Check signer interface availability
        tracing::debug!("Step 4: Validating signer interface");
        self.validate_signer_interface(signer).await.map_err(|e| {
            let error = AmpError::validation(format!("Signer interface validation failed: {e}"));
            tracing::error!("Signer interface validation failed: {}", e);
            error.with_context("Step 4: Signer interface validation")
        })?;
        tracing::debug!("Signer interface validation passed");

        tracing::info!("✓ All input validations completed successfully");

        // Step 5: Authenticate with AMP API using existing TokenManager
        tracing::debug!("Step 5: Authenticating with AMP API");
        let _token = self.token_strategy.get_token().await.map_err(|e| {
            tracing::error!("AMP API authentication failed: {}", e);
            let amp_error = AmpError::Existing(e);
            if amp_error.is_retryable() {
                if let Some(instructions) = amp_error.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }
            amp_error.with_context("Step 5: AMP API authentication")
        })?;
        tracing::info!("✓ Successfully authenticated with AMP API");

        // Step 6: Create distribution request and parse response data
        tracing::debug!(
            "Step 6: Creating distribution request with {} assignments",
            assignments.len()
        );
        let distribution_response = self
            .create_distribution(asset_uuid, assignments)
            .await
            .map_err(|e| {
                tracing::error!("Distribution creation failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 6: Distribution creation")
            })?;

        tracing::info!(
            "✓ Distribution created successfully: {} with asset_id: {}",
            distribution_response.distribution_uuid,
            distribution_response.asset_id
        );

        // Step 7: Verify Elements node status and execute transaction workflow
        tracing::debug!("Step 7: Verifying Elements node status");
        let (network_info, blockchain_info) = node_rpc.get_node_status().await.map_err(|e| {
            tracing::error!("Elements node status verification failed: {}", e);
            if e.is_retryable() {
                if let Some(instructions) = e.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }
            e.with_context("Step 7: Elements node status verification")
        })?;

        tracing::info!(
            "✓ Elements node verified - chain: {}, blocks: {}, connections: {}",
            blockchain_info.chain,
            blockchain_info.blocks,
            network_info.connections
        );

        // Step 8: Send distribution transaction using Elements' sendmany
        tracing::debug!("Step 8: Sending distribution transaction using Elements sendmany");

        // Create asset amounts map for sendmany (all outputs use the same asset)
        let mut asset_amounts = std::collections::HashMap::new();
        for address in distribution_response.map_address_amount.keys() {
            asset_amounts.insert(address.clone(), distribution_response.asset_id.clone());
        }

        tracing::info!(
            "Using sendmany for {} outputs with asset {}",
            distribution_response.map_address_amount.len(),
            distribution_response.asset_id
        );

        // Use Elements' sendmany which properly handles confidential transactions
        let txid = node_rpc
            .sendmany(
                wallet_name,
                distribution_response.map_address_amount.clone(),
                asset_amounts,
                Some(0), // min_conf: 0 to include unconfirmed UTXOs (matches Python implementation)
                Some("AMP asset distribution"), // comment
                None,    // subtract_fee_from: let Elements handle fees automatically
                Some(false), // replaceable: false for final transactions
                Some(1), // conf_target: 1 block for faster confirmation
                Some("UNSET"), // estimate_mode: let Elements choose
            )
            .await
            .map_err(|e| {
                tracing::error!("Sendmany transaction failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 8: Sendmany transaction")
            })?;

        tracing::info!("✓ Transaction sent successfully with ID: {}", txid);

        // Step 9: Wait for confirmations
        tracing::debug!("Step 9: Waiting for blockchain confirmations (minimum 2 confirmations, 10-minute timeout)");
        let confirmation_start = std::time::Instant::now();
        let tx_detail = node_rpc.wait_for_confirmations(wallet_name, &txid, Some(2), Some(10)).await
            .map_err(|e| {
                let elapsed = confirmation_start.elapsed();
                tracing::error!(
                    "Confirmation waiting failed after {:?}: {}",
                    elapsed,
                    e
                );

                if let AmpError::Timeout(_) = &e {
                    tracing::warn!(
                        "Confirmation timeout - transaction {} may still be pending. \
                        Use this txid to manually confirm the distribution if it gets confirmed later.",
                        txid
                    );
                    let timeout_error = AmpError::timeout(format!(
                        "Confirmation timeout for txid: {txid}. Use this txid to manually confirm the distribution."
                    ));
                    timeout_error.with_context("Step 9: Confirmation waiting")
                } else {
                    if e.is_retryable() {
                        if let Some(instructions) = e.retry_instructions() {
                            tracing::warn!("Retry instructions: {}", instructions);
                        }
                    }
                    e.with_context(format!("Step 9: Confirmation waiting for txid: {txid}"))
                }
            })?;

        let confirmation_duration = confirmation_start.elapsed();
        tracing::info!(
            "✓ Transaction confirmed with {} confirmations at block height: {:?} (took {:?})",
            tx_detail.confirmations,
            tx_detail.blockheight,
            confirmation_duration
        );

        // Step 10: Collect change data for confirmation
        tracing::debug!("Step 10: Collecting change data for distribution confirmation");
        let change_data = node_rpc
            .collect_change_data(
                &distribution_response.asset_id,
                &txid,
                node_rpc,
                wallet_name,
            )
            .await
            .map_err(|e| {
                tracing::error!("Change data collection failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 10: Change data collection")
            })?;

        tracing::info!("✓ Collected {} change UTXOs", change_data.len());
        if !change_data.is_empty() {
            tracing::debug!("Change UTXOs: {:?}", change_data);
        }

        // Step 11: Submit final confirmation to AMP API
        tracing::debug!("Step 11: Submitting final confirmation to AMP API");

        // Extract the details field from the transaction (matching Python implementation)
        // Python: details = rpc.call('gettransaction', txid).get('details')
        let transaction_details = tx_detail.details.unwrap_or_else(Vec::new);
        tracing::debug!(
            "Transaction details for confirmation: {:?}",
            transaction_details
        );

        let amp_tx_data = crate::model::AmpTxData {
            details: serde_json::Value::Array(transaction_details),
            txid: txid.clone(),
        };

        // Log the exact payload being sent to AMP for debugging
        tracing::info!("Sending confirmation payload to AMP:");
        tracing::info!("  tx_data.txid: {}", amp_tx_data.txid);
        tracing::info!("  tx_data.details: {:?}", amp_tx_data.details);
        tracing::info!("  change_data: {} UTXOs", change_data.len());

        let confirmation_request = crate::model::ConfirmDistributionRequest {
            tx_data: amp_tx_data.clone(),
            change_data: change_data.clone(),
        };

        if let Ok(payload_json) = serde_json::to_string_pretty(&confirmation_request) {
            tracing::debug!("Full confirmation payload: {}", payload_json);
        }

        self.confirm_distribution(
            asset_uuid,
            &distribution_response.distribution_uuid,
            amp_tx_data,
            change_data,
        )
        .await
        .map_err(|e| {
            tracing::error!("Distribution confirmation failed: {}", e);

            // For confirmation failures, always provide retry instructions with txid
            let confirmation_error = AmpError::api(format!(
                "Failed to confirm distribution {}: {}. \
                IMPORTANT: Transaction {} was successful on blockchain. \
                Use this txid to manually retry confirmation.",
                distribution_response.distribution_uuid, e, txid
            ));

            if e.is_retryable() {
                if let Some(instructions) = e.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }

            confirmation_error.with_context("Step 11: Distribution confirmation")
        })?;

        tracing::info!(
            "🎉 Asset distribution completed successfully for asset: {} with transaction: {}",
            asset_uuid,
            txid
        );

        Ok(())
    }

    /// Reissues an asset through a comprehensive workflow
    ///
    /// This method orchestrates the complete asset reissuance process:
    /// 1. Validates input parameters (asset UUID format, amount)
    /// 2. Verifies `ElementsRpc` connection and signer interface availability
    /// 3. Authenticates with the AMP API using the client's token
    /// 4. Creates a reissuance request via the AMP API
    /// 5. Waits for transaction propagation and checks for lost outputs
    /// 6. Verifies reissuance token UTXOs are available
    /// 7. Calls the Elements node's `reissueasset` RPC method
    /// 8. Waits for blockchain confirmations (2 confirmations minimum)
    /// 9. Retrieves transaction details and issuance information
    /// 10. Confirms the reissuance with the AMP API
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to reissue (must be valid UUID format)
    /// * `amount_to_reissue` - The amount to reissue (in satoshis for the asset)
    /// * `node_rpc` - `ElementsRpc` client for blockchain operations
    /// * `wallet_name` - Name of the Elements wallet to use for transaction queries
    /// * `signer` - Signer implementation for future support (currently not used, node RPC signs)
    ///
    /// # Returns
    /// Returns `Ok(())` if the reissuance completes successfully, or an `AmpError` if:
    /// - Input validation fails (invalid UUID format, invalid amount, etc.)
    /// - `ElementsRpc` connection cannot be established
    /// - Signer interface is not available
    /// - Authentication with AMP API fails
    /// - Reissuance request creation fails
    /// - Lost outputs are detected
    /// - Required UTXOs are not available
    /// - Reissuance transaction creation fails
    /// - Confirmation timeout occurs
    /// - Reissuance confirmation with AMP API fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, ElementsRpc, AmpError};
    /// # use amp_rs::signer::LwkSoftwareSigner;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await?;
    /// let elements_rpc = ElementsRpc::from_env()?;
    /// let (_, signer) = LwkSoftwareSigner::generate_new()?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let amount = 1000000; // 0.01 of an asset with 8 decimals
    /// let wallet_name = "test_wallet";
    ///
    /// client.reissue_asset(asset_uuid, amount, &elements_rpc, wallet_name, &signer).await?;
    /// println!("Reissuance completed successfully");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`reissue_request`](Self::reissue_request) - Create a reissuance request only
    /// - [`reissue_confirm`](Self::reissue_confirm) - Confirm a reissuance transaction only
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub async fn reissue_asset(
        &self,
        asset_uuid: &str,
        amount_to_reissue: i64,
        node_rpc: &ElementsRpc,
        wallet_name: &str,
        signer: &dyn Signer,
    ) -> Result<(), AmpError> {
        let reissue_span = tracing::info_span!(
            "reissue_asset",
            asset_uuid = %asset_uuid,
            amount_to_reissue = amount_to_reissue
        );
        let _enter = reissue_span.enter();

        tracing::info!(
            "Starting asset reissuance workflow for asset: {} with amount: {}",
            asset_uuid,
            amount_to_reissue
        );

        // Step 1: Input validation - asset_uuid format
        tracing::debug!("Step 1: Validating asset UUID format");
        Self::validate_asset_uuid(asset_uuid).map_err(|e| {
            let error = AmpError::validation(format!("Invalid asset UUID: {e}"));
            tracing::error!("Asset UUID validation failed: {}", e);
            error.with_context("Step 1: Asset UUID validation")
        })?;
        tracing::debug!("Asset UUID validation passed");

        // Step 2: Input validation - amount
        tracing::debug!("Step 2: Validating reissuance amount");
        if amount_to_reissue <= 0 {
            let error = AmpError::validation("Amount to reissue must be positive".to_string());
            tracing::error!("Amount validation failed: amount must be positive");
            return Err(error.with_context("Step 2: Amount validation"));
        }
        tracing::debug!("Amount validation passed");

        // Step 3: Check ElementsRpc connection availability
        tracing::debug!("Step 3: Validating Elements RPC connection");
        self.validate_elements_rpc_connection(node_rpc)
            .await
            .map_err(|e| {
                let error = AmpError::rpc(format!("ElementsRpc connection validation failed: {e}"));
                tracing::error!("Elements RPC connection validation failed: {}", e);
                error.with_context("Step 3: Elements RPC connection validation")
            })?;
        tracing::debug!("Elements RPC connection validation passed");

        // Step 4: Check signer interface availability (for future support)
        tracing::debug!("Step 4: Validating signer interface");
        self.validate_signer_interface(signer).await.map_err(|e| {
            let error = AmpError::validation(format!("Signer interface validation failed: {e}"));
            tracing::error!("Signer interface validation failed: {}", e);
            error.with_context("Step 4: Signer interface validation")
        })?;
        tracing::debug!("Signer interface validation passed");

        tracing::info!("✓ All input validations completed successfully");

        // Step 5: Authenticate with AMP API using existing TokenManager
        tracing::debug!("Step 5: Authenticating with AMP API");
        let _token = self.token_strategy.get_token().await.map_err(|e| {
            tracing::error!("AMP API authentication failed: {}", e);
            let amp_error = AmpError::Existing(e);
            if amp_error.is_retryable() {
                if let Some(instructions) = amp_error.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }
            amp_error.with_context("Step 5: AMP API authentication")
        })?;
        tracing::info!("✓ Successfully authenticated with AMP API");

        // Step 6: Create reissuance request and parse response data
        tracing::debug!(
            "Step 6: Creating reissuance request with amount {}",
            amount_to_reissue
        );
        let reissue_response = self
            .reissue_request(asset_uuid, amount_to_reissue)
            .await
            .map_err(|e| {
                tracing::error!("Reissuance request creation failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 6: Reissuance request creation")
            })?;

        tracing::info!(
            "✓ Reissuance request created successfully: asset_id={}, amount={}",
            reissue_response.asset_id,
            reissue_response.amount
        );

        // Step 7: Verify Elements node status
        tracing::debug!("Step 7: Verifying Elements node status");
        let (network_info, blockchain_info) = node_rpc.get_node_status().await.map_err(|e| {
            tracing::error!("Elements node status verification failed: {}", e);
            if e.is_retryable() {
                if let Some(instructions) = e.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }
            e.with_context("Step 7: Elements node status verification")
        })?;

        tracing::info!(
            "✓ Elements node verified - chain: {}, blocks: {}, connections: {}",
            blockchain_info.chain,
            blockchain_info.blocks,
            network_info.connections
        );

        // Step 8: Wait for transaction propagation (60 seconds as per Python script)
        tracing::debug!("Step 8: Waiting for transaction propagation (60 seconds)");
        tracing::info!("Waiting 60 seconds for transaction propagation...");
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        tracing::debug!("Transaction propagation wait completed");

        // Step 9: Check for lost outputs
        tracing::debug!("Step 9: Checking for lost outputs");
        let balance_response: serde_json::Value = self
            .request_json(Method::GET, &["assets", asset_uuid, "balance"], None::<&()>)
            .await
            .map_err(|e| {
                tracing::error!("Failed to check lost outputs: {}", e);
                AmpError::api(format!("Balance check failed: {e}"))
                    .with_context("Step 9: Lost outputs check")
            })?;

        // Check if lost_outputs field exists and is not empty
        if let Some(lost_outputs) = balance_response.get("lost_outputs") {
            if let Some(lost_outputs_array) = lost_outputs.as_array() {
                if !lost_outputs_array.is_empty() {
                    let error_msg = format!(
                        "Lost outputs detected: {}. Transaction will not be sent.",
                        serde_json::to_string(&lost_outputs_array).unwrap_or_default()
                    );
                    tracing::error!("{}", error_msg);
                    return Err(AmpError::api(error_msg).with_context("Step 9: Lost outputs check"));
                }
            }
        }

        tracing::info!("✓ No lost outputs detected");

        // Step 10: Check UTXOs match reissuance_utxos from response
        tracing::debug!(
            "Step 10: Verifying {} reissuance token UTXOs are available",
            reissue_response.reissuance_utxos.len()
        );

        let available_utxos = node_rpc.list_unspent(None).await.map_err(|e| {
            tracing::error!("Failed to list UTXOs: {}", e);
            AmpError::rpc(format!("Failed to list UTXOs: {e}"))
                .with_context("Step 10: UTXO verification")
        })?;

        // Check that all required reissuance UTXOs are available
        let local_utxos: std::collections::HashSet<(String, i64)> = available_utxos
            .iter()
            .map(|utxo| (utxo.txid.clone(), i64::from(utxo.vout)))
            .collect();

        let mut missing_utxos = Vec::new();
        for required_utxo in &reissue_response.reissuance_utxos {
            if !local_utxos.contains(&(required_utxo.txid.clone(), required_utxo.vout)) {
                missing_utxos.push(format!("{}:{}", required_utxo.txid, required_utxo.vout));
            }
        }

        if !missing_utxos.is_empty() {
            let error_msg = format!(
                "Missing reissuance token UTXOs: {}. Ensure reissuance tokens are available in the wallet.",
                missing_utxos.join(", ")
            );
            tracing::error!("{}", error_msg);
            return Err(AmpError::rpc(error_msg).with_context("Step 10: UTXO verification"));
        }

        tracing::info!(
            "✓ All {} reissuance token UTXOs are available",
            reissue_response.reissuance_utxos.len()
        );

        // Step 11: Call Elements node's reissueasset RPC method
        tracing::debug!("Step 11: Calling Elements reissueasset RPC method");
        let reissuance_output = node_rpc
            .reissueasset(&reissue_response.asset_id, reissue_response.amount)
            .await
            .map_err(|e| {
                tracing::error!("Reissuance transaction creation failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 11: Reissuance transaction creation")
            })?;

        // Extract txid and vin from reissuance output
        let txid = reissuance_output
            .get("txid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AmpError::rpc("Reissuance output missing txid field".to_string())
                    .with_context("Step 11: Reissuance transaction creation")
            })?;
        let vin = reissuance_output
            .get("vin")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                AmpError::rpc("Reissuance output missing vin field".to_string())
                    .with_context("Step 11: Reissuance transaction creation")
            })?;

        tracing::info!(
            "✓ Reissuance transaction created: txid={}, vin={}",
            txid,
            vin
        );

        // Step 12: Wait for confirmations
        tracing::debug!("Step 12: Waiting for blockchain confirmations (minimum 2 confirmations, 10-minute timeout)");
        let confirmation_start = std::time::Instant::now();

        // First, wait for 1 confirmation before spawning treasury address task
        node_rpc
            .wait_for_confirmations(wallet_name, txid, Some(1), Some(10))
            .await
            .map_err(|e| {
                let elapsed = confirmation_start.elapsed();
                tracing::error!(
                    "Confirmation waiting (1 conf) failed after {:?}: {}",
                    elapsed,
                    e
                );
                e.with_context(format!(
                    "Step 12: Waiting for 1 confirmation for txid: {txid}"
                ))
            })?;

        tracing::info!(
            "✓ Transaction has 1 confirmation, spawning treasury address extraction task"
        );

        // Spawn async task to extract and submit reissuance token change address
        // This runs in parallel with the remaining confirmation wait
        let asset_uuid_clone = asset_uuid.to_string();
        let txid_clone = txid.to_string();
        let client_clone = self.clone();
        let node_rpc_clone = node_rpc.clone();
        let wallet_name_clone = wallet_name.to_string();

        tokio::spawn(async move {
            if let Err(e) = client_clone
                .extract_and_submit_reissuance_token_change_address(
                    &asset_uuid_clone,
                    &txid_clone,
                    &node_rpc_clone,
                    &wallet_name_clone,
                )
                .await
            {
                tracing::warn!(
                    "Failed to extract/submit reissuance token change address: {}. \
                    This is non-critical and does not affect the reissuance operation.",
                    e
                );
            }
        });

        // Continue waiting for the full 2 confirmations
        let _tx_detail = node_rpc
            .wait_for_confirmations(wallet_name, txid, Some(2), Some(10))
            .await
            .map_err(|e| {
                let elapsed = confirmation_start.elapsed();
                tracing::error!(
                    "Confirmation waiting failed after {:?}: {}",
                    elapsed,
                    e
                );

                if let AmpError::Timeout(_) = &e {
                    tracing::warn!(
                        "Confirmation timeout - transaction {} may still be pending. \
                        Use this txid to manually confirm the reissuance if it gets confirmed later.",
                        txid
                    );
                    let timeout_error = AmpError::timeout(format!(
                        "Confirmation timeout for txid: {txid}. Use this txid to manually confirm the reissuance."
                    ));
                    timeout_error.with_context("Step 12: Confirmation waiting")
                } else {
                    if e.is_retryable() {
                        if let Some(instructions) = e.retry_instructions() {
                            tracing::warn!("Retry instructions: {}", instructions);
                        }
                    }
                    e.with_context(format!("Step 12: Confirmation waiting for txid: {txid}"))
                }
            })?;

        tracing::info!("✓ Transaction confirmed with at least 2 confirmations");

        // Step 13: Get transaction details and issuance information
        tracing::debug!("Step 13: Retrieving transaction details and issuance information");

        // Get transaction details
        let tx_detail = node_rpc.get_transaction_from_wallet(wallet_name, txid).await.map_err(|e| {
            tracing::error!("Failed to get transaction details: {}", e);
            AmpError::rpc(format!("Failed to get transaction details: {e}"))
                .with_context("Step 13: Transaction details retrieval")
        })?;

        // Convert details to JSON Value
        let details = serde_json::to_value(tx_detail.details).map_err(|e| {
            tracing::error!("Failed to serialize transaction details: {}", e);
            AmpError::api(format!("Failed to serialize transaction details: {e}"))
                .with_context("Step 13: Transaction details serialization")
        })?;

        // Get all issuances and filter by txid
        let all_issuances = node_rpc.list_issuances(None).await.map_err(|e| {
            tracing::error!("Failed to list issuances: {}", e);
            AmpError::rpc(format!("Failed to list issuances: {e}"))
                .with_context("Step 13: Issuance listing")
        })?;

        let listissuances: Vec<serde_json::Value> = all_issuances
            .into_iter()
            .filter(|issuance| {
                issuance
                    .get("txid")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|tid| tid == txid)
            })
            .collect();

        tracing::info!(
            "✓ Retrieved transaction details and {} issuance(s) for txid {}",
            listissuances.len(),
            txid
        );

        // Step 14: Confirm reissuance with AMP API
        tracing::debug!("Step 14: Confirming reissuance with AMP API");

        let reissuance_output_value = serde_json::json!({
            "txid": txid,
            "vin": vin
        });

        self.reissue_confirm(asset_uuid, details, listissuances, reissuance_output_value)
            .await
            .map_err(|e| {
                tracing::error!("Reissuance confirmation failed: {}", e);

                // For confirmation failures, always provide retry instructions with txid
                let confirmation_error = AmpError::api(format!(
                    "Failed to confirm reissuance: {e}. \
                IMPORTANT: Transaction {txid} was successful on blockchain. \
                Use this txid to manually retry confirmation."
                ));

                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }

                confirmation_error.with_context("Step 14: Reissuance confirmation")
            })?;

        tracing::info!(
            "🎉 Asset reissuance completed successfully for asset: {} with transaction: {}",
            asset_uuid,
            txid
        );

        Ok(())
    }

    /// Extracts the reissuance token change address from a reissuance transaction
    /// and submits it to the asset's treasury addresses list.
    ///
    /// This method is called asynchronously after a reissuance transaction has 1 confirmation.
    /// It runs in a separate task to avoid blocking the main reissuance flow.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset that was reissued
    /// * `txid` - The transaction ID of the reissuance transaction
    /// * `node_rpc` - Elements RPC client to query transaction details
    ///
    /// # Returns
    /// Returns `Ok(())` if successful, or an error if extraction/submission fails.
    /// Errors are logged but do not affect the main reissuance operation.
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    async fn extract_and_submit_reissuance_token_change_address(
        &self,
        asset_uuid: &str,
        txid: &str,
        node_rpc: &ElementsRpc,
        wallet_name: &str,
    ) -> Result<(), AmpError> {
        tracing::info!(
            "[Treasury Address Task] Starting extraction for asset {} from txid {}",
            asset_uuid,
            txid
        );

        // Get asset to retrieve reissuance token ID
        let asset = self.get_asset(asset_uuid).await.map_err(|e| {
            tracing::error!("[Treasury Address Task] Failed to get asset: {}", e);
            AmpError::api(format!("Failed to get asset: {e}"))
        })?;

        let reissuance_token_id = asset.reissuance_token_id.as_ref().ok_or_else(|| {
            tracing::error!("[Treasury Address Task] Asset has no reissuance token ID");
            AmpError::validation("Asset has no reissuance token ID".to_string())
        })?;

        tracing::debug!(
            "[Treasury Address Task] Looking for reissuance token ID: {}",
            reissuance_token_id
        );

        // Get transaction details
        let tx_detail = node_rpc.get_transaction_from_wallet(wallet_name, txid).await.map_err(|e| {
            tracing::error!(
                "[Treasury Address Task] Failed to get transaction details: {}",
                e
            );
            e
        })?;

        // Find the reissuance token change address
        let mut change_address: Option<String> = None;
        if let Some(details) = &tx_detail.details {
            tracing::debug!(
                "[Treasury Address Task] Scanning {} transaction detail entries",
                details.len()
            );

            for (index, detail) in details.iter().enumerate() {
                if let (Some(category), Some(asset_id), Some(address)) = (
                    detail.get("category").and_then(|v| v.as_str()),
                    detail.get("asset").and_then(|v| v.as_str()),
                    detail.get("address").and_then(|v| v.as_str()),
                ) {
                    if category == "receive" && asset_id == reissuance_token_id {
                        tracing::info!(
                            "[Treasury Address Task] Found reissuance token receive address at index {}: {}",
                            index,
                            address
                        );
                        change_address = Some(address.to_string());
                        break;
                    }
                }
            }
        }

        if let Some(address) = change_address {
            tracing::info!(
                "[Treasury Address Task] Extracted change address: {}",
                address
            );

            // Check if address is already in treasury addresses
            let treasury_addresses = self
                .get_asset_treasury_addresses(asset_uuid)
                .await
                .map_err(|e| {
                    tracing::error!(
                        "[Treasury Address Task] Failed to get treasury addresses: {}",
                        e
                    );
                    AmpError::api(format!("Failed to get treasury addresses: {e}"))
                })?;

            if treasury_addresses.contains(&address) {
                tracing::info!(
                    "[Treasury Address Task] Address {} is already in treasury addresses list",
                    address
                );
                return Ok(());
            }

            // Submit address to treasury addresses
            tracing::debug!(
                "[Treasury Address Task] Submitting address {} to treasury addresses",
                address
            );

            self.add_asset_treasury_addresses(asset_uuid, std::slice::from_ref(&address))
                .await
                .map_err(|e| {
                    tracing::error!(
                        "[Treasury Address Task] Failed to add treasury address: {}",
                        e
                    );
                    AmpError::api(format!("Failed to add treasury address: {e}"))
                })?;

            tracing::info!(
                "[Treasury Address Task] Successfully added {} to treasury addresses for asset {}",
                address,
                asset_uuid
            );

            Ok(())
        } else {
            tracing::warn!(
                "[Treasury Address Task] Could not find reissuance token change address in transaction {}. \
                This may be normal if all reissuance tokens were consumed.",
                txid
            );
            Err(AmpError::validation(
                "No reissuance token change address found".to_string(),
            ))
        }
    }

    /// Burns (destroys) a specific amount of an asset
    ///
    /// This method orchestrates the complete burn workflow:
    /// 1. Validates input parameters (asset UUID format, amount)
    /// 2. Validates Elements RPC connection and signer interface
    /// 3. Authenticates with AMP API
    /// 4. Creates a burn request via the AMP API
    /// 5. Waits for transaction propagation and checks for lost outputs
    /// 6. Verifies required UTXOs are available
    /// 7. Verifies sufficient balance exists
    /// 8. Calls the Elements node's `destroyamount` RPC method
    /// 9. Waits for blockchain confirmations (2 confirmations minimum)
    /// 10. Retrieves transaction data and change information
    /// 11. Confirms the burn with the AMP API
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to burn (must be valid UUID format)
    /// * `amount_to_burn` - The amount to burn (in satoshis for the asset)
    /// * `node_rpc` - `ElementsRpc` client for blockchain operations
    /// * `wallet_name` - Name of the Elements wallet containing the asset to burn
    /// * `signer` - Signer implementation for future support (currently not used, node RPC signs)
    ///
    /// # Returns
    /// Returns `Ok(())` if the burn completes successfully, or an `AmpError` if:
    /// - Input validation fails (invalid UUID format, invalid amount, etc.)
    /// - `ElementsRpc` connection cannot be established
    /// - Signer interface is not available
    /// - Authentication with AMP API fails
    /// - Burn request creation fails
    /// - Lost outputs are detected
    /// - Required UTXOs are not available
    /// - Insufficient balance exists
    /// - Burn transaction creation fails
    /// - Confirmation timeout occurs
    /// - Burn confirmation with AMP API fails
    ///
    /// # Examples
    /// ```no_run
    /// # use amp_rs::{ApiClient, ElementsRpc, AmpError};
    /// # use amp_rs::signer::LwkSoftwareSigner;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), AmpError> {
    /// let client = ApiClient::new().await?;
    /// let elements_rpc = ElementsRpc::from_env()?;
    /// let (_, signer) = LwkSoftwareSigner::generate_new()?;
    ///
    /// let asset_uuid = "550e8400-e29b-41d4-a716-446655440000";
    /// let amount = 1000000; // 0.01 of an asset with 8 decimals
    /// let wallet_name = "test_wallet";
    ///
    /// client.burn_asset(asset_uuid, amount, &elements_rpc, wallet_name, &signer).await?;
    /// println!("Burn completed successfully");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    /// - [`burn_request`](Self::burn_request) - Create a burn request only
    /// - [`burn_confirm`](Self::burn_confirm) - Confirm a burn transaction only
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub async fn burn_asset(
        &self,
        asset_uuid: &str,
        amount_to_burn: i64,
        node_rpc: &ElementsRpc,
        wallet_name: &str,
        signer: &dyn Signer,
    ) -> Result<(), AmpError> {
        let burn_span = tracing::info_span!(
            "burn_asset",
            asset_uuid = %asset_uuid,
            amount_to_burn = amount_to_burn
        );
        let _enter = burn_span.enter();

        tracing::info!(
            "Starting asset burn workflow for asset: {} with amount: {}",
            asset_uuid,
            amount_to_burn
        );

        // Step 1: Input validation - asset_uuid format
        tracing::debug!("Step 1: Validating asset UUID format");
        Self::validate_asset_uuid(asset_uuid).map_err(|e| {
            let error = AmpError::validation(format!("Invalid asset UUID: {e}"));
            tracing::error!("Asset UUID validation failed: {}", e);
            error.with_context("Step 1: Asset UUID validation")
        })?;
        tracing::debug!("Asset UUID validation passed");

        // Step 2: Input validation - amount
        tracing::debug!("Step 2: Validating burn amount");
        if amount_to_burn <= 0 {
            let error = AmpError::validation("Amount to burn must be positive".to_string());
            tracing::error!("Amount validation failed: amount must be positive");
            return Err(error.with_context("Step 2: Amount validation"));
        }
        tracing::debug!("Amount validation passed");

        // Step 3: Check ElementsRpc connection availability
        tracing::debug!("Step 3: Validating Elements RPC connection");
        self.validate_elements_rpc_connection(node_rpc)
            .await
            .map_err(|e| {
                let error = AmpError::rpc(format!("ElementsRpc connection validation failed: {e}"));
                tracing::error!("Elements RPC connection validation failed: {}", e);
                error.with_context("Step 3: Elements RPC connection validation")
            })?;
        tracing::debug!("Elements RPC connection validation passed");

        // Step 4: Check signer interface availability (for future support)
        tracing::debug!("Step 4: Validating signer interface");
        self.validate_signer_interface(signer).await.map_err(|e| {
            let error = AmpError::validation(format!("Signer interface validation failed: {e}"));
            tracing::error!("Signer interface validation failed: {}", e);
            error.with_context("Step 4: Signer interface validation")
        })?;
        tracing::debug!("Signer interface validation passed");

        tracing::info!("✓ All input validations completed successfully");

        // Step 5: Authenticate with AMP API using existing TokenManager
        tracing::debug!("Step 5: Authenticating with AMP API");
        let _token = self.token_strategy.get_token().await.map_err(|e| {
            tracing::error!("AMP API authentication failed: {}", e);
            let amp_error = AmpError::Existing(e);
            if amp_error.is_retryable() {
                if let Some(instructions) = amp_error.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }
            amp_error.with_context("Step 5: AMP API authentication")
        })?;
        tracing::info!("✓ Successfully authenticated with AMP API");

        // Step 6: Create burn request and parse response data
        tracing::debug!(
            "Step 6: Creating burn request with amount {}",
            amount_to_burn
        );
        let burn_response = self
            .burn_request(asset_uuid, amount_to_burn)
            .await
            .map_err(|e| {
                tracing::error!("Burn request creation failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 6: Burn request creation")
            })?;

        tracing::info!(
            "✓ Burn request created successfully: asset_id={}, amount={}",
            burn_response.asset_id,
            burn_response.amount
        );

        // Step 7: Verify Elements node status
        tracing::debug!("Step 7: Verifying Elements node status");
        let (network_info, blockchain_info) = node_rpc.get_node_status().await.map_err(|e| {
            tracing::error!("Elements node status verification failed: {}", e);
            if e.is_retryable() {
                if let Some(instructions) = e.retry_instructions() {
                    tracing::warn!("Retry instructions: {}", instructions);
                }
            }
            e.with_context("Step 7: Elements node status verification")
        })?;

        tracing::info!(
            "✓ Elements node verified - chain: {}, blocks: {}, connections: {}",
            blockchain_info.chain,
            blockchain_info.blocks,
            network_info.connections
        );

        // Step 8: Wait for transaction propagation (60 seconds as per Python script)
        tracing::debug!("Step 8: Waiting for transaction propagation (60 seconds)");
        tracing::info!("Waiting 60 seconds for transaction propagation...");
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        tracing::debug!("Transaction propagation wait completed");

        // Step 9: Check for lost outputs
        tracing::debug!("Step 9: Checking for lost outputs");
        let balance_response: serde_json::Value = self
            .request_json(Method::GET, &["assets", asset_uuid, "balance"], None::<&()>)
            .await
            .map_err(|e| {
                tracing::error!("Failed to check lost outputs: {}", e);
                AmpError::api(format!("Balance check failed: {e}"))
                    .with_context("Step 9: Lost outputs check")
            })?;

        // Check if lost_outputs field exists and is not empty
        if let Some(lost_outputs) = balance_response.get("lost_outputs") {
            if let Some(lost_outputs_array) = lost_outputs.as_array() {
                if !lost_outputs_array.is_empty() {
                    let error_msg = format!(
                        "Lost outputs detected: {}. Transaction will not be sent.",
                        serde_json::to_string(&lost_outputs_array).unwrap_or_default()
                    );
                    tracing::error!("{}", error_msg);
                    return Err(AmpError::api(error_msg).with_context("Step 9: Lost outputs check"));
                }
            }
        }

        tracing::info!("✓ No lost outputs detected");

        // Step 10: Check UTXOs match expected UTXOs from response
        tracing::debug!(
            "Step 10: Verifying {} required UTXOs are available",
            burn_response.utxos.len()
        );

        let available_utxos = node_rpc.list_unspent(None).await.map_err(|e| {
            tracing::error!("Failed to list UTXOs: {}", e);
            AmpError::rpc(format!("Failed to list UTXOs: {e}"))
                .with_context("Step 10: UTXO verification")
        })?;

        // Check that all required UTXOs are available
        let local_utxos: std::collections::HashSet<(String, i64)> = available_utxos
            .iter()
            .map(|utxo| (utxo.txid.clone(), i64::from(utxo.vout)))
            .collect();

        let mut missing_utxos = Vec::new();
        for required_utxo in &burn_response.utxos {
            if !local_utxos.contains(&(required_utxo.txid.clone(), required_utxo.vout)) {
                missing_utxos.push(format!("{}:{}", required_utxo.txid, required_utxo.vout));
            }
        }

        if !missing_utxos.is_empty() {
            let error_msg = format!(
                "Missing required UTXOs: {}. Ensure the asset UTXOs are available in the wallet.",
                missing_utxos.join(", ")
            );
            tracing::error!("{}", error_msg);
            return Err(AmpError::rpc(error_msg).with_context("Step 10: UTXO verification"));
        }

        tracing::info!(
            "✓ All {} required UTXOs are available",
            burn_response.utxos.len()
        );

        // Step 11: Check local balance >= requested amount
        tracing::debug!("Step 11: Verifying sufficient balance");
        let balances = node_rpc.get_balance(None).await.map_err(|e| {
            tracing::error!("Failed to get balance: {}", e);
            AmpError::rpc(format!("Failed to get balance: {e}"))
                .with_context("Step 11: Balance verification")
        })?;

        // Extract balance for the specific asset_id (getbalance returns a map)
        let local_amount = balances
            .get(&burn_response.asset_id)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let requested_amount = burn_response.amount;

        if local_amount < requested_amount {
            let error_msg = format!(
                "Insufficient balance: local balance ({local_amount}) is lower than requested amount ({requested_amount})"
            );
            tracing::error!("{}", error_msg);
            return Err(AmpError::rpc(error_msg).with_context("Step 11: Balance verification"));
        }

        tracing::info!(
            "✓ Sufficient balance verified: local={}, requested={}",
            local_amount,
            requested_amount
        );

        // Step 12: Call Elements node's destroyamount RPC method
        tracing::debug!("Step 12: Calling Elements destroyamount RPC method");
        let txid = node_rpc
            .destroyamount(&burn_response.asset_id, requested_amount)
            .await
            .map_err(|e| {
                tracing::error!("Burn transaction creation failed: {}", e);
                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }
                e.with_context("Step 12: Burn transaction creation")
            })?;

        tracing::info!("✓ Burn transaction created: txid={}", txid);

        // Step 13: Wait for confirmations
        tracing::debug!("Step 13: Waiting for blockchain confirmations (minimum 2 confirmations, 10-minute timeout)");
        let confirmation_start = std::time::Instant::now();
        let _tx_detail = node_rpc
            .wait_for_confirmations(wallet_name, &txid, Some(2), Some(10))
            .await
            .map_err(|e| {
                let elapsed = confirmation_start.elapsed();
                tracing::error!(
                    "Confirmation waiting failed after {:?}: {}",
                    elapsed,
                    e
                );

                if let AmpError::Timeout(_) = &e {
                    tracing::warn!(
                        "Confirmation timeout - transaction {} may still be pending. \
                        Use this txid to manually confirm the burn if it gets confirmed later.",
                        txid
                    );
                    let timeout_error = AmpError::timeout(format!(
                        "Confirmation timeout for txid: {txid}. Use this txid to manually confirm the burn."
                    ));
                    timeout_error.with_context("Step 13: Confirmation waiting")
                } else {
                    if e.is_retryable() {
                        if let Some(instructions) = e.retry_instructions() {
                            tracing::warn!("Retry instructions: {}", instructions);
                        }
                    }
                    e.with_context(format!("Step 13: Confirmation waiting for txid: {txid}"))
                }
            })?;

        tracing::info!("✓ Transaction confirmed with at least 2 confirmations");

        // Step 14: Get transaction data and change data
        tracing::debug!("Step 14: Retrieving transaction data and change information");

        // Get transaction details (we only need txid for tx_data)
        let tx_data = serde_json::json!({
            "txid": txid
        });

        // Get change_data from listunspent with blinding data filtered by asset_id and txid
        // We need to use list_unspent_with_blinding_data to get amountblinder and assetblinder fields
        // required by the AMP API
        let all_unspent = node_rpc
            .list_unspent_with_blinding_data(wallet_name)
            .await
            .map_err(|e| {
                tracing::error!("Failed to list unspent outputs with blinding data: {}", e);
                AmpError::rpc(format!(
                    "Failed to list unspent outputs with blinding data: {e}"
                ))
                .with_context("Step 14: Change data retrieval")
            })?;

        // Filter and convert to JSON values, preserving all fields including blinding data
        let change_data: Vec<serde_json::Value> = all_unspent
            .into_iter()
            .filter(|utxo| utxo.asset == burn_response.asset_id && utxo.txid == txid)
            .map(|utxo| {
                // Serialize the full Unspent struct to JSON to include all fields
                // including amountblinder and assetblinder which are required by the API
                serde_json::to_value(&utxo).unwrap_or_else(|e| {
                    tracing::warn!("Failed to serialize UTXO to JSON: {}", e);
                    // Fallback to manual construction if serialization fails
                    serde_json::json!({
                        "txid": utxo.txid,
                        "vout": utxo.vout,
                        "address": utxo.address,
                        "amount": utxo.amount,
                        "asset": utxo.asset,
                        "spendable": utxo.spendable,
                        "amountblinder": utxo.amountblinder,
                        "assetblinder": utxo.assetblinder
                    })
                })
            })
            .collect();

        tracing::info!(
            "✓ Retrieved transaction data and {} change output(s) for txid {}",
            change_data.len(),
            txid
        );

        // Step 15: Confirm burn with AMP API
        tracing::debug!("Step 15: Confirming burn with AMP API");

        self.burn_confirm(asset_uuid, tx_data, change_data)
            .await
            .map_err(|e| {
                tracing::error!("Burn confirmation failed: {}", e);

                // For confirmation failures, always provide retry instructions with txid
                let confirmation_error = AmpError::api(format!(
                    "Failed to confirm burn: {e}. \
                IMPORTANT: Transaction {txid} was successful on blockchain. \
                Use this txid to manually retry confirmation."
                ));

                if e.is_retryable() {
                    if let Some(instructions) = e.retry_instructions() {
                        tracing::warn!("Retry instructions: {}", instructions);
                    }
                }

                confirmation_error.with_context("Step 15: Burn confirmation")
            })?;

        tracing::info!(
            "🎉 Asset burn completed successfully for asset: {} with transaction: {}",
            asset_uuid,
            txid
        );

        Ok(())
    }

    /// Validates the asset UUID format
    ///
    /// Ensures the asset UUID follows the standard UUID format (8-4-4-4-12 hexadecimal digits)
    ///
    /// # Arguments
    /// * `asset_uuid` - The asset UUID string to validate
    ///
    /// # Returns
    /// Returns `Ok(())` if valid, or an error describing the validation failure
    ///
    /// # Errors
    /// - Empty or whitespace-only UUID
    /// - Invalid UUID format (not matching standard UUID pattern)
    /// - UUID contains invalid characters
    fn validate_asset_uuid(asset_uuid: &str) -> Result<(), String> {
        if asset_uuid.trim().is_empty() {
            return Err("Asset UUID cannot be empty".to_string());
        }

        // Basic UUID format validation (8-4-4-4-12 pattern)
        // Expected format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        let parts: Vec<&str> = asset_uuid.split('-').collect();
        if parts.len() != 5 {
            return Err(format!(
                "Asset UUID '{asset_uuid}' does not have 5 parts separated by hyphens"
            ));
        }

        // Check each part has the correct length and contains only hex characters
        let expected_lengths = [8, 4, 4, 4, 12];
        for (i, (part, &expected_len)) in parts.iter().zip(expected_lengths.iter()).enumerate() {
            if part.len() != expected_len {
                return Err(format!(
                    "Asset UUID part {} has length {} but expected {}",
                    i + 1,
                    part.len(),
                    expected_len
                ));
            }

            // Check if all characters are valid hexadecimal
            if !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!(
                    "Asset UUID part {} contains non-hexadecimal characters: '{}'",
                    i + 1,
                    part
                ));
            }
        }

        tracing::debug!("Asset UUID validation passed: {}", asset_uuid);
        Ok(())
    }

    /// Validates the assignments data structure
    ///
    /// Ensures assignments vector is not empty and each assignment has valid data
    ///
    /// # Arguments
    /// * `assignments` - Vector of assignments to validate
    ///
    /// # Returns
    /// Returns `Ok(())` if valid, or an error describing the validation failure
    ///
    /// # Errors
    /// - Empty assignments vector
    /// - Assignment with empty `user_id`
    /// - Assignment with empty address
    /// - Assignment with non-positive amount
    /// - Assignment with invalid address format
    #[allow(clippy::cognitive_complexity)]
    fn validate_assignments(assignments: &[AssetDistributionAssignment]) -> Result<(), String> {
        tracing::debug!("Validating {} assignments", assignments.len());

        if assignments.is_empty() {
            tracing::error!("Assignments validation failed: empty assignments vector");
            return Err("Assignments vector cannot be empty".to_string());
        }

        let mut total_amount = 0.0;
        let mut unique_addresses = std::collections::HashSet::new();
        let mut unique_users = std::collections::HashSet::new();

        for (index, assignment) in assignments.iter().enumerate() {
            tracing::trace!(
                "Validating assignment {}: user_id={}, address={}, amount={}",
                index,
                assignment.user_id,
                assignment.address,
                assignment.amount
            );

            // Validate user_id
            if assignment.user_id.trim().is_empty() {
                tracing::error!("Assignment {} validation failed: empty user_id", index);
                return Err(format!("Assignment {index} has empty user_id"));
            }

            // Validate address
            if assignment.address.trim().is_empty() {
                tracing::error!("Assignment {} validation failed: empty address", index);
                return Err(format!("Assignment {index} has empty address"));
            }

            // Basic address format validation (should start with appropriate prefix for Liquid)
            if !assignment.address.starts_with("lq")
                && !assignment.address.starts_with("vj")
                && !assignment.address.starts_with("VJ")
                && !assignment.address.starts_with("VT")
            {
                tracing::error!(
                    "Assignment {} validation failed: invalid address format '{}' (should start with 'lq', 'vj', 'VJ', or 'VT')",
                    index, assignment.address
                );
                return Err(format!(
                    "Assignment {} has invalid address format: '{}' (should start with 'lq', 'vj', 'VJ', or 'VT')",
                    index, assignment.address
                ));
            }

            // Validate amount
            if assignment.amount <= 0.0 {
                tracing::error!(
                    "Assignment {} validation failed: non-positive amount {}",
                    index,
                    assignment.amount
                );
                return Err(format!(
                    "Assignment {} has non-positive amount: {}",
                    index, assignment.amount
                ));
            }

            // Check for reasonable amount limits (prevent overflow issues)
            if assignment.amount > 21_000_000.0 {
                tracing::error!(
                    "Assignment {} validation failed: unreasonably large amount {} (max: 21,000,000)",
                    index, assignment.amount
                );
                return Err(format!(
                    "Assignment {} has unreasonably large amount: {} (max: 21,000,000)",
                    index, assignment.amount
                ));
            }

            // Check for precision issues (more than 8 decimal places)
            let amount_str = format!("{:.8}", assignment.amount);
            if amount_str.len() > 20 {
                // Reasonable length check
                tracing::warn!(
                    "Assignment {} has high precision amount: {} - may cause precision issues",
                    index,
                    assignment.amount
                );
            }

            // Track duplicates for warnings
            if !unique_addresses.insert(&assignment.address) {
                tracing::warn!(
                    "Assignment {} uses duplicate address: {} (this may be intentional)",
                    index,
                    assignment.address
                );
            }

            if !unique_users.insert(&assignment.user_id) {
                tracing::warn!(
                    "Assignment {} uses duplicate user_id: {} (this may be intentional)",
                    index,
                    assignment.user_id
                );
            }

            total_amount += assignment.amount;
        }

        tracing::debug!(
            "Assignments validation passed - {} assignments, total amount: {}, unique addresses: {}, unique users: {}",
            assignments.len(),
            total_amount,
            unique_addresses.len(),
            unique_users.len()
        );

        if total_amount > 100_000_000.0 {
            tracing::warn!(
                "Total distribution amount is very large: {} - ensure this is intentional",
                total_amount
            );
        }

        Ok(())
    }

    /// Validates `ElementsRpc` connection availability
    ///
    /// Attempts to connect to the Elements node and verify basic functionality
    ///
    /// # Arguments
    /// * `node_rpc` - `ElementsRpc` client to validate
    ///
    /// # Returns
    /// Returns `Ok(())` if connection is valid, or an error describing the failure
    ///
    /// # Errors
    /// - Cannot connect to Elements node
    /// - Node is not synchronized
    /// - Node version is incompatible
    /// - RPC authentication fails
    #[allow(clippy::cognitive_complexity)]
    async fn validate_elements_rpc_connection(&self, node_rpc: &ElementsRpc) -> Result<(), String> {
        tracing::debug!("Validating Elements RPC connection");

        // Test basic connectivity by getting network info
        tracing::trace!("Testing Elements RPC connectivity with getnetworkinfo");
        let network_info = node_rpc.get_network_info().await.map_err(|e| {
            tracing::error!("Failed to get network info from Elements node: {}", e);
            format!("Failed to get network info: {e}")
        })?;

        tracing::debug!(
            "Network info retrieved - version: {}, connections: {}, network_active: {}",
            network_info.version,
            network_info.connections,
            network_info.networkactive
        );

        // Check if network is active
        if !network_info.networkactive {
            tracing::error!("Elements node network is not active");
            return Err("Elements node network is not active".to_string());
        }

        // Verify we have active connections (for non-regtest environments)
        if network_info.connections == 0 {
            tracing::warn!("Elements node has no peer connections (may be regtest environment)");
        } else {
            tracing::debug!(
                "Elements node has {} peer connections",
                network_info.connections
            );
        }

        // Test blockchain info to ensure node is operational
        tracing::trace!("Testing Elements RPC with getblockchaininfo");
        let blockchain_info = node_rpc.get_blockchain_info().await.map_err(|e| {
            tracing::error!("Failed to get blockchain info from Elements node: {}", e);
            format!("Failed to get blockchain info: {e}")
        })?;

        let sync_progress = blockchain_info.verificationprogress.unwrap_or(1.0) * 100.0;
        tracing::debug!(
            "Blockchain info retrieved - chain: {}, blocks: {}, sync_progress: {:.2}%",
            blockchain_info.chain,
            blockchain_info.blocks,
            sync_progress
        );

        // Check if node is still in initial block download
        if blockchain_info.initialblockdownload.unwrap_or(false) {
            tracing::error!(
                "Elements node is still in initial block download (sync progress: {:.2}%)",
                sync_progress
            );
            return Err(format!(
                "Elements node is still in initial block download (sync progress: {sync_progress:.2}%)"
            ));
        }

        // Check sync progress
        if blockchain_info.verificationprogress.unwrap_or(1.0) < 0.99 {
            tracing::warn!(
                "Elements node may not be fully synced (sync progress: {:.2}%)",
                sync_progress
            );
        }

        // Check for warnings
        if !network_info.warnings.is_empty() {
            tracing::warn!("Elements node network warnings: {}", network_info.warnings);
        }

        if let Some(warnings) = &blockchain_info.warnings {
            if !warnings.is_empty() {
                tracing::warn!("Elements node blockchain warnings: {}", warnings);
            }
        }

        tracing::debug!(
            "ElementsRpc connection validation passed - chain: {}, blocks: {}, connections: {}, sync: {:.2}%",
            blockchain_info.chain,
            blockchain_info.blocks,
            network_info.connections,
            sync_progress
        );

        Ok(())
    }

    /// Validates signer interface availability
    ///
    /// Tests the signer interface with a dummy transaction to ensure it's functional
    ///
    /// # Arguments
    /// * `signer` - Signer implementation to validate
    ///
    /// # Returns
    /// Returns `Ok(())` if signer is functional, or an error describing the failure
    ///
    /// # Errors
    /// - Signer interface is not responsive
    /// - Signer fails basic functionality test
    #[allow(clippy::cognitive_complexity)]
    async fn validate_signer_interface(&self, signer: &dyn Signer) -> Result<(), String> {
        tracing::debug!("Validating signer interface");

        // Test signer with a minimal dummy transaction hex
        // This is a minimal Elements transaction structure that should parse but not be valid for signing
        let dummy_tx = "0200000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

        tracing::trace!("Testing signer interface with dummy transaction");

        // Attempt to sign the dummy transaction - we expect this to fail with a specific error
        // but the signer should be responsive and not panic
        let validation_start = std::time::Instant::now();
        match signer.sign_transaction(dummy_tx).await {
            Ok(signed_tx) => {
                // Unexpected success with dummy transaction - this might indicate an issue
                tracing::warn!(
                    "Signer unexpectedly succeeded with dummy transaction (returned: {} chars)",
                    signed_tx.len()
                );
                tracing::debug!("Signer validation passed despite unexpected success");
            }
            Err(SignerError::InvalidTransaction(msg)) => {
                // Expected error - signer is working and correctly identified invalid transaction
                tracing::debug!(
                    "Signer interface validation passed - correctly rejected dummy transaction: {}",
                    msg
                );
            }
            Err(SignerError::HexParse(msg)) => {
                // Also acceptable - signer is working and correctly identified parsing issue
                tracing::debug!(
                    "Signer interface validation passed - correctly identified hex parsing issue: {}",
                    msg
                );
            }
            Err(SignerError::Lwk(msg)) => {
                // LWK-specific errors might be acceptable depending on the message
                if msg.contains("invalid") || msg.contains("parse") || msg.contains("decode") {
                    tracing::debug!(
                        "Signer interface validation passed - LWK correctly identified invalid transaction: {}",
                        msg
                    );
                } else {
                    tracing::error!("Signer interface test failed with LWK error: {}", msg);
                    return Err(format!(
                        "Signer interface test failed with LWK error: {msg}"
                    ));
                }
            }
            Err(e) => {
                // Other errors might indicate signer interface issues
                tracing::error!("Signer interface test failed: {}", e);
                return Err(format!("Signer interface test failed: {e}"));
            }
        }

        let validation_duration = validation_start.elapsed();
        tracing::debug!(
            "Signer interface validation completed in {:?}",
            validation_duration
        );

        // Warn if signer is very slow (might indicate performance issues)
        if validation_duration > std::time::Duration::from_secs(5) {
            tracing::warn!(
                "Signer interface validation took {:?} - this may indicate performance issues",
                validation_duration
            );
        }

        Ok(())
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
/// Returns an error if the `TokenManager` cannot be initialized
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
    use crate::signer::LwkSoftwareSigner;
    use tokio;

    #[tokio::test]
    async fn test_mock_token_strategy_basic_functionality() {
        let mock_token = "mock_token_12_345".to_string();
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

        let token_manager =
            Arc::new(TokenManager::with_mock_token(config, base_url, mock_token.clone()).unwrap());

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

        let token_manager =
            Arc::new(TokenManager::with_mock_token(config, base_url, mock_token.clone()).unwrap());

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
        let debug_output = format!("{mock_strategy:?}");

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

    #[tokio::test]
    async fn test_distribute_asset_input_validation() {
        // Create a mock client for testing
        let client = ApiClient::with_mock_token(
            reqwest::Url::parse("http://localhost:8080/api").unwrap(),
            "test_token".to_string(),
        )
        .unwrap();

        // Test invalid asset UUID
        let assignments = vec![AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 100.0,
        }];

        // Create a mock ElementsRpc (this will fail connection validation, but that's expected)
        let elements_rpc = ElementsRpc::new(
            "http://localhost:18884".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        // Create a mock signer
        let (_, signer) = LwkSoftwareSigner::generate_new().unwrap();

        // Test with invalid UUID format
        let result = client
            .distribute_asset(
                "invalid-uuid",
                assignments.clone(),
                &elements_rpc,
                "test_wallet",
                &signer,
            )
            .await;

        assert!(result.is_err());
        if let Err(AmpError::Validation(msg)) = result {
            assert!(msg.contains("Invalid asset UUID"));
        } else {
            panic!("Expected validation error for invalid UUID");
        }

        // Test with empty assignments
        let result = client
            .distribute_asset(
                "550e8400-e29b-41d4-a716-446655440000",
                vec![],
                &elements_rpc,
                "test_wallet",
                &signer,
            )
            .await;

        assert!(result.is_err());
        if let Err(AmpError::Validation(msg)) = result {
            assert!(msg.contains("Invalid assignments"));
        } else {
            panic!("Expected validation error for empty assignments");
        }
    }

    #[test]
    fn test_validate_asset_uuid() {
        let _client = ApiClient::with_mock_token(
            reqwest::Url::parse("http://localhost:8080/api").unwrap(),
            "test_token".to_string(),
        )
        .unwrap();

        // Valid UUID
        assert!(ApiClient::validate_asset_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());

        // Invalid UUIDs
        assert!(ApiClient::validate_asset_uuid("").is_err());
        assert!(ApiClient::validate_asset_uuid("invalid").is_err());
        assert!(ApiClient::validate_asset_uuid("550e8400-e29b-41d4-a716").is_err()); // Too short
        assert!(
            ApiClient::validate_asset_uuid("550e8400-e29b-41d4-a716-446655440000-extra").is_err()
        ); // Too long
        assert!(ApiClient::validate_asset_uuid("550e8400xe29bx41d4xa716x446655440000").is_err()); // Wrong separators
        assert!(ApiClient::validate_asset_uuid("550e8400-e29g-41d4-a716-446655440000").is_err());
        // Invalid hex char
    }

    #[test]
    fn test_validate_assignments() {
        let _client = ApiClient::with_mock_token(
            reqwest::Url::parse("http://localhost:8080/api").unwrap(),
            "test_token".to_string(),
        )
        .unwrap();

        // Valid assignments
        let valid_assignments = vec![AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 100.0,
        }];
        assert!(ApiClient::validate_assignments(&valid_assignments).is_ok());

        // Empty assignments
        assert!(ApiClient::validate_assignments(&[]).is_err());

        // Assignment with empty user_id
        let invalid_assignments = vec![AssetDistributionAssignment {
            user_id: "".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 100.0,
        }];
        assert!(ApiClient::validate_assignments(&invalid_assignments).is_err());

        // Assignment with empty address
        let invalid_assignments = vec![AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "".to_string(),
            amount: 100.0,
        }];
        assert!(ApiClient::validate_assignments(&invalid_assignments).is_err());

        // Assignment with invalid address format
        let invalid_assignments = vec![AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "invalid_address".to_string(),
            amount: 100.0,
        }];
        assert!(ApiClient::validate_assignments(&invalid_assignments).is_err());

        // Assignment with non-positive amount
        let invalid_assignments = vec![AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 0.0,
        }];
        assert!(ApiClient::validate_assignments(&invalid_assignments).is_err());

        // Assignment with unreasonably large amount
        let invalid_assignments = vec![AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 25_000_000.0,
        }];
        assert!(ApiClient::validate_assignments(&invalid_assignments).is_err());
    }

    #[test]
    fn test_enhanced_error_handling_and_logging() {
        // Test AmpError creation and context enhancement
        let api_error = AmpError::api("Distribution creation failed");
        let contextual_error = api_error.with_context("Step 6: Distribution creation");

        match contextual_error {
            AmpError::Api(msg) => {
                assert!(msg.contains("Step 6: Distribution creation"));
                assert!(msg.contains("Distribution creation failed"));
            }
            _ => panic!("Expected Api error variant"),
        }

        // Test retry instructions for different error types
        let rpc_error = AmpError::rpc("Connection failed");
        assert!(rpc_error.is_retryable());
        assert!(rpc_error.retry_instructions().is_some());
        assert!(rpc_error
            .retry_instructions()
            .unwrap()
            .contains("Elements node"));

        let validation_error = AmpError::validation("Invalid UUID");
        assert!(!validation_error.is_retryable());
        assert!(validation_error.retry_instructions().is_none());

        let timeout_error = AmpError::timeout("Confirmation timeout for txid abc123");
        assert!(!timeout_error.is_retryable());
        let instructions = timeout_error.retry_instructions();
        assert!(instructions.is_some());
        assert!(instructions.unwrap().contains("transaction ID"));

        // Test error helper methods
        let signer_error =
            AmpError::Signer(crate::signer::SignerError::Lwk("Test error".to_string()));
        assert!(!signer_error.is_retryable());
        assert!(signer_error.retry_instructions().is_none());

        // Test serialization error
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let serialization_error = AmpError::from(json_error);
        assert!(matches!(serialization_error, AmpError::Serialization(_)));
        assert!(!serialization_error.is_retryable());
    }
}

// ============================================================================
// AmpClient Trait Implementation
// ============================================================================

use crate::client_trait::AmpClient;

#[async_trait::async_trait]
impl AmpClient for ApiClient {
    async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        self.get_assets().await
    }

    async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.get_asset(asset_uuid).await
    }

    async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error> {
        self.get_asset_ownerships(asset_uuid, height).await
    }

    async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error> {
        self.get_asset_activities(asset_uuid, params).await
    }

    async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        self.get_asset_summary(asset_uuid).await
    }

    async fn get_asset_reissuances(&self, asset_uuid: &str) -> Result<Vec<Reissuance>, Error> {
        self.get_asset_reissuances(asset_uuid).await
    }

    async fn get_registered_users(&self) -> Result<Vec<RegisteredUserResponse>, Error> {
        self.get_registered_users().await
    }

    async fn get_registered_user(
        &self,
        registered_id: i64,
    ) -> Result<RegisteredUserResponse, Error> {
        self.get_registered_user(registered_id).await
    }

    async fn get_registered_user_gaids(&self, registered_id: i64) -> Result<Vec<String>, Error> {
        self.get_registered_user_gaids(registered_id).await
    }

    async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        self.get_categories().await
    }

    async fn get_category(&self, registered_id: i64) -> Result<CategoryResponse, Error> {
        self.get_category(registered_id).await
    }

    async fn get_gaid_address(&self, gaid: &str) -> Result<AddressGaidResponse, Error> {
        self.get_gaid_address(gaid).await
    }

    async fn get_gaid_balance(&self, gaid: &str) -> Result<Vec<GaidBalanceEntry>, Error> {
        self.get_gaid_balance(gaid).await
    }

    async fn validate_gaid(&self, gaid: &str) -> Result<ValidateGaidResponse, Error> {
        self.validate_gaid(gaid).await
    }

    // Write methods

    async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error> {
        self.register_asset(asset_uuid).await
    }

    async fn add_registered_user(
        &self,
        new_user: &crate::model::RegisteredUserAdd,
    ) -> Result<RegisteredUserResponse, Error> {
        self.add_registered_user(new_user).await
    }

    async fn edit_registered_user(
        &self,
        registered_user_id: i64,
        edit_data: &crate::model::RegisteredUserEdit,
    ) -> Result<RegisteredUserResponse, Error> {
        self.edit_registered_user(registered_user_id, edit_data)
            .await
    }

    async fn add_gaid_to_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error> {
        self.add_gaid_to_registered_user(registered_user_id, gaid)
            .await
    }

    async fn add_category(
        &self,
        new_category: &crate::model::CategoryAdd,
    ) -> Result<CategoryResponse, Error> {
        self.add_category(new_category).await
    }

    async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        self.add_registered_user_to_category(category_id, user_id)
            .await
    }

    async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        self.remove_registered_user_from_category(category_id, user_id)
            .await
    }

    async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        self.add_asset_to_category(category_id, asset_uuid).await
    }
}
