# Implementation Plan

- [x] 1. Add data model for memo operations
  - Create `SetAssetMemoRequest` struct in `src/model.rs`
  - Add proper serialization attributes following existing patterns
  - _Requirements: 4.1, 4.2_

- [ ] 2. Implement asset memo methods in client
  - [x] 2.1 Implement `get_asset_memo` method
    - Add method to `ApiClient` impl block in `src/client.rs`
    - Use GET request to `/api/assets/{assetUuid}/memo` endpoint
    - Return `String` result following existing error handling patterns
    - Add comprehensive documentation with error conditions
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  
  - [x] 2.2 Implement `set_asset_memo` method
    - Add method to `ApiClient` impl block in `src/client.rs`
    - Use POST request to `/api/assets/{assetUuid}/memo/set` endpoint
    - Accept memo string parameter and create `SetAssetMemoRequest`
    - Return empty result on success following existing patterns
    - Add comprehensive documentation with error conditions
    - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [ ] 3. Create mock implementations for testing
  - [x] 3.1 Add `mock_get_asset_memo` function
    - Create mock function in `src/mocks.rs`
    - Mock GET request to `/assets/mock_asset_uuid/memo`
    - Return sample memo string in response
    - Follow existing mock patterns and naming conventions
    - _Requirements: 5.2_
  
  - [x] 3.2 Add `mock_set_asset_memo` function
    - Create mock function in `src/mocks.rs`
    - Mock POST request to `/assets/mock_asset_uuid/memo/set`
    - Expect `SetAssetMemoRequest` in request body
    - Return success response following existing patterns
    - _Requirements: 5.2_

- [ ] 4. Implement mock tests for memo operations
  - [x] 4.1 Create `test_get_asset_memo_mock` test
    - Add test function in `tests/api.rs`
    - Use mock server with `mock_get_asset_memo`
    - Create client with mock token
    - Test successful memo retrieval
    - Verify returned memo matches expected value
    - Include proper test setup and cleanup
    - _Requirements: 5.1, 5.2_
  
  - [x] 4.2 Create `test_set_asset_memo_mock` test
    - Add test function in `tests/api.rs`
    - Use mock server with `mock_set_asset_memo`
    - Create client with mock token
    - Test successful memo setting with sample memo
    - Verify operation completes without error
    - Include proper test setup and cleanup
    - _Requirements: 5.1, 5.2_

- [ ] 5. Implement live tests for memo operations
  - [x] 5.1 Create `test_get_asset_memo_live` test
    - Add test function in `tests/api.rs`
    - Skip test if not in live mode (`AMP_TESTS != "live"`)
    - Use `get_shared_client()` for authentication
    - Find existing asset using patterns from cleanup_resources
    - Test memo retrieval on existing asset
    - Handle case where asset may not have memo
    - _Requirements: 5.1, 5.3, 5.5_
  
  - [x] 5.2 Create `test_set_asset_memo_live` test
    - Add test function in `tests/api.rs`
    - Skip test if not in live mode (`AMP_TESTS != "live"`)
    - Use `get_shared_client()` for authentication
    - Find existing asset using patterns from cleanup_resources
    - Set a test memo on the asset
    - Retrieve the memo to verify it was set correctly
    - Clean up by setting empty memo or leaving test memo
    - _Requirements: 5.1, 5.3, 5.5_

- [ ] 6. Implement live tests for category operations
  - [x] 6.1 Create `test_add_asset_to_category_live` test
    - Add test function in `tests/api.rs`
    - Skip test if not in live mode (`AMP_TESTS != "live"`)
    - Use `get_shared_client()` for authentication
    - Create temporary test category using unique timestamp
    - Create temporary test asset using GAID patterns
    - Add asset to category using existing `add_asset_to_category` method
    - Verify operation succeeds
    - Clean up by removing asset from category and deleting both resources
    - _Requirements: 5.1, 5.3, 5.4, 5.6_
  
  - [x] 6.2 Create `test_remove_asset_from_category_live` test
    - Add test function in `tests/api.rs`
    - Skip test if not in live mode (`AMP_TESTS != "live"`)
    - Use `get_shared_client()` for authentication
    - Create temporary test category using unique timestamp
    - Create temporary test asset using GAID patterns
    - Add asset to category first
    - Remove asset from category using existing `remove_asset_from_category` method
    - Verify operation succeeds
    - Clean up by deleting both category and asset
    - _Requirements: 5.1, 5.3, 5.4, 5.6_

- [ ] 7. Add mock tests for category operations
  - [x] 7.1 Create `test_add_asset_to_category_mock` test
    - Add test function in `tests/api.rs`
    - Use existing mock functions for categories and assets
    - Create mock for add asset to category operation
    - Test successful asset addition to category
    - Verify response matches expected CategoryResponse
    - Include proper test setup and cleanup
    - _Requirements: 5.1, 5.2_
  
  - [x] 7.2 Create `test_remove_asset_from_category_mock` test
    - Add test function in `tests/api.rs`
    - Use existing mock functions for categories and assets
    - Create mock for remove asset from category operation
    - Test successful asset removal from category
    - Verify response matches expected CategoryResponse
    - Include proper test setup and cleanup
    - _Requirements: 5.1, 5.2_