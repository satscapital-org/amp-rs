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

#[test]
fn test_asset_transaction_serialization() {
    use amp_rs::model::AssetTransaction;

    let tx = AssetTransaction {
        txid: "abc123def456".to_string(),
        transaction_type: "transfer".to_string(),
        amount: 100_000,
        datetime: Some("2024-06-15T12:00:00Z".to_string()),
        blockheight: Some(500),
        confirmations: Some(10),
        registered_user: Some(1),
        description: Some("Test transaction".to_string()),
        vout: Some(0),
        asset_blinder: Some("blinder123".to_string()),
        amount_blinder: Some("blinder456".to_string()),
        from_address: Some("from_addr".to_string()),
        to_address: Some("to_addr".to_string()),
        gaid: Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string()),
    };

    // Test serialization
    let json = serde_json::to_string(&tx).expect("Serialization failed");
    assert!(json.contains("abc123def456"));
    assert!(json.contains("\"type\":\"transfer\""));
    assert!(json.contains("100000"));
    assert!(json.contains("\"GAID\":\"GAbYScu6jkWUND2jo3L4KJxyvo55d\""));

    // Test deserialization
    let deserialized: AssetTransaction =
        serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.txid, "abc123def456");
    assert_eq!(deserialized.transaction_type, "transfer");
    assert_eq!(deserialized.amount, 100_000);
    assert_eq!(deserialized.registered_user, Some(1));
    assert_eq!(
        deserialized.gaid,
        Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string())
    );
}

#[test]
fn test_asset_transaction_optional_fields() {
    use amp_rs::model::AssetTransaction;

    // Minimal transaction with only required fields
    let minimal_json = r#"{
        "txid": "minimal-tx",
        "type": "issuance",
        "amount": 1000
    }"#;

    let tx: AssetTransaction = serde_json::from_str(minimal_json).expect("Deserialization failed");

    assert_eq!(tx.txid, "minimal-tx");
    assert_eq!(tx.transaction_type, "issuance");
    assert_eq!(tx.amount, 1000);
    assert!(tx.datetime.is_none());
    assert!(tx.blockheight.is_none());
    assert!(tx.confirmations.is_none());
    assert!(tx.registered_user.is_none());
    assert!(tx.description.is_none());
    assert!(tx.gaid.is_none());
}

#[test]
fn test_asset_transaction_clone() {
    use amp_rs::model::AssetTransaction;

    let tx = AssetTransaction {
        txid: "test-txid".to_string(),
        transaction_type: "burn".to_string(),
        amount: 50_000,
        datetime: Some("2024-01-01T00:00:00Z".to_string()),
        blockheight: Some(100),
        confirmations: Some(6),
        registered_user: None,
        description: None,
        vout: Some(1),
        asset_blinder: None,
        amount_blinder: None,
        from_address: None,
        to_address: None,
        gaid: None,
    };

    let cloned = tx.clone();
    assert_eq!(tx.txid, cloned.txid);
    assert_eq!(tx.transaction_type, cloned.transaction_type);
    assert_eq!(tx.amount, cloned.amount);
}

#[test]
fn test_asset_transaction_params_serialization() {
    use amp_rs::model::AssetTransactionParams;

    let params = AssetTransactionParams {
        start: Some(0),
        count: Some(25),
        sortcolumn: Some("blockheight".to_string()),
        sortorder: Some("desc".to_string()),
        height_start: Some(1000),
        height_stop: Some(2000),
        transaction_type: Some("transfer".to_string()),
    };

    // Test serialization
    let json = serde_json::to_string(&params).expect("Serialization failed");
    assert!(json.contains("\"start\":0"));
    assert!(json.contains("\"count\":25"));
    assert!(json.contains("\"sortcolumn\":\"blockheight\""));
    assert!(json.contains("\"sortorder\":\"desc\""));
    assert!(json.contains("\"height_start\":1000"));
    assert!(json.contains("\"height_stop\":2000"));
    assert!(json.contains("\"type\":\"transfer\""));
}

#[test]
fn test_asset_transaction_params_default() {
    use amp_rs::model::AssetTransactionParams;

    let params = AssetTransactionParams::default();

    assert!(params.start.is_none());
    assert!(params.count.is_none());
    assert!(params.sortcolumn.is_none());
    assert!(params.sortorder.is_none());
    assert!(params.height_start.is_none());
    assert!(params.height_stop.is_none());
    assert!(params.transaction_type.is_none());
}

#[test]
fn test_asset_transaction_params_skip_serializing_none() {
    use amp_rs::model::AssetTransactionParams;

    let params = AssetTransactionParams {
        count: Some(10),
        ..Default::default()
    };

    let json = serde_json::to_string(&params).expect("Serialization failed");

    // Should only contain count, not other fields
    assert!(json.contains("\"count\":10"));
    assert!(!json.contains("start"));
    assert!(!json.contains("sortcolumn"));
    assert!(!json.contains("height_start"));
}
