# Design Document

## Overview

This design implements a robust authentication token management system for the AMP API client library. The system enhances the existing basic token functionality with proactive refresh, comprehensive error handling, retry logic, and secure credential management. The design ensures continuous authentication without service interruption while maintaining thread safety and providing debugging utilities.

## Architecture

### Current State Analysis

The existing codebase already has basic token management with:
- Static token storage using `OnceCell<Arc<Mutex<Option<String>>>>`
- Basic token expiry tracking with `OnceCell<Arc<Mutex<Option<DateTime<Utc>>>>`
- Simple token acquisition via `obtain_amp_token()`
- Basic token retrieval with expiry check in `get_token()`

### Enhanced Architecture

The enhanced system will build upon the existing foundation with these improvements:

```rust
// Enhanced token storage with serialization support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub token: Secret<String>,
    pub expires_at: DateTime<Utc>,
    pub obtained_at: DateTime<Utc>,
}

// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub timeout_seconds: u64,
}

// Token manager with enhanced capabilities
pub struct TokenManager {
    token_data: Arc<Mutex<Option<TokenData>>>,
    retry_config: RetryConfig,
    client: reqwest::Client,
}
```

## Components and Interfaces

### 1. Token Storage Component

**Purpose**: Thread-safe storage of token data with serialization support

**Key Features**:
- Serializable `TokenData` struct for persistence between test runs
- Secure token storage using `Secret<String>` from secrecy crate
- Timestamp tracking for both expiry and acquisition times

**Interface**:
```rust
impl TokenData {
    pub fn new(token: String, expires_at: DateTime<Utc>) -> Self;
    pub fn is_expired(&self) -> bool;
    pub fn expires_soon(&self, threshold: Duration) -> bool;
    pub fn age(&self) -> Duration;
}
```

### 2. Retry Configuration Component

**Purpose**: Configurable retry behavior for different environments

**Key Features**:
- Environment variable configuration with sensible defaults
- Test environment optimizations
- Configurable timeouts and delays

**Interface**:
```rust
impl RetryConfig {
    pub fn from_env() -> Self;
    pub fn for_tests() -> Self;
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self;
}
```

### 3. Token Manager Component

**Purpose**: Central token lifecycle management with enhanced capabilities

**Key Features**:
- Proactive token refresh (5 minutes before expiry)
- Automatic fallback from refresh to obtain
- Comprehensive error handling with retry logic
- Debug and monitoring utilities

**Interface**:
```rust
impl TokenManager {
    pub fn new() -> Result<Self, Error>;
    pub fn with_config(config: RetryConfig) -> Result<Self, Error>;
    
    // Core token operations
    pub async fn get_token(&self) -> Result<String, Error>;
    pub async fn obtain_token(&self) -> Result<String, Error>;
    pub async fn refresh_token(&self) -> Result<String, Error>;
    
    // Utility operations
    pub async fn get_token_info(&self) -> Result<Option<TokenInfo>, Error>;
    pub async fn clear_token(&self) -> Result<(), Error>;
    pub async fn force_refresh(&self) -> Result<String, Error>;
}
```

### 4. Retry Client Component

**Purpose**: HTTP client with sophisticated retry logic

**Key Features**:
- Exponential backoff with jitter
- Rate limiting respect (429 responses)
- Configurable retry policies
- Request timeout enforcement

**Interface**:
```rust
pub struct RetryClient {
    client: reqwest::Client,
    config: RetryConfig,
}

impl RetryClient {
    pub fn new(config: RetryConfig) -> Self;
    pub async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T, Error>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, Error>> + Send,
        T: Send;
}
```

## Data Models

### TokenData Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    #[serde(with = "secret_serde")]
    pub token: Secret<String>,
    pub expires_at: DateTime<Utc>,
    pub obtained_at: DateTime<Utc>,
}

// Custom serialization module for Secret<String>
mod secret_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use secrecy::{ExposeSecret, Secret};
    
    pub fn serialize<S>(secret: &Secret<String>, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer;
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Secret<String>, D::Error>
    where D: Deserializer<'de>;
}
```

### TokenInfo Structure (for debugging)
```rust
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub expires_at: DateTime<Utc>,
    pub obtained_at: DateTime<Utc>,
    pub expires_in: Duration,
    pub age: Duration,
    pub is_expired: bool,
    pub expires_soon: bool,
}
```

### Enhanced Error Types
```rust
#[derive(Error, Debug)]
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
}
```

## Error Handling

### Retry Strategy

1. **Exponential Backoff**: Base delay doubles with each retry, capped at max delay
2. **Jitter**: Random variation added to prevent thundering herd
3. **Rate Limiting**: Respect 429 responses with appropriate delays
4. **Circuit Breaking**: Stop retrying after max attempts reached

### Error Classification

- **Retryable Errors**: Network timeouts, 5xx responses, 429 rate limiting
- **Non-Retryable Errors**: 4xx authentication errors (except 429), malformed requests
- **Fatal Errors**: Configuration errors, serialization failures

### Fallback Logic

```rust
async fn get_token_with_fallback(&self) -> Result<String, Error> {
    // 1. Check if token exists and is valid
    if let Some(token_data) = self.get_stored_token().await? {
        if !token_data.expires_soon(Duration::minutes(5)) {
            return Ok(token_data.token.expose_secret().clone());
        }
        
        // 2. Try refresh if token exists but expires soon
        if !token_data.is_expired() {
            match self.refresh_token_internal().await {
                Ok(token) => return Ok(token),
                Err(e) => tracing::warn!("Token refresh failed, falling back to obtain: {}", e),
            }
        }
    }
    
    // 3. Fallback to obtaining new token
    self.obtain_token_internal().await
}
```

## Testing Strategy

### Unit Tests

1. **Token Lifecycle Tests**:
   - Token creation and expiry calculation
   - Proactive refresh timing (5-minute threshold)
   - Serialization/deserialization of TokenData

2. **Retry Logic Tests**:
   - Exponential backoff behavior
   - Rate limiting respect
   - Max attempts enforcement
   - Timeout handling

3. **Error Handling Tests**:
   - Fallback from refresh to obtain
   - Non-retryable error handling
   - Configuration validation

### Integration Tests

1. **Mock Server Tests**:
   - Token obtain endpoint simulation
   - Token refresh endpoint simulation
   - Rate limiting simulation (429 responses)
   - Network failure simulation

2. **Concurrency Tests**:
   - Multiple threads accessing tokens simultaneously
   - Race condition prevention
   - Thread safety validation

### Test Configuration

```rust
impl RetryConfig {
    pub fn for_tests() -> Self {
        Self {
            max_attempts: 2,
            base_delay_ms: 100,
            max_delay_ms: 1000,
            timeout_seconds: 5,
        }
    }
}
```

## Integration with Existing ApiClient

### Minimal Changes to Public API

The existing `ApiClient` interface remains unchanged. Internal implementation will be enhanced:

```rust
impl ApiClient {
    // Enhanced constructor with token manager
    pub fn new() -> Result<Self, Error> {
        let token_manager = TokenManager::new()?;
        Ok(Self {
            client: Client::new(),
            base_url: get_amp_api_base_url()?,
            token_manager: Arc::new(token_manager),
        })
    }
    
    // Enhanced token retrieval (replaces get_token)
    async fn get_token(&self) -> Result<String, Error> {
        self.token_manager.get_token().await
    }
}
```

### Backward Compatibility

- Existing static token storage will be migrated to new TokenManager
- Environment variable names remain the same
- Public API methods maintain same signatures
- Error types are enhanced but remain compatible

## Configuration

### Environment Variables

```bash
# Authentication credentials (existing)
AMP_USERNAME=your_username
AMP_PASSWORD=your_password
AMP_API_BASE_URL=https://amp-test.blockstream.com/api

# Retry configuration (new)
API_RETRY_MAX_ATTEMPTS=3          # Default: 3
API_RETRY_BASE_DELAY_MS=1000      # Default: 1000ms
API_RETRY_MAX_DELAY_MS=30000      # Default: 30000ms
API_REQUEST_TIMEOUT_SECONDS=10    # Default: 10 seconds

# Test environment detection (new)
AMP_TESTS=live                    # Enables live API tests
```

### Default Configuration

```rust
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
```

## Security Considerations

### Credential Protection

1. **Memory Safety**: Use `Secret<String>` for all sensitive data
2. **Zeroization**: Implement `Zeroize` for custom types containing secrets
3. **Logging Safety**: Ensure secrets never appear in logs or debug output
4. **Serialization Safety**: Custom serde implementation for Secret types

### Token Storage

1. **Thread Safety**: All token access protected by Arc<Mutex>
2. **Atomic Operations**: Token updates are atomic to prevent partial states
3. **Secure Defaults**: Fail closed on configuration errors

## Performance Considerations

### Optimization Strategies

1. **Proactive Refresh**: Prevent authentication delays during API calls
2. **Connection Reuse**: Single HTTP client instance with connection pooling
3. **Minimal Locking**: Short-lived mutex locks to prevent contention
4. **Efficient Serialization**: Optimized serde implementation for TokenData

### Monitoring and Observability

```rust
// Metrics and logging integration
impl TokenManager {
    async fn get_token(&self) -> Result<String, Error> {
        let start = Instant::now();
        
        let result = self.get_token_internal().await;
        
        match &result {
            Ok(_) => {
                tracing::debug!("Token retrieved successfully in {:?}", start.elapsed());
                // Increment success metric
            }
            Err(e) => {
                tracing::error!("Token retrieval failed: {} (took {:?})", e, start.elapsed());
                // Increment error metric
            }
        }
        
        result
    }
}
```