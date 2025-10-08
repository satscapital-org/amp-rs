# Requirements Document

## Introduction

This feature extends the AMP client library with additional API methods for comprehensive registered user and GAID management. The enhancement adds 10 new methods that provide full CRUD operations for registered users, GAID associations, balance queries, and category management. These methods will follow the existing patterns in the codebase for authentication, error handling, retry logic, and testing (both mocked and live tests).

## Requirements

### Requirement 1

**User Story:** As a developer using the AMP client library, I want to edit registered user information, so that I can update user details when needed.

#### Acceptance Criteria

1. WHEN I call `edit_registered_user` with a valid registered user ID and edit data THEN the system SHALL send a PUT request to `/api/registered_users/{registeredUserId}/edit`
2. WHEN the edit request is successful THEN the system SHALL return the updated registered user data
3. WHEN the registered user ID is invalid THEN the system SHALL return an appropriate error
4. WHEN the request body contains a `RegisteredUserEdit` struct with optional name field THEN the system SHALL serialize it correctly

### Requirement 2

**User Story:** As a developer using the AMP client library, I want to get a summary of a registered user including asset information, so that I can display comprehensive user data.

#### Acceptance Criteria

1. WHEN I call `get_registered_user_summary` with a valid registered user ID THEN the system SHALL send a GET request to `/api/registered_users/{registeredUserId}/summary`
2. WHEN the request is successful THEN the system SHALL return a summary containing asset information
3. WHEN the registered user ID is invalid THEN the system SHALL return an appropriate error

### Requirement 3

**User Story:** As a developer using the AMP client library, I want to list all GAIDs associated with a registered user, so that I can manage user's GAID associations.

#### Acceptance Criteria

1. WHEN I call `get_registered_user_gaids` with a valid registered user ID THEN the system SHALL send a GET request to `/api/registered_users/{registeredUserId}/gaids`
2. WHEN the request is successful THEN the system SHALL return a list of GAIDs associated with the user
3. WHEN the registered user ID is invalid THEN the system SHALL return an appropriate error

### Requirement 4

**User Story:** As a developer using the AMP client library, I want to associate a new GAID with a registered user, so that I can link users to their blockchain addresses.

#### Acceptance Criteria

1. WHEN I call `add_gaid_to_registered_user` with a valid registered user ID and GAID THEN the system SHALL send a POST request to `/api/registered_users/{registeredUserId}/gaids/add`
2. WHEN the request body contains a struct with `gaid` field THEN the system SHALL serialize it correctly
3. WHEN the association is successful THEN the system SHALL return confirmation
4. WHEN no existing GAIDs exist for the user THEN the system SHALL set the new GAID as default
5. WHEN the GAID is invalid or already associated THEN the system SHALL return an appropriate error

### Requirement 5

**User Story:** As a developer using the AMP client library, I want to set an existing GAID as the default for a registered user, so that I can manage which GAID is primary for the user.

#### Acceptance Criteria

1. WHEN I call `set_default_gaid_for_registered_user` with a valid registered user ID and GAID THEN the system SHALL send a POST request to `/api/registered_users/{registeredUserId}/gaids/set-default`
2. WHEN the request body contains a struct with `gaid` field THEN the system SHALL serialize it correctly
3. WHEN the operation is successful THEN the system SHALL return confirmation
4. WHEN the GAID is not associated with the user THEN the system SHALL return an appropriate error

### Requirement 6

**User Story:** As a developer using the AMP client library, I want to retrieve the registered user associated with a GAID, so that I can look up users by their blockchain address.

#### Acceptance Criteria

1. WHEN I call `get_gaid_registered_user` with a valid GAID THEN the system SHALL send a GET request to `/api/gaids/{gaid}/registered_user`
2. WHEN the request is successful THEN the system SHALL return the associated registered user data
3. WHEN the GAID has no associated user THEN the system SHALL return an appropriate error

### Requirement 7

**User Story:** As a developer using the AMP client library, I want to list asset balances for a GAID, so that I can display all assets owned by a specific address.

#### Acceptance Criteria

1. WHEN I call `get_gaid_balance` with a valid GAID THEN the system SHALL send a GET request to `/api/gaids/{gaid}/balance`
2. WHEN the request is successful THEN the system SHALL return a list of asset balances
3. WHEN the GAID is invalid THEN the system SHALL return an appropriate error

### Requirement 8

**User Story:** As a developer using the AMP client library, I want to retrieve the specific asset balance for a GAID, so that I can check how much of a particular asset an address holds.

#### Acceptance Criteria

1. WHEN I call `get_gaid_asset_balance` with a valid GAID and asset UUID THEN the system SHALL send a GET request to `/api/gaids/{gaid}/balance/{assetUuid}`
2. WHEN the request is successful THEN the system SHALL return the specific asset balance
3. WHEN the GAID or asset UUID is invalid THEN the system SHALL return an appropriate error

### Requirement 9

**User Story:** As a developer using the AMP client library, I want to associate categories with a registered user, so that I can organize users by business categories.

#### Acceptance Criteria

1. WHEN I call `add_categories_to_registered_user` with a valid registered user ID and category list THEN the system SHALL send a PUT request to `/api/registered_users/{registeredUserId}/categories/add`
2. WHEN the request body contains a struct with `categories` field as Vec<i64> THEN the system SHALL serialize it correctly
3. WHEN the operation is successful THEN the system SHALL return confirmation
4. WHEN any category ID is invalid THEN the system SHALL return an appropriate error

### Requirement 10

**User Story:** As a developer using the AMP client library, I want to remove categories from a registered user, so that I can update user categorization when needed.

#### Acceptance Criteria

1. WHEN I call `remove_categories_from_registered_user` with a valid registered user ID and category list THEN the system SHALL send a PUT request to `/api/registered_users/{registeredUserId}/categories/delete`
2. WHEN the request body contains a struct with `categories` field as Vec<i64> THEN the system SHALL serialize it correctly
3. WHEN the operation is successful THEN the system SHALL return confirmation
4. WHEN any category ID is not associated with the user THEN the system SHALL return an appropriate error

### Requirement 11

**User Story:** As a developer using the AMP client library, I want all new methods to follow existing patterns for error handling and retry logic, so that the API remains consistent and reliable.

#### Acceptance Criteria

1. WHEN any new method encounters a network error THEN the system SHALL apply the same retry logic as existing methods
2. WHEN any new method receives an authentication error THEN the system SHALL attempt token refresh as existing methods do
3. WHEN any new method fails THEN the system SHALL return errors using the existing Error enum
4. WHEN any new method is called THEN the system SHALL use the same authentication headers as existing methods

### Requirement 12

**User Story:** As a developer using the AMP client library, I want comprehensive test coverage for all new methods, so that I can rely on the functionality in production.

#### Acceptance Criteria

1. WHEN implementing any new method THEN the system SHALL provide both mock and live tests
2. WHEN running mock tests THEN the system SHALL use httpmock for isolated testing without external dependencies
3. WHEN running live tests THEN the system SHALL require AMP_TESTS=live environment variable
4. WHEN live tests modify state THEN the system SHALL clean up created resources when possible
5. WHEN tests use mock data THEN the system SHALL follow existing patterns for mock server setup and teardown