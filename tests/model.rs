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
    use amp_rs::model::{AssetTransaction, AssetTransactionOutput};

    let tx = AssetTransaction {
        txid: "abc123def456".to_string(),
        datetime: "2024-06-15T12:00:00Z".to_string(),
        blockheight: 500,
        is_issuance: false,
        is_reissuance: false,
        is_distribution: true,
        inputs: vec![],
        outputs: vec![AssetTransactionOutput {
            asset_id: "test_asset_id".to_string(),
            vout: 0,
            amount: 100_000,
            asset_blinder: "blinder123".to_string(),
            amount_blinder: "blinder456".to_string(),
            registered_user: Some(1),
            gaid: Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string()),
            is_treasury: false,
            is_spent: false,
            is_burnt: false,
        }],
        unblinded_url: "https://example.com/tx".to_string(),
    };

    // Test serialization
    let json = serde_json::to_string(&tx).expect("Serialization failed");
    assert!(json.contains("abc123def456"));
    assert!(json.contains("\"is_distribution\":true"));
    assert!(json.contains("100000"));
    assert!(json.contains("\"GAID\":\"GAbYScu6jkWUND2jo3L4KJxyvo55d\""));

    // Test deserialization
    let deserialized: AssetTransaction =
        serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(deserialized.txid, "abc123def456");
    assert_eq!(deserialized.transaction_type(), "distribution");
    assert_eq!(deserialized.total_output_amount(), 100_000);
    assert_eq!(deserialized.outputs[0].registered_user, Some(1));
    assert_eq!(
        deserialized.outputs[0].gaid,
        Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string())
    );
}

#[test]
fn test_asset_transaction_type_method() {
    use amp_rs::model::AssetTransaction;

    // Test issuance
    let issuance_tx = AssetTransaction {
        txid: "tx1".to_string(),
        datetime: "2024-01-01T00:00:00Z".to_string(),
        blockheight: 1,
        is_issuance: true,
        is_reissuance: false,
        is_distribution: false,
        inputs: vec![],
        outputs: vec![],
        unblinded_url: "https://example.com".to_string(),
    };
    assert_eq!(issuance_tx.transaction_type(), "issuance");

    // Test reissuance
    let reissuance_tx = AssetTransaction {
        txid: "tx2".to_string(),
        datetime: "2024-01-01T00:00:00Z".to_string(),
        blockheight: 2,
        is_issuance: false,
        is_reissuance: true,
        is_distribution: false,
        inputs: vec![],
        outputs: vec![],
        unblinded_url: "https://example.com".to_string(),
    };
    assert_eq!(reissuance_tx.transaction_type(), "reissuance");

    // Test distribution
    let distribution_tx = AssetTransaction {
        txid: "tx3".to_string(),
        datetime: "2024-01-01T00:00:00Z".to_string(),
        blockheight: 3,
        is_issuance: false,
        is_reissuance: false,
        is_distribution: true,
        inputs: vec![],
        outputs: vec![],
        unblinded_url: "https://example.com".to_string(),
    };
    assert_eq!(distribution_tx.transaction_type(), "distribution");

    // Test transfer (none of the flags set)
    let transfer_tx = AssetTransaction {
        txid: "tx4".to_string(),
        datetime: "2024-01-01T00:00:00Z".to_string(),
        blockheight: 4,
        is_issuance: false,
        is_reissuance: false,
        is_distribution: false,
        inputs: vec![],
        outputs: vec![],
        unblinded_url: "https://example.com".to_string(),
    };
    assert_eq!(transfer_tx.transaction_type(), "transfer");
}

#[test]
fn test_asset_transaction_clone() {
    use amp_rs::model::AssetTransaction;

    let tx = AssetTransaction {
        txid: "test-txid".to_string(),
        datetime: "2024-01-01T00:00:00Z".to_string(),
        blockheight: 100,
        is_issuance: true,
        is_reissuance: false,
        is_distribution: false,
        inputs: vec![],
        outputs: vec![],
        unblinded_url: "https://example.com".to_string(),
    };

    let cloned = tx.clone();
    assert_eq!(tx.txid, cloned.txid);
    assert_eq!(tx.transaction_type(), cloned.transaction_type());
    assert_eq!(tx.blockheight, cloned.blockheight);
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
    };

    // Test serialization
    let json = serde_json::to_string(&params).expect("Serialization failed");
    assert!(json.contains("\"start\":0"));
    assert!(json.contains("\"count\":25"));
    assert!(json.contains("\"sortcolumn\":\"blockheight\""));
    assert!(json.contains("\"sortorder\":\"desc\""));
    assert!(json.contains("\"height_start\":1000"));
    assert!(json.contains("\"height_stop\":2000"));
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

#[test]
fn test_update_blinders_request_serialization() {
    use amp_rs::UpdateBlindersRequest;

    let request = UpdateBlindersRequest {
        txid: "abc123def456".to_string(),
        vout: 0,
        asset_blinder: "0011223344556677889900112233445566778899001122334455667788990011".to_string(),
        amount_blinder: "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100".to_string(),
    };

    // Test serialization
    let json = serde_json::to_string(&request).expect("Serialization failed");
    assert!(json.contains("abc123def456"));
    assert!(json.contains("\"vout\":0"));
    assert!(json.contains("0011223344556677889900112233445566778899001122334455667788990011"));
    assert!(json.contains("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100"));

    // Verify the JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON parsing failed");
    assert_eq!(parsed["txid"], "abc123def456");
    assert_eq!(parsed["vout"], 0);
}

#[test]
fn test_update_blinders_request_with_different_vout() {
    use amp_rs::UpdateBlindersRequest;

    let request = UpdateBlindersRequest {
        txid: "test_txid".to_string(),
        vout: 5,
        asset_blinder: "asset_blinder_hex".to_string(),
        amount_blinder: "amount_blinder_hex".to_string(),
    };

    let json = serde_json::to_string(&request).expect("Serialization failed");
    assert!(json.contains("\"vout\":5"));
    assert!(json.contains("test_txid"));
}
