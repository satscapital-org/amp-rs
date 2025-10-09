# Test Environment Setup

This document describes how to set up and maintain a test environment for the AMP client library, specifically for the GAID balance tests.

## Overview

The test environment consists of:
- A test user with GAID `GAbzSbgCZ6M6WU85rseKTrfehPsjt`
- A test asset named "Test Environment Asset" with ticker "TENV"
- A test category named "Test Environment Category"
- Asset assignments linking the user and asset with 100,000 satoshis (0.001 BTC)

## Setup

### 1. Run the Setup Example

First, ensure you have the required environment variables set:

```bash
export AMP_USERNAME="your_username"
export AMP_PASSWORD="your_password"
export AMP_TESTS="live"
```

Then run the setup example:

```bash
cargo run --example setup_test_environment
```

This will:
1. Create or find the test category
2. Create or find the test user with the specified GAID
3. Assign the user to the category
4. Create or find the test asset
5. Issue the asset to the target address `vjTwqhz69nh7xHhtsHnx7mezsJV95EYHPqxshuoVXEMS5sqVzok57YVWYKDLcanqdSq54oTNhNM1NuTB`
6. Assign the asset to the category
7. Create asset assignments for the test user

### 2. Verify the Setup

After running the setup, you can verify it worked by running the GAID balance tests:

```bash
# Test GAID balance
AMP_TESTS=live cargo test test_get_gaid_balance_live -- --ignored

# Test GAID asset balance
AMP_TESTS=live cargo test test_get_gaid_asset_balance_live -- --ignored
```

## Cleanup

### Protected Resources

The cleanup example has been modified to preserve the test environment resources:

- **Test Category**: "Test Environment Category" - will not be deleted
- **Test User**: User with GAID `GAbzSbgCZ6M6WU85rseKTrfehPsjt` - will not be deleted
- **Test Asset**: "Test Environment Asset" - will not be deleted

### Running Cleanup

To clean up all other resources while preserving the test environment:

```bash
cargo run --example cleanup_resources
```

This will delete all assets, categories, and users EXCEPT the test environment resources.

## Test Details

### test_get_gaid_balance_live

This test:
1. Validates the test GAID `GAbzSbgCZ6M6WU85rseKTrfehPsjt`
2. Retrieves the balance for the GAID
3. Verifies the response format and expected balance entries

### test_get_gaid_asset_balance_live

This test:
1. Validates the test GAID `GAbzSbgCZ6M6WU85rseKTrfehPsjt`
2. Retrieves the asset balance for the specific test asset
3. Verifies the ownership entry shows 100,000 satoshis (0.001 BTC)

## Troubleshooting

### Setup Issues

If the setup fails:
1. Ensure you have valid AMP credentials
2. Check that the `address.py` script is available and working
3. Verify network connectivity to the AMP API

### Test Failures

If tests fail:
1. Run the setup example again to ensure the environment is correct
2. Check that the test GAID is valid using the validate_gaid endpoint
3. Verify the asset assignments exist using the AMP web interface

### Cleanup Issues

If cleanup fails to preserve test resources:
1. Check the protected resource constants in `cleanup_resources.rs`
2. Manually verify the test resources exist before running cleanup
3. Re-run the setup example if test resources were accidentally deleted

## Constants

The following constants define the test environment:

```rust
const TEST_CATEGORY_NAME: &str = "Test Environment Category";
const TEST_USER_GAID: &str = "GAbzSbgCZ6M6WU85rseKTrfehPsjt";
const TEST_ASSET_NAME: &str = "Test Environment Asset";
```

These are used in both the setup and cleanup examples to ensure consistency.