# Implementation Plan

- [x] 1. Add new data models to model.rs
  - Create `RegisteredUserEdit` struct with optional name field for editing registered users
  - Create `GaidRequest` struct with gaid field for GAID operations
  - Create `CategoriesRequest` struct with categories Vec<i64> field for category operations
  - Add necessary serde derive macros for serialization
  - _Requirements: 1.4, 4.2, 5.2, 9.2, 10.2_

- [x] 2. Implement edit_registered_user method
  - Add `edit_registered_user` method to ApiClient impl block
  - Use PUT request to `/api/registered_users/{registeredUserId}/edit` endpoint
  - Accept registered_user_id: i64 and edit_data: &RegisteredUserEdit parameters
  - Return Result<RegisteredUserResponse, Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 1.1, 1.2, 1.3, 11.1, 11.2, 11.3, 11.4_

- [x] 3. Implement get_registered_user_summary method
  - Add `get_registered_user_summary` method to ApiClient impl block
  - Use GET request to `/api/registered_users/{registeredUserId}/summary` endpoint
  - Accept registered_user_id: i64 parameter
  - Return Result<RegisteredUserSummary, Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 2.1, 2.2, 2.3, 11.1, 11.2, 11.3, 11.4_

- [x] 4. Implement get_registered_user_gaids method
  - Add `get_registered_user_gaids` method to ApiClient impl block
  - Use GET request to `/api/registered_users/{registeredUserId}/gaids` endpoint
  - Accept registered_user_id: i64 parameter
  - Return Result<Vec<String>, Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 3.1, 3.2, 3.3, 11.1, 11.2, 11.3, 11.4_

- [x] 5. Implement add_gaid_to_registered_user method
  - Add `add_gaid_to_registered_user` method to ApiClient impl block
  - Use POST request to `/api/registered_users/{registeredUserId}/gaids/add` endpoint
  - Accept registered_user_id: i64 and gaid: &str parameters
  - Create GaidRequest struct instance for request body
  - Return Result<(), Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 11.1, 11.2, 11.3, 11.4_

- [x] 6. Implement set_default_gaid_for_registered_user method
  - Add `set_default_gaid_for_registered_user` method to ApiClient impl block
  - Use POST request to `/api/registered_users/{registeredUserId}/gaids/set-default` endpoint
  - Accept registered_user_id: i64 and gaid: &str parameters
  - Create GaidRequest struct instance for request body
  - Return Result<(), Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 11.1, 11.2, 11.3, 11.4_

- [x] 7. Implement get_gaid_registered_user method
  - Add `get_gaid_registered_user` method to ApiClient impl block
  - Use GET request to `/api/gaids/{gaid}/registered_user` endpoint
  - Accept gaid: &str parameter
  - Return Result<RegisteredUserResponse, Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 6.1, 6.2, 6.3, 11.1, 11.2, 11.3, 11.4_

- [x] 8. Implement get_gaid_balance method
  - Add `get_gaid_balance` method to ApiClient impl block
  - Use GET request to `/api/gaids/{gaid}/balance` endpoint
  - Accept gaid: &str parameter
  - Return Result<Balance, Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 7.1, 7.2, 7.3, 11.1, 11.2, 11.3, 11.4_

- [x] 9. Implement get_gaid_asset_balance method
  - Add `get_gaid_asset_balance` method to ApiClient impl block
  - Use GET request to `/api/gaids/{gaid}/balance/{assetUuid}` endpoint
  - Accept gaid: &str and asset_uuid: &str parameters
  - Return Result<Ownership, Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 8.1, 8.2, 8.3, 11.1, 11.2, 11.3, 11.4_

- [x] 10. Implement add_categories_to_registered_user method
  - Add `add_categories_to_registered_user` method to ApiClient impl block
  - Use PUT request to `/api/registered_users/{registeredUserId}/categories/add` endpoint
  - Accept registered_user_id: i64 and categories: &[i64] parameters
  - Create CategoriesRequest struct instance for request body
  - Return Result<(), Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 11.1, 11.2, 11.3, 11.4_

- [x] 11. Implement remove_categories_from_registered_user method
  - Add `remove_categories_from_registered_user` method to ApiClient impl block
  - Use PUT request to `/api/registered_users/{registeredUserId}/categories/delete` endpoint
  - Accept registered_user_id: i64 and categories: &[i64] parameters
  - Create CategoriesRequest struct instance for request body
  - Return Result<(), Error>
  - Use existing make_request helper with proper authentication
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 11.1, 11.2, 11.3, 11.4_

- [x] 12. Add mock implementations for all new methods
  - Create `mock_edit_registered_user` function in mocks.rs
  - Create `mock_get_registered_user_summary` function in mocks.rs
  - Create `mock_get_registered_user_gaids` function in mocks.rs
  - Create `mock_add_gaid_to_registered_user` function in mocks.rs
  - Create `mock_set_default_gaid_for_registered_user` function in mocks.rs
  - Create `mock_get_gaid_registered_user` function in mocks.rs
  - Create `mock_get_gaid_balance` function in mocks.rs
  - Create `mock_get_gaid_asset_balance` function in mocks.rs
  - Create `mock_add_categories_to_registered_user` function in mocks.rs
  - Create `mock_remove_categories_from_registered_user` function in mocks.rs
  - Each mock should return realistic test data and handle proper HTTP methods and paths
  - _Requirements: 12.2, 12.4_


- [x] 13. Create mock tests for all new methods
  - Write `test_edit_registered_user_mock` test function
  - Write `test_get_registered_user_summary_mock` test function
  - Write `test_get_registered_user_gaids_mock` test function
  - Write `test_add_gaid_to_registered_user_mock` test function
  - Write `test_set_default_gaid_for_registered_user_mock` test function
  - Write `test_get_gaid_registered_user_mock` test function
  - Write `test_get_gaid_balance_mock` test function
  - Write `test_get_gaid_asset_balance_mock` test function
  - Write `test_add_categories_to_registered_user_mock` test function
  - Write `test_remove_categories_from_registered_user_mock` test function
  - Each test should use setup_mock_test() and cleanup_mock_test() helpers
  - Each test must call cleanup_mock_test() in a defer/finally block or at test end to ensure cleanup occurs even if test fails
  - Verify request/response serialization and success cases
  - _Requirements: 12.1, 12.2, 12.3, 12.4_

- [x] 14. Create live tests for registered user and GAID management methods
  - Write `test_edit_registered_user_live` test function
  - Write `test_get_registered_user_summary_live` test function
  - Write `test_get_registered_user_gaids_live` test function
  - Write `test_add_gaid_to_registered_user_live` test function using GAID `GA44YYwPM8vuRMmjFL8i5kSqXhoTW2`
  - Write `test_set_default_gaid_for_registered_user_live` test function
  - Write `test_get_gaid_registered_user_live` test function
  - Each test should check for AMP_TESTS=live environment variable
  - Create registered user with GAID `GA44YYwPM8vuRMmjFL8i5kSqXhoTW2` for GAID-related tests
  - Use existing registered users and assets where possible for other tests
  - Each test must implement comprehensive cleanup in a defer/finally block or at test end to ensure cleanup occurs even if test fails
  - Cleanup must include: removing any created registered users, removing GAID associations, reverting any user edits made during testing
  - Store original state before making changes and restore it during cleanup
  - _Requirements: 12.1, 12.3, 12.4, 12.5_

- [x] 15. Create live test for get_gaid_balance method
  - Write `test_get_gaid_balance_live` test function using GAID `GAbzSbgCZ6M6WU85rseKTrfehPsjt`
  - Verify balance returns 3 balance entries including the test environment asset
  - Test should check for AMP_TESTS=live environment variable
  - Use the specific test GAID for consistent balance validation
  - Implement cleanup to ensure test isolation (though this is a read-only operation, ensure any setup is cleaned up)
  - _Requirements: 7.1, 7.2, 12.1, 12.3, 12.4_



 # NOTE: Find an asset and GAID for task 16 before starting.


- [x] 16. Create live test for get_gaid_asset_balance method
  - Write `test_get_gaid_asset_balance_live` test function using GAID `GAQzmXM7jVaKAwtHGXHENgn5KUUmL` for asset UUID `716cb816-6cc7-469d-a41f-f4ed1c0d2dce`
  - Verify balance returns `0` (zero balance)
  - Test should check for AMP_TESTS=live environment variable
  - Use the specific test GAID and asset UUID for consistent balance validation
  - Implement cleanup to ensure test isolation (though this is a read-only operation, ensure any setup is cleaned up)
  - _Requirements: 8.1, 8.2, 12.1, 12.3, 12.4_

- [x] 17. Create live tests for category management methods
  - Write `test_add_categories_to_registered_user_live` test function
  - Write `test_remove_categories_from_registered_user_live` test function
  - Each test should check for AMP_TESTS=live environment variable
  - Each test should create its own registered user and category for testing
  - Each test must implement comprehensive cleanup in a defer/finally block or at test end to ensure cleanup occurs even if test fails
  - Cleanup must include: deleting any created categories, deleting any created registered users
  - Tests should be fully self-contained and not depend on existing data
  - Use unique timestamps in names to avoid conflicts with concurrent test runs
  - _Requirements: 9.1, 9.3, 10.1, 10.3, 12.1, 12.3, 12.4, 12.5_


- [x] 18. Run comprehensive test suite and validate implementation
  - Execute all mock tests to verify isolated functionality
  - Execute all live tests with proper credentials to verify API compatibility
  - Run existing test suite to ensure no regressions
  - Verify all new methods follow existing code patterns and conventions
  - Test retry logic and error handling under various network conditions
  - Validate serialization/deserialization of all request and response types
  - Verify that all tests properly clean up after themselves by running tests multiple times and checking for resource leaks
  - Validate that cleanup functions work correctly even when tests fail or are interrupted
  - Confirm that test isolation is maintained - each test should be able to run independently without affecting others
  - _Requirements: 11.1, 11.2, 11.3, 11.4, 12.1, 12.2, 12.3, 12.4, 12.5_