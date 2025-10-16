use thiserror::Error;

/// Comprehensive error types for signer operations
/// 
/// This enum covers all possible error scenarios that can occur during
/// transaction signing operations, providing detailed context for debugging
/// and proper error handling in client applications.
#[derive(Error, Debug)]
pub enum SignerError {
    /// LWK-specific errors from the Liquid Wallet Kit
    /// 
    /// This variant captures errors from LWK operations including:
    /// - SwSigner creation failures
    /// - Transaction signing failures  
    /// - PSET (Partially Signed Element Transaction) operations
    /// - Key derivation and cryptographic operations
    #[error("LWK signing operation failed: {0}")]
    Lwk(String),

    /// Invalid mnemonic phrase errors
    /// 
    /// This variant captures mnemonic-related errors including:
    /// - Invalid word count (not 12, 15, 18, 21, or 24 words)
    /// - Invalid characters or formatting
    /// - BIP39 checksum validation failures
    /// - Empty or malformed mnemonic phrases
    #[error("Invalid mnemonic phrase: {0}")]
    InvalidMnemonic(String),

    /// Hex string parsing and decoding errors
    /// 
    /// This variant captures errors when parsing hex-encoded data including:
    /// - Invalid hex characters
    /// - Odd-length hex strings
    /// - Empty hex strings
    /// - Malformed transaction hex data
    #[error("Hex parsing failed: {0}")]
    HexParse(#[from] hex::FromHexError),

    /// Invalid transaction structure or content errors
    /// 
    /// This variant captures transaction validation errors including:
    /// - Malformed transaction structure
    /// - Missing inputs or outputs
    /// - Invalid transaction serialization
    /// - PSET conversion failures
    /// - Transaction size or format issues
    #[error("Invalid transaction structure: {0}")]
    InvalidTransaction(String),

    /// Network-related communication errors
    /// 
    /// This variant captures network errors that may occur during
    /// remote operations or API calls (reserved for future use).
    #[error("Network communication failed: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON serialization and deserialization errors
    /// 
    /// This variant captures JSON processing errors including:
    /// - Mnemonic file parsing failures
    /// - Invalid JSON structure in storage files
    /// - Serialization failures when writing storage
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    /// File system I/O operation errors
    /// 
    /// This variant captures file operation errors including:
    /// - Mnemonic file read/write failures
    /// - Permission denied errors
    /// - Disk space or filesystem issues
    /// - Atomic write operation failures
    #[error("File I/O operation failed: {0}")]
    FileIo(#[from] std::io::Error),
}

// Additional error conversions for external library errors
// These provide seamless integration with third-party error types

/// Convert BIP39 mnemonic errors to SignerError
/// 
/// This conversion handles all BIP39-related errors including:
/// - Invalid word count
/// - Invalid words not in BIP39 wordlist  
/// - Checksum validation failures
/// - Language detection issues
impl From<bip39::Error> for SignerError {
    fn from(err: bip39::Error) -> Self {
        SignerError::InvalidMnemonic(format!("BIP39 validation failed: {}", err))
    }
}

/// Convert Elements transaction encoding errors to SignerError
/// 
/// This conversion handles transaction serialization/deserialization errors
/// from the Elements library including:
/// - Consensus encoding failures
/// - Invalid transaction structure
/// - Serialization format errors
impl From<elements::encode::Error> for SignerError {
    fn from(err: elements::encode::Error) -> Self {
        SignerError::InvalidTransaction(format!("Elements transaction encoding failed: {}", err))
    }
}

// Note: LWK errors are handled manually in the implementation code
// rather than through automatic conversion. This provides better control
// over error context and allows for operation-specific error messages
// that help with debugging and troubleshooting.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_lwk_error_variant() {
        let error_msg = "SwSigner creation failed";
        let error = SignerError::Lwk(error_msg.to_string());
        
        // Test error message formatting
        let formatted = format!("{}", error);
        assert_eq!(formatted, "LWK signing operation failed: SwSigner creation failed");
        
        // Test debug formatting
        let debug_formatted = format!("{:?}", error);
        assert!(debug_formatted.contains("Lwk"));
        assert!(debug_formatted.contains(error_msg));
        
        // Test error source (should be None for this variant)
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn test_invalid_mnemonic_error_variant() {
        let error_msg = "Invalid word count: expected 12, got 5";
        let error = SignerError::InvalidMnemonic(error_msg.to_string());
        
        // Test error message formatting
        let formatted = format!("{}", error);
        assert_eq!(formatted, "Invalid mnemonic phrase: Invalid word count: expected 12, got 5");
        
        // Test debug formatting
        let debug_formatted = format!("{:?}", error);
        assert!(debug_formatted.contains("InvalidMnemonic"));
        assert!(debug_formatted.contains(error_msg));
        
        // Test error source (should be None for this variant)
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn test_hex_parse_error_conversion() {
        // Create a hex parsing error by trying to decode invalid hex
        let hex_result = hex::decode("invalid_hex_zz");
        let hex_error = hex_result.unwrap_err();
        let signer_error = SignerError::from(hex_error);
        
        // Test error variant
        match signer_error {
            SignerError::HexParse(_) => {}, // Expected
            other => panic!("Expected HexParse variant, got: {:?}", other),
        }
        
        // Test error message formatting
        let formatted = format!("{}", signer_error);
        assert!(formatted.starts_with("Hex parsing failed:"));
        
        // Test error source preservation
        let source = std::error::Error::source(&signer_error);
        assert!(source.is_some());
    }

    #[test]
    fn test_invalid_transaction_error_variant() {
        let error_msg = "Transaction deserialization failed: invalid input count";
        let error = SignerError::InvalidTransaction(error_msg.to_string());
        
        // Test error message formatting
        let formatted = format!("{}", error);
        assert_eq!(formatted, "Invalid transaction structure: Transaction deserialization failed: invalid input count");
        
        // Test debug formatting
        let debug_formatted = format!("{:?}", error);
        assert!(debug_formatted.contains("InvalidTransaction"));
        assert!(debug_formatted.contains(error_msg));
        
        // Test error source (should be None for this variant)
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn test_network_error_conversion() {
        // Create a reqwest error by making an invalid request
        let client = reqwest::Client::new();
        let request_result = client.get("http://").build();
        let reqwest_error = request_result.unwrap_err();
        let signer_error = SignerError::from(reqwest_error);
        
        // Test error variant
        match signer_error {
            SignerError::Network(_) => {}, // Expected
            other => panic!("Expected Network variant, got: {:?}", other),
        }
        
        // Test error message formatting
        let formatted = format!("{}", signer_error);
        assert!(formatted.starts_with("Network communication failed:"));
        
        // Test error source preservation
        let source = std::error::Error::source(&signer_error);
        assert!(source.is_some());
    }

    #[test]
    fn test_serialization_error_conversion() {
        // Create a JSON serialization error by trying to parse invalid JSON
        let invalid_json = r#"{"invalid": json syntax"#;
        let json_error = serde_json::from_str::<serde_json::Value>(invalid_json).unwrap_err();
        let signer_error = SignerError::from(json_error);
        
        // Test error variant
        match signer_error {
            SignerError::Serialization(_) => {}, // Expected
            other => panic!("Expected Serialization variant, got: {:?}", other),
        }
        
        // Test error message formatting
        let formatted = format!("{}", signer_error);
        assert!(formatted.starts_with("JSON serialization failed:"));
        
        // Test error source preservation
        let source = std::error::Error::source(&signer_error);
        assert!(source.is_some());
    }

    #[test]
    fn test_file_io_error_conversion() {
        // Create an I/O error
        let io_error = IoError::new(ErrorKind::NotFound, "File not found");
        let signer_error = SignerError::from(io_error);
        
        // Test error variant
        match signer_error {
            SignerError::FileIo(_) => {}, // Expected
            other => panic!("Expected FileIo variant, got: {:?}", other),
        }
        
        // Test error message formatting
        let formatted = format!("{}", signer_error);
        assert!(formatted.starts_with("File I/O operation failed:"));
        assert!(formatted.contains("File not found"));
        
        // Test error source preservation
        let source = std::error::Error::source(&signer_error);
        assert!(source.is_some());
        assert_eq!(source.unwrap().to_string(), "File not found");
    }

    #[test]
    fn test_bip39_error_conversion() {
        // Create a BIP39 error (invalid word count)
        let bip39_error = bip39::Error::BadWordCount(5);
        let signer_error = SignerError::from(bip39_error);
        
        // Test error variant
        match &signer_error {
            SignerError::InvalidMnemonic(msg) => {
                assert!(msg.contains("BIP39 validation failed"));
                assert!(msg.contains("word count"));
            },
            other => panic!("Expected InvalidMnemonic variant, got: {:?}", other),
        }
        
        // Test error message formatting
        let formatted = format!("{}", signer_error);
        assert!(formatted.starts_with("Invalid mnemonic phrase:"));
        assert!(formatted.contains("BIP39 validation failed"));
    }

    #[test]
    fn test_elements_encode_error_conversion() {
        // Create an Elements encoding error
        let elements_error = elements::encode::Error::ParseFailed("Invalid transaction format");
        let signer_error = SignerError::from(elements_error);
        
        // Test error variant
        match &signer_error {
            SignerError::InvalidTransaction(msg) => {
                assert!(msg.contains("Elements transaction encoding failed"));
                assert!(msg.contains("Invalid transaction format"));
            },
            other => panic!("Expected InvalidTransaction variant, got: {:?}", other),
        }
        
        // Test error message formatting
        let formatted = format!("{}", signer_error);
        assert!(formatted.starts_with("Invalid transaction structure:"));
        assert!(formatted.contains("Elements transaction encoding failed"));
    }

    #[test]
    fn test_error_chain_preservation() {
        // Test that error chains are properly preserved through conversions
        
        // Create a nested I/O error
        let inner_error = IoError::new(ErrorKind::PermissionDenied, "Access denied");
        let signer_error = SignerError::from(inner_error);
        
        // Test that the source chain is preserved
        let source = std::error::Error::source(&signer_error);
        assert!(source.is_some());
        assert_eq!(source.unwrap().to_string(), "Access denied");
        
        // Test error message includes context
        let formatted = format!("{}", signer_error);
        assert!(formatted.contains("File I/O operation failed"));
        assert!(formatted.contains("Access denied"));
    }

    #[test]
    fn test_error_send_sync_traits() {
        // Test that SignerError implements Send and Sync for async compatibility
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        
        assert_send::<SignerError>();
        assert_sync::<SignerError>();
    }

    #[test]
    fn test_error_static_lifetime() {
        // Test that SignerError can be used with 'static lifetime
        fn assert_static<T: 'static>() {}
        assert_static::<SignerError>();
    }

    #[test]
    fn test_all_error_variants_display() {
        // Test that all error variants have proper Display implementations
        let errors = vec![
            SignerError::Lwk("test lwk error".to_string()),
            SignerError::InvalidMnemonic("test mnemonic error".to_string()),
            SignerError::HexParse(hex::decode("invalid_hex_zz").unwrap_err()),
            SignerError::InvalidTransaction("test transaction error".to_string()),
            SignerError::Network(reqwest::Client::new().get("http://").build().unwrap_err()),
            SignerError::Serialization(serde_json::from_str::<serde_json::Value>(r#"{"invalid": json"#).unwrap_err()),
            SignerError::FileIo(IoError::new(ErrorKind::NotFound, "test file error")),
        ];
        
        for error in errors {
            let display_str = format!("{}", error);
            assert!(!display_str.is_empty(), "Error display should not be empty: {:?}", error);
            
            let debug_str = format!("{:?}", error);
            assert!(!debug_str.is_empty(), "Error debug should not be empty: {:?}", error);
        }
    }

    #[test]
    fn test_error_message_preservation() {
        // Test that custom error messages are preserved correctly
        let test_cases = vec![
            ("LWK operation failed with code 123", SignerError::Lwk("LWK operation failed with code 123".to_string())),
            ("Mnemonic has invalid checksum", SignerError::InvalidMnemonic("Mnemonic has invalid checksum".to_string())),
            ("Transaction missing required inputs", SignerError::InvalidTransaction("Transaction missing required inputs".to_string())),
        ];
        
        for (expected_msg, error) in test_cases {
            let formatted = format!("{}", error);
            assert!(formatted.contains(expected_msg), 
                   "Error message should contain '{}', got: '{}'", expected_msg, formatted);
        }
    }

    #[test]
    fn test_error_propagation_through_result() {
        // Test error propagation through Result types (simulating call stack)
        fn level_3() -> Result<(), SignerError> {
            Err(SignerError::Lwk("Deep error".to_string()))
        }
        
        fn level_2() -> Result<(), SignerError> {
            level_3()?;
            Ok(())
        }
        
        fn level_1() -> Result<(), SignerError> {
            level_2()?;
            Ok(())
        }
        
        let result = level_1();
        assert!(result.is_err());
        
        match result.unwrap_err() {
            SignerError::Lwk(msg) => assert_eq!(msg, "Deep error"),
            other => panic!("Expected Lwk error, got: {:?}", other),
        }
    }

    #[test]
    fn test_error_conversion_preserves_context() {
        // Test that automatic conversions preserve error context
        
        // Test hex error conversion
        let hex_result: Result<Vec<u8>, hex::FromHexError> = hex::decode("invalid_hex_zz");
        let hex_error = hex_result.unwrap_err();
        let signer_error = SignerError::from(hex_error);
        
        let formatted = format!("{}", signer_error);
        assert!(formatted.contains("Hex parsing failed"));
        
        // Test I/O error conversion
        let io_error = IoError::new(ErrorKind::PermissionDenied, "Cannot write to read-only file");
        let signer_error = SignerError::from(io_error);
        
        let formatted = format!("{}", signer_error);
        assert!(formatted.contains("File I/O operation failed"));
        assert!(formatted.contains("Cannot write to read-only file"));
    }

    #[test]
    fn test_error_debug_format_completeness() {
        // Test that debug format includes all relevant information
        let error = SignerError::InvalidMnemonic("Test mnemonic error with details".to_string());
        let debug_str = format!("{:#?}", error);
        
        // Debug format should include variant name and message
        assert!(debug_str.contains("InvalidMnemonic"));
        assert!(debug_str.contains("Test mnemonic error with details"));
    }

    #[test]
    fn test_multiple_error_conversions() {
        // Test multiple error conversions in sequence
        let test_cases = vec![
            "invalid_hex_zz",  // Invalid character
            "abc",             // Odd length
        ];
        
        for invalid_hex in test_cases {
            let hex_error = hex::decode(invalid_hex).unwrap_err();
            let signer_error = SignerError::from(hex_error);
            match signer_error {
                SignerError::HexParse(_) => {}, // Expected
                other => panic!("Expected HexParse variant, got: {:?}", other),
            }
            
            // Verify error message is meaningful
            let formatted = format!("{}", signer_error);
            assert!(formatted.starts_with("Hex parsing failed:"));
        }
    }

    #[test]
    fn test_error_equality_and_comparison() {
        // Test that errors can be compared for debugging purposes
        let error1 = SignerError::Lwk("same message".to_string());
        let error2 = SignerError::Lwk("same message".to_string());
        let error3 = SignerError::Lwk("different message".to_string());
        
        // Note: SignerError doesn't implement PartialEq by design (errors often contain
        // non-comparable types), but we can test that the debug representations are consistent
        let debug1 = format!("{:?}", error1);
        let debug2 = format!("{:?}", error2);
        let debug3 = format!("{:?}", error3);
        
        assert_eq!(debug1, debug2);
        assert_ne!(debug1, debug3);
    }
}