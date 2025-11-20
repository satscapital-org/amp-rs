/// Comprehensive showcase of all enhanced error variants
///
/// This example demonstrates the improved error reporting across all
/// error types in the amp-rs crate.
use amp_rs::{AmpError, Error, SignerError};

fn main() {
    println!("amp-rs Enhanced Error Reporting Showcase");
    println!("==========================================\n");

    // Error::RequestFailedDetailed
    println!("1. Error::RequestFailedDetailed");
    println!("   (HTTP request failures with full context)\n");
    let error = Error::RequestFailedDetailed {
        method: "POST".to_string(),
        endpoint: "https://amp-test.blockstream.com/api/assets/issue".to_string(),
        status: reqwest::StatusCode::BAD_REQUEST,
        error_message: "Missing required field: amount".to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // AmpError::ApiDetailed
    println!("2. AmpError::ApiDetailed");
    println!("   (API operation failures with endpoint details)\n");
    let error = AmpError::ApiDetailed {
        endpoint: "/distributions/create".to_string(),
        method: "POST".to_string(),
        error_message: "Insufficient funds in treasury".to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // AmpError::RpcDetailed
    println!("3. AmpError::RpcDetailed");
    println!("   (Elements RPC failures with method and raw response)\n");
    let error = AmpError::RpcDetailed {
        rpc_method: "sendrawtransaction".to_string(),
        params: r#"["020000000001..."]"#.to_string(),
        error_message: "Transaction rejected by network".to_string(),
        raw_response: r#"{"error":{"code":-26,"message":"bad-txns-inputs-missingorspent","data":"Missing inputs"}}"#.to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // AmpError::SerializationDetailed
    println!("4. AmpError::SerializationDetailed");
    println!("   (JSON serialization failures with context)\n");
    let error = AmpError::SerializationDetailed {
        operation: "deserialize".to_string(),
        data_type: "IssuanceResponse".to_string(),
        context: "Parsing asset issuance API response".to_string(),
        serde_error: "missing field `txid` at line 1 column 45".to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // SignerError::LwkDetailed
    println!("5. SignerError::LwkDetailed");
    println!("   (LWK signing failures with operation context)\n");
    let error = SignerError::LwkDetailed {
        operation: "sign_transaction".to_string(),
        context: "Signing distribution transaction with 3 inputs and 5 outputs".to_string(),
        error_message: "Failed to sign input at index 2: Missing signing key".to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // SignerError::HexParseDetailed
    println!("6. SignerError::HexParseDetailed");
    println!("   (Hex parsing failures with preview)\n");
    let error = SignerError::HexParseDetailed {
        parsing_context: "Raw transaction hex from AMP API response".to_string(),
        hex_preview: "0200000000010a5f3e2b1c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b...".to_string(),
        hex_error: "Invalid hex character 'z' at position 127".to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // SignerError::InvalidTransactionDetailed
    println!("7. SignerError::InvalidTransactionDetailed");
    println!("   (Transaction validation failures with specifics)\n");
    let error = SignerError::InvalidTransactionDetailed {
        txid: "abc123def456789".to_string(),
        validation_details: "Input 0 references UTXO abc:1 which does not exist in wallet"
            .to_string(),
        error_message: "Transaction validation failed: missing input UTXOs".to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    // SignerError::SerializationDetailed
    println!("8. SignerError::SerializationDetailed");
    println!("   (Mnemonic/wallet serialization failures)\n");
    let error = SignerError::SerializationDetailed {
        operation: "serialize".to_string(),
        data_type: "MnemonicStorage".to_string(),
        context: "Saving encrypted mnemonic to ~/.amp-rs/mnemonic.json".to_string(),
        serde_error:
            "invalid type: integer `12345`, expected a string for field 'encrypted_mnemonic'"
                .to_string(),
    };
    println!("{}\n", error);
    println!("{}\n", "-".repeat(80));

    println!("\nKey Benefits:");
    println!("=============");
    println!("• Faster debugging with immediate visibility into error context");
    println!("• Complete diagnostic information for API and RPC failures");
    println!("• Raw response data included for malformed responses");
    println!("• Clear indication of what operation failed and where");
    println!("• Structured error data that applications can programmatically handle");
    println!("\nAll enhanced errors maintain backward compatibility through legacy variants.");
}
