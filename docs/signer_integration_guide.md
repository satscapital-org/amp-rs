# Signer Integration Guide

## ⚠️ CRITICAL SECURITY WARNING ⚠️

**THIS GUIDE IS FOR TESTNET/REGTEST ONLY**

The signer implementations described in this guide store mnemonic phrases in plain text and are designed exclusively for development and testing environments. **NEVER** use these patterns in production or with real funds.

## Overview

This guide demonstrates how to integrate the `LwkSoftwareSigner` with asset operations in the amp-rust crate. The signer provides transaction signing capabilities for Elements/Liquid transactions using the Signer trait interface.

## Basic Integration Pattern

### 1. Signer Creation and Setup

```rust
use amp_rs::signer::{Signer, LwkSoftwareSigner, SignerError};
use amp_rs::client::ApiClient;

// Create signer from existing mnemonic
let signer = LwkSoftwareSigner::new(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
)?;

// Or generate/load from persistent storage
let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
```

### 2. Asset Operation Integration

The signer integrates with asset operations through a standard pattern:

1. **Create Request**: Use AMP API to create an asset operation request
2. **Get Unsigned TX**: Extract unsigned transaction from API response
3. **Sign Transaction**: Use signer to sign the unsigned transaction
4. **Submit Result**: Send signed transaction back to AMP API

```rust
async fn asset_operation_pattern(
    client: &ApiClient,
    signer: &dyn Signer,
) -> Result<String, Box<dyn std::error::Error>> {
    // 1. Create request through AMP API
    let request = client.create_asset_request(/* parameters */).await?;
    
    // 2. Get unsigned transaction
    let unsigned_tx = request.unsigned_transaction;
    
    // 3. Sign transaction
    let signed_tx = signer.sign_transaction(&unsigned_tx).await?;
    
    // 4. Submit signed transaction
    let result = client.submit_signed_transaction(&signed_tx).await?;
    
    Ok(result.transaction_id)
}
```## 
Asset Operation Examples

### Asset Issuance

```rust
async fn issue_asset(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_name: &str,
    total_supply: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    // Create issuance request
    let issuance_request = client.create_asset_issuance(asset_name, total_supply).await?;
    
    // Sign the issuance transaction
    let signed_tx = signer.sign_transaction(&issuance_request.unsigned_transaction).await?;
    
    // Submit and get asset ID
    let result = client.submit_signed_transaction(&signed_tx).await?;
    
    Ok(result.asset_id)
}
```

### Asset Reissuance

```rust
async fn reissue_asset(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
    additional_amount: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    // Create reissuance request
    let reissuance_request = client.create_asset_reissuance(asset_id, additional_amount).await?;
    
    // Sign the reissuance transaction
    let signed_tx = signer.sign_transaction(&reissuance_request.unsigned_transaction).await?;
    
    // Submit transaction
    let result = client.submit_signed_transaction(&signed_tx).await?;
    
    Ok(result.transaction_id)
}
```

### Asset Distribution

```rust
async fn distribute_asset(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
    recipients: Vec<(String, u64)>, // (address, amount) pairs
) -> Result<String, Box<dyn std::error::Error>> {
    // Create distribution request
    let distribution_request = client.create_asset_distribution(asset_id, recipients).await?;
    
    // Sign the distribution transaction
    let signed_tx = signer.sign_transaction(&distribution_request.unsigned_transaction).await?;
    
    // Submit transaction
    let result = client.submit_signed_transaction(&signed_tx).await?;
    
    Ok(result.transaction_id)
}
```

### Asset Burning

```rust
async fn burn_asset(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
    burn_amount: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    // Validate signer configuration
    if !signer.is_testnet() {
        return Err("Signer must be configured for testnet".into());
    }
    
    // Create burn request
    let burn_request = client.create_asset_burn(asset_id, burn_amount).await?;
    
    // Sign the burn transaction
    let signed_tx = signer.sign_transaction(&burn_request.unsigned_transaction).await?;
    
    // Submit transaction
    let result = client.submit_signed_transaction(&signed_tx).await?;
    
    Ok(result.transaction_id)
}
```## Mu
lti-Signer Patterns

### Role-Based Asset Management

```rust
struct AssetManagementRoles {
    issuer: LwkSoftwareSigner,
    distributor: LwkSoftwareSigner,
    compliance: LwkSoftwareSigner,
    treasury: LwkSoftwareSigner,
}

impl AssetManagementRoles {
    async fn new() -> Result<Self, SignerError> {
        // Use consistent indices for predictable testing
        let (_, issuer) = LwkSoftwareSigner::generate_new_indexed(1000)?;
        let (_, distributor) = LwkSoftwareSigner::generate_new_indexed(1001)?;
        let (_, compliance) = LwkSoftwareSigner::generate_new_indexed(1002)?;
        let (_, treasury) = LwkSoftwareSigner::generate_new_indexed(1003)?;
        
        Ok(Self { issuer, distributor, compliance, treasury })
    }
}

async fn managed_asset_lifecycle(
    client: &ApiClient,
    roles: &AssetManagementRoles,
) -> Result<String, Box<dyn std::error::Error>> {
    // 1. Issue asset (issuer role)
    let asset_id = issue_asset(client, &roles.issuer, "ManagedCoin", 1000000).await?;
    
    // 2. Compliance approval (compliance role)
    let approval_tx = approve_asset_compliance(client, &roles.compliance, &asset_id).await?;
    
    // 3. Initial distribution (distributor role)
    let recipients = vec![
        ("user_address_1".to_string(), 10000),
        ("user_address_2".to_string(), 15000),
    ];
    let distribution_tx = distribute_asset(client, &roles.distributor, &asset_id, recipients).await?;
    
    // 4. Treasury operations (treasury role)
    let treasury_tx = treasury_buyback(client, &roles.treasury, &asset_id, 5000).await?;
    
    Ok(asset_id)
}
```

### Multi-Signature Operations

```rust
async fn multi_sig_asset_operation(
    client: &ApiClient,
    primary_signer: &dyn Signer,
    secondary_signer: &dyn Signer,
    asset_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Create operation request
    let operation_request = client.create_multi_sig_operation(asset_id).await?;
    
    // First signature
    let partially_signed_tx = primary_signer
        .sign_transaction(&operation_request.unsigned_transaction)
        .await?;
    
    // Second signature (if required by the transaction structure)
    let fully_signed_tx = secondary_signer
        .sign_transaction(&partially_signed_tx)
        .await?;
    
    // Submit final transaction
    let result = client.submit_signed_transaction(&fully_signed_tx).await?;
    
    Ok(result.transaction_id)
}
```##
 Test Isolation Patterns

### Index-Based Test Organization

```rust
// Test suite organization using index ranges
mod asset_tests {
    use super::*;
    
    // Basic functionality tests (indices 0-99)
    #[tokio::test]
    async fn test_basic_asset_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (_, signer) = LwkSoftwareSigner::generate_new_indexed(0)?;
        // Test basic operations...
        Ok(())
    }
    
    // Multi-user tests (indices 100-199)
    #[tokio::test]
    async fn test_multi_user_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        let (_, user_a) = LwkSoftwareSigner::generate_new_indexed(100)?;
        let (_, user_b) = LwkSoftwareSigner::generate_new_indexed(101)?;
        let (_, user_c) = LwkSoftwareSigner::generate_new_indexed(102)?;
        // Test multi-user interactions...
        Ok(())
    }
    
    // Role-based tests (indices 200-299)
    #[tokio::test]
    async fn test_role_based_operations() -> Result<(), Box<dyn std::error::Error>> {
        let (_, issuer) = LwkSoftwareSigner::generate_new_indexed(200)?;
        let (_, distributor) = LwkSoftwareSigner::generate_new_indexed(201)?;
        let (_, treasury) = LwkSoftwareSigner::generate_new_indexed(202)?;
        // Test role-based workflows...
        Ok(())
    }
}
```

### Environment-Specific Testing

```rust
mod environment_tests {
    use super::*;
    
    // Regtest environment (indices 1000-1999)
    async fn get_regtest_signers() -> Result<Vec<LwkSoftwareSigner>, SignerError> {
        let mut signers = Vec::new();
        for i in 1000..1005 {
            let (_, signer) = LwkSoftwareSigner::generate_new_indexed(i)?;
            signers.push(signer);
        }
        Ok(signers)
    }
    
    // Liquid testnet environment (indices 2000-2999)
    async fn get_testnet_signers() -> Result<Vec<LwkSoftwareSigner>, SignerError> {
        let mut signers = Vec::new();
        for i in 2000..2005 {
            let (_, signer) = LwkSoftwareSigner::generate_new_indexed(i)?;
            signers.push(signer);
        }
        Ok(signers)
    }
    
    #[tokio::test]
    async fn test_cross_environment_compatibility() -> Result<(), Box<dyn std::error::Error>> {
        let regtest_signers = get_regtest_signers().await?;
        let testnet_signers = get_testnet_signers().await?;
        
        // Verify all signers are testnet-configured
        for signer in regtest_signers.iter().chain(testnet_signers.iter()) {
            assert!(signer.is_testnet());
        }
        
        Ok(())
    }
}
```

### Concurrent Testing Patterns

```rust
#[tokio::test]
async fn test_concurrent_asset_operations() -> Result<(), Box<dyn std::error::Error>> {
    let base_index = 3000;
    let num_concurrent = 10;
    
    // Create concurrent tasks with isolated signers
    let handles: Vec<_> = (0..num_concurrent).map(|i| {
        tokio::spawn(async move {
            let (_, signer) = LwkSoftwareSigner::generate_new_indexed(base_index + i)?;
            
            // Each task performs isolated operations
            let asset_name = format!("ConcurrentAsset_{}", i);
            perform_isolated_asset_test(&signer, &asset_name).await
        })
    }).collect();
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await??;
    }
    
    Ok(())
}

async fn perform_isolated_asset_test(
    signer: &LwkSoftwareSigner,
    asset_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Isolated test operations using the provided signer
    // Each concurrent task has its own mnemonic and derived addresses
    Ok(())
}
```## Err
or Handling Patterns

### Comprehensive Error Handling

```rust
use amp_rs::signer::SignerError;

async fn robust_asset_operation(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    match signer.sign_transaction("unsigned_tx_hex").await {
        Ok(signed_tx) => {
            // Transaction signed successfully
            match client.submit_signed_transaction(&signed_tx).await {
                Ok(result) => Ok(result.transaction_id),
                Err(api_error) => {
                    tracing::error!("API submission failed: {}", api_error);
                    Err(format!("Failed to submit transaction: {}", api_error).into())
                }
            }
        },
        Err(SignerError::HexParse(e)) => {
            tracing::error!("Invalid transaction hex: {}", e);
            Err(format!("Transaction format error: {}", e).into())
        },
        Err(SignerError::InvalidTransaction(msg)) => {
            tracing::error!("Invalid transaction structure: {}", msg);
            Err(format!("Transaction validation failed: {}", msg).into())
        },
        Err(SignerError::Lwk(msg)) => {
            tracing::error!("LWK signing failed: {}", msg);
            Err(format!("Signing operation failed: {}", msg).into())
        },
        Err(SignerError::InvalidMnemonic(msg)) => {
            tracing::error!("Mnemonic validation failed: {}", msg);
            Err(format!("Signer configuration error: {}", msg).into())
        },
        Err(e) => {
            tracing::error!("Unexpected signing error: {}", e);
            Err(format!("Unexpected error: {}", e).into())
        }
    }
}
```

### Retry Patterns

```rust
use tokio::time::{sleep, Duration};

async fn asset_operation_with_retry(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
    max_retries: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut attempts = 0;
    
    loop {
        match perform_asset_operation(client, signer, asset_id).await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < max_retries => {
                attempts += 1;
                tracing::warn!("Asset operation failed (attempt {}): {}", attempts, e);
                
                // Exponential backoff
                let delay = Duration::from_millis(1000 * 2_u64.pow(attempts - 1));
                sleep(delay).await;
            },
            Err(e) => {
                tracing::error!("Asset operation failed after {} attempts: {}", attempts + 1, e);
                return Err(e);
            }
        }
    }
}
```

## Best Practices

### 1. Signer Lifecycle Management

- **Create signers once**: Reuse signer instances across multiple operations
- **Use consistent indices**: Employ predictable index patterns for testing
- **Validate configuration**: Always verify `is_testnet()` returns true
- **Handle errors gracefully**: Implement comprehensive error handling

### 2. Test Organization

- **Index ranges**: Use distinct index ranges for different test suites
- **Role separation**: Create dedicated signers for different roles
- **Environment isolation**: Use different index ranges for different environments
- **Concurrent safety**: Ensure each concurrent task uses unique indices

### 3. Security Considerations

- **Testnet only**: Never use in production or with real funds
- **File management**: Don't commit `mnemonic.local.json` to version control
- **Mnemonic rotation**: Regularly rotate test mnemonics
- **Access control**: Restrict access to mnemonic files in development environments

### 4. Integration Patterns

- **Trait usage**: Use `&dyn Signer` for polymorphic operations
- **Error propagation**: Properly handle and log all error types
- **Transaction validation**: Validate transactions before and after signing
- **Logging**: Include comprehensive logging for debugging

## Running Examples

```bash
# Run basic signer usage example
cargo run --example signer_usage

# Run asset operations integration example
cargo run --example asset_operations_integration

# Run tests with signer integration
cargo test signer -- --test-threads=1
```

## Troubleshooting

### Common Issues

1. **File Permission Errors**: Ensure write permissions for `mnemonic.local.json`
2. **Index Conflicts**: Use unique index ranges for different test scenarios
3. **Network Configuration**: Verify all signers return `is_testnet() == true`
4. **Transaction Format**: Ensure unsigned transactions are valid hex strings

### Debug Tips

- Enable logging with `RUST_LOG=debug` to see detailed signer operations
- Check `mnemonic.local.json` structure if file operations fail
- Verify transaction hex format before signing attempts
- Use different index ranges to isolate test failures

Remember: This integration guide is for testnet/regtest development only. Never use these patterns with real funds or in production environments.