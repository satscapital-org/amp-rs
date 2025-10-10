# Implementation Plan

- [x] 1. Implement lock_manager method in ApiClient
  - Add `lock_manager` method to `ApiClient` struct in `src/client.rs`
  - Use PUT request to `/api/managers/{managerId}/lock` endpoint
  - Follow existing authentication and error handling patterns
  - Return `Result<(), Error>` for success/failure indication
  - _Requirements: 1.1, 1.2, 1.3, 5.1, 5.2, 5.3_

- [x] 2. Implement add_asset_to_manager method in ApiClient
  - Add `add_asset_to_manager` method to `ApiClient` struct in `src/client.rs`
  - Use PUT request to `/api/managers/{managerId}/assets/{assetUuid}/add` endpoint
  - Follow existing authentication and error handling patterns
  - Return `Result<(), Error>` for success/failure indication
  - _Requirements: 2.1, 2.2, 2.3, 5.1, 5.2, 5.3_

- [x] 3. Implement get_asset_assignment method in ApiClient
  - Add `get_asset_assignment` method to `ApiClient` struct in `src/client.rs`
  - Use GET request to `/api/assets/{assetUuid}/assignments/{assignmentId}` endpoint
  - Follow existing authentication and error handling patterns
  - Return `Result<Assignment, Error>` with deserialized assignment data
  - _Requirements: 3.1, 3.2, 3.3, 5.1, 5.2, 5.3_

- [ ] 4. Create mock implementations for testing
- [x] 4.1 Implement mock_lock_manager function
  - Add `mock_lock_manager` function to `src/mocks.rs`
  - Mock PUT request to `/api/managers/{managerId}/lock` with success response
  - Follow existing mock patterns using httpmock
  - _Requirements: 4.1, 4.2_

- [x] 4.2 Implement mock_add_asset_to_manager function
  - Add `mock_add_asset_to_manager` function to `src/mocks.rs`
  - Mock PUT request to `/api/managers/{managerId}/assets/{assetUuid}/add` with success response
  - Follow existing mock patterns using httpmock
  - _Requirements: 4.1, 4.2_

- [x] 4.3 Implement mock_get_asset_assignment function
  - Add `mock_get_asset_assignment` function to `src/mocks.rs`
  - Mock GET request to `/api/assets/{assetUuid}/assignments/{assignmentId}` with Assignment JSON response
  - Use existing Assignment struct for response data
  - Follow existing mock patterns using httpmock
  - _Requirements: 4.1, 4.2_

- [ ] 5. Create mock tests for all new methods
- [x] 5.1 Write test_lock_manager_mock test
  - Create mock test in `tests/api.rs` for lock_manager method
  - Use mock server setup and mock_lock_manager function
  - Test successful manager locking scenario
  - Verify correct request format and response handling
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 5.2 Write test_add_asset_to_manager_mock test
  - Create mock test in `tests/api.rs` for add_asset_to_manager method
  - Use mock server setup and mock_add_asset_to_manager function
  - Test successful asset authorization scenario
  - Verify correct request format and response handling
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 5.3 Write test_get_asset_assignment_mock test
  - Create mock test in `tests/api.rs` for get_asset_assignment method
  - Use mock server setup and mock_get_asset_assignment function
  - Test successful assignment retrieval scenario
  - Verify correct request format and Assignment deserialization
  - _Requirements: 4.1, 4.2, 4.3_

- [ ] 6. Create live tests for all new methods
- [x] 6.1 Write test_lock_manager_live_slow test
  - Create live test in `tests/api.rs` for lock_manager method marked with #[ignore]
  - Create test manager using existing create_manager method
  - Call lock_manager method with created manager ID
  - Verify operation success
  - Clean up by deleting the test manager
  - _Requirements: 1.4, 4.4, 4.5_

- [x] 6.2 Write test_add_asset_to_manager_live_slow test
  - Create live test in `tests/api.rs` for add_asset_to_manager method marked with #[ignore]
  - Create test manager using existing create_manager method
  - Find preserved "Test Environment Asset" or use first available asset
  - Call add_asset_to_manager method with manager ID and asset UUID
  - Verify operation success
  - Clean up by deleting the test manager
  - _Requirements: 2.4, 4.4, 4.5_

- [x] 6.3 Write test_get_asset_assignment_live_slow test
  - Create live test in `tests/api.rs` for get_asset_assignment method marked with #[ignore]
  - Use create_asset_assignment workflow for setup (get/create user, category, asset, create assignment)
  - Call get_asset_assignment method with asset UUID and assignment ID
  - Verify returned assignment data matches created assignment
  - Use create_asset_assignment cleanup to delete created assignments
  - _Requirements: 3.4, 4.4, 4.5_

- [ ] 7. Add comprehensive error handling tests
- [x] 7.1 Write error handling tests for lock_manager
  - Test invalid manager ID scenarios
  - Test network error scenarios using mock server
  - Verify proper Error enum variants are returned
  - _Requirements: 5.2_

- [x] 7.2 Write error handling tests for add_asset_to_manager
  - Test invalid manager ID and asset UUID scenarios
  - Test network error scenarios using mock server
  - Verify proper Error enum variants are returned
  - _Requirements: 5.2_

- [x] 7.3 Write error handling tests for get_asset_assignment
  - Test invalid asset UUID and assignment ID scenarios
  - Test non-existent assignment scenarios
  - Test network error scenarios using mock server
  - Verify proper Error enum variants are returned
  - _Requirements: 5.2_

- [ ] 8. Update documentation and examples
- [x] 8.1 Add doc comments to all new methods
  - Write comprehensive doc comments for lock_manager method with usage examples
  - Write comprehensive doc comments for add_asset_to_manager method with usage examples
  - Write comprehensive doc comments for get_asset_assignment method with usage examples
  - Follow existing documentation patterns in the codebase
  - _Requirements: 5.5_

- [x] 8.2 Update lib.rs exports if needed
  - Ensure new methods are properly accessible through the public API
  - Verify no additional exports are needed since methods are added to existing ApiClient
  - _Requirements: 5.1_