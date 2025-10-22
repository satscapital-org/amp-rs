# End-to-End Distribution Workflow Example

This example demonstrates a complete asset distribution workflow that mirrors the `test_end_to_end_distribution_workflow` test but is structured as a standalone, reusable example.

## Features

- **Idempotent Operation**: Can be run multiple times safely
- **Distinctive Asset Naming**: Uses timestamps to avoid conflicts
- **Maximum Circulation**: Issues assets with 21M maximum supply
- **Configurable GAID**: Easy to change target GAID for demonstrations
- **Complete Workflow**: Handles asset creation, authorization, and distribution

## Configuration

### GAID Setting
The target GAID is configured at the top of the file for easy modification:

```rust
// GAID Configuration - Change this for different demonstration purposes
const DEMO_GAID: &str = "GAbzSbgCZ6M6WU85rseKTrfehPsjt";
```

### Asset Configuration
Assets are named distinctively using timestamps:

```rust
const ASSET_NAME_PREFIX: &str = "Demo Distribution Asset";
const ASSET_TICKER_PREFIX: &str = "DDA";
```

### Distribution Amount
The distribution amount is configurable:

```rust
const DISTRIBUTION_AMOUNT_SATS: u64 = 1000; // 1000 satoshis for demo
```

## Workflow Steps

1. **Infrastructure Setup**: Initialize API client and configuration
2. **Asset Management**: 
   - Check for existing asset by name
   - Create new asset with maximum circulation if needed
3. **Treasury Setup**: Configure treasury address for funding
4. **Authorization**: Register asset as authorized for distribution
5. **User Setup**: Register user and get destination address
6. **Assignment Creation**: Create asset assignments for distribution
7. **Distribution Execution**: Execute the actual distribution
8. **Verification**: Verify distribution completion

## Usage

### Prerequisites

1. Set up environment variables in `.env` file:
   ```
   AMP_USERNAME=your_username
   AMP_PASSWORD=your_password
   ```

2. Ensure Python scripts are available:
   - `gaid-scripts/address.py` for GAID address conversion

### Running the Example

```bash
# Run the example
cargo run --example end_to_end_distribution_workflow

# Check compilation only
cargo check --example end_to_end_distribution_workflow
```

### Expected Output

The example provides detailed logging of each step:

```
🚀 End-to-End Distribution Workflow Example
==========================================
📁 Loading environment configuration
🏗️  Initializing infrastructure components
✅ Infrastructure initialized
   - ApiClient: Live strategy
🎯 Asset Configuration
   - Name: Demo Distribution Asset 1640995200
   - Ticker: DDA5200
   - Target GAID: GAbzSbgCZ6M6WU85rseKTrfehPsjt
...
🎉 End-to-End Distribution Workflow Completed Successfully!
```

## Customization for Demonstrations

### Changing the Target GAID

Simply modify the `DEMO_GAID` constant at the top of the file:

```rust
const DEMO_GAID: &str = "YOUR_TARGET_GAID_HERE";
```

### Adjusting Distribution Amount

Modify the `DISTRIBUTION_AMOUNT_SATS` constant:

```rust
const DISTRIBUTION_AMOUNT_SATS: u64 = 5000; // 5000 satoshis
```

### Asset Naming

Customize the asset naming by changing the prefix constants:

```rust
const ASSET_NAME_PREFIX: &str = "My Custom Asset";
const ASSET_TICKER_PREFIX: &str = "MCA";
```

## Differences from Test Version

This example differs from the test in several key ways:

1. **Simplified Distribution**: Uses `create_distribution` instead of `distribute_asset` to avoid requiring Elements RPC setup
2. **Configurable Parameters**: All key parameters are constants at the top of the file
3. **Better Error Handling**: More user-friendly error messages and recovery
4. **Documentation**: Extensive comments and logging for educational purposes
5. **Standalone Operation**: Doesn't require test infrastructure or cleanup

## Integration with Full Blockchain Distribution

To upgrade this example to use full blockchain distribution (like the test), you would need to:

1. Add Elements RPC setup
2. Add LwkSoftwareSigner integration
3. Replace `create_distribution` with `distribute_asset`
4. Add wallet management and UTXO handling

See the `test_end_to_end_distribution_workflow` test for the complete blockchain integration pattern.