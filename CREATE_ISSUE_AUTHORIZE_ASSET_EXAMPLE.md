# Create, Issue, and Authorize Asset Example

## Overview

The `create_issue_authorize_asset.rs` example demonstrates the complete workflow for creating a new asset that can be used for distribution tests and other testing scenarios. This example implements all the requirements specified:

## Workflow Steps

### 1. Address Derivation ✅
- Derives both confidential and unconfidential addresses as a pair from the Elements wallet `amp_elements_wallet_static_for_funding`
- First derives the unconfidential address using native segwit (bech32) format for compatibility
- Then derives the corresponding confidential address from the unconfidential one
- Displays both addresses clearly for reference
- Throws an error if the wallet is not accessible or address derivation fails

### 2. Asset Issuance ✅
- Issues an asset with the maximum possible circulation amount (21 million with 8 decimal precision)
- Issues to the confidential address for privacy
- Uses reasonable defaults for domain, ticker, and other asset parameters
- Throws an error if the AMP API refuses the issuance request

### 3. Transaction Confirmation ✅
- Monitors the issuance transaction for confirmation in 30-second intervals
- Waits for 3 confirmations to ensure transaction stability
- Continues checking for up to 5 minutes (300 seconds)
- Throws an error if the transaction cannot reach 3 confirmations within the timeout period
- Provides detailed status updates during the waiting process

### 4. Treasury Address Management ✅
- Adds the issuance address to the asset's list of treasury addresses
- Verifies that the treasury address was successfully added
- Throws an error if the treasury address addition fails

### 5. Asset Authorization ✅
- Authorizes the asset for distribution using the AMP API
- Verifies that the asset is properly authorized and registered
- Throws an error if the authorization process fails

### 6. Success Information Display ✅
- Displays the Asset UUID and treasury address in a comprehensive success message
- Provides all relevant asset information for use in other tests
- Includes usage instructions and next steps

## Usage

```bash
# Set required environment variables
export AMP_TESTS=live
export AMP_USERNAME=your_username
export AMP_PASSWORD=your_password
export ELEMENTS_RPC_URL=http://localhost:18884
export ELEMENTS_RPC_USER=your_rpc_user
export ELEMENTS_RPC_PASSWORD=your_rpc_password

# Run the example
cargo run --example create_issue_authorize_asset
```

## Prerequisites

1. **Running Elements Node**: The example requires a running Elements node with RPC access
2. **Elements Wallet**: The wallet `amp_elements_wallet_static_for_funding` must exist and be loaded
3. **AMP API Access**: Valid AMP API credentials must be configured
4. **Live Environment**: The example requires `AMP_TESTS=live` to be set

## Output

Upon successful completion, the example outputs:

- Asset UUID (for use in other examples and tests)
- Asset ID (blockchain identifier)
- Treasury address (confidential address for asset management)
- Transaction ID (issuance transaction)
- Authorization status
- Usage instructions for the created asset

## Error Handling

The example includes comprehensive error handling for:

- Elements wallet connectivity issues
- Address derivation failures
- Asset issuance rejections
- Transaction confirmation timeouts
- Treasury address management failures
- Asset authorization failures

## Integration with Testing

This example creates assets that are ready for:

- Distribution tests
- Assignment creation
- Balance queries
- Transaction operations
- Other AMP API testing scenarios

The created assets have:
- Maximum circulation for extensive testing
- No transfer restrictions for flexible distribution
- Proper authorization for all operations
- Treasury addresses configured for management operations

## Security Considerations

- Uses confidential addresses for privacy
- Implements proper timeout handling to avoid infinite waits
- Validates all operations before proceeding to the next step
- Provides clear error messages for troubleshooting

This example serves as a foundation for creating test assets that can be used across the entire AMP testing suite.