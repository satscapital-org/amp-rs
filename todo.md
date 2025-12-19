- [ ] Methods That Need Wallet-Specific Endpoints
      get_balance() (line ~2939)
        Uses rpc_call("getbalance") with base URL
        Called in burn_asset() (line ~13922) where wallet_name is available
        Needs wallet blinding keys for confidential balances
      destroyamount() (line ~2892)
        Uses rpc_call("destroyamount") with base URL
        Called in burn_asset() (line ~13952) where wallet_name is available
        Wallet operation that needs wallet-specific endpoint
      reissueasset() (line ~2777)
        Uses rpc_call("reissueasset") with base URL
        Called in reissue_asset() (line ~13336) where wallet_name should be available (but isn't currently passed)
        Wallet operation that needs wallet-specific endpoint
      list_unspent() (line ~1206)
        Uses rpc_call("listunspent") with base URL
        Called in reissue_asset() (line ~13300) and burn_asset() (line ~13887) where wallet_name is available
        Note: There's already list_unspent_for_wallet() that uses wallet-specific endpoint, but the code calls need to be migrated to use this instead of list_unspent
- [ ] Fix get_gaid_registered_user  
- [ ] Add error handling tests for edge cases
  - Create mock tests for invalid registered user IDs
  - Create mock tests for invalid GAIDs
  - Create mock tests for invalid asset UUIDs
  - Create mock tests for invalid category IDs
  - Create mock tests for authentication failures
  - Create mock tests for network errors and retry scenarios
  - Verify proper error types are returned for each failure mode
  - Each test must use setup_mock_test() and cleanup_mock_test() helpers
  - Each test must call cleanup_mock_test() in a defer/finally block or at test end to ensure cleanup occurs even if test fails
