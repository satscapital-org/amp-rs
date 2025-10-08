# Requirements Document

## Introduction

This feature addresses token management issues where mock tests interfere with live tests by implementing proper test isolation. The system will ensure that mock tests operate without any token management or persistence, while live tests can properly reuse tokens between runs for efficiency.

## Requirements

### Requirement 1

**User Story:** As a developer running tests, I want mock tests to work without any token management so that they don't interfere with live test token state.

#### Acceptance Criteria

1. WHEN mock tests are executed THEN the system SHALL NOT use the global token manager
2. WHEN mock tests are executed THEN the system SHALL NOT attempt token persistence to disk
3. WHEN mock tests are executed THEN the system SHALL use a completely isolated token management approach
4. WHEN mock tests use `ApiClient::with_mock_token()` THEN the system SHALL bypass all token acquisition and refresh logic
5. WHEN mock tests complete THEN the system SHALL NOT leave any token state that affects subsequent tests

### Requirement 2

**User Story:** As a developer running live tests, I want token persistence to work between test runs so that I don't need to obtain new tokens for every test execution.

#### Acceptance Criteria

1. WHEN live tests are executed THEN the system SHALL use the global token manager for token sharing
2. WHEN live tests run shortly after previous live tests THEN the system SHALL reuse existing valid tokens from disk
3. WHEN live tests obtain a new token THEN the system SHALL persist it to disk for future test runs
4. WHEN live tests are executed with `AMP_TESTS=live` THEN the system SHALL enable token persistence
5. WHEN live tests complete THEN the system SHALL leave valid tokens available for subsequent live test runs

### Requirement 3

**User Story:** As a developer, I want proper test isolation so that running all tests together produces the same results as running mock and live tests separately.

#### Acceptance Criteria

1. WHEN running "cargo test" (all tests) THEN mock and live tests SHALL NOT interfere with each other
2. WHEN running "cargo test mock" THEN only mock-specific token behavior SHALL be used
3. WHEN running "cargo test -- --skip mock" THEN only live token behavior SHALL be used
4. WHEN tests are run in any order THEN the results SHALL be consistent and predictable
5. WHEN mock tests run before live tests THEN the live tests SHALL still have access to proper token management

### Requirement 4

**User Story:** As a developer, I want clear separation between mock and live test environments so that the token management strategy is appropriate for each context.

#### Acceptance Criteria

1. WHEN the system detects mock test environment THEN it SHALL disable all token persistence mechanisms
2. WHEN the system detects live test environment THEN it SHALL enable full token management with persistence
3. WHEN mock credentials are detected THEN the system SHALL automatically use mock-only token behavior
4. WHEN real credentials are detected with `AMP_TESTS=live` THEN the system SHALL use full token management
5. WHEN environment detection is ambiguous THEN the system SHALL default to safe mock behavior

### Requirement 5

**User Story:** As a developer, I want mock tests to be completely self-contained so that they don't require any external token management infrastructure.

#### Acceptance Criteria

1. WHEN mock tests execute THEN they SHALL NOT read or write token files from disk
2. WHEN mock tests execute THEN they SHALL NOT use shared token storage between test instances
3. WHEN mock tests execute THEN they SHALL use only the provided mock token without validation or refresh
4. WHEN mock tests execute THEN they SHALL NOT attempt network requests for token operations
5. WHEN mock tests complete THEN they SHALL clean up any temporary token state

### Requirement 6

**User Story:** As a developer, I want the existing test suite to work without modification so that the fix doesn't require rewriting existing tests.

#### Acceptance Criteria

1. WHEN existing mock tests run THEN they SHALL continue to work without code changes
2. WHEN existing live tests run THEN they SHALL continue to work without code changes
3. WHEN existing test helper functions are used THEN they SHALL automatically use the appropriate token management strategy
4. WHEN `ApiClient::with_mock_token()` is called THEN it SHALL automatically enable mock-only behavior
5. WHEN `ApiClient::new()` is called in live test context THEN it SHALL automatically enable full token management