/// Example demonstrating enhanced error reporting for deserialization failures
///
/// This example creates a mock scenario where the API returns malformed JSON
/// to demonstrate how the enhanced error messages help diagnose issues.
use amp_rs::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This example demonstrates what the enhanced error looks like
    // when deserialization fails

    println!("Enhanced Error Reporting Example");
    println!("=================================\n");

    // Create a mock error to demonstrate the format
    let mock_error = Error::ResponseDeserializationFailed {
        method: "GET".to_string(),
        endpoint: "https://amp-test.blockstream.com/api/assets/550e8400-e29b-41d4-a716-446655440000/ownerships".to_string(),
        expected_type: "alloc::vec::Vec<amp_rs::model::Ownership>".to_string(),
        serde_error: "invalid type: null, expected a string at line 1 column 54".to_string(),
        raw_response: r#"[{"owner":"user123","amount":1000,"gaid":"abc"},{"owner":null,"amount":500,"gaid":"xyz"}]"#.to_string(),
    };

    println!("Example of enhanced error message:\n");
    println!("{}\n", mock_error);

    println!("Debug format:\n");
    println!("{:?}\n", mock_error);

    println!("This enhanced error includes:");
    println!("- HTTP method used");
    println!("- Full endpoint URL");
    println!("- Expected Rust type name");
    println!("- Original serde error message");
    println!("- Complete raw response body");

    Ok(())
}
