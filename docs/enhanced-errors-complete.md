# Complete Enhanced Error Reporting Documentation

## Overview

The amp-rs library now provides **8 enhanced error variants** across all major error types, offering comprehensive diagnostic information for debugging and error handling. Each enhanced variant includes detailed context about what failed, where it failed, and the complete data that caused the failure.

## Enhanced Error Variants

### 1. Error::RequestFailedDetailed

**Purpose:** HTTP request failures with complete context  
**Use Cases:** API calls that fail with non-success status codes

**Fields:**
- `method` - HTTP method (GET, POST, PUT, DELETE)
- `endpoint` - Full URL that was called
- `status` - HTTP status code
- `error_message` - Error message or response body

**Example:**
```rust
Error::RequestFailedDetailed {
    method: "POST".to_string(),
    endpoint: "https://amp-test.blockstream.com/api/assets/issue".to_string(),
    status: reqwest::StatusCode::BAD_REQUEST,
    error_message: "Missing required field: amount".to_string(),
}
```

**Output:**
```
AMP request failed

Method: POST
Endpoint: https://amp-test.blockstream.com/api/assets/issue
Status: 400 Bad Request

Error: Missing required field: amount
```

---

### 2. AmpError::ApiDetailed

**Purpose:** API operation failures with endpoint details  
**Use Cases:** Distribution operations, asset management errors

**Fields:**
- `endpoint` - API endpoint that was called
- `method` - HTTP method used
- `error_message` - Detailed error message

**Example:**
```rust
AmpError::ApiDetailed {
    endpoint: "/distributions/create".to_string(),
    method: "POST".to_string(),
    error_message: "Insufficient funds in treasury".to_string(),
}
```

---

### 3. AmpError::RpcDetailed

**Purpose:** Elements RPC failures with method and raw response  
**Use Cases:** Blockchain operations, wallet queries, transaction broadcast

**Fields:**
- `rpc_method` - RPC method name (e.g., "sendrawtransaction")
- `params` - Parameters passed to the RPC call
- `error_message` - Error description
- `raw_response` - Complete raw response from RPC server

**Example:**
```rust
AmpError::RpcDetailed {
    rpc_method: "sendrawtransaction".to_string(),
    params: r#"["020000000001..."]"#.to_string(),
    error_message: "Transaction rejected by network".to_string(),
    raw_response: r#"{"error":{"code":-26,"message":"bad-txns-inputs-missingorspent"}}"#.to_string(),
}
```

**Output:**
```
RPC error: Transaction rejected by network

Method: sendrawtransaction
Parameters: ["020000000001..."]

Raw Response:
{"error":{"code":-26,"message":"bad-txns-inputs-missingorspent"}}
```

---

### 4. AmpError::SerializationDetailed

**Purpose:** JSON serialization/deserialization failures with context  
**Use Cases:** API response parsing, data structure conversions

**Fields:**
- `operation` - "serialize" or "deserialize"
- `data_type` - Type being processed (e.g., "IssuanceResponse")
- `context` - Description of what was being done
- `serde_error` - Original serde error message

**Example:**
```rust
AmpError::SerializationDetailed {
    operation: "deserialize".to_string(),
    data_type: "IssuanceResponse".to_string(),
    context: "Parsing asset issuance API response".to_string(),
    serde_error: "missing field `txid` at line 1 column 45".to_string(),
}
```

---

### 5. SignerError::LwkDetailed

**Purpose:** LWK signing failures with operation context  
**Use Cases:** Transaction signing, signer initialization

**Fields:**
- `operation` - LWK operation (e.g., "sign_transaction", "create_signer")
- `context` - Additional operation context
- `error_message` - Error from LWK

**Example:**
```rust
SignerError::LwkDetailed {
    operation: "sign_transaction".to_string(),
    context: "Signing distribution transaction with 3 inputs and 5 outputs".to_string(),
    error_message: "Failed to sign input at index 2: Missing signing key".to_string(),
}
```

---

### 6. SignerError::HexParseDetailed

**Purpose:** Hex parsing failures with context  
**Use Cases:** Transaction hex parsing, address validation

**Fields:**
- `parsing_context` - What was being parsed
- `hex_preview` - First 100 characters of the hex string
- `hex_error` - Original parsing error

**Example:**
```rust
SignerError::HexParseDetailed {
    parsing_context: "Raw transaction hex from AMP API response".to_string(),
    hex_preview: "0200000000010a5f3e2b1c9d8e7f6a5b4c3d2e1f0a9b...".to_string(),
    hex_error: "Invalid hex character 'z' at position 127".to_string(),
}
```

---

### 7. SignerError::InvalidTransactionDetailed

**Purpose:** Transaction validation failures with specifics  
**Use Cases:** Transaction structure validation, UTXO verification

**Fields:**
- `txid` - Transaction ID (if available)
- `validation_details` - Specific validation failure information
- `error_message` - High-level error description

**Example:**
```rust
SignerError::InvalidTransactionDetailed {
    txid: "abc123def456789".to_string(),
    validation_details: "Input 0 references UTXO abc:1 which does not exist".to_string(),
    error_message: "Transaction validation failed: missing input UTXOs".to_string(),
}
```

---

### 8. SignerError::SerializationDetailed

**Purpose:** Mnemonic/wallet serialization failures  
**Use Cases:** Saving mnemonics, wallet state persistence

**Fields:**
- `operation` - "serialize" or "deserialize"
- `data_type` - Type being processed
- `context` - File path or operation description
- `serde_error` - Original error message

**Example:**
```rust
SignerError::SerializationDetailed {
    operation: "serialize".to_string(),
    data_type: "MnemonicStorage".to_string(),
    context: "Saving encrypted mnemonic to ~/.amp-rs/mnemonic.json".to_string(),
    serde_error: "invalid type: integer `12345`, expected a string".to_string(),
}
```

---

## Usage Guidelines

### When to Use Enhanced Variants

**Use enhanced variants when:**
- Building detailed error messages for end users
- Implementing error logging/monitoring
- Debugging production issues
- Need to programmatically handle specific error details

**Use legacy variants when:**
- Simple error reporting is sufficient
- Backward compatibility is critical
- Error context is already available from calling code

### Migration from Legacy Variants

All legacy variants remain available for backward compatibility:

- `Error::RequestFailed(String)` → `Error::RequestFailedDetailed { ... }`
- `AmpError::Api(String)` → `AmpError::ApiDetailed { ... }`
- `AmpError::Rpc(String)` → `AmpError::RpcDetailed { ... }`
- `AmpError::Serialization(serde_json::Error)` → `AmpError::SerializationDetailed { ... }`
- `SignerError::Lwk(String)` → `SignerError::LwkDetailed { ... }`
- `SignerError::HexParse(hex::FromHexError)` → `SignerError::HexParseDetailed { ... }`
- `SignerError::InvalidTransaction(String)` → `SignerError::InvalidTransactionDetailed { ... }`
- `SignerError::Serialization(serde_json::Error)` → `SignerError::SerializationDetailed { ... }`

### Example Integration

```rust
use amp_rs::{ApiClient, Error};

async fn handle_asset_issuance() {
    let client = ApiClient::new().await.unwrap();
    
    match client.issue_asset(&request).await {
        Ok(response) => {
            println!("Asset issued: {}", response.txid);
        }
        Err(Error::RequestFailedDetailed { 
            method, 
            endpoint, 
            status, 
            error_message 
        }) => {
            // Log detailed error for debugging
            log::error!(
                "Asset issuance failed: {} {} returned {}: {}",
                method, endpoint, status, error_message
            );
            
            // Show user-friendly message
            eprintln!("Failed to issue asset. Please check your request and try again.");
        }
        Err(e) => {
            eprintln!("Unexpected error: {}", e);
        }
    }
}
```

---

## Key Benefits

1. **Faster Debugging**
   - Immediate visibility into error context
   - No need to add logging or use debugging tools
   - Complete picture of what failed and why

2. **Better Diagnostics**
   - Full HTTP/RPC context (method, endpoint, parameters)
   - Raw response data for analyzing malformed responses
   - Structured data that's easy to parse and handle

3. **Production Ready**
   - Suitable for error monitoring and alerting
   - Can be serialized and sent to logging services
   - Helps identify API schema changes or breaking updates

4. **Developer Experience**
   - Clear, formatted error messages
   - Easy to understand what went wrong
   - Reduces time spent investigating issues

---

## Testing

All enhanced error variants include comprehensive tests:

```bash
# Run all enhanced error tests
cargo test --test enhanced_errors_comprehensive_test

# Run showcase example
cargo run --example enhanced_errors_showcase
```

---

## Backward Compatibility

All legacy error variants remain available and functional. Existing code will continue to work without modifications. The enhanced variants are additions, not replacements.

**Migration is optional and can be done incrementally.**

---

## Error Inventory

**Total Enhanced Variants:** 8
**Total Error Types:** 4 (Error, AmpError, TokenError, SignerError)
**Backward Compatible:** Yes
**Breaking Changes:** None
