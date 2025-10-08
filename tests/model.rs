use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};

#[test]
fn test_secret_serde_module() {
    // Test the custom serialization module directly
    let secret = Secret::new("test_secret".to_string());

    // Create a simple struct to test the serde module
    #[derive(Serialize, Deserialize)]
    struct TestStruct {
        #[serde(with = "amp_rs::model::secret_serde")]
        secret_field: Secret<String>,
    }

    let test_struct = TestStruct {
        secret_field: secret,
    };

    let json = serde_json::to_string(&test_struct).expect("Serialization failed");
    assert!(json.contains("test_secret"));

    let restored: TestStruct = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(restored.secret_field.expose_secret(), "test_secret");
}
#[test]
fn test_create_asset_assignment_request_defaults() {
    use amp_rs::model::CreateAssetAssignmentRequest;

    // Test that ready_for_distribution defaults to false
    let request = CreateAssetAssignmentRequest {
        registered_user: 123,
        amount: 1000,
        vesting_timestamp: None,
        ready_for_distribution: false, // Explicitly set to test
    };

    let json = serde_json::to_string(&request).expect("Serialization failed");
    assert!(json.contains("\"ready_for_distribution\":false"));

    // Test deserialization with missing ready_for_distribution field
    let json_without_field = r#"{"registered_user":456,"amount":2000,"vesting_timestamp":null}"#;
    let deserialized: CreateAssetAssignmentRequest =
        serde_json::from_str(json_without_field).expect("Deserialization failed");

    assert_eq!(deserialized.registered_user, 456);
    assert_eq!(deserialized.amount, 2000);
    assert_eq!(deserialized.vesting_timestamp, None);
    assert_eq!(deserialized.ready_for_distribution, false); // Should default to false

    // Test with ready_for_distribution explicitly set to true
    let json_with_true = r#"{"registered_user":789,"amount":3000,"vesting_timestamp":1234567890,"ready_for_distribution":true}"#;
    let deserialized_true: CreateAssetAssignmentRequest =
        serde_json::from_str(json_with_true).expect("Deserialization failed");

    assert_eq!(deserialized_true.registered_user, 789);
    assert_eq!(deserialized_true.amount, 3000);
    assert_eq!(deserialized_true.vesting_timestamp, Some(1234567890));
    assert_eq!(deserialized_true.ready_for_distribution, true);
}
