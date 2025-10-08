# Design Document

## Overview

This design extends the AMP client library with 10 new API methods for comprehensive registered user and GAID management. The implementation follows the established patterns in the codebase for HTTP client operations, authentication, error handling, and testing. All new methods will be added to the `ApiClient` struct in `src/client.rs` and will include corresponding data models in `src/model.rs`, mock implementations in `src/mocks.rs`, and comprehensive test coverage.

## Architecture

### Client Methods Architecture

The new methods will follow the existing pattern established in the `ApiClient` implementation:

1. **Authentication**: All methods use the existing token management system via `get_token()` 
2. **HTTP Operations**: Utilize the existing `make_request()` helper method for consistent request handling
3. **Error Handling**: Return `Result<T, Error>` using the existing `Error` enum
4. **Retry Logic**: Inherit retry behavior from the underlying `RetryClient`
5. **Serialization**: Use serde for JSON serialization/deserialization

### Method Categories

The 10 new methods can be grouped into 4 functional categories:

1. **Registered User Management** (2 methods)
   - `edit_registered_user` - Update user information
   - `get_registered_user_summary` - Get comprehensive user data

2. **GAID Association Management** (4 methods)
   - `get_registered_user_gaids` - List user's GAIDs
   - `add_gaid_to_registered_user` - Associate GAID with user
   - `set_default_gaid_for_registered_user` - Set default GAID
   - `get_gaid_registered_user` - Lookup user by GAID

3. **Balance and Asset Queries** (2 methods)
   - `get_gaid_balance` - Get all asset balances for GAID
   - `get_gaid_asset_balance` - Get specific asset balance for GAID

4. **Category Management** (2 methods)
   - `add_categories_to_registered_user` - Associate categories with user
   - `remove_categories_from_registered_user` - Remove categories from user

## Components and Interfaces

### New Data Models

The following new data structures will be added to `src/model.rs`:

```rust
// Request body for editing registered users
#[derive(Debug, Serialize)]
pub struct RegisteredUserEdit {
    pub name: Option<String>,
}

// Request body for GAID operations
#[derive(Debug, Serialize)]
pub struct GaidRequest {
    pub gaid: String,
}

// Request body for category operations
#[derive(Debug, Serialize)]
pub struct CategoriesRequest {
    pub categories: Vec<i64>,
}

// Response types will reuse existing models:
// - RegisteredUserResponse for user data
// - Vec<String> for GAID lists
// - Balance-related types for balance queries
// - Standard success responses for operations
```

### API Client Method Signatures

All methods will be added to the `ApiClient` implementation with these signatures:

```rust
impl ApiClient {
    // Registered User Management
    pub async fn edit_registered_user(
        &self,
        registered_user_id: i64,
        edit_data: &RegisteredUserEdit,
    ) -> Result<RegisteredUserResponse, Error>;

    pub async fn get_registered_user_summary(
        &self,
        registered_user_id: i64,
    ) -> Result<RegisteredUserSummary, Error>;

    // GAID Association Management
    pub async fn get_registered_user_gaids(
        &self,
        registered_user_id: i64,
    ) -> Result<Vec<String>, Error>;

    pub async fn add_gaid_to_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error>;

    pub async fn set_default_gaid_for_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error>;

    pub async fn get_gaid_registered_user(
        &self,
        gaid: &str,
    ) -> Result<RegisteredUserResponse, Error>;

    // Balance and Asset Queries
    pub async fn get_gaid_balance(
        &self,
        gaid: &str,
    ) -> Result<Balance, Error>;

    pub async fn get_gaid_asset_balance(
        &self,
        gaid: &str,
        asset_uuid: &str,
    ) -> Result<Ownership, Error>;

    // Category Management
    pub async fn add_categories_to_registered_user(
        &self,
        registered_user_id: i64,
        categories: &[i64],
    ) -> Result<(), Error>;

    pub async fn remove_categories_from_registered_user(
        &self,
        registered_user_id: i64,
        categories: &[i64],
    ) -> Result<(), Error>;
}
```

### HTTP Request Patterns

Each method will follow these patterns based on the HTTP verb:

**GET Requests** (4 methods):
- Use `make_request` with `Method::GET`
- No request body
- Path parameters embedded in URL
- Return deserialized response data

**PUT Requests** (3 methods):
- Use `make_request` with `Method::PUT`
- JSON request body with appropriate data structure
- Path parameters embedded in URL
- Return deserialized response or unit type for operations

**POST Requests** (3 methods):
- Use `make_request` with `Method::POST`
- JSON request body with appropriate data structure
- Path parameters embedded in URL
- Return deserialized response or unit type for operations

## Data Models

### Request Models

```rust
#[derive(Debug, Serialize)]
pub struct RegisteredUserEdit {
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GaidRequest {
    pub gaid: String,
}

#[derive(Debug, Serialize)]
pub struct CategoriesRequest {
    pub categories: Vec<i64>,
}
```

### Response Models

The methods will reuse existing response models where appropriate:

- `RegisteredUserResponse` - For user data responses
- `RegisteredUserSummary` - For user summary data (already exists)
- `Balance` - For balance information (already exists)
- `Ownership` - For specific asset balance (already exists)
- `Vec<String>` - For GAID lists
- Unit type `()` - For operation confirmations

## Error Handling

### Error Types

All methods will use the existing `Error` enum and follow established error handling patterns:

1. **Network Errors**: Handled by the underlying `RetryClient` with exponential backoff
2. **Authentication Errors**: Automatic token refresh via existing token management
3. **HTTP Status Errors**: Mapped to appropriate `Error` variants
4. **Serialization Errors**: Handled via `ResponseParsingFailed` variant
5. **Validation Errors**: Returned as `RequestFailed` with descriptive messages

### Error Propagation

Each method will:
1. Use `?` operator for error propagation
2. Provide context-specific error messages
3. Maintain error chain for debugging
4. Follow existing patterns for error mapping

## Testing Strategy

### Mock Testing

Each method will have corresponding mock implementations in `src/mocks.rs`:

```rust
// Example mock function structure
pub fn mock_edit_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/api/registered_users/1/edit")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "name": "Updated User Name",
                "GAID": "mock_gaid",
                "is_company": false,
                "categories": [],
                "creator": 1
            }));
    });
}
```

### Test Structure

Each method will have two test functions following the established pattern:

1. **Mock Tests**: Fast, isolated tests using `httpmock`
   - Test function naming: `test_{method_name}_mock`
   - Use `setup_mock_test()` and `cleanup_mock_test()`
   - Verify request/response serialization
   - Test error conditions

2. **Live Tests**: Integration tests against real API
   - Test function naming: `test_{method_name}_live`
   - Require `AMP_TESTS=live` environment variable
   - Use real credentials from environment
   - Clean up created resources when possible
   - Skip if credentials not available

### Test Data Management

- **Mock Data**: Consistent test data across all mock tests
- **Live Data**: Use existing registered users and assets where possible
- **Specific Test GAID**: Live tests should create a registered user with GAID `GA44YYwPM8vuRMmjFL8i5kSqXhoTW2`
- **Balance Validation**: GAID balance tests should verify balance of `100000` satoshi (or `0.001` if expressed in whole tL-BTC)
- **Cleanup**: Implement cleanup for state-changing operations
- **Isolation**: Each test should be independent and not rely on other tests

### Test Coverage Areas

1. **Happy Path**: Successful operations with valid data
2. **Error Conditions**: Invalid IDs, missing resources, malformed data
3. **Authentication**: Token refresh scenarios
4. **Serialization**: Request/response data integrity
5. **Edge Cases**: Empty lists, null values, boundary conditions

## Implementation Approach

### Phase 1: Data Models
1. Add new request/response structures to `src/model.rs`
2. Implement serialization/deserialization
3. Add necessary imports and dependencies

### Phase 2: Client Methods
1. Implement each method in `ApiClient`
2. Follow existing patterns for HTTP operations
3. Add proper error handling and documentation
4. Ensure consistent parameter validation

### Phase 3: Mock Implementation
1. Add mock functions to `src/mocks.rs`
2. Create realistic test data
3. Handle various request scenarios
4. Support both success and error cases

### Phase 4: Test Implementation
1. Create mock tests for each method
2. Implement live tests with proper cleanup
3. Add edge case and error condition tests
4. Verify test isolation and independence

### Phase 5: Integration and Validation
1. Run full test suite (mock and live)
2. Verify API compatibility with live endpoints
3. Performance testing for retry logic
4. Documentation updates

## Integration Points

### Existing Code Dependencies

The new methods will integrate with existing components:

1. **Token Management**: Use existing `TokenManager` and `TokenStrategy`
2. **HTTP Client**: Leverage existing `RetryClient` and `make_request` helper
3. **Error Handling**: Extend existing `Error` enum if needed
4. **Serialization**: Use existing serde patterns and configurations
5. **Testing Infrastructure**: Utilize existing test utilities and patterns

### Backward Compatibility

The implementation will maintain full backward compatibility:
- No changes to existing method signatures
- No modifications to existing data structures
- Additive changes only to public API
- Existing tests will continue to pass

### Performance Considerations

1. **Request Efficiency**: Each method makes a single HTTP request
2. **Memory Usage**: Minimal additional memory overhead
3. **Connection Reuse**: Leverage existing HTTP client connection pooling
4. **Retry Logic**: Inherit existing exponential backoff strategy
5. **Token Caching**: Use existing token management for efficiency