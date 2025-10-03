# AMP Client

A Rust client for the Blockstream AMP API.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
amp_client = "0.1.0"
```

## Examples

For more detailed examples, please refer to the [crate documentation](https://docs.rs/amp-client).

### Get a registered user

```rust
use amp_client::client::ApiClient;

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
use amp_client::client::ApiClient;

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
use amp_client::client::ApiClient;
use amp_client::model::CategoryAdd;

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
