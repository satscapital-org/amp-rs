# Enhanced API Error Reporting

## Overview

The amp-rs library now provides comprehensive diagnostic information when API response deserialization fails. This enhancement helps developers quickly identify and resolve issues with malformed or unexpected API responses.

## What Changed

### New Error Variant

Added `ResponseDeserializationFailed` to the `Error` enum with the following fields:

- **method**: The HTTP method used (GET, POST, PUT, DELETE, etc.)
- **endpoint**: The full URL that was called
- **expected_type**: The Rust type name that was expected (e.g., `Vec<Ownership>`)
- **serde_error**: The original serde deserialization error message
- **raw_response**: The complete raw response body text that failed to parse

### Updated Implementation

Modified the `request_json()` method in `src/client.rs` to:
1. Capture request context (method, endpoint URL, expected type) before making the request
2. Read the full response body as text
3. Attempt deserialization with detailed error context on failure
4. Include all diagnostic information in the error object

## Example Error Output

When a deserialization failure occurs, the error message now looks like this:

```
Failed to parse AMP response: invalid type: null, expected a string at line 1 column 54

Method: GET
Endpoint: https://amp-test.blockstream.com/api/assets/550e8400-e29b-41d4-a716-446655440000/ownerships
Expected Type: alloc::vec::Vec<amp_rs::model::Ownership>

Raw Response:
[{"owner":"user123","amount":1000,"gaid":"abc"},{"owner":null,"amount":500,"gaid":"xyz"}]
```

This format clearly shows:
- What went wrong (null value where string expected)
- Which endpoint was called
- What type was expected
- The actual malformed data that caused the issue

## Benefits

1. **Faster Debugging**: Developers can immediately see the actual malformed response data without needing to add logging or use debugging tools

2. **Better Diagnostics**: The full context (method, URL, expected type) helps identify which API call failed and why

3. **Downstream Flexibility**: Applications can decide how to handle errors:
   - Log to files
   - Display in UIs
   - Send to monitoring systems
   - Store for analysis

4. **API Issue Detection**: Helps identify issues with the AMP API itself, such as:
   - Schema changes
   - Missing fields
   - Unexpected null values
   - Type mismatches

## Affected Methods

All API methods that parse JSON responses now provide enhanced error information, including:

- `get_assets()`
- `get_asset()`
- `get_asset_ownerships()`
- `get_asset_balance()`
- `get_asset_summary()`
- `get_asset_utxos()`
- `issue_asset()`
- `edit_asset()`
- And all other methods that use `request_json()`

## Testing

The implementation includes:
- Unit tests verifying all error fields are populated
- Tests validating the Display and Debug formatting
- Example code demonstrating the error format
- All existing tests continue to pass

## Example Usage

```rust
use amp_rs::ApiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ApiClient::new().await?;
    
    match client.get_asset_ownerships("some-uuid", None).await {
        Ok(ownerships) => {
            println!("Got {} ownerships", ownerships.len());
        }
        Err(e) => {
            // The error now includes full diagnostic information
            eprintln!("Failed to get ownerships: {}", e);
            // Could also log to a file, send to monitoring, etc.
        }
    }
    
    Ok(())
}
```

## Running the Example

To see the enhanced error format in action:

```bash
cargo run --example test_enhanced_errors
```

## Running the Tests

To verify the enhanced error handling:

```bash
cargo test --test enhanced_error_test
```
