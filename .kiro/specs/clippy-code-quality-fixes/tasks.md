# Implementation Plan

- [x] 1. Fix simple literal and format issues
  - Fix numeric literal separators in mocks.rs by adding underscores to large numbers
  - Update format! macros to use inline variable syntax in client.rs
  - Replace single-character string patterns with character literals
  - Run tests to ensure no regressions
  - _Requirements: 2.1, 2.2, 4.1, 4.2, 10.1, 10.2_

- [ ] 2. Eliminate redundant operations
  - Remove redundant clone operations where ownership can be transferred
  - Replace redundant closures with direct method references in mocks.rs
  - Run tests to verify functionality is preserved
  - _Requirements: 8.1, 8.2, 11.1, 11.2_

- [ ] 3. Enhance documentation formatting
  - Add backticks around code identifiers in documentation comments
  - Update all TokenManager, ApiClient, and function name references
  - Ensure consistent documentation formatting throughout client.rs
  - Run tests to verify no impact on functionality
  - _Requirements: 5.1, 5.2_

- [ ] 4. Add missing error documentation
  - Add comprehensive "# Errors" sections to functions returning Result types
  - Document specific error conditions for force_cleanup_token_files()
  - Document initialization errors for reset_global_instance()
  - Run tests to ensure documentation changes don't affect compilation
  - _Requirements: 7.1, 7.2_

- [ ] 5. Optimize type usage and annotations
  - Replace explicit type names with Self where appropriate in TokenManager
  - Add #[must_use] attributes to functions that return important values
  - Update function signatures to use Self consistently
  - Run tests to verify type system improvements work correctly
  - _Requirements: 6.1, 6.2, 9.1, 9.2_

- [ ] 6. Remove unnecessary async keywords
  - Remove async from functions that don't contain await statements
  - Update with_mock_token functions in both TokenManager and ApiClient
  - Ensure function signatures accurately reflect synchronous nature
  - Run tests to verify async removal doesn't break functionality
  - _Requirements: 12.1, 12.2_

- [ ] 7. Refactor high-complexity functions - Part 1
  - Extract helper methods from TokenManager::detect() function to reduce complexity from 38 to under 25
  - Break down environment detection logic into smaller, focused functions
  - Maintain the same public API and behavior
  - Run tests to ensure detection logic works correctly
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 8. Refactor high-complexity functions - Part 2
  - Extract helper methods from TokenManager::create_strategy() to reduce complexity from 47 to under 25
  - Separate strategy creation logic into focused helper functions
  - Preserve existing strategy creation behavior and error handling
  - Run tests to verify strategy creation functionality
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 9. Refactor high-complexity functions - Part 3
  - Extract helper methods from TokenManager::clear_token() to reduce complexity from 34 to under 25
  - Separate token clearing logic into manageable helper functions
  - Maintain existing token clearing behavior and error handling
  - Run tests to ensure token management works correctly
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 10. Refactor high-complexity functions - Part 4
  - Extract helper methods from TokenManager::load_token_from_disk() to reduce complexity from 43 to under 25
  - Break down file loading and validation logic into focused functions
  - Preserve existing token loading behavior and error handling
  - Run tests to verify token persistence functionality
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 11. Final validation and cleanup
  - Run comprehensive clippy check to verify all warnings are resolved
  - Execute full test suite to ensure no regressions
  - Verify cargo build completes without warnings
  - Confirm all requirements are satisfied
  - _Requirements: 1.1, 1.2, 1.3_