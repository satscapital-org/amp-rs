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

### List asset groups

```rust
use amp_client::client::ApiClient;

#[tokio::main]
async fn main() {
    let client = ApiClient::new().unwrap();
    let asset_groups = client.list_asset_groups().await.unwrap();
    println!("{:?}", asset_groups);
}
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
