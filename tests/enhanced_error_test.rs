/// Integration test for enhanced error reporting on deserialization failures
///
/// This test verifies that when the API returns malformed JSON, the error
/// includes comprehensive diagnostic information.
use amp_rs::Error;

#[test]
fn test_enhanced_error_contains_all_fields() {
    let error = Error::ResponseDeserializationFailed {
        method: "GET".to_string(),
        endpoint: "https://amp-test.blockstream.com/api/assets/test-uuid/ownerships".to_string(),
        expected_type: "Vec<Ownership>".to_string(),
        serde_error: "invalid type: null, expected a string".to_string(),
        raw_response: r#"[{"owner":null}]"#.to_string(),
    };

    let error_string = format!("{}", error);

    // Verify all required information is present in the error message
    assert!(
        error_string.contains("GET"),
        "Error should contain HTTP method"
    );
    assert!(
        error_string.contains("https://amp-test.blockstream.com/api/assets/test-uuid/ownerships"),
        "Error should contain full endpoint URL"
    );
    assert!(
        error_string.contains("Vec<Ownership>"),
        "Error should contain expected type"
    );
    assert!(
        error_string.contains("invalid type: null, expected a string"),
        "Error should contain serde error message"
    );
    assert!(
        error_string.contains(r#"[{"owner":null}]"#),
        "Error should contain raw response body"
    );
}

#[test]
fn test_enhanced_error_display_format() {
    let error = Error::ResponseDeserializationFailed {
        method: "POST".to_string(),
        endpoint: "https://amp-test.blockstream.com/api/assets/issue".to_string(),
        expected_type: "IssuanceResponse".to_string(),
        serde_error: "missing field `txid`".to_string(),
        raw_response: r#"{"status":"success"}"#.to_string(),
    };

    let display = format!("{}", error);

    // Verify the format is readable and structured
    assert!(
        display.contains("Method:"),
        "Display should have 'Method:' label"
    );
    assert!(
        display.contains("Endpoint:"),
        "Display should have 'Endpoint:' label"
    );
    assert!(
        display.contains("Expected Type:"),
        "Display should have 'Expected Type:' label"
    );
    assert!(
        display.contains("Raw Response:"),
        "Display should have 'Raw Response:' label"
    );
}

#[test]
fn test_enhanced_error_debug_format() {
    let error = Error::ResponseDeserializationFailed {
        method: "PUT".to_string(),
        endpoint: "https://amp-test.blockstream.com/api/assets/uuid/edit".to_string(),
        expected_type: "Asset".to_string(),
        serde_error: "invalid value".to_string(),
        raw_response: "{}".to_string(),
    };

    let debug = format!("{:?}", error);

    // Verify debug format includes the variant name and all fields
    assert!(
        debug.contains("ResponseDeserializationFailed"),
        "Debug format should include variant name"
    );
    assert!(
        debug.contains("method"),
        "Debug format should include method field"
    );
    assert!(
        debug.contains("endpoint"),
        "Debug format should include endpoint field"
    );
    assert!(
        debug.contains("expected_type"),
        "Debug format should include expected_type field"
    );
    assert!(
        debug.contains("serde_error"),
        "Debug format should include serde_error field"
    );
    assert!(
        debug.contains("raw_response"),
        "Debug format should include raw_response field"
    );
}
