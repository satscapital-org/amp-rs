# Signer Module Documentation

## ⚠️ CRITICAL SECURITY WARNING ⚠️

**THIS MODULE IS FOR TESTNET/REGTEST ONLY**

The signer implementations in this module store mnemonic phrases in **PLAIN TEXT** and are designed exclusively for development and testing environments. **NEVER** use these signers in production or with real funds.

## Overview

The signer module provides transaction signing capabilities for Elements/Liquid transactions using Blockstream's Liquid Wallet Kit (LWK). It supports:

- Software-based signing with mnemonic phrases
- Persistent mnemonic storage in JSON format
- Multiple signers with indexed access for test isolation
- Full BIP39 mnemonic validation and generation
- Async transaction signing interface

## JSON File Format

### File Location
- **Filename**: `mnemonic.local.json`
- **Location**: Current working directory
- **Encoding**: UTF-8 JSON

### File Structure

```json
{
  "mnemonic": [
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
    "legal winner thank year wave sausage worth useful legal winner thank yellow",
    "additional mnemonics as needed for test scenarios..."
  ]
}
```

### Field Specifications

#### `mnemonic` (Array of Strings)
- **Type**: Array of BIP39 mnemonic phrases
- **Format**: Space-separated lowercase English words
- **Word Count**: 12, 15, 18, 21, or 24 words (BIP39 standard)
- **Validation**: Full BIP39 checksum validation on load
- **Indexing**: Zero-based array indexing for consistent access

### File Operations

#### Automatic Creation
- File is created automatically when first mnemonic is generated
- Missing file is handled gracefully (returns empty storage)
- Empty file is treated as empty storage

#### Atomic Updates
- Updates use temporary file + rename for atomic writes
- Prevents corruption during concurrent access
- Maintains data integrity during system failures

#### Error Handling
- Invalid JSON structure returns `SignerError::Serialization`
- File I/O errors return `SignerError::FileIo`
- Mnemonic validation errors return `SignerError::InvalidMnemonic`

## Usage Examples

### Basic Signer Creation

```rust
use amp_rs::signer::{Signer, LwkSoftwareSigner};

// From existing mnemonic
let signer = LwkSoftwareSigner::new(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
)?;

// Generate new or load from file
let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
```

### Integration with Asset Operations

The signer integrates seamlessly with asset operation functions through the `Signer` trait:

```rust
use amp_rs::signer::{Signer, LwkSoftwareSigner, SignerError};
use amp_rs::client::ApiClient;

/// Example asset reissuance with integrated signing
async fn reissue_asset_with_signer(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
    amount: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    // 1. Create reissuance request through AMP API
    let reissuance_request = client.create_reissuance_request(asset_id, amount).await?;
    
    // 2. Get unsigned transaction from the response
    let unsigned_tx = reissuance_request.unsigned_transaction;
    
    // 3. Sign the transaction using the signer
    let signed_tx = signer.sign_transaction(&unsigned_tx).await?;
    
    // 4. Submit signed transaction back to AMP
    let result = client.submit_signed_transaction(&signed_tx).await?;
    
    Ok(result.transaction_id)
}

/// Example asset distribution with multiple signers
async fn distribute_asset_multi_signer(
    client: &ApiClient,
    issuer_signer: &dyn Signer,
    distributor_signer: &dyn Signer,
    asset_id: &str,
    recipients: Vec<(String, u64)>, // (address, amount) pairs
) -> Result<String, Box<dyn std::error::Error>> {
    // 1. Create distribution request (requires issuer signature)
    let distribution_request = client.create_distribution_request(asset_id, recipients).await?;
    
    // 2. Sign with issuer signer first
    let issuer_signed_tx = issuer_signer.sign_transaction(&distribution_request.unsigned_transaction).await?;
    
    // 3. If multi-sig required, sign with distributor
    let final_signed_tx = distributor_signer.sign_transaction(&issuer_signed_tx).await?;
    
    // 4. Submit final signed transaction
    let result = client.submit_signed_transaction(&final_signed_tx).await?;
    
    Ok(result.transaction_id)
}

/// Example asset burning with validation
async fn burn_asset_with_validation(
    client: &ApiClient,
    signer: &dyn Signer,
    asset_id: &str,
    amount: u64,
) -> Result<String, SignerError> {
    // 1. Validate signer is configured for testnet
    if !signer.is_testnet() {
        return Err(SignerError::Network("Signer not configured for testnet".to_string()));
    }
    
    // 2. Create burn request
    let burn_request = client.create_burn_request(asset_id, amount).await
        .map_err(|e| SignerError::Lwk(format!("API request failed: {}", e)))?;
    
    // 3. Sign and submit
    let signed_tx = signer.sign_transaction(&burn_request.unsigned_transaction).await?;
    let result = client.submit_signed_transaction(&signed_tx).await
        .map_err(|e| SignerError::Lwk(format!("Transaction submission failed: {}", e)))?;
    
    Ok(result.transaction_id)
}
```

### Indexed Mnemonic Access

```rust
use amp_rs::signer::LwkSoftwareSigner;

// Get specific mnemonic by index (generates if needed)
let (mnemonic_0, signer_0) = LwkSoftwareSigner::generate_new_indexed(0)?;
let (mnemonic_1, signer_1) = LwkSoftwareSigner::generate_new_indexed(1)?;
let (mnemonic_5, signer_5) = LwkSoftwareSigner::generate_new_indexed(5)?;

// This creates mnemonics at indices 0, 1, 2, 3, 4, 5
// Indices 2, 3, 4 are automatically generated
```

### Test Isolation Patterns

The indexed mnemonic system enables consistent test isolation and role-based testing:

```rust
use amp_rs::signer::LwkSoftwareSigner;

/// Test pattern: Role-based signers with consistent indices
#[tokio::test]
async fn test_asset_lifecycle_with_roles() -> Result<(), Box<dyn std::error::Error>> {
    // Use consistent indices for different roles across test runs
    let (_, issuer_signer) = LwkSoftwareSigner::generate_new_indexed(100)?;      // Asset issuer
    let (_, distributor_signer) = LwkSoftwareSigner::generate_new_indexed(101)?; // Asset distributor  
    let (_, user_a_signer) = LwkSoftwareSigner::generate_new_indexed(102)?;      // End user A
    let (_, user_b_signer) = LwkSoftwareSigner::generate_new_indexed(103)?;      // End user B
    
    // Each test run uses the same mnemonics for consistent addresses
    // This enables predictable testing of multi-party asset operations
    
    // Test asset issuance
    let asset_id = issue_asset(&issuer_signer).await?;
    
    // Test asset distribution
    distribute_to_users(&distributor_signer, &asset_id, vec![
        (get_address(&user_a_signer), 1000),
        (get_address(&user_b_signer), 2000),
    ]).await?;
    
    // Test asset transfers between users
    transfer_asset(&user_a_signer, &user_b_signer, &asset_id, 500).await?;
    
    Ok(())
}

/// Test pattern: Isolated test scenarios with unique index ranges
#[tokio::test]
async fn test_concurrent_operations() -> Result<(), Box<dyn std::error::Error>> {
    // Use index range 200-299 for this test to avoid conflicts
    let test_signers: Vec<_> = (200..210)
        .map(|i| LwkSoftwareSigner::generate_new_indexed(i))
        .collect::<Result<Vec<_>, _>>()?;
    
    // Run concurrent operations with isolated signers
    let handles: Vec<_> = test_signers.into_iter().enumerate().map(|(i, (_, signer))| {
        tokio::spawn(async move {
            // Each task has its own signer with unique mnemonic
            perform_asset_operation(signer, format!("test_asset_{}", i)).await
        })
    }).collect();
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await??;
    }
    
    Ok(())
}

/// Test pattern: Multi-environment testing
#[cfg(test)]
mod test_environments {
    use super::*;
    
    /// Test signers for regtest environment (indices 1000-1999)
    pub async fn get_regtest_signer(role: &str) -> Result<LwkSoftwareSigner, SignerError> {
        let index = match role {
            "issuer" => 1000,
            "distributor" => 1001,
            "user_a" => 1002,
            "user_b" => 1003,
            "treasury" => 1004,
            _ => return Err(SignerError::InvalidMnemonic(format!("Unknown role: {}", role))),
        };
        
        let (_, signer) = LwkSoftwareSigner::generate_new_indexed(index)?;
        Ok(signer)
    }
    
    /// Test signers for liquid testnet environment (indices 2000-2999)
    pub async fn get_testnet_signer(role: &str) -> Result<LwkSoftwareSigner, SignerError> {
        let index = match role {
            "issuer" => 2000,
            "distributor" => 2001,
            "user_a" => 2002,
            "user_b" => 2003,
            "treasury" => 2004,
            _ => return Err(SignerError::InvalidMnemonic(format!("Unknown role: {}", role))),
        };
        
        let (_, signer) = LwkSoftwareSigner::generate_new_indexed(index)?;
        Ok(signer)
    }
}
```

### Transaction Signing

```rust
use amp_rs::signer::{Signer, LwkSoftwareSigner};

let (_, signer) = LwkSoftwareSigner::generate_new()?;
let unsigned_tx = "020000000001..."; // Your unsigned transaction hex
let signed_tx = signer.sign_transaction(unsigned_tx).await?;
```

### Error Handling

```rust
use amp_rs::signer::{SignerError, LwkSoftwareSigner};

match LwkSoftwareSigner::new("invalid mnemonic") {
    Ok(signer) => { /* Use signer */ },
    Err(SignerError::InvalidMnemonic(msg)) => {
        eprintln!("Invalid mnemonic: {}", msg);
    },
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Mnemonic Management

### Generation
- Uses cryptographically secure randomness (`OsRng`)
- Generates 12-word mnemonics by default
- Full BIP39 compliance with checksum validation
- English wordlist only

### Validation
- Word count validation (12, 15, 18, 21, or 24 words)
- Character validation (lowercase letters only)
- BIP39 checksum validation
- Format validation (no multiple spaces, empty words)

### Storage
- Automatic persistence to `mnemonic.local.json`
- Array-based storage for multiple mnemonics
- Indexed access for consistent test identification
- Atomic file updates to prevent corruption

## Network Configuration

All signers are configured for **testnet/regtest only**:
- `is_testnet()` always returns `true`
- Compatible with Elements regtest and Liquid testnet
- Supports confidential transactions and Liquid features
- **Never** configured for mainnet (security restriction)

## Thread Safety

The signer is fully thread-safe:
- Implements `Send + Sync` traits
- Safe for concurrent signing operations
- No internal mutable state after creation
- Can be shared across async tasks

## Security Considerations

### Development Only
- Plain text mnemonic storage
- Unencrypted private keys in memory
- No password protection
- No hardware security features

### Production Alternatives
For production use, consider:
- **Hardware Wallets**: Ledger, Trezor
- **Encrypted Storage**: Key derivation with passwords
- **Remote Signing**: HSM-backed signing services
- **Multi-signature**: Distributed key management

### Best Practices
- Use only in isolated test environments
- Never commit `mnemonic.local.json` to version control
- Regularly rotate test mnemonics
- Use different mnemonics for different test scenarios
- Monitor file permissions on mnemonic storage

## Error Reference

### `SignerError::InvalidMnemonic`
- Invalid word count
- Invalid characters or formatting
- BIP39 checksum validation failure
- Empty or malformed mnemonic

### `SignerError::Lwk`
- SwSigner creation failure
- Transaction signing failure
- PSET operation errors

### `SignerError::HexParse`
- Invalid hex characters
- Odd-length hex strings
- Empty hex input

### `SignerError::InvalidTransaction`
- Malformed transaction structure
- Missing inputs or outputs
- Transaction deserialization failure

### `SignerError::FileIo`
- File read/write errors
- Permission denied
- Disk space issues

### `SignerError::Serialization`
- JSON parsing errors
- Invalid file structure
- Serialization failures

## Testing

Run the signer usage example:

```bash
cargo run --example signer_usage
```

This example demonstrates:
- Signer creation patterns
- Mnemonic management
- Error handling
- Multi-signer scenarios
- JSON file operations

## File Management

### Backup
```bash
# Backup your test mnemonics
cp mnemonic.local.json mnemonic.backup.json
```

### Reset
```bash
# Start fresh (removes all test mnemonics)
rm mnemonic.local.json
```

### Inspect
```bash
# View current mnemonics
cat mnemonic.local.json | jq .
```

Remember: These are test mnemonics only. Never use with real funds!