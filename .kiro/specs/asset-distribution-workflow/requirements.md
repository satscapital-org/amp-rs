# Requirements Document

## Introduction

This feature implements a comprehensive asset distribution workflow that wraps the entire process into a single async method on the ApiClient struct. The function will handle authentication, distribution creation, transaction signing via callback, blockchain interaction, confirmation waiting, and final confirmation with the AMP API. This streamlines the complex multi-step process of distributing assets to registered users through the Blockstream AMP platform.

## Requirements

### Requirement 1

**User Story:** As a developer using the AMP client library, I want a single method to distribute assets to multiple users, so that I can complete the entire distribution workflow without managing individual steps.

#### Acceptance Criteria

1. WHEN I call `distribute_asset` with valid parameters THEN the system SHALL authenticate with the AMP API using the client's token
2. WHEN authentication succeeds THEN the system SHALL create a distribution request via POST to `/api/assets/{asset_uuid}/distributions/create/`
3. WHEN the distribution is created THEN the system SHALL return a distribution_uuid and address mapping data
4. WHEN I provide a signing callback THEN the system SHALL use it to sign the raw transaction during the process
5. WHEN the transaction is signed and broadcast THEN the system SHALL wait for 2 blockchain confirmations with a 10-minute timeout
6. WHEN confirmations are received THEN the system SHALL confirm the distribution with the AMP API
7. WHEN any step fails THEN the system SHALL return a descriptive AmpError with context

### Requirement 2

**User Story:** As a developer, I want to provide assignment details for distribution, so that I can specify which users receive which amounts at which addresses.

#### Acceptance Criteria

1. WHEN I create an Assignment struct THEN it SHALL contain user_id, address, and amount fields
2. WHEN I pass a vector of assignments THEN the system SHALL serialize them correctly for the AMP API
3. WHEN assignments are processed THEN the system SHALL validate that all required fields are present
4. WHEN assignment data is invalid THEN the system SHALL return a validation error

### Requirement 3

**User Story:** As a developer, I want to use a signing callback interface, so that I can provide different signing implementations without coupling to specific signer types.

#### Acceptance Criteria

1. WHEN I provide a signer implementing the Signer trait THEN the system SHALL call `sign_transaction` with the unsigned transaction hex
2. WHEN the signer returns a signed transaction THEN the system SHALL use it for broadcasting
3. WHEN signing fails THEN the system SHALL propagate the signer error appropriately
4. WHEN using LwkSoftwareSigner in tests THEN it SHALL successfully sign Liquid testnet transactions

### Requirement 4

**User Story:** As a developer, I want Elements node integration, so that the system can interact with the blockchain for transaction creation and broadcasting.

#### Acceptance Criteria

1. WHEN the system connects to Elements node THEN it SHALL verify node version, sync status, and passphrase
2. WHEN creating transactions THEN the system SHALL query available UTXOs for the asset via `listunspent`
3. WHEN building raw transactions THEN the system SHALL use `createrawtransaction` with proper inputs and outputs
4. WHEN broadcasting THEN the system SHALL use `sendrawtransaction` and return the transaction ID
5. WHEN waiting for confirmations THEN the system SHALL poll `gettransaction` every 15 seconds
6. WHEN retrieving transaction details THEN the system SHALL get change data filtered by asset_id and txid

### Requirement 5

**User Story:** As a developer, I want comprehensive error handling, so that I can understand and respond to different failure scenarios.

#### Acceptance Criteria

1. WHEN API calls fail THEN the system SHALL return AmpError::Api with details
2. WHEN RPC calls fail THEN the system SHALL return AmpError::Rpc with context
3. WHEN signing fails THEN the system SHALL return AmpError::Signer with the underlying error
4. WHEN confirmations timeout THEN the system SHALL return AmpError::Timeout with instructions
5. WHEN confirmation fails THEN the system SHALL provide retry instructions with the txid

### Requirement 6

**User Story:** As a developer running tests, I want a comprehensive test workflow, so that I can verify the entire distribution process works end-to-end.

#### Acceptance Criteria

1. WHEN running tests THEN the system SHALL use environment variables for AMP credentials and Elements RPC configuration
2. WHEN setting up tests THEN the system SHALL generate a new mnemonic and LwkSoftwareSigner
3. WHEN preparing test data THEN the system SHALL issue an asset, register a user, create categories, and set up assignments
4. WHEN executing the test THEN the system SHALL call distribute_asset with the LwkSoftwareSigner as callback
5. WHEN test completes THEN the system SHALL verify the distribution is confirmed and clean up test entities
6. WHEN cleanup occurs THEN the system SHALL properly detach users and assets from categories before deletion

### Requirement 7

**User Story:** As a developer, I want proper logging and monitoring, so that I can track the distribution process and debug issues.

#### Acceptance Criteria

1. WHEN each step executes THEN the system SHALL log progress using the tracing crate
2. WHEN errors occur THEN the system SHALL log detailed error information
3. WHEN waiting for confirmations THEN the system SHALL log polling attempts and status
4. WHEN retries are needed THEN the system SHALL log retry attempts and reasons

### Requirement 8

**User Story:** As a developer, I want the function to handle Liquid-specific transaction requirements, so that distributions work correctly on the Liquid network.

#### Acceptance Criteria

1. WHEN creating transactions THEN the system SHALL handle Liquid fees appropriately
2. WHEN building outputs THEN the system SHALL use confidential addresses and asset commitments
3. WHEN calculating change THEN the system SHALL create proper change outputs for remaining asset amounts
4. WHEN working with testnet THEN the system SHALL use appropriate testnet parameters and addresses