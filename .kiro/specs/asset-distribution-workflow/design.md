# Design Document

## Overview

The `distribute_asset` function provides a comprehensive, single-method solution for distributing assets through the Blockstream AMP platform. It orchestrates the entire workflow from authentication through blockchain confirmation, integrating AMP API calls, Elements node RPC operations, and transaction signing via a callback interface.

The design follows the existing codebase patterns using async/await, comprehensive error handling with custom error types, and the established retry mechanisms. The function acts as a high-level orchestrator that coordinates multiple subsystems while maintaining clean separation of concerns through the Signer trait abstraction.

## Architecture

### Core Components

```mermaid
graph TB
    A[ApiClient::distribute_asset] --> B[AMP API Authentication]
    A --> C[Distribution Creation]
    A --> D[Elements Node Interaction]
    A --> E[Transaction Signing]
    A --> F[Blockchain Confirmation]
    A --> G[Distribution Confirmation]
    
    B --> H[TokenManager]
    C --> I[POST /api/assets/{uuid}/distributions/create/]
    D --> J[ElementsRpc]
    E --> K[Signer Trait]
    F --> L[Confirmation Polling]
    G --> M[POST /api/assets/{uuid}/distributions/{uuid}/confirm]
    
    J --> N[listunspent]
    J --> O[createrawtransaction]
    J --> P[sendrawtransaction]
    J --> Q[gettransaction]
    
    K --> R[LwkSoftwareSigner]
    K --> S[Future Hardware Signers]
```

### Data Flow

1. **Input Validation**: Validate asset_uuid, assignments, and signer interface
2. **Authentication**: Use existing TokenManager for AMP API authentication
3. **Distribution Setup**: Create distribution request with AMP API
4. **Node Verification**: Verify Elements node status and capabilities
5. **UTXO Management**: Query and select appropriate UTXOs for the asset
6. **Transaction Construction**: Build raw transaction with proper inputs/outputs
7. **Signing**: Use callback interface to sign the transaction
8. **Broadcasting**: Submit signed transaction to Elements network
9. **Confirmation**: Wait for blockchain confirmations with timeout
10. **Finalization**: Confirm distribution with AMP API

## Components and Interfaces

### Primary Function Signature

```rust
impl ApiClient {
    pub async fn distribute_asset(
        &self,
        asset_uuid: &str,
        assignments: Vec<Assignment>,
        node_rpc: &ElementsRpc,
        signer: &dyn Signer,
    ) -> Result<(), AmpError>
}
```

### Supporting Data Structures

```rust
/// Assignment for asset distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub user_id: String,
    pub address: String,
    pub amount: f64,
}

/// Response from distribution creation API
#[derive(Debug, Deserialize)]
pub struct DistributionResponse {
    pub distribution_uuid: String,
    pub map_address_amount: HashMap<String, f64>,
    pub map_address_asset: HashMap<String, String>,
    pub asset_id: String,
}

/// Elements RPC client for blockchain operations
#[derive(Debug)]
pub struct ElementsRpc {
    client: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

/// Transaction details from Elements node
#[derive(Debug, Deserialize)]
pub struct TransactionDetail {
    pub txid: String,
    pub confirmations: u32,
    pub blockheight: Option<u64>,
    pub hex: String,
    // Additional fields as needed
}

/// UTXO information from Elements node
#[derive(Debug, Deserialize)]
pub struct Unspent {
    pub txid: String,
    pub vout: u32,
    pub amount: f64,
    pub asset: String,
    pub address: String,
    pub spendable: bool,
}

/// Enhanced error enum for distribution operations
#[derive(Error, Debug)]
pub enum AmpError {
    #[error("API error: {0}")]
    Api(String),
    
    #[error("RPC error: {0}")]
    Rpc(String),
    
    #[error("Signer error: {0}")]
    Signer(#[from] SignerError),
    
    #[error("Timeout waiting for confirmations: {0}")]
    Timeout(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    // Existing Error variants...
    #[error(transparent)]
    Existing(#[from] crate::client::Error),
}
```

### ElementsRpc Implementation

```rust
impl ElementsRpc {
    pub fn new(url: String, username: String, password: String) -> Self;
    
    pub async fn get_network_info(&self) -> Result<NetworkInfo, AmpError>;
    pub async fn get_blockchain_info(&self) -> Result<BlockchainInfo, AmpError>;
    pub async fn wallet_passphrase(&self, passphrase: &str, timeout: u64) -> Result<(), AmpError>;
    
    pub async fn list_unspent(&self, asset_id: Option<&str>) -> Result<Vec<Unspent>, AmpError>;
    pub async fn create_raw_transaction(
        &self,
        inputs: Vec<TxInput>,
        outputs: HashMap<String, f64>,
        assets: HashMap<String, String>,
    ) -> Result<String, AmpError>;
    
    pub async fn send_raw_transaction(&self, hex: &str) -> Result<String, AmpError>;
    pub async fn get_transaction(&self, txid: &str) -> Result<TransactionDetail, AmpError>;
    
    // Helper methods
    async fn rpc_call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, AmpError>;
}
```

### Signer Integration

The design leverages the existing Signer trait without modification:

```rust
// Existing trait - no changes needed
#[async_trait]
pub trait Signer: Send + Sync {
    async fn sign_transaction(&self, unsigned_tx: &str) -> Result<String, SignerError>;
}
```

## Data Models

### Distribution Request Payload

```json
{
  "assignments": [
    {
      "user_uuid": "string",
      "amount": 100.0,
      "address": "confidential_address"
    }
  ]
}
```

### Distribution Response Structure

```json
{
  "distribution_uuid": "uuid-string",
  "map_address_amount": {
    "address1": 100.0,
    "address2": 50.0
  },
  "map_address_asset": {
    "address1": "asset_id_hex",
    "address2": "asset_id_hex"
  },
  "asset_id": "asset_id_hex"
}
```

### Confirmation Payload

```json
{
  "tx_data": {
    "details": { /* transaction details from gettransaction */ },
    "txid": "transaction_id"
  },
  "change_data": [
    { /* change UTXOs from listunspent */ }
  ]
}
```

## Error Handling

### Error Categories

1. **Validation Errors**: Invalid inputs, missing parameters
2. **Authentication Errors**: Token issues, API access problems
3. **RPC Errors**: Elements node communication failures
4. **Signing Errors**: Transaction signing failures via callback
5. **Network Errors**: Blockchain communication issues
6. **Timeout Errors**: Confirmation waiting timeouts

### Error Recovery Strategies

- **Retry Logic**: Use existing RetryClient for transient failures
- **Timeout Handling**: Configurable timeouts with clear error messages
- **State Recovery**: Provide transaction ID for manual confirmation retry
- **Logging**: Comprehensive tracing for debugging and monitoring

### Error Context Enhancement

```rust
impl AmpError {
    pub fn with_context(self, context: &str) -> Self {
        // Add contextual information to errors
    }
    
    pub fn is_retryable(&self) -> bool {
        // Determine if error condition is retryable
    }
    
    pub fn retry_instructions(&self) -> Option<String> {
        // Provide user-friendly retry instructions
    }
}
```

## Testing Strategy

### Test Environment Setup

1. **Environment Variables**: Load from .env using dotenvy
   - `ELEMENTS_RPC_URL`: Elements node RPC endpoint
   - `ELEMENTS_RPC_USER`: RPC authentication username
   - `ELEMENTS_RPC_PASSWORD`: RPC authentication password
   - `AMP_USERNAME`, `AMP_PASSWORD`: AMP API credentials

2. **Test Infrastructure**:
   - Use existing `AMP_TESTS=live` pattern for integration tests
   - LwkSoftwareSigner for testnet transaction signing
   - Cleanup procedures for test data isolation

### Test Workflow Structure

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_distribute_asset_workflow() -> Result<(), AmpError> {
        // 1. Environment setup
        dotenvy::dotenv().ok();
        let client = ApiClient::new(/* testnet config */)?;
        let elements_rpc = ElementsRpc::new(/* from env vars */);
        let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
        
        // 2. Asset and user setup
        let asset_uuid = setup_test_asset(&client).await?;
        let user_id = setup_test_user(&client).await?;
        let treasury_address = derive_treasury_address(&signer).await?;
        
        // 3. Category and assignment setup
        let category_id = setup_test_category(&client).await?;
        associate_user_to_category(&client, &user_id, &category_id).await?;
        assign_asset_to_treasury(&client, &asset_uuid, &treasury_address).await?;
        
        // 4. Distribution execution
        let assignments = vec![Assignment {
            user_id,
            address: "test_address".to_string(),
            amount: 100.0,
        }];
        
        client.distribute_asset(&asset_uuid, assignments, &elements_rpc, &signer).await?;
        
        // 5. Verification and cleanup
        verify_distribution_confirmed(&client, &asset_uuid).await?;
        cleanup_test_data(&client, &asset_uuid, &user_id, &category_id).await?;
        
        Ok(())
    }
}
```

### Mock Testing Strategy

- **ElementsRpc Mocking**: Create mock implementation for unit tests
- **Signer Mocking**: Mock signer for testing error conditions
- **API Response Mocking**: Use existing httpmock patterns for AMP API calls

### Integration Test Patterns

- **End-to-End Flow**: Complete workflow with real testnet
- **Error Scenarios**: Network failures, signing failures, timeout conditions
- **Edge Cases**: Insufficient UTXOs, invalid addresses, duplicate distributions

## Implementation Phases

### Phase 1: Core Infrastructure
- ElementsRpc implementation with basic RPC methods
- AmpError enum extension with new error types
- Assignment and response data structures

### Phase 2: Distribution Logic
- Main distribute_asset function implementation
- AMP API integration for distribution creation/confirmation
- UTXO selection and transaction construction logic

### Phase 3: Signing Integration
- Signer callback integration
- Transaction hex conversion and validation
- Error handling for signing failures

### Phase 4: Confirmation System
- Blockchain polling for confirmations
- Timeout handling with configurable limits
- Change data collection and processing

### Phase 5: Testing and Documentation
- Comprehensive test suite implementation
- Integration with existing test patterns
- Documentation and usage examples

## Security Considerations

### Credential Management
- Use existing secrecy patterns for RPC credentials
- Leverage dotenvy for secure environment variable loading
- No hardcoded credentials or sensitive data

### Transaction Security
- Validate transaction structure before signing
- Use callback pattern to isolate signing logic
- Comprehensive input validation for all parameters

### Network Security
- Use existing retry and timeout patterns
- Validate all API responses before processing
- Secure RPC communication with authentication

## Performance Considerations

### Async Operations
- All operations use async/await for non-blocking execution
- Concurrent operations where possible (API calls, RPC calls)
- Efficient polling with exponential backoff for confirmations

### Resource Management
- Connection pooling for RPC clients
- Memory-efficient UTXO selection algorithms
- Cleanup of temporary data structures

### Scalability
- Support for batch operations in future iterations
- Configurable timeouts and retry limits
- Efficient error propagation without excessive allocations