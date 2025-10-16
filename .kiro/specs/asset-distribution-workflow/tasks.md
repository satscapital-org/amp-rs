# Implementation Plan

- [ ] 1. Set up core data structures and error handling
  - Create Assignment struct with user_id, address, and amount fields
  - Extend AmpError enum with new variants for RPC, Signer, Timeout, and Validation errors
  - Implement error conversion traits and helper methods for error context
  - _Requirements: 2.1, 2.2, 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] 2. Implement ElementsRpc client for blockchain operations
  - [x] 2.1 Create ElementsRpc struct with connection management
    - Implement constructor with URL, username, password from environment variables
    - Add basic RPC call method with authentication and error handling
    - Create connection validation and network info retrieval methods
    - _Requirements: 4.1, 4.2_

  - [x] 2.2 Implement UTXO and transaction management methods
    - Add list_unspent method to query available UTXOs for specific assets
    - Implement create_raw_transaction for building unsigned transactions with Liquid-specific outputs
    - Add send_raw_transaction method for broadcasting signed transactions
    - Implement get_transaction method for retrieving transaction details and confirmations
    - _Requirements: 4.2, 4.3, 4.4, 4.6, 8.1, 8.2, 8.3_

  - [x] 2.3 Write unit tests for ElementsRpc methods
    - Create mock RPC responses for testing UTXO queries and transaction operations
    - Test error handling for network failures and invalid RPC responses
    - Verify proper authentication and request formatting
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.6_

- [ ] 3. Create distribution API integration structures
  - [x] 3.1 Define distribution request and response data models
    - Create DistributionResponse struct with distribution_uuid and address mappings
    - Implement TransactionDetail struct for blockchain transaction information
    - Add Unspent struct for UTXO representation from Elements node
    - _Requirements: 2.1, 4.6_

  - [x] 3.2 Implement distribution creation API call
    - Add method to POST assignments to /api/assets/{uuid}/distributions/create/
    - Parse response to extract distribution_uuid and address mappings
    - Handle API errors and validation failures appropriately
    - _Requirements: 1.2, 1.3, 2.3, 5.1_

  - [x] 3.3 Implement distribution confirmation API call
    - Add method to POST transaction data to /api/assets/{uuid}/distributions/{uuid}/confirm
    - Format tx_data and change_data payloads according to API specification
    - Handle confirmation failures with retry instructions including txid
    - _Requirements: 1.6, 5.5_

- [ ] 4. Implement transaction construction and signing workflow
  - [x] 4.1 Create UTXO selection and transaction building logic
    - Query available UTXOs for the asset using ElementsRpc.list_unspent
    - Select appropriate UTXOs to cover distribution amounts plus fees
    - Build raw transaction with inputs from UTXOs and outputs from address mappings
    - Calculate and include proper change outputs for remaining asset amounts
    - _Requirements: 4.2, 4.3, 8.1, 8.2, 8.3, 8.4_

  - [x] 4.2 Integrate signer callback for transaction signing
    - Convert raw transaction to hex format for signing
    - Call signer.sign_transaction with unsigned transaction hex
    - Validate signed transaction format and structure
    - Handle signing errors and propagate them appropriately
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 5.3_

  - [x] 4.3 Write unit tests for transaction construction
    - Test UTXO selection algorithms with various scenarios
    - Mock signer interface to test signing integration
    - Verify transaction structure and Liquid-specific formatting
    - _Requirements: 4.1, 4.2, 4.3, 3.1, 3.2_

- [ ] 5. Implement confirmation polling and timeout handling
  - [x] 5.1 Create blockchain confirmation monitoring
    - Poll ElementsRpc.get_transaction every 15 seconds for confirmation status
    - Track confirmation count and wait for 2 confirmations minimum
    - Implement configurable timeout with 10-minute default limit
    - _Requirements: 1.5, 4.5, 5.4_

  - [x] 5.2 Add change data collection for confirmation
    - Query ElementsRpc.list_unspent filtered by asset_id and txid after confirmation
    - Format change data according to API specification for confirmation payload
    - Handle cases where no change outputs exist
    - _Requirements: 1.6, 4.6_

  - [x] 5.3 Write unit tests for confirmation logic
    - Mock blockchain polling with various confirmation scenarios
    - Test timeout handling and error conditions
    - Verify change data collection and formatting
    - _Requirements: 1.5, 1.6, 4.5, 4.6, 5.4_

- [ ] 6. Implement main distribute_asset function
  - [x] 6.1 Create function signature and input validation
    - Implement distribute_asset method on ApiClient with specified signature
    - Validate asset_uuid format and assignments data structure
    - Check ElementsRpc connection and signer interface availability
    - _Requirements: 1.1, 2.2, 2.4, 5.1_

  - [x] 6.2 Orchestrate the complete distribution workflow
    - Authenticate with AMP API using existing TokenManager
    - Create distribution request and parse response data
    - Verify Elements node status and execute transaction workflow
    - Wait for confirmations and submit final confirmation to AMP API
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 4.1_

  - [x] 6.3 Add comprehensive error handling and logging
    - Implement tracing for each step of the distribution process
    - Provide detailed error context and retry instructions
    - Handle all error scenarios with appropriate AmpError variants
    - _Requirements: 1.7, 5.1, 5.2, 5.3, 5.4, 5.5, 7.1, 7.2, 7.3, 7.4_

- [ ] 7. Create comprehensive integration test suite
  - [x] 7.1 Set up test environment and infrastructure
    - Load environment variables using dotenvy for RPC and AMP credentials
    - Create ApiClient with testnet configuration and ElementsRpc instance
    - Generate LwkSoftwareSigner with new mnemonic for test isolation
    - _Requirements: 6.1, 6.2, 6.3_

  - [x] 7.2 Implement test asset and user setup workflow
    - Issue test asset with proper treasury address assignment
    - Register test user with valid GAID and address verification
    - Create test category and associate user and asset appropriately
    - Set up initial asset assignments to treasury for distribution funding
    - _Requirements: 6.4, 6.5_

  - [x] 7.3 Execute end-to-end distribution test workflow
    - Create assignment vector with test user and address
    - Call distribute_asset with LwkSoftwareSigner as signing callback
    - Verify distribution completion through AMP API queries
    - Validate blockchain transaction confirmation and asset transfer
    - _Requirements: 6.4, 6.5_

  - [x] 7.4 Implement test cleanup and data isolation
    - Detach users and assets from categories before deletion
    - Delete test entities in proper order to avoid constraint violations
    - Ensure test isolation and cleanup for repeated test execution
    - _Requirements: 6.6_

  - [x] 7.5 Add error scenario and edge case testing
    - Test network failures, signing failures, and timeout conditions
    - Verify error handling for insufficient UTXOs and invalid addresses
    - Test duplicate distribution prevention and retry scenarios
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_