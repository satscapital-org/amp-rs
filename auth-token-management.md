## AMP API Token Management Paradigm

### Token Lifecycle

1. **Initial Token Duration: 1 Day**
   - When a token is obtained via `obtain_amp_token()`, it's set to expire after 24 hours
   - Expiry is tracked using `chrono::Duration::days(1)`

2. **Proactive Token Refresh: 5 Minutes Before Expiry**
   - The system checks if the token expires within the next 5 minutes
   - If so, it automatically refreshes the token before it expires
   - This prevents authentication failures during API calls

### Token Renewal Conditions

The AMP token is renewed under these conditions:

1. **No Token Exists**: When `get_amp_token()` is called and no token is stored
2. **Token Expires Soon**: When the token will expire in less than 5 minutes
3. **Token Already Expired**: If the expiry time has passed, the system obtains a new token instead of refreshing
4. **Manual Refresh**: Can be triggered via the `/refresh-token` endpoint

### Token Management Architecture

```rust
// Thread-safe token storage using Tokio's OnceCell and Arc<Mutex>
static AMP_TOKEN: OnceCell<Arc<Mutex<Option<String>>>> = OnceCell::const_new();
static AMP_TOKEN_EXPIRY: OnceCell<Arc<Mutex<Option<DateTime<Utc>>>>> = OnceCell::const_new();
```
### Key Token Management Methods

1. **`obtain_amp_token()`**
   - Authenticates using username/password from environment variables
   - Makes POST to `/user/obtain_token`
   - Stores token with 1-day expiry

2. **`refresh_amp_token()`**
   - Uses existing token to get a new one via GET `/user/refresh-token`
   - Falls back to `obtain_amp_token()` if current token is expired
   - Updates stored token and expiry

3. **`get_amp_token()`**
   - Primary method used by all API calls
   - Automatically handles token lifecycle:
```rust
     if expires_at <= five_minutes_from_now {
         return refresh_amp_token().await;
     }
```
4. **`get_amp_token_info()`**
   - Returns both token and expiry information
   - Useful for debugging and monitoring

5. **`clear_amp_token()`**
   - Testing utility to reset token state

### Retry and Error Handling

The implementation includes sophisticated retry logic:

1. **Retry Client Configuration**:
   - Uses `RetryClient` with AMP-specific retry policy
   - Handles 429 (Too Many Requests) responses
   - Configurable via environment variables:
     - `API_RETRY_MAX_ATTEMPTS` (default: 3)
     - `API_RETRY_BASE_DELAY_MS` (default: 1000ms)
     - `API_RETRY_MAX_DELAY_MS` (default: 30000ms)

2. **Test Environment Optimizations**:
   - Shorter timeouts in tests (2 attempts, 500ms base delay)
   - 10-second request timeout to prevent hanging

### Best Practices for Implementation

When implementing similar token management in another project:

1. **Use Thread-Safe Storage**: The `Arc<Mutex<Option<T>>>` pattern ensures safe concurrent access

2. **Implement Proactive Refresh**: Check expiry before making API calls, not after failures

3. **Track Expiry Explicitly**: Store expiry time alongside the token for precise management

4. **Provide Graceful Fallbacks**: If refresh fails on expired token, obtain a new one

5. **Environment-Based Configuration**: Use environment variables for credentials and retry settings

6. **Debug Utilities**: Include methods to check token status and clear tokens for testing

7. **Integrate with Retry Logic**: Combine token management with retry mechanisms for resilience

This token management system ensures continuous authentication with minimal downtime and automatic recovery from token expiration scenarios.
