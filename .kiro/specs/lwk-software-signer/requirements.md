# Requirements Document

## Introduction

This feature implements a Rust struct called `LwkSoftwareSigner` that provides transaction signing capabilities for the amp-rust crate using Blockstream's Liquid Wallet Kit (LWK). The signer will handle signing unsigned transaction hex for Elements/Liquid transactions in testnet/regtest environments, specifically to support reissue_asset, distribute_asset, and burn_asset functions. The implementation uses LWK's SwSigner for in-memory private key storage, making it suitable for testing scenarios while maintaining security best practices.

## Requirements

### Requirement 1

**User Story:** As a developer using the amp-rust crate, I want a software-based transaction signer that can sign Liquid/Elements transactions using mnemonic phrases, so that I can test asset operations in testnet/regtest environments.

#### Acceptance Criteria

1. WHEN a valid mnemonic phrase is provided THEN the system SHALL create a LwkSoftwareSigner instance successfully
2. WHEN an invalid mnemonic phrase is provided THEN the system SHALL return an InvalidMnemonic error
3. WHEN the signer is created THEN it SHALL be configured for testnet/regtest networks only
4. WHEN the signer is queried for network type THEN it SHALL return true for is_testnet()

### Requirement 2

**User Story:** As a developer, I want to generate and manage multiple mnemonic phrases with persistent JSON storage, so that I can maintain multiple test signers and reference them by index for consistent testing across development sessions.

#### Acceptance Criteria

1. WHEN generate_new() is called AND mnemonic.local.json exists with valid content THEN the system SHALL use the first mnemonic from the array
2. WHEN generate_new() is called AND mnemonic.local.json does not exist or is empty THEN the system SHALL create a 12-word mnemonic phrase
3. WHEN a new mnemonic is generated THEN the system SHALL save it to mnemonic.local.json as the first entry in a "mnemonic" array
4. WHEN generate_new_indexed(index) is called AND the index exists THEN the system SHALL use the mnemonic at that array position
5. WHEN generate_new_indexed(index) is called AND the index does not exist THEN the system SHALL generate a new mnemonic and add it to the array at the next available position
6. WHEN generate_new() or generate_new_indexed() is called THEN the system SHALL return both the mnemonic string and a configured signer instance
7. WHEN mnemonics from file are used THEN they SHALL be valid for creating signer instances
8. WHEN mnemonic file operations fail THEN the system SHALL return an appropriate error

### Requirement 3

**User Story:** As a developer, I want the signer to implement the Signer trait with async transaction signing, so that it can be used polymorphically with other signer implementations in the codebase.

#### Acceptance Criteria

1. WHEN sign_transaction() is called with valid unsigned transaction hex THEN the system SHALL return a signed transaction hex string
2. WHEN sign_transaction() is called with invalid hex THEN the system SHALL return a HexParse error
3. WHEN sign_transaction() is called with malformed transaction data THEN the system SHALL return an InvalidTransaction error
4. WHEN signing succeeds THEN the system SHALL log the transaction ID for debugging purposes
5. WHEN LWK signing operations fail THEN the system SHALL return an Lwk error with descriptive message

### Requirement 4

**User Story:** As a developer, I want comprehensive error handling for all signer operations, so that I can properly handle and debug issues during transaction signing.

#### Acceptance Criteria

1. WHEN LWK operations fail THEN the system SHALL provide Lwk error variant with original error message
2. WHEN mnemonic parsing fails THEN the system SHALL provide InvalidMnemonic error with details
3. WHEN hex parsing fails THEN the system SHALL provide HexParse error
4. WHEN transaction deserialization fails THEN the system SHALL provide InvalidTransaction error with context
5. WHEN network operations fail THEN the system SHALL provide Network error
6. WHEN serialization operations fail THEN the system SHALL provide Serialization error
7. WHEN file I/O operations fail THEN the system SHALL provide FileIo error with context

### Requirement 5

**User Story:** As a developer, I want proper dependency management and imports, so that the signer integrates seamlessly with the existing amp-rust crate architecture.

#### Acceptance Criteria

1. WHEN the crate is built THEN all required LWK dependencies SHALL be available and compatible
2. WHEN the signer module is imported THEN all necessary traits and types SHALL be accessible
3. WHEN async operations are performed THEN tokio runtime SHALL handle them correctly
4. WHEN the crate is compiled THEN there SHALL be no dependency conflicts or version issues

### Requirement 6

**User Story:** As a developer, I want security warnings and best practices documentation, so that I understand the appropriate use cases and limitations of the software signer.

#### Acceptance Criteria

1. WHEN reviewing the code THEN there SHALL be clear warnings about testnet/regtest-only usage
2. WHEN using in production scenarios THEN documentation SHALL recommend hardware signers or encrypted storage
3. WHEN handling mnemonics THEN the implementation SHALL follow secure memory practices where possible
4. WHEN signing confidential transactions THEN the signer SHALL properly support Liquid's privacy features through LWK

### Requirement 7

**User Story:** As a developer, I want comprehensive test coverage for the signer functionality with indexed mnemonic management, so that I can verify correct behavior, maintain test isolation, and replace mnemonics when needed.

#### Acceptance Criteria

1. WHEN running tests THEN signer creation with generated mnemonics SHALL be verified
2. WHEN running tests THEN mnemonic generation SHALL produce valid 12-word phrases
3. WHEN running tests THEN network configuration SHALL be verified as testnet
4. WHEN running tests THEN basic transaction signing flow SHALL be testable (with appropriate mocking if needed)
5. WHEN tests reference mnemonics THEN they SHALL use array indices for consistent identification
6. WHEN tests need fresh mnemonics THEN they SHALL be able to generate new ones and add them to the array
7. WHEN tests are executed THEN they SHALL complete without external dependencies