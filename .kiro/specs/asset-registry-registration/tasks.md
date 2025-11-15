# Implementation Plan

- [x] 1. Add RegisterAssetResponse model structure
  - Create the `RegisterAssetResponse` struct in `src/model.rs` with fields: `success: bool`, `message: Option<String>`, and `asset_id: String`
  - Add `Debug`, `Deserialize`, and `Serialize` derive macros for standard Rust patterns
  - Place the struct after `IssuanceResponse` in the model file for logical grouping with other asset-related responses
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [x] 2. Implement register_asset method in ApiClient
  - Add the `register_asset` method to `src/client.rs` in the asset operations section
  - Method signature: `pub async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error>`
  - Implementation uses `self.request_json(Method::POST, &["assets", asset_uuid, "register"], None::<&()>).await`
  - Place the method after `edit_asset` and before `delete_asset` for logical grouping
  - Add comprehensive documentation including purpose, parameters, return value, error conditions, and usage example
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5, 3.5, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4, 5.5_

- [x] 3. Create mock function for testing
  - Add `mock_register_asset` function to `src/mocks.rs` after other asset-related mock functions
  - Configure mock to respond to POST requests to `/assets/mock_asset_uuid/register`
  - Return 200 status with JSON body containing `success: true`, appropriate message, and realistic asset_id from liquidtestnet.com
  - Use L-BTC asset ID: `6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d`
  - Follow existing mock function patterns for consistency
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 4. Implement mock-based tests
- [x] 4.1 Create success case test
  - Add `test_register_asset_mock` function to `tests/api.rs` in the asset operations test section
  - Setup mock server with `mock_register_asset()` and create ApiClient with mock token
  - Call `register_asset("mock_asset_uuid")` and verify response is Ok
  - Assert `success` is true, `asset_id` matches expected value, and `message` is present
  - Include cleanup to reload .env file
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 4.2 Create error handling tests
  - Add `test_register_asset_not_found_mock` to verify 404 handling for non-existent asset UUID
  - Add `test_register_asset_server_error_mock` to verify 500 error handling
  - Add `test_register_asset_already_registered_mock` to verify handling when asset is already registered
  - Each test should setup appropriate mock responses and verify error handling or success conditions
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 6.1, 6.2, 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 4.3 Create authentication verification test
  - Add `test_register_asset_authentication_mock` to verify authorization header is included
  - Mock should verify "Authorization: token mock_token" header is present in request
  - Assert request succeeds with correct authentication
  - _Requirements: 2.5, 6.4, 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 5. Verify implementation and run tests
  - Run `cargo check` to verify compilation without errors
  - Run `cargo test test_register_asset` to execute all register_asset tests
  - Verify all tests pass and use only mock responses (no live API calls)
  - Run `cargo clippy` to check for any linting issues
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_
