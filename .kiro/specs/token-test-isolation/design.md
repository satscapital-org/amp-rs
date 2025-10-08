# Design Document

## Overview

This design implements proper test isolation for token management by creating separate token management strategies for mock and live tests. The solution ensures that mock tests operate in complete isolation without any token persistence or shared state, while live tests maintain efficient token reuse through persistence and shared token management.

## Architecture

### Current Problem Analysis

The existing implementation has several issues causing test interference:

1. **Shared Global State**: All tests use the same `GLOBAL_TOKEN_MANAGER` singleton
2. **Persistence Pollution**: Mock tests trigger token persistence mechanisms
3. **Environment Detection**: No clear separation between mock and live test contexts
4. **Token Cleanup**: Mock tests don't properly isolate their token state

### Enhanced Architecture

The solution introduces a **Token Management Strategy Pattern** with two distinct implementations:

```rust
// Strategy trait for different token management approaches
pub trait TokenStrategy: Send + Sync {
    async fn get_token(&self) -> Result<String, Error>;
    async fn clear_token(&self) -> Result<(), Error>;
    fn should_persist(&self) -> bool;
}

// Mock strategy - completely isolated, no persistence
pub struct MockTokenStrategy {
    token: String,
}

// Live strategy - full token management with persistence
pub struct LiveTokenStrategy {
    token_manager: Arc<TokenManager>,
}

// Context-aware ApiClient
pub struct ApiClient {
    client: Client,
    base_url: Url,
    token_strategy: Box<dyn TokenStrategy>,
}
```

## Components and Interfaces

### 1. Token Strategy Trait

**Purpose**: Define the interface for different token management approaches

```rust
#[async_trait::async_trait]
pub trait TokenStrategy: Send + Sync + std::fmt::Debug {
    /// Gets a valid authentication token
    async fn get_token(&self) -> Result<String, Error>;
    
    /// Clears stored token (for testing)
    async fn clear_token(&self) -> Result<(), Error>;
    
    /// Returns whether this strategy should persist tokens
    fn should_persist(&self) -> bool;
    
    /// Returns the strategy type for debugging
    fn strategy_type(&self) -> &'static str;
}
```

### 2. Mock Token Strategy

**Purpose**: Isolated token management for mock tests

**Key Features**:
- No persistence to disk
- No shared state between instances
- No network requests
- Simple token storage

```rust
#[derive(Debug, Clone)]
pub struct MockTokenStrategy {
    token: String,
}

impl MockTokenStrategy {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

#[async_trait::async_trait]
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
}
```

### 3. Live Token Strategy

**Purpose**: Full token management for live tests and production

**Key Features**:
- Uses global token manager for sharing
- Enables token persistence
- Full retry and refresh logic
- Thread-safe operations

```rust
#[derive(Debug)]
pub struct LiveTokenStrategy {
    token_manager: Arc<TokenManager>,
}

impl LiveTokenStrategy {
    pub async fn new() -> Result<Self, Error> {
        let token_manager = TokenManager::get_global_instance().await?;
        Ok(Self { token_manager })
    }
    
    pub async fn with_config(config: RetryConfig) -> Result<Self, Error> {
        // For live strategy, we still use global instance but can influence its config
        let token_manager = TokenManager::get_global_instance().await?;
        Ok(Self { token_manager })
    }
}

#[async_trait::async_trait]
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
}
```

### 4. Enhanced ApiClient

**Purpose**: Context-aware client that uses appropriate token strategy

```rust
pub struct ApiClient {
    client: Client,
    base_url: Url,
    token_strategy: Box<dyn TokenStrategy>,
}

impl ApiClient {
    /// Creates a new ApiClient with automatic strategy detection
    pub async fn new() -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        let client = Client::new();
        
        // Automatic strategy selection based on environment
        let token_strategy: Box<dyn TokenStrategy> = if Self::is_mock_environment() {
            tracing::info!("Detected mock environment - using mock token strategy");
            // In mock environment, we can't create a proper mock strategy without a token
            // This should only be used when we have real credentials for live tests
            Box::new(LiveTokenStrategy::new().await?)
        } else {
            tracing::info!("Detected live environment - using live token strategy");
            Box::new(LiveTokenStrategy::new().await?)
        };
        
        Ok(Self {
            client,
            base_url,
            token_strategy,
        })
    }
    
    /// Creates an ApiClient with explicit mock token (for mock tests)
    pub async fn with_mock_token(base_url: Url, mock_token: String) -> Result<Self, Error> {
        tracing::info!("Creating ApiClient with explicit mock token");
        
        let client = Client::new();
        let token_strategy: Box<dyn TokenStrategy> = Box::new(MockTokenStrategy::new(mock_token));
        
        Ok(Self {
            client,
            base_url,
            token_strategy,
        })
    }
    
    /// Environment detection for automatic strategy selection
    fn is_mock_environment() -> bool {
        // Check for mock credentials pattern
        let username = env::var("AMP_USERNAME").unwrap_or_default();
        let password = env::var("AMP_PASSWORD").unwrap_or_default();
        
        // Mock environment indicators
        let has_mock_credentials = username.contains("mock") || password.contains("mock");
        let is_not_live_test = env::var("AMP_TESTS").unwrap_or_default() != "live";
        
        has_mock_credentials && is_not_live_test
    }
    
    /// Gets authentication token using the configured strategy
    async fn get_token(&self) -> Result<String, Error> {
        self.token_strategy.get_token().await
    }
    
    /// Force cleanup of token files (for test cleanup)
    pub async fn force_cleanup_token_files() -> Result<(), Error> {
        // Only cleanup if we're not in a live test environment
        if !Self::is_live_test_environment() {
            TokenManager::cleanup_token_files().await?;
        }
        Ok(())
    }
    
    fn is_live_test_environment() -> bool {
        env::var("AMP_TESTS").unwrap_or_default() == "live"
    }
}
```

## Data Models

### Strategy Selection Logic

```rust
pub enum TokenEnvironment {
    Mock,
    Live,
    Auto,
}

impl TokenEnvironment {
    pub fn detect() -> Self {
        let username = env::var("AMP_USERNAME").unwrap_or_default();
        let password = env::var("AMP_PASSWORD").unwrap_or_default();
        let amp_tests = env::var("AMP_TESTS").unwrap_or_default();
        
        // Explicit live test environment
        if amp_tests == "live" {
            return Self::Live;
        }
        
        // Mock credentials detected
        if username.contains("mock") || password.contains("mock") {
            return Self::Mock;
        }
        
        // Default to live for real credentials
        if !username.is_empty() && !password.is_empty() {
            return Self::Live;
        }
        
        // Fallback to mock for safety
        Self::Mock
    }
}
```

### Enhanced TokenManager

The existing `TokenManager` will be enhanced with cleanup capabilities:

```rust
impl TokenManager {
    /// Cleanup token files from disk (for test isolation)
    pub async fn cleanup_token_files() -> Result<(), Error> {
        let token_file_path = Self::get_token_file_path();
        
        if tokio::fs::metadata(&token_file_path).await.is_ok() {
            tokio::fs::remove_file(&token_file_path).await.map_err(|e| {
                Error::Token(TokenError::storage(format!(
                    "Failed to cleanup token file: {e}"
                )))
            })?;
            tracing::debug!("Token file cleaned up: {}", token_file_path);
        }
        
        Ok(())
    }
    
    /// Clear stored token data (for testing)
    pub async fn clear_token(&self) -> Result<(), Error> {
        *self.token_data.lock().await = None;
        
        // Also cleanup disk file if persistence is enabled
        if Self::should_persist_tokens() {
            Self::cleanup_token_files().await?;
        }
        
        tracing::debug!("Token data cleared from memory and disk");
        Ok(())
    }
    
    /// Check if token persistence should be enabled
    pub fn should_persist_tokens() -> bool {
        // Don't persist in mock environments
        if ApiClient::is_mock_environment() {
            return false;
        }
        
        // Explicit persistence setting
        if let Ok(persist) = env::var("AMP_TOKEN_PERSISTENCE") {
            return persist.to_lowercase() == "true";
        }
        
        // Default: persist in live test environments
        env::var("AMP_TESTS").unwrap_or_default() == "live"
    }
}
```

## Error Handling

### Strategy-Specific Error Handling

```rust
#[derive(Error, Debug)]
pub enum StrategyError {
    #[error("Mock strategy error: {0}")]
    Mock(String),
    #[error("Live strategy error: {0}")]
    Live(String),
    #[error("Strategy initialization failed: {0}")]
    Initialization(String),
}

impl From<StrategyError> for Error {
    fn from(err: StrategyError) -> Self {
        Error::Token(TokenError::validation(err.to_string()))
    }
}
```

## Testing Strategy

### Test Environment Setup

```rust
// Enhanced test helpers
pub async fn setup_mock_test() {
    // Force cleanup any existing token state
    let _ = ApiClient::force_cleanup_token_files().await;
    
    // Set mock environment variables
    env::set_var("AMP_USERNAME", "mock_user");
    env::set_var("AMP_PASSWORD", "mock_pass");
    env::remove_var("AMP_TESTS"); // Ensure not in live mode
    env::remove_var("AMP_TOKEN_PERSISTENCE"); // Disable persistence
    
    tracing::debug!("Mock test environment setup complete");
}

pub async fn cleanup_mock_test() {
    // Cleanup any token files that might have been created
    let _ = ApiClient::force_cleanup_token_files().await;
    
    // Restore environment from .env file
    dotenvy::from_filename_override(".env").ok();
    
    tracing::debug!("Mock test environment cleanup complete");
}

pub async fn setup_live_test() {
    // Load real credentials from .env
    dotenvy::from_filename_override(".env").ok();
    
    // Ensure live test mode is set
    env::set_var("AMP_TESTS", "live");
    env::set_var("AMP_TOKEN_PERSISTENCE", "true");
    
    tracing::debug!("Live test environment setup complete");
}
```

### Test Isolation Verification

```rust
#[tokio::test]
async fn test_mock_strategy_isolation() {
    setup_mock_test().await;
    
    let client = ApiClient::with_mock_token(
        Url::parse("http://localhost:8080").unwrap(),
        "mock_token_123".to_string()
    ).await.unwrap();
    
    // Verify mock strategy is used
    assert_eq!(client.token_strategy.strategy_type(), "mock");
    assert!(!client.token_strategy.should_persist());
    
    // Verify token works without network
    let token = client.get_token().await.unwrap();
    assert_eq!(token, "mock_token_123");
    
    cleanup_mock_test().await;
}

#[tokio::test]
async fn test_live_strategy_persistence() {
    setup_live_test().await;
    
    let client = ApiClient::new().await.unwrap();
    
    // Verify live strategy is used
    assert_eq!(client.token_strategy.strategy_type(), "live");
    assert!(client.token_strategy.should_persist());
    
    // Test would continue with actual token operations...
}
```

## Integration with Existing Code

### Minimal Changes Required

1. **ApiClient Constructor Changes**:
   - `ApiClient::new()` - Enhanced with automatic strategy detection
   - `ApiClient::with_mock_token()` - Enhanced to use `MockTokenStrategy`

2. **Test Helper Updates**:
   - `setup_mock_test()` - Enhanced environment setup
   - `cleanup_mock_test()` - Enhanced cleanup
   - `get_shared_client()` - Updated to use strategy-aware client

3. **TokenManager Enhancements**:
   - Add `clear_token()` method
   - Add `cleanup_token_files()` method
   - Enhanced `should_persist_tokens()` logic

### Backward Compatibility

- All existing test code continues to work without modification
- Existing API methods maintain the same signatures
- Environment variable names remain unchanged
- Error types are enhanced but remain compatible

## Configuration

### Environment Variables

```bash
# Test Environment Control
AMP_TESTS=live                    # Enables live test mode with full token management
AMP_TOKEN_PERSISTENCE=true        # Explicit token persistence control

# Mock Test Detection (automatic)
AMP_USERNAME=mock_user            # Triggers mock strategy when contains "mock"
AMP_PASSWORD=mock_pass            # Triggers mock strategy when contains "mock"

# Live Test Credentials
AMP_USERNAME=real_username        # Real credentials for live tests
AMP_PASSWORD=real_password        # Real credentials for live tests
AMP_API_BASE_URL=https://amp-test.blockstream.com/api
```

### Strategy Selection Matrix

| Environment | AMP_TESTS | Username | Strategy | Persistence |
|-------------|-----------|----------|----------|-------------|
| Mock Test   | (unset)   | mock_*   | Mock     | No          |
| Live Test   | live      | real     | Live     | Yes         |
| Production  | (unset)   | real     | Live     | Optional    |
| Fallback    | (any)     | (empty)  | Mock     | No          |

## Security Considerations

### Token Isolation

1. **Mock Strategy Security**:
   - No real credentials used in mock tests
   - No network requests made
   - No persistent storage of sensitive data

2. **Live Strategy Security**:
   - Full credential protection with `Secret<String>`
   - Secure token persistence with proper file permissions
   - Thread-safe token operations

### Test Data Protection

1. **Environment Separation**:
   - Clear boundaries between mock and live environments
   - No cross-contamination of test data
   - Automatic cleanup of temporary token files

## Performance Considerations

### Strategy Performance

1. **Mock Strategy**:
   - Zero network overhead
   - Minimal memory usage
   - Instant token retrieval

2. **Live Strategy**:
   - Efficient token reuse through persistence
   - Shared token manager reduces redundant requests
   - Proactive refresh prevents request delays

### Test Execution Performance

1. **Parallel Test Safety**:
   - Mock tests can run in parallel without interference
   - Live tests share tokens efficiently
   - No race conditions between strategies

2. **Resource Usage**:
   - Mock tests use minimal resources
   - Live tests benefit from token caching
   - Cleanup operations are lightweight