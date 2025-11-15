# Wallet Migration Examples

This directory contains examples for migrating Elements wallets between nodes.

## Overview

Elements wallets can be either **legacy wallets** or **descriptor wallets**. The migration approach differs:

### Legacy Wallets
- Use `dumpwallet` / `importwallet`
- Exports all private keys AND the master blinding key
- Blinding keys are automatically derived from the master blinding key
- **No need to export/import individual blinding keys**

### Descriptor Wallets  
- Use `listdescriptors` / `importdescriptors`
- Exports HD wallet descriptors with master keys
- Blinding keys are derived from the HD seed
- Preserves the full HD wallet structure

## Examples

### `migrate_test_wallet_to_cloud.rs`
Migrates a **legacy wallet** from local to cloud Elements node.

**Usage:**
```bash
cargo run --example migrate_test_wallet_to_cloud
```

**What it does:**
1. Detects wallet type (legacy vs descriptor)
2. Exports wallet using `dumpwallet` (includes master blinding key)
3. Provides instructions for manual file transfer and import

**For different servers:**
The script will provide manual steps like:
```bash
# 1. Copy wallet file
scp /tmp/wallet_export.dat user@cloud-server:/tmp/

# 2. Import on cloud node
elements-cli -rpcwallet=wallet_name importwallet /tmp/wallet_export.dat

# 3. Rescan blockchain
elements-cli -rpcwallet=wallet_name rescanblockchain
```

### `migrate_test_wallet_to_cloud_descriptors.rs`
Migrates a **descriptor wallet** from local to cloud Elements node.

**Usage:**
```bash
cargo run --example migrate_test_wallet_to_cloud_descriptors
```

**What it does:**
1. Exports all descriptors with private keys using `listdescriptors`
2. Creates descriptor wallet on cloud node
3. Imports all descriptors
4. Preserves HD wallet structure and blinding key derivation

## Important Notes

### Blinding Keys
- **Legacy wallets**: Blinding keys are derived from the master blinding key stored in the wallet
- **Descriptor wallets**: Blinding keys are derived from the HD seed
- **You do NOT need to export/import individual blinding keys** - they are automatically derived

### Confidential Addresses
- Confidential addresses are ~90 characters long (e.g., `tlq1qq...`)
- Unconfidential addresses are ~42-44 characters (e.g., `tex1q...`)
- Both types work with the same private key
- The blinding key enables confidential transactions

### Rescanning
After importing a wallet, you must rescan the blockchain to see existing transactions:
```bash
elements-cli -rpcwallet=wallet_name rescanblockchain
```

## Environment Variables

Set these in your `.env` file:

```bash
# Local Elements node
ELEMENTS_RPC_URL=http://127.0.0.1:18891
ELEMENTS_RPC_USER=liquiduser
ELEMENTS_RPC_PASSWORD=password

# Cloud Elements node  
CLOUD_ELEMENTS_RPC_URL=http://cloud-server:18891
CLOUD_ELEMENTS_RPC_USER=elements
CLOUD_ELEMENTS_RPC_PASSWORD=password
```

## Troubleshooting

### "RPC request failed with status: 500"
- Check that the wallet file is accessible to the Elements node
- Verify file permissions
- Ensure the path is correct

### "Wallet already exists"
- The import will add keys to the existing wallet
- Or unload/remove the existing wallet first

### "0 transactions after import"
- This is normal - run `rescanblockchain` to find existing transactions
- Rescanning can take time depending on blockchain size
