# Implementation Plan

- [x] 1. Create token strategy trait and base implementations
  - Define `TokenStrategy` trait with async methods for get_token, clear_token, and persistence control
  - Implement `MockTokenStrategy` struct with isolated token storage and no persistence
  - Implement `LiveTokenStrategy` struct that wraps the existing TokenManager
  - Add strategy type identification methods for debugging and testing
  - Write unit tests for both strategy implementations
  - _Requirements: 1.1, 1.3, 1.4, 5.3, 5.4_

- [x] 2. Implement environment detection and strategy selection logic
  - Create `TokenEnvironment` enum with Mock, Live, and Auto variants
  - Implement environment detection logic based on AMP_TESTS and credential patterns
  - Add helper methods to detect mock credentials (containing "mock" string)
  - Create strategy factory methods for automatic and explicit strategy selection
  - Write unit tests for environment detection logic with various credential combinations
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [x] 3. Enhance ApiClient with strategy-based token management
  - Modify `ApiClient` struct to use `Box<dyn TokenStrategy>` instead of direct TokenManager
  - Update `ApiClient::new()` to automatically select appropriate strategy based on environment
  - Enhance `ApiClient::with_mock_token()` to use `MockTokenStrategy` explicitly
  - Update `get_token()` method to delegate to the configured strategy
  - Write integration tests for ApiClient with both mock and live strategies
  - _Requirements: 1.1, 1.3, 6.4, 6.5_

- [ ] 4. Add token cleanup and isolation utilities
  - Implement `TokenManager::cleanup_token_files()` method for removing token persistence files
  - Add `TokenManager::clear_token()` method for clearing in-memory token state
  - Create `ApiClient::force_cleanup_token_files()` static method for test cleanup
  - Implement environment-aware persistence control in `TokenManager::should_persist_tokens()`
  - Write unit tests for cleanup operations and persistence control logic
  - _Requirements: 1.2, 1.5, 5.1, 5.2, 5.5_

- [ ] 5. Update test helper functions for proper environment isolation
  - Enhance `setup_mock_test()` to set mock environment variables and disable persistence
  - Enhance `cleanup_mock_test()` to force cleanup token files and restore environment
  - Update `get_shared_client()` to use environment-appropriate token strategy
  - Add `setup_live_test()` helper for explicit live test environment configuration
  - Write tests to verify test helper functions properly isolate environments
  - _Requirements: 3.1, 3.2, 3.3, 6.1, 6.2, 6.3_

- [ ] 6. Implement strategy-specific error handling and logging
  - Create `StrategyError` enum for strategy-specific error types
  - Add conversion from `StrategyError` to existing `Error` types
  - Implement comprehensive logging for strategy selection and token operations
  - Add debug information for strategy type and persistence settings
  - Write unit tests for error handling and error conversion logic
  - _Requirements: 4.5, 6.1, 6.2_

- [ ] 7. Add comprehensive test suite for token isolation
  - Create test to verify mock strategy complete isolation (no network, no persistence)
  - Create test to verify live strategy proper token management and persistence
  - Create test to verify running all tests together produces consistent results
  - Create test to verify mock tests don't affect live test token state
  - Create test to verify environment detection works correctly with various configurations
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 8. Update existing tests to use enhanced test helpers
  - Modify existing mock tests to use enhanced `setup_mock_test()` and `cleanup_mock_test()`
  - Ensure existing live tests properly use live test environment detection
  - Verify all existing tests continue to pass without code changes to test logic
  - Add explicit strategy verification to key tests for debugging purposes
  - Run full test suite to verify no regressions in existing functionality
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [ ] 9. Implement token persistence file management
  - Add `get_token_file_path()` method to determine token file location
  - Implement safe file cleanup with proper error handling
  - Add file existence checks before cleanup operations
  - Ensure token file cleanup only happens in appropriate environments
  - Write tests for file management operations and error conditions
  - _Requirements: 2.3, 5.1, 5.2_

- [ ] 10. Add configuration validation and documentation
  - Implement validation for strategy selection configuration
  - Add comprehensive logging for strategy selection decisions
  - Update environment variable documentation with new persistence control options
  - Add troubleshooting guide for token management issues
  - Write configuration tests to verify all environment variable combinations work correctly
  - _Requirements: 4.4, 4.5_