# Design Document

## Overview

This design document outlines the implementation of asset registry registration functionality for the AMP Rust client library. The feature adds a `register_asset` method to the `ApiClient` that allows users to register assets with the Blockstream Asset Registry through a simple API call. The implementation follows established patterns in the codebase for authentication, error handling, retry logic, and testing.

## Architecture

### High-Level Flow

```
User Code
    ↓
ApiClient::register_asset(&self, asset_uuid: &str)
    ↓
request_json(Method::POST, ["assets", asset_uuid, "register"], None)
    ↓
request_raw() - handles authentication, retries, timeouts
    ↓
TokenStrategy::get_token() - automatic token management
    ↓
HTTP POST to /api/assets/{assetUuid}/register
    ↓
Response deserialization to RegisterAssetResponse
    ↓
Result<RegisterAssetResponse, Error>
```

### Integration Points

1. **ApiClient**: New public method `register_asset` added to the existing client
2. **Model Module**: New `RegisterAssetResponse` struct for response deserialization
3. **Mocks Module**: New `mock_register_asset` function for testing
4. **Tests Module**: New mock-based tests in `tests/api.rs`

## Components and Interfaces

### 1. RegisterAssetResponse Struct

**Location**: `src/model.rs`

**Purpose**: Represents the API response from the asset registration endpoint

**Structure**:
```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterAssetResponse {
    pub success: bool,
    pub message: Option<String>,
    pub asset_id: String,
}
```

**Fields**:
- `success`: Boolean indicating whether the registration was successful
- `message`: Optional status message from the API (e.g., "Asset registered successfully" or error details)
- `asset_id`: The registered asset identifier (hex string)

**Traits**: Implements `Debug`, `Deserialize`, and `Serialize` for standard Rust patterns

### 2. ApiClient::register_asset Method

**Location**: `src/client.rs`

**Signature**:
```rust
pub async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error>
```

**Parameters**:
- `&self`: Reference to the ApiClient instance (provides access to HTTP client, base URL, and token strategy)
- `asset_uuid`: String slice containing the UUID of the asset to register

**Return Type**: `Result<RegisterAssetResponse, Error>`
- Success: Returns `RegisterAssetResponse` with registration details
- Failure: Returns `Error` enum variant (RequestFailed, ResponseParsingFailed, Token errors, etc.)

**Implementation Pattern**:
```rust
pub async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error> {
    self.request_json(
        Method::POST,
        &["assets", asset_uuid, "register"],
        None::<&()>
    )
    .await
}
```

**Key Design Decisions**:
- Uses existing `request_json` helper method for consistency
- No request body required (None::<&()>)
- Path segments: `["assets", asset_uuid, "register"]` → `/api/assets/{asset_uuid}/register`
- Leverages all existing infrastructure (authentication, retries, timeouts, error handling)

### 3. Mock Function

**Location**: `src/mocks.rs`

**Signature**:
```rust
pub fn mock_register_asset(server: &MockServer)
```

**Purpose**: Provides HTTP mock for testing without live API calls

**Implementation**:
```rust
pub fn mock_register_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/assets/mock_asset_uuid/register");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "success": true,
                "message": "Asset registered successfully with Blockstream Asset Registry",
                "asset_id": "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
            }));
    });
}
```

**Mock Response Details**:
- Uses realistic asset_id from liquidtestnet.com (L-BTC asset ID)
- Success message matches expected API response format
- Matches path pattern with mock_asset_uuid for consistency with other tests

## Data Models

### RegisterAssetResponse

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterAssetResponse {
    /// Indicates whether the registration was successful
    pub success: bool,
    
    /// Optional message providing additional context about the registration
    /// Examples: "Asset registered successfully", "Asset already registered"
    pub message: Option<String>,
    
    /// The asset identifier (hex string) that was registered
    /// Example: "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
    pub asset_id: String,
}
```

**Serialization Format** (JSON):
```json
{
  "success": true,
  "message": "Asset registered successfully with Blockstream Asset Registry",
  "asset_id": "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
}
```

## Error Handling

### Error Flow

The `register_asset` method leverages the existing error handling infrastructure:

1. **Authentication Errors**: Handled by `TokenStrategy::get_token()`
   - Token loading failures
   - Token refresh failures
   - Token expiration

2. **Network Errors**: Handled by `request_raw()` with automatic retries
   - Connection failures (3 retries with exponential backoff)
   - Timeout errors (60-second timeout)
   - Transient network issues

3. **HTTP Errors**: Handled by `request_raw()` status code checking
   - 4xx client errors (invalid asset UUID, unauthorized, etc.)
   - 5xx server errors (API unavailable, internal errors)
   - Returns `Error::RequestFailed` with status code and error text

4. **Deserialization Errors**: Handled by `request_json()`
   - Invalid JSON response
   - Missing required fields
   - Type mismatches
   - Returns `Error::ResponseParsingFailed`

### Error Types

All errors are returned as the existing `Error` enum:

```rust
pub enum Error {
    RequestFailed(String),           // HTTP errors, API errors
    ResponseParsingFailed(String),   // JSON deserialization errors
    Token(TokenError),                // Authentication errors
    Reqwest(reqwest::Error),         // Low-level network errors
    // ... other variants
}
```

### Retry Logic

Inherited from `request_raw()`:
- **Max Retries**: 3 attempts
- **Retry Conditions**: Network/connection errors only (not client errors)
- **Backoff Strategy**: Linear backoff (1s, 2s, 3s)
- **Timeout**: 60 seconds per request

## Testing Strategy

### Mock-Only Testing Approach

**Critical Requirement**: All tests MUST use mocks to avoid live API calls to the Blockstream testnet.

**Rationale**:
- Minimize load on service provider infrastructure
- Maintain good business relationship with Blockstream
- Faster test execution
- Deterministic test results
- No dependency on external service availability

### Test Coverage

#### 1. Success Case Test

**Test Name**: `test_register_asset_mock`

**Purpose**: Verify successful asset registration flow

**Setup**:
- Mock server with `mock_register_asset()`
- ApiClient with mock token
- Asset UUID: "mock_asset_uuid"

**Assertions**:
- Response is Ok
- `success` field is true
- `asset_id` matches expected value
- `message` is present and contains success text

#### 2. Error Handling Tests

**Test Name**: `test_register_asset_not_found_mock`

**Purpose**: Verify handling of non-existent asset UUID

**Setup**:
- Mock server returning 404 status
- Error message: "Asset not found"

**Assertions**:
- Response is Err
- Error variant is `Error::RequestFailed`
- Error message contains "404" and "Asset not found"

**Test Name**: `test_register_asset_already_registered_mock`

**Purpose**: Verify handling of already-registered assets

**Setup**:
- Mock server returning 200 status
- Response with `success: true` and message indicating already registered

**Assertions**:
- Response is Ok (not an error condition)
- `success` field is true
- `message` indicates asset was already registered

**Test Name**: `test_register_asset_server_error_mock`

**Purpose**: Verify handling of server errors

**Setup**:
- Mock server returning 500 status
- Error message: "Internal server error"

**Assertions**:
- Response is Err
- Error variant is `Error::RequestFailed`
- Error message contains "500"

#### 3. Authentication Test

**Test Name**: `test_register_asset_authentication_mock`

**Purpose**: Verify that authentication token is included in request

**Setup**:
- Mock server with header verification
- Checks for "Authorization: token mock_token" header

**Assertions**:
- Request includes correct authorization header
- Response is successful

### Test Implementation Pattern

All tests follow this structure:

```rust
#[tokio::test]
async fn test_register_asset_mock() {
    // Setup mock environment
    setup_mock_test().await;
    
    // Create mock server and configure mocks
    let server = MockServer::start();
    mocks::mock_register_asset(&server);
    
    // Create client with mock token
    let client = ApiClient::with_mock_token(
        Url::parse(&server.base_url()).unwrap(),
        "mock_token".to_string(),
    )
    .unwrap();
    
    // Execute test
    let result = client.register_asset("mock_asset_uuid").await;
    
    // Assertions
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.success);
    assert_eq!(
        response.asset_id,
        "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
    );
    
    // Cleanup
    dotenvy::from_filename_override(".env").ok();
}
```

### Test Data

**Asset Domain**: All test references use "liquidtestnet.com" as specified in requirements

**Mock Asset IDs**: Use realistic hex strings from liquidtestnet:
- L-BTC: `6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d`

**Mock Asset UUIDs**: Use consistent "mock_asset_uuid" for test predictability

## Implementation Notes

### Code Location Summary

1. **Model Addition** (`src/model.rs`):
   - Add `RegisterAssetResponse` struct after existing response types
   - Place near other asset-related response structures (e.g., after `IssuanceResponse`)

2. **Client Method** (`src/client.rs`):
   - Add `register_asset` method in the asset operations section
   - Place after `edit_asset` and before `delete_asset` for logical grouping
   - Add comprehensive documentation with examples

3. **Mock Function** (`src/mocks.rs`):
   - Add `mock_register_asset` function after other asset-related mocks
   - Follow existing naming convention (mock_<operation>_<resource>)

4. **Tests** (`tests/api.rs`):
   - Add test functions in the asset operations test section
   - Group with other asset-related tests
   - Follow existing test naming convention (test_<operation>_<resource>_mock)

### Documentation Requirements

Each component must include:

1. **Struct Documentation**:
   - Purpose and usage
   - Field descriptions
   - Example JSON representation

2. **Method Documentation**:
   - Purpose and behavior
   - Parameter descriptions
   - Return value description
   - Error conditions
   - Usage example with code

3. **Test Documentation**:
   - Test purpose
   - What is being verified
   - Expected behavior

### Consistency with Existing Patterns

The implementation maintains consistency with existing codebase patterns:

1. **Method Signature**: Matches other simple API methods (e.g., `get_asset`, `delete_asset`)
2. **Error Handling**: Uses existing `Error` enum, no new error types needed
3. **Authentication**: Leverages existing `TokenStrategy` infrastructure
4. **Retry Logic**: Inherits from `request_raw()` implementation
5. **Testing**: Follows established mock-based testing pattern
6. **Naming**: Follows Rust naming conventions (snake_case for functions, PascalCase for types)

## Security Considerations

1. **Token Management**: Uses existing secure token handling with `Secret<String>` wrapper
2. **Input Validation**: Asset UUID validation handled by API (returns 404 for invalid UUIDs)
3. **No Sensitive Data**: Response contains only public asset information
4. **HTTPS**: Assumes HTTPS transport (handled by reqwest client configuration)

## Performance Considerations

1. **Async Operation**: Non-blocking async/await pattern
2. **Connection Reuse**: Leverages existing reqwest client connection pooling
3. **Timeout**: 60-second timeout prevents indefinite hangs
4. **Retry Overhead**: Maximum 3 attempts with linear backoff (worst case: ~6 seconds additional delay)
5. **Token Caching**: Automatic token reuse via `TokenStrategy` (no unnecessary token refreshes)

## Future Enhancements

Potential future improvements (not in scope for this implementation):

1. **Batch Registration**: Register multiple assets in a single API call
2. **Registration Status Check**: Query registration status of an asset
3. **Unregister Operation**: Remove asset from registry
4. **Registry Metadata**: Retrieve additional registry information
5. **Validation**: Client-side UUID format validation before API call
