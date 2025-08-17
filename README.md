# Blockstream AMP API Client

A Rust client for the Blockstream AMP API.

## Usage

See the examples directory for usage.

## Testing with Mocks

This client provides a public `mocks` module that can be used by downstream applications for testing purposes. The mocks are built using `httpmock` and allow you to test your application's integration with the AMP API without making live network calls.

To use the mocks in your tests, you will need to:

1.  Add `amp-rs` and `httpmock` to your `[dev-dependencies]` in `Cargo.toml`.
2.  In your test, start a `MockServer` from `httpmock`.
3.  Use the functions in the `amp_rs::mocks` module to set up the desired mock responses on the server.
4.  Instantiate the `ApiClient` using `ApiClient::with_base_url`, passing it the URL of the mock server.

### Example

```rust
#[cfg(test)]
mod tests {
    use amp_rs::{mocks, ApiClient};
    use httpmock::prelude::*;
    use url::Url;

    #[tokio::test]
    async fn my_app_test() {
        // Arrange
        std::env::set_var("AMP_USERNAME", "mock_user");
        std::env::set_var("AMP_PASSWORD", "mock_pass");
        let server = MockServer::start();

        mocks::mock_obtain_token(&server);
        mocks::mock_get_assets(&server);

        let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();

        // Act
        let assets = client.get_assets().await;

        // Assert
        assert!(assets.is_ok());
        assert!(!assets.unwrap().is_empty());
    }
}
```
