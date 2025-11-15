# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

This is a Rust client library for the Blockstream AMP API. The library provides a comprehensive client for interacting with AMP (Asset Management Platform) endpoints, including asset issuance, management, user registration, and category operations.

## Common Development Commands

### Building
```bash
cargo build
cargo build --release
```

### Running Tests
```bash
# Run only mocked tests (default)
cargo test

# Run live tests against the actual AMP API (requires credentials)
AMP_USERNAME=... AMP_PASSWORD=... AMP_TESTS=live cargo test

# Run state-changing tests (ignored by default)
AMP_USERNAME=... AMP_PASSWORD=... AMP_TESTS=live cargo test -- --ignored

# Run a specific test
cargo test test_get_changelog_mock
```

### Code Quality
```bash
# Format code
cargo fmt

# Run clippy linter
cargo clippy -- -D warnings

# Check for compilation errors without building
cargo check
```

### Examples
```bash
# Run the changelog example
cargo run --example changelog
```

## Architecture

### Core Structure

- **`src/client.rs`**: Main API client implementation
  - `ApiClient` struct handles authentication, token management, and HTTP requests
  - Implements automatic token refresh with 1-day expiry
  - All API endpoints are implemented as async methods

- **`src/model.rs`**: Data models for requests and responses
  - Uses serde for serialization/deserialization
  - Includes security features with `secrecy` crate for sensitive data
  - Models follow the Blockstream AMP API specification

- **`src/mocks.rs`**: Mock server implementations for testing
  - Provides mock responses for all API endpoints
  - Used by the `_mock` test variants

- **`tests/api.rs`**: Integration tests
  - Each endpoint has both `_live` and `_mock` test variants
  - Live tests are skipped unless `AMP_TESTS=live` is set

### Environment Configuration

The client expects the following environment variables:
- `AMP_USERNAME`: Required for authentication
- `AMP_PASSWORD`: Required for authentication  
- `AMP_API_BASE_URL`: Optional, defaults to `https://amp-test.blockstream.com/api`

For testing:
- `AMP_TESTS=live`: Enable live API tests
- `DESTINATION_ADDRESS`: Required for asset issuance tests

Use dotenvy (preferred over sourcing .env files) to load environment variables to ensure working on testnet and avoid breaking other tests.

### API Client Pattern

The client follows a consistent pattern for all endpoints:
1. Authentication token is obtained/refreshed automatically
2. Requests are made with the token in the Authorization header
3. Responses are deserialized into strongly-typed structs
4. Errors are propagated with context via the custom Error enum

### Testing Strategy

All API functionality follows a dual testing approach:
1. **Mock tests** run by default and test client logic in isolation
2. **Live tests** verify compatibility with the actual AMP API

When adding new endpoints:
1. Add the data models in `src/model.rs`
2. Implement the endpoint method in `src/client.rs`
3. Create a mock function in `src/mocks.rs`
4. Write both `_mock` and `_live` tests in `tests/api.rs`

## Key Implementation Details

### Asset Registration
Registering an asset with the Blockstream Asset Registry ensures the asset's name and ticker appear in user wallets when the asset is issued and distributed.

- Method: `ApiClient::register_asset(asset_uuid)`
- When to use: After creating an asset in AMP and before distributing it to users
- Effect: Publishes the asset metadata so wallets can display human‑readable name/ticker alongside the asset ID
- Notes:
  - Requires valid AMP credentials (dotenvy is used to load `.env`)
  - Idempotent: calling on an already-registered asset returns a success message

Example:
```rust
use amp_rs::ApiClient;

#[tokio::main]
async fn main() {
    let client = ApiClient::new().unwrap();
    let asset_uuid = "your_asset_uuid";
    let response = client.register_asset(asset_uuid).await.unwrap();
    assert!(response.success);
}
```

### Terminology Note
The codebase uses "manager" terminology in alignment with the AMP API. In the context of this project:
- `manager_id` field remains unchanged
- API entities continue to use "manager" terminology
- User-facing code may refer to this role as "Issuer"

### Security Considerations
- Password and token fields use the `secrecy` crate for secure handling
- The `zeroize` crate ensures sensitive data is cleared from memory
- Authentication tokens are stored globally with mutex protection
