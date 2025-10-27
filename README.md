# Rust Amp Client (amp-rust)

> **⚠️ DEVELOPMENT STATUS**: This package is currently in development and undergoing early integration testing. The API may change and some endpoints are not yet implemented. Don't use in production environments.

A Rust client for the Blockstream AMP API.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
amp-rust = "0.0.2"
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

# Validate a GAID (Global Asset ID)
cargo run --example validate_gaid GAbYScu6jkWUND2jo3L4KJxyvo55d

# Get information about a specific distribution
cargo run --example get_distribution_info asset-uuid-123 distribution-uuid-456

# Create, issue, and authorize a new asset for distribution tests (requires live API)
AMP_TESTS=live cargo run --example create_issue_authorize_asset

# Run end-to-end asset distribution workflow with specific asset and user (requires live API)
cargo run --example end_to_end_distribution_example
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

## Signer Setup and Usage

The AMP client includes a comprehensive signer implementation for handling asset operations like distribution, reissuance, and burning. The `LwkSoftwareSigner` provides testnet-focused transaction signing using Blockstream's Liquid Wallet Kit (LWK).

### ⚠️ Security Warning

**TESTNET/REGTEST ONLY**: The `LwkSoftwareSigner` is designed exclusively for testnet and regtest environments. It stores mnemonic phrases in plain text and should NEVER be used in production or with real funds.

### Basic Signer Setup

#### Creating a Signer from Existing Mnemonic

```rust
use amp_rs::signer::{LwkSoftwareSigner, Signer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create signer from existing mnemonic
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let signer = LwkSoftwareSigner::new(mnemonic)?;

    // Verify testnet configuration
    assert!(signer.is_testnet());
    println!("Signer ready for testnet operations");

    Ok(())
}
```

#### Generating a New Signer

```rust
use amp_rs::signer::LwkSoftwareSigner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate new signer with automatic mnemonic management
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;

    println!("Generated mnemonic: {}...", &mnemonic[..50]);
    println!("Mnemonic saved to mnemonic.local.json");

    // Signer is ready for use
    assert!(signer.is_testnet());

    Ok(())
}
```

#### Indexed Mnemonic Access for Testing

For test isolation and consistent test environments, use indexed mnemonic access:

```rust
use amp_rs::signer::LwkSoftwareSigner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate signers with specific indices for different test roles
    let (_, issuer_signer) = LwkSoftwareSigner::generate_new_indexed(100)?;
    let (_, distributor_signer) = LwkSoftwareSigner::generate_new_indexed(101)?;
    let (_, user_signer) = LwkSoftwareSigner::generate_new_indexed(102)?;

    // Each signer uses a different mnemonic for test isolation
    println!("Created role-based signers for testing");

    Ok(())
}
```

### Generating Addresses for Asset Issuance

Before issuing assets, you need to generate addresses that can receive the issued assets:

```rust
use amp_rs::signer::LwkSoftwareSigner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create or load signer
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;

    // Generate a receiving address for asset issuance
    let treasury_address = signer.derive_address(0, 0)?; // First receiving address
    println!("Treasury address: {}", treasury_address);

    // This address can be used as the treasury address for asset operations
    // and should be added to your asset's treasury addresses via the API

    Ok(())
}
```

### Using Signer with Asset Distribution

The signer integrates seamlessly with the `distribute_asset` method and will be essential for future burn and reissuance operations:

```rust
use amp_rs::{ApiClient, ElementsRpc, signer::LwkSoftwareSigner, model::AssetDistributionAssignment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup API client and Elements RPC
    let api_client = ApiClient::new().await?;
    let elements_rpc = ElementsRpc::from_env()?;

    // Create signer for signing transactions
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(300)?;
    println!("Using signer with mnemonic: {}...", &mnemonic[..50]);

    // Setup wallet and distribution assignments
    let wallet_name = "amp_distribution_wallet".to_string();
    let asset_uuid = "your-asset-uuid";

    let assignments = vec![AssetDistributionAssignment {
        user_id: "user123".to_string(),
        address: "tlq1qq...".to_string(), // User's receiving address
        amount: 0.00000001, // Amount in BTC units
    }];

    // Execute distribution with signer
    api_client.distribute_asset(
        asset_uuid,
        assignments,
        &elements_rpc,
        &wallet_name,
        &signer, // Signer handles transaction signing
    ).await?;

    println!("Asset distribution completed successfully");

    Ok(())
}
```

### Wallet Integration

For Elements wallet integration, you can generate descriptors from the signer:

```rust
use amp_rs::signer::LwkSoftwareSigner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (_, signer) = LwkSoftwareSigner::generate_new()?;

    // Generate descriptor for wallet import
    let descriptor = signer.get_wpkh_slip77_descriptor()?;
    println!("Descriptor for wallet import: {}", descriptor);

    // This descriptor can be imported into Elements using importdescriptors RPC
    // to enable the wallet to recognize addresses and UTXOs from this signer

    Ok(())
}
```

### Future Operations

The same signer setup and passing pattern will be used for upcoming operations:

- **Asset Reissuance**: `reissue_asset(asset_uuid, amount, &signer)`
- **Asset Burning**: `burn_asset(asset_uuid, amount, &signer)`
- **Advanced Distribution**: Enhanced distribution workflows with complex signing requirements

The signer abstraction ensures consistent transaction signing across all asset operations while maintaining security best practices for testnet development.

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
