/// Comprehensive tests for all enhanced error variants
///
/// This test suite verifies that all newly enhanced error types include
/// comprehensive diagnostic information and format correctly.

use amp_rs::{AmpError, Error, SignerError};

#[test]
fn test_error_request_failed_detailed() {
    let error = Error::RequestFailedDetailed {
        method: "POST".to_string(),
        endpoint: "https://amp-test.blockstream.com/api/assets/issue".to_string(),
        status: reqwest::StatusCode::BAD_REQUEST,
        error_message: "Invalid issuance request".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(error_string.contains("POST"), "Should contain HTTP method");
    assert!(
        error_string.contains("https://amp-test.blockstream.com/api/assets/issue"),
        "Should contain endpoint"
    );
    assert!(
        error_string.contains("400"),
        "Should contain status code"
    );
    assert!(
        error_string.contains("Invalid issuance request"),
        "Should contain error message"
    );
}

#[test]
fn test_amp_error_api_detailed() {
    let error = AmpError::ApiDetailed {
        endpoint: "/distribution/create".to_string(),
        method: "POST".to_string(),
        error_message: "Insufficient funds".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(
        error_string.contains("/distribution/create"),
        "Should contain endpoint"
    );
    assert!(error_string.contains("POST"), "Should contain method");
    assert!(
        error_string.contains("Insufficient funds"),
        "Should contain error message"
    );
}

#[test]
fn test_amp_error_rpc_detailed() {
    let error = AmpError::RpcDetailed {
        rpc_method: "sendrawtransaction".to_string(),
        params: r#"["0200000000..."]"#.to_string(),
        error_message: "Transaction rejected".to_string(),
        raw_response: r#"{"error":{"code":-26,"message":"bad-txns-inputs-missingorspent"}}"#
            .to_string(),
    };

    let error_string = format!("{}", error);

    assert!(
        error_string.contains("sendrawtransaction"),
        "Should contain RPC method"
    );
    assert!(
        error_string.contains(r#"["0200000000..."]"#),
        "Should contain params"
    );
    assert!(
        error_string.contains("Transaction rejected"),
        "Should contain error message"
    );
    assert!(
        error_string.contains("bad-txns-inputs-missingorspent"),
        "Should contain raw response"
    );
}

#[test]
fn test_amp_error_serialization_detailed() {
    let error = AmpError::SerializationDetailed {
        operation: "deserialize".to_string(),
        data_type: "BroadcastResponse".to_string(),
        context: "Parsing transaction broadcast response".to_string(),
        serde_error: "missing field `txid`".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(
        error_string.contains("deserialize"),
        "Should contain operation"
    );
    assert!(
        error_string.contains("BroadcastResponse"),
        "Should contain data type"
    );
    assert!(
        error_string.contains("Parsing transaction broadcast response"),
        "Should contain context"
    );
    assert!(
        error_string.contains("missing field `txid`"),
        "Should contain serde error"
    );
}

#[test]
fn test_signer_error_lwk_detailed() {
    let error = SignerError::LwkDetailed {
        operation: "sign_transaction".to_string(),
        context: "Signing distribution transaction with 3 inputs".to_string(),
        error_message: "Failed to sign input 2".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(
        error_string.contains("sign_transaction"),
        "Should contain operation"
    );
    assert!(
        error_string.contains("Signing distribution transaction with 3 inputs"),
        "Should contain context"
    );
    assert!(
        error_string.contains("Failed to sign input 2"),
        "Should contain error message"
    );
}

#[test]
fn test_signer_error_hex_parse_detailed() {
    let error = SignerError::HexParseDetailed {
        parsing_context: "Transaction hex from API response".to_string(),
        hex_preview: "0200000000010abc123...".to_string(),
        hex_error: "Invalid character 'z' at position 15".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(
        error_string.contains("Transaction hex from API response"),
        "Should contain parsing context"
    );
    assert!(
        error_string.contains("0200000000010abc123..."),
        "Should contain hex preview"
    );
    assert!(
        error_string.contains("Invalid character 'z' at position 15"),
        "Should contain hex error"
    );
}

#[test]
fn test_signer_error_invalid_transaction_detailed() {
    let error = SignerError::InvalidTransactionDetailed {
        txid: "abc123def456".to_string(),
        validation_details: "Input 0 references non-existent UTXO".to_string(),
        error_message: "Invalid transaction inputs".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(error_string.contains("abc123def456"), "Should contain txid");
    assert!(
        error_string.contains("Input 0 references non-existent UTXO"),
        "Should contain validation details"
    );
    assert!(
        error_string.contains("Invalid transaction inputs"),
        "Should contain error message"
    );
}

#[test]
fn test_signer_error_serialization_detailed() {
    let error = SignerError::SerializationDetailed {
        operation: "serialize".to_string(),
        data_type: "MnemonicStorage".to_string(),
        context: "Saving mnemonic to disk".to_string(),
        serde_error: "invalid value: integer `123`, expected a string".to_string(),
    };

    let error_string = format!("{}", error);

    assert!(
        error_string.contains("serialize"),
        "Should contain operation"
    );
    assert!(
        error_string.contains("MnemonicStorage"),
        "Should contain data type"
    );
    assert!(
        error_string.contains("Saving mnemonic to disk"),
        "Should contain context"
    );
    assert!(
        error_string.contains("invalid value: integer `123`, expected a string"),
        "Should contain serde error"
    );
}

#[test]
fn test_amp_error_with_context_detailed_variants() {
    let error = AmpError::ApiDetailed {
        endpoint: "/test".to_string(),
        method: "GET".to_string(),
        error_message: "Original error".to_string(),
    };

    let contextual_error = error.with_context("During asset retrieval");

    match contextual_error {
        AmpError::ApiDetailed { error_message, .. } => {
            assert!(error_message.contains("During asset retrieval"));
            assert!(error_message.contains("Original error"));
        }
        _ => panic!("Expected ApiDetailed variant"),
    }

    let rpc_error = AmpError::RpcDetailed {
        rpc_method: "test".to_string(),
        params: "[]".to_string(),
        error_message: "RPC failed".to_string(),
        raw_response: "{}".to_string(),
    };

    let contextual_rpc = rpc_error.with_context("During wallet query");

    match contextual_rpc {
        AmpError::RpcDetailed { error_message, .. } => {
            assert!(error_message.contains("During wallet query"));
            assert!(error_message.contains("RPC failed"));
        }
        _ => panic!("Expected RpcDetailed variant"),
    }
}

#[test]
fn test_amp_error_is_retryable_with_detailed() {
    let rpc_detailed = AmpError::RpcDetailed {
        rpc_method: "getblockchaininfo".to_string(),
        params: "[]".to_string(),
        error_message: "Connection refused".to_string(),
        raw_response: "".to_string(),
    };

    assert!(
        rpc_detailed.is_retryable(),
        "RpcDetailed should be retryable"
    );

    let api_detailed = AmpError::ApiDetailed {
        endpoint: "/test".to_string(),
        method: "GET".to_string(),
        error_message: "Not found".to_string(),
    };

    assert!(
        !api_detailed.is_retryable(),
        "ApiDetailed should not be retryable"
    );
}

#[test]
fn test_amp_error_retry_instructions_with_detailed() {
    let rpc_detailed = AmpError::RpcDetailed {
        rpc_method: "test".to_string(),
        params: "[]".to_string(),
        error_message: "Connection failed".to_string(),
        raw_response: "".to_string(),
    };

    let instructions = rpc_detailed.retry_instructions();
    assert!(instructions.is_some());
    assert!(instructions
        .unwrap()
        .contains("Elements node connection and retry"));
}

#[test]
fn test_all_detailed_variants_debug_format() {
    let errors: Vec<Box<dyn std::fmt::Debug>> = vec![
        Box::new(Error::RequestFailedDetailed {
            method: "GET".to_string(),
            endpoint: "test".to_string(),
            status: reqwest::StatusCode::NOT_FOUND,
            error_message: "test".to_string(),
        }),
        Box::new(AmpError::ApiDetailed {
            endpoint: "test".to_string(),
            method: "GET".to_string(),
            error_message: "test".to_string(),
        }),
        Box::new(AmpError::RpcDetailed {
            rpc_method: "test".to_string(),
            params: "[]".to_string(),
            error_message: "test".to_string(),
            raw_response: "{}".to_string(),
        }),
        Box::new(AmpError::SerializationDetailed {
            operation: "test".to_string(),
            data_type: "test".to_string(),
            context: "test".to_string(),
            serde_error: "test".to_string(),
        }),
        Box::new(SignerError::LwkDetailed {
            operation: "test".to_string(),
            context: "test".to_string(),
            error_message: "test".to_string(),
        }),
        Box::new(SignerError::HexParseDetailed {
            parsing_context: "test".to_string(),
            hex_preview: "test".to_string(),
            hex_error: "test".to_string(),
        }),
        Box::new(SignerError::InvalidTransactionDetailed {
            txid: "test".to_string(),
            validation_details: "test".to_string(),
            error_message: "test".to_string(),
        }),
        Box::new(SignerError::SerializationDetailed {
            operation: "test".to_string(),
            data_type: "test".to_string(),
            context: "test".to_string(),
            serde_error: "test".to_string(),
        }),
    ];

    for error in errors {
        let debug_str = format!("{:?}", error);
        assert!(
            !debug_str.is_empty(),
            "Error debug format should not be empty"
        );
    }
}
