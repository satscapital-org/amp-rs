# AMP Client

> **⚠️ DEVELOPMENT STATUS**: This package is currently in development and undergoing early integration testing. The API may change and some endpoints are not yet implemented. Don't use in production environments.

A Rust client for the Blockstream AMP API.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
amp_client = "0.0.1"
```

## Examples

For more detailed examples, please refer to the [crate documentation](https://docs.rs/amp-client).

### Running Examples

You can run the included examples using cargo:

```bash
# Show a summary of all assets issued by your credentials
cargo run --example asset_summary

# View the API changelog
cargo run --example changelog
```

Make sure to set up your `.env` file with the required credentials first.

### Get a registered user

```rust
use amp_rs::ApiClient;

#[tokio::main]
async fn main() {
    let client = ApiClient::new().unwrap();
    let users = client.get_registered_users().await.unwrap();
    let user_id = users.first().unwrap().id;
    let user = client.get_registered_user(user_id).await.unwrap();
    println!("{:?}", user);
}
```

### Get an asset

```rust
use amp_rs::ApiClient;

#[tokio::main]
async fn main() {
    let client = ApiClient::new().unwrap();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets.first().unwrap().asset_uuid.clone();
    let asset = client.get_asset(&asset_uuid).await.unwrap();
    println!("{:?}", asset);
}
```

### Create a category

```rust
use amp_rs::{ApiClient, model::CategoryAdd};

#[tokio::main]
async fn main() {
    let client = ApiClient::new().unwrap();
    let new_category = CategoryAdd {
        name: "Test Category".to_string(),
        description: Some("A test category".to_string()),
    };
    let category = client.add_category(&new_category).await.unwrap();
    println!("{:?}", category);
}
```

### Manage asset assignments

```rust
use amp_rs::ApiClient;

#[tokio::main]
async fn main() {
    let client = ApiClient::new().unwrap();
    let asset_uuid = "your_asset_uuid";
    let assignment_id = "assignment_id";

    // Lock an assignment
    let locked_assignment = client
        .lock_asset_assignment(asset_uuid, assignment_id)
        .await
        .unwrap();
    println!("Locked assignment: {:?}", locked_assignment);

    // Unlock an assignment
    let unlocked_assignment = client
        .unlock_asset_assignment(asset_uuid, assignment_id)
        .await
        .unwrap();
    println!("Unlocked assignment: {:?}", unlocked_assignment);

    // Delete an assignment (destructive operation)
    client
        .delete_asset_assignment(asset_uuid, assignment_id)
        .await
        .unwrap();
    println!("Assignment deleted");
}
```



## Missing Endpoints

The following AMP API endpoints are not yet implemented in this client library. This list may not be exhaustive:

### Asset Operations
- `POST /api/assets/{assetUuid}/reissue-request` - Request asset reissuance
- `POST /api/assets/{assetUuid}/reissue-confirm` - Confirm asset reissuance
- `POST /api/assets/{assetUuid}/burn-request` - Request asset burn
- `POST /api/assets/{assetUuid}/burn-confirm` - Confirm asset burn
- `GET /api/assets/{assetUuid}/reissuances` - Get asset reissuances
- `GET /api/assets/{assetUuid}/distributions/create/` - Create asset distribution
- `POST /api/assets/{assetUuid}/distributions/{distributionUuid}/confirm` - Confirm distribution
- `DELETE /api/assets/{assetUuid}/distributions/{distributionUuid}/cancel` - Cancel distribution
- `GET /api/assets/{assetUuid}/distributions` - Get asset distributions
- `GET /api/assets/{assetUuid}/distributions/{distributionUuid}` - Get specific distribution
- `GET /api/assets/{assetUuid}/txs` - Get asset transactions
- `GET /api/assets/{assetUuid}/txs/{txid}` - Get specific asset transaction
- `GET /api/assets/{assetUuid}/lost-outputs` - Get asset lost outputs
- `POST /api/assets/{assetUuid}/update-blinders` - Update asset blinders

### Manager Operations
- `POST /api/managers/{managerId}/change-password` - Change manager password

These and potentially other endpoints will be added in future releases. If you need any of these endpoints urgently, please open an issue on the project repository.

## Token Management

The AMP client includes sophisticated token management with automatic persistence and refresh capabilities.

### Features

- **Automatic Token Persistence**: Tokens are automatically saved to `token.json` and loaded on subsequent runs
- **Proactive Refresh**: Tokens are automatically refreshed 5 minutes before expiry
- **Thread-Safe Operations**: All token operations are thread-safe and prevent race conditions
- **Retry Logic**: Built-in retry logic with exponential backoff for token operations
- **Secure Storage**: Tokens are stored securely using the `secrecy` crate

### Token Lifecycle

1. **First Run**: Client obtains a new token from the API and persists it to disk
2. **Subsequent Runs**: Client loads the existing token from disk if still valid
3. **Automatic Refresh**: Token is automatically refreshed when it expires soon (within 5 minutes)
4. **Fallback**: If refresh fails, client automatically obtains a new token

### Usage Examples

```rust
use amp_rs::ApiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ApiClient::new()?;

    // Get token (automatically handles persistence and refresh)
    let token = client.get_token().await?;
    println!("Token: {}...", &token[..20]);

    // Get token information
    if let Some(info) = client.get_token_info().await? {
        println!("Token expires at: {}", info.expires_at);
        println!("Token age: {:?}", info.age);
        println!("Expires in: {:?}", info.expires_in);
    }

    // Force refresh token
    let refreshed_token = client.force_refresh().await?;
    println!("Refreshed token: {}...", &refreshed_token[..20]);

    // Clear token (useful for testing)
    client.clear_token().await?;

    Ok(())
}
```

### Token Persistence Configuration

Token persistence is automatically enabled in the following scenarios:
- When `AMP_TESTS=live` (for live API testing)
- When `AMP_TOKEN_PERSISTENCE=true` is set
- In test environments (`cfg!(test)`)

The token file (`token.json`) contains:
```json
{
  "token": "your_jwt_token_here",
  "expires_at": "2024-01-01T12:00:00Z",
  "obtained_at": "2024-01-01T11:00:00Z"
}
```

## Configuration

### Environment Variables

The client can be configured using the following environment variables:

#### Authentication (Required for live tests)
- `AMP_USERNAME`: Username for AMP API authentication
- `AMP_PASSWORD`: Password for AMP API authentication
- `AMP_API_BASE_URL`: Base URL for the AMP API (default: `https://amp-test.blockstream.com/api`)

#### Retry Configuration (Optional)
- `API_RETRY_MAX_ATTEMPTS`: Maximum number of retry attempts (default: 3)
- `API_RETRY_BASE_DELAY_MS`: Base delay between retries in milliseconds (default: 1000)
- `API_RETRY_MAX_DELAY_MS`: Maximum delay between retries in milliseconds (default: 30000)
- `API_REQUEST_TIMEOUT_SECONDS`: Request timeout in seconds (default: 10)

#### Test Configuration
- `AMP_TESTS`: Set to `live` to run tests against the actual API

#### Token Persistence (Optional)
- `AMP_TOKEN_PERSISTENCE`: Set to `true` to enable token persistence to disk (default: enabled for live tests)

### Example Configuration

```bash
# Authentication
export AMP_USERNAME=your_username
export AMP_PASSWORD=your_password
export AMP_API_BASE_URL=https://amp-test.blockstream.com/api

# Retry configuration (optional)
export API_RETRY_MAX_ATTEMPTS=5
export API_RETRY_BASE_DELAY_MS=2000
export API_RETRY_MAX_DELAY_MS=60000
export API_REQUEST_TIMEOUT_SECONDS=30

# Enable live tests
export AMP_TESTS=live

# Enable token persistence (optional)
export AMP_TOKEN_PERSISTENCE=true
```

## Testing

To run the tests, you will need to set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.

```
AMP_USERNAME=... AMP_PASSWORD=... cargo test
```

To run the live tests, you will also need to set the `AMP_TESTS` environment variable to `live`.

```
AMP_USERNAME=... AMP_PASSWORD=... AMP_TESTS=live cargo test
```

Some tests that perform state-changing operations are ignored by default. To run them, use the `--ignored` flag.

```
AMP_USERNAME=... AMP_PASSWORD=... AMP_TESTS=live cargo test -- --ignored
```
