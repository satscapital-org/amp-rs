# Design Document

## Overview

This design document outlines the implementation of three new methods for the AMP client library: `lock_manager`, `add_asset_to_manager`, and `get_asset_assignment`. These methods will extend the existing `ApiClient` struct with manager operations and asset assignment retrieval capabilities, following the established patterns for authentication, error handling, and testing.

## Architecture

The implementation will follow the existing client architecture:

- **Client Layer**: New methods added to `ApiClient` struct in `src/client.rs`
- **Model Layer**: Request/response structures added to `src/model.rs`
- **Mock Layer**: Mock implementations added to `src/mocks.rs`
- **Test Layer**: Comprehensive tests added to `tests/api.rs`

All methods will use the existing authentication mechanism via JWT tokens and follow the async/await patterns established in the codebase.

## Components and Interfaces

### 1. Lock Manager Method

**Method Signature:**
```rust
pub async fn lock_manager(&self, manager_id: i64) -> Result<(), Error>
```

**HTTP Request:**
- Method: PUT
- Endpoint: `/api/managers/{managerId}/lock`
- Authentication: JWT token via Authorization header
- Body: Empty

**Response:**
- Success: HTTP 200/204 with empty body
- Error: Standard error response with appropriate HTTP status codes

### 2. Add Asset to Manager Method

**Method Signature:**
```rust
pub async fn add_asset_to_manager(&self, manager_id: i64, asset_uuid: &str) -> Result<(), Error>
```

**HTTP Request:**
- Method: PUT
- Endpoint: `/api/managers/{managerId}/assets/{assetUuid}/add`
- Authentication: JWT token via Authorization header
- Body: Empty

**Response:**
- Success: HTTP 200/204 with empty body
- Error: Standard error response with appropriate HTTP status codes

### 3. Get Asset Assignment Method

**Method Signature:**
```rust
pub async fn get_asset_assignment(&self, asset_uuid: &str, assignment_id: &str) -> Result<Assignment, Error>
```

**HTTP Request:**
- Method: GET
- Endpoint: `/api/assets/{assetUuid}/assignments/{assignmentId}`
- Authentication: JWT token via Authorization header
- Body: None

**Response:**
- Success: HTTP 200 with `Assignment` JSON object
- Error: Standard error response with appropriate HTTP status codes

## Data Models

No new data models are required as the methods will use existing structures:

- `Assignment` struct (already exists in `model.rs`)
- Standard error responses handled by existing `Error` enum
- Empty responses for PUT operations

## Error Handling

All methods will use the existing error handling patterns:

1. **Network Errors**: Wrapped in `Error::Reqwest`
2. **Authentication Errors**: Handled by existing token management
3. **HTTP Status Errors**: Converted to appropriate `Error::RequestFailed` variants
4. **JSON Parsing Errors**: Wrapped in `Error::ResponseParsingFailed`

Error handling will follow the established pattern:
```rust
let response = self.make_authenticated_request(method, &url, body).await?;
if !response.status().is_success() {
    return Err(Error::RequestFailed(format!("Operation failed: {}", response.status())));
}
```

## Testing Strategy

### Mock Tests

Each method will have corresponding mock implementations in `src/mocks.rs`:

1. **`mock_lock_manager`**: Simulates successful manager locking
2. **`mock_add_asset_to_manager`**: Simulates successful asset authorization
3. **`mock_get_asset_assignment`**: Returns mock assignment data

Mock tests will:
- Use `httpmock` for HTTP simulation
- Test success and error scenarios
- Verify correct request formatting
- Validate response parsing

### Live Tests

Live tests will follow the established patterns with proper setup and cleanup:

#### Lock Manager Tests
- Create a test manager using existing `create_manager` method
- Perform lock operation
- Verify manager is locked (if verification endpoint exists)
- Clean up by deleting the manager

#### Add Asset to Manager Tests
- Create a test manager using existing `create_manager` method
- Use preserved test asset "Test Environment Asset" (as defined in `cleanup_resources.rs`) or fallback to first available asset
- Perform asset authorization
- Verify authorization (if verification endpoint exists)
- Clean up by deleting the manager

#### Get Asset Assignment Tests
- Use the `create_asset_assignment` workflow for setup:
  - Get or create registered user
  - Get or create category
  - Add user to category
  - Get or reuse existing asset
  - Add asset to category
  - Create asset assignment
- Retrieve the assignment using new method
- Verify assignment data matches created assignment
- Use `create_asset_assignment` cleanup:
  - Delete created assignments
  - Leave reusable resources (users, assets, categories) for other tests

### Test Isolation and Cleanup

Following the established patterns:
- Tests marked with `#[ignore]` for live API interaction
- Proper cleanup to maintain test isolation
- Reuse of existing resources where appropriate
- Environment variable checks for live test execution

## Implementation Details

### URL Construction

URLs will be constructed using the existing pattern:
```rust
let mut url = self.base_url.clone();
url.path_segments_mut()
    .unwrap()
    .extend(&["api", "managers", &manager_id.to_string(), "lock"]);
```

### Request Execution

All methods will use the existing `make_authenticated_request` helper:
```rust
let response = self.make_authenticated_request(Method::PUT, &url, None::<()>).await?;
```

### Response Handling

- **Empty responses**: Check status code only
- **JSON responses**: Use existing deserialization patterns with `Assignment` struct

### Authentication

All methods will leverage the existing token management system:
- Automatic token acquisition and refresh
- Proper error handling for authentication failures
- Token strategy support (mock vs live)

## Integration Points

The new methods will integrate seamlessly with existing functionality:

1. **Manager Operations**: Complement existing manager creation/deletion methods
2. **Asset Management**: Extend asset-related operations
3. **Assignment System**: Provide read access to complement existing assignment creation
4. **Testing Framework**: Follow established mock and live testing patterns

### Test Environment Resources

The implementation will leverage preserved test environment resources as defined in `cleanup_resources.rs`:
- **Test Asset**: "Test Environment Asset" - preserved for testing purposes
- **Protected Users**: IDs 1194, 1203 - available for assignment testing
- **Test Category**: "Test Environment Category" - available for categorization

## Performance Considerations

- Methods will use existing retry logic and timeout configurations
- No additional caching required as operations are typically infrequent
- Leverage existing connection pooling and HTTP client optimizations

## Security Considerations

- All methods require authentication via JWT tokens
- No sensitive data exposure in logs (following existing patterns)
- Proper error message sanitization to avoid information leakage
- Input validation for manager IDs and UUIDs