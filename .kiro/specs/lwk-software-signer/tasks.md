# Implementation Plan

- [x] 1. Set up project dependencies and module structure
  - Add LWK and related dependencies to Cargo.toml (lwk_signer, lwk_common, bip39, elements, hex)
  - Create new signer module in src/signer/ with mod.rs, lwk.rs, and error.rs files
  - Update src/lib.rs to export the signer module
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 2. Implement core error handling and data structures
  - [x] 2.1 Create SignerError enum with all required error variants
    - Define Lwk, InvalidMnemonic, HexParse, InvalidTransaction, Network, Serialization, and FileIo variants
    - Implement proper error conversion traits and Display formatting
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_
  
  - [x] 2.2 Create MnemonicStorage struct for JSON serialization
    - Define struct with mnemonic Vec<String> field
    - Implement Serialize and Deserialize traits
    - Add validation methods for mnemonic format
    - _Requirements: 2.1, 2.2, 2.8_

- [ ] 3. Implement Signer trait definition
  - [x] 3.1 Define the async Signer trait
    - Create trait with sign_transaction method signature
    - Add Send + Sync bounds for async compatibility
    - Document trait usage and expected behavior
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 4. Implement JSON file operations for mnemonic persistence
  - [x] 4.1 Create file reading functionality
    - Implement function to read and parse mnemonic.local.json
    - Handle missing file scenarios gracefully
    - Validate JSON structure and mnemonic format
    - _Requirements: 2.1, 2.8_
  
  - [x] 4.2 Create file writing functionality
    - Implement function to serialize and write MnemonicStorage to JSON
    - Handle file creation and update scenarios
    - Ensure atomic writes to prevent corruption
    - _Requirements: 2.3, 2.5_
  
  - [x] 4.3 Implement indexed mnemonic access
    - Create function to get mnemonic by index from storage
    - Handle out-of-bounds access gracefully
    - Support appending new mnemonics to array
    - _Requirements: 2.4, 2.5_

- [ ] 5. Implement LwkSoftwareSigner struct and methods
  - [x] 5.1 Create LwkSoftwareSigner struct
    - Define struct with SwSigner and is_testnet fields
    - Implement basic constructor and getter methods
    - _Requirements: 1.1, 1.4_
  
  - [x] 5.2 Implement new() method for direct mnemonic creation
    - Parse mnemonic string and validate format
    - Create SwSigner instance with testnet configuration
    - Handle mnemonic parsing errors appropriately
    - _Requirements: 1.1, 1.2_
  
  - [x] 5.3 Implement generate_new() method with file persistence
    - Check for existing mnemonic.local.json file
    - Load first mnemonic if file exists, generate new if not
    - Save new mnemonics to file when generated
    - Return both mnemonic string and signer instance
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.6_
  
  - [x] 5.4 Implement generate_new_indexed() method
    - Load mnemonic at specified index if it exists
    - Generate new mnemonic and append to array if index doesn't exist
    - Update JSON file with new mnemonic when added
    - Return mnemonic string and signer instance
    - _Requirements: 2.4, 2.5, 2.6_

- [ ] 6. Implement Signer trait for LwkSoftwareSigner
  - [x] 6.1 Implement sign_transaction method
    - Parse unsigned transaction hex to elements::Transaction
    - Use SwSigner to sign the transaction
    - Serialize signed transaction back to hex string
    - Add logging for successful signing operations
    - _Requirements: 3.1, 3.4_
  
  - [x] 6.2 Add comprehensive error handling for signing
    - Handle hex parsing errors with HexParse variant
    - Handle transaction deserialization with InvalidTransaction variant
    - Handle LWK signing errors with Lwk variant
    - Ensure all error paths provide meaningful context
    - _Requirements: 3.2, 3.3, 3.5_

- [ ] 7. Create comprehensive test suite
  - [x] 7.1 Create unit tests for error handling
    - Test all SignerError variants and conversions
    - Verify error message preservation and formatting
    - Test error propagation through the call stack
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_
  
  - [x] 7.2 Create tests for mnemonic generation and validation
    - Test generate_new() with missing and existing files
    - Test generate_new_indexed() with various index scenarios
    - Verify 12-word mnemonic generation and validation
    - Test mnemonic persistence and loading
    - _Requirements: 7.1, 7.2, 7.6_
  
  - [x] 7.3 Create tests for JSON file operations
    - Test reading valid and invalid JSON files
    - Test writing and updating mnemonic arrays
    - Test file I/O error scenarios and recovery
    - Test atomic write operations
    - _Requirements: 2.1, 2.3, 2.8_
  
  - [x] 7.4 Create tests for signer functionality
    - Test signer creation with various mnemonic inputs
    - Test network configuration validation (is_testnet)
    - Test basic transaction signing flow with mock data
    - Verify thread safety and async compatibility
    - _Requirements: 7.3, 7.4, 7.5, 7.7_
  
  - [ ]* 7.5 Create integration tests for LWK compatibility
    - Test SwSigner integration and configuration
    - Verify Liquid transaction support and confidential features
    - Test mnemonic-to-signer pipeline end-to-end
    - _Requirements: 1.3, 6.4_

- [ ] 8. Add documentation and security warnings
  - [x] 8.1 Add comprehensive code documentation
    - Document all public methods with usage examples
    - Add security warnings for testnet-only usage
    - Document JSON file format and structure
    - _Requirements: 6.1, 6.2_
  
  - [x] 8.2 Create usage examples and integration guides
    - Create example showing basic signer usage
    - Document integration with asset operation functions
    - Show indexed mnemonic access patterns for tests
    - _Requirements: 6.3_

- [ ] 9. Final integration and validation
  - [x] 9.1 Update module exports and public API
    - Export Signer trait and LwkSoftwareSigner from lib.rs
    - Ensure clean public API surface
    - Verify no internal implementation details are exposed
    - _Requirements: 5.2_
  
  - [x] 9.2 Run comprehensive test suite and validation
    - Execute all unit and integration tests
    - Verify no compilation warnings or errors
    - Test with both existing and fresh mnemonic.local.json files
    - Validate async/await compatibility across the codebase
    - _Requirements: 7.7_