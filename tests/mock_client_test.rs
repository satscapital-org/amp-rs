//! Tests for MockApiClient

use amp_rs::model::{
    Asset, AssetTransaction, AssetTransactionParams, CreateAssetAssignmentRequest,
};
use amp_rs::MockApiClient;
use secrecy::ExposeSecret;

#[tokio::test]
async fn test_mock_client_creation() {
    let client = MockApiClient::new();

    // Should be able to get assets
    let assets = client.get_assets().await.unwrap();
    assert!(
        !assets.is_empty(),
        "Default client should have at least one asset"
    );

    // Should be able to get users
    let users = client.get_registered_users().await.unwrap();
    assert!(
        !users.is_empty(),
        "Default client should have at least one user"
    );
}

#[tokio::test]
async fn test_get_assets() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();

    assert!(!assets.is_empty());
    let asset = &assets[0];
    assert!(!asset.asset_uuid.is_empty());
    assert!(!asset.name.is_empty());
}

#[tokio::test]
async fn test_get_asset_by_uuid() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    let asset = client.get_asset(&asset_uuid).await.unwrap();
    assert_eq!(asset.asset_uuid, asset_uuid);
}

#[tokio::test]
async fn test_get_registered_users() {
    let client = MockApiClient::new();
    let users = client.get_registered_users().await.unwrap();

    assert!(!users.is_empty());
    let user = &users[0];
    assert!(!user.name.is_empty());
}

#[tokio::test]
async fn test_validate_gaid() {
    let client = MockApiClient::new();

    // Test with default GAID
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let response = client.validate_gaid(gaid).await.unwrap();
    assert!(response.is_valid);

    // Test with invalid GAID
    let response = client.validate_gaid("INVALID").await.unwrap();
    assert!(!response.is_valid);
}

#[tokio::test]
async fn test_get_gaid_address() {
    let client = MockApiClient::new();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";

    let response = client.get_gaid_address(gaid).await.unwrap();
    assert!(!response.address.is_empty());
}

#[tokio::test]
async fn test_get_gaid_balance() {
    let client = MockApiClient::new();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";

    let balance = client.get_gaid_balance(gaid).await.unwrap();
    // Default GAID should have at least one balance entry
    assert!(!balance.is_empty());
}

#[tokio::test]
async fn test_issue_asset() {
    let client = MockApiClient::new();

    let request = amp_rs::model::IssuanceRequest {
        name: "Test Asset".to_string(),
        amount: 1_000_000_000,
        destination_address:
            "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
                .to_string(),
        domain: "test.com".to_string(),
        ticker: "TEST".to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(false),
    };

    let response = client.issue_asset(&request).await.unwrap();
    assert_eq!(response.name, "Test Asset");
    assert_eq!(response.amount, 1_000_000_000);
    assert!(!response.asset_uuid.is_empty());

    // Asset should now be retrievable
    let asset = client.get_asset(&response.asset_uuid).await.unwrap();
    assert_eq!(asset.name, "Test Asset");
}

#[tokio::test]
async fn test_create_asset_assignments() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    let requests = vec![CreateAssetAssignmentRequest {
        registered_user: 1,
        amount: 1000,
        vesting_timestamp: None,
        ready_for_distribution: true,
    }];

    let assignments = client
        .create_asset_assignments(&asset_uuid, &requests)
        .await
        .unwrap();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].amount, 1000);

    // Should be able to retrieve assignments
    let all_assignments = client.get_asset_assignments(&asset_uuid).await.unwrap();
    assert!(!all_assignments.is_empty());
}

#[tokio::test]
async fn test_create_distribution() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    let assignments = vec![amp_rs::model::AssetDistributionAssignment {
        user_id: "1".to_string(),
        address: "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
            .to_string(),
        amount: 100.0,
    }];

    let response = client
        .create_distribution(&asset_uuid, assignments)
        .await
        .unwrap();
    assert!(!response.distribution_uuid.is_empty());
    assert!(!response.asset_id.is_empty());
}

#[tokio::test]
async fn test_burn_request() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    let response = client.burn_request(&asset_uuid, 100_000).await.unwrap();
    assert_eq!(response.amount, 100_000.0);
    assert!(!response.asset_id.is_empty());
}

#[tokio::test]
async fn test_get_categories() {
    let client = MockApiClient::new();
    let categories = client.get_categories().await.unwrap();

    assert!(!categories.is_empty());
    let category = &categories[0];
    assert!(!category.name.is_empty());
}

#[tokio::test]
async fn test_builder_pattern() {
    let custom_asset = Asset {
        name: "Custom Asset".to_string(),
        asset_uuid: "custom-uuid".to_string(),
        issuer: 1,
        asset_id: "custom_asset_id".to_string(),
        reissuance_token_id: None,
        requirements: vec![],
        ticker: Some("CUSTOM".to_string()),
        precision: 8,
        domain: Some("custom.com".to_string()),
        pubkey: Some("pubkey".to_string()),
        is_registered: true,
        is_authorized: true,
        is_locked: false,
        issuer_authorization_endpoint: None,
        transfer_restricted: false,
    };

    let client = MockApiClient::new()
        .with_asset(custom_asset.clone())
        .build();

    let asset = client.get_asset("custom-uuid").await.unwrap();
    assert_eq!(asset.name, "Custom Asset");
}

#[tokio::test]
async fn test_token_methods() {
    let client = MockApiClient::new();

    let token = client.get_token().await.unwrap();
    assert_eq!(token, "mock_token");

    let token_info = client.get_token_info().await.unwrap();
    assert!(token_info.is_none());

    client.clear_token().await.unwrap();
    let refreshed = client.force_refresh().await.unwrap();
    assert_eq!(refreshed, "mock_token");
}

#[tokio::test]
async fn test_get_asset_reissuances() {
    // Create a reissuable asset
    let reissuable_asset = Asset {
        name: "Reissuable Asset".to_string(),
        asset_uuid: "reissuable-uuid".to_string(),
        issuer: 1,
        asset_id: "reissuable_asset_id".to_string(),
        reissuance_token_id: Some("reissuance_token_id_123".to_string()),
        requirements: vec![],
        ticker: Some("REIS".to_string()),
        precision: 8,
        domain: Some("reissuable.com".to_string()),
        pubkey: Some("pubkey".to_string()),
        is_registered: true,
        is_authorized: true,
        is_locked: false,
        issuer_authorization_endpoint: None,
        transfer_restricted: false,
    };

    let client = MockApiClient::new().with_asset(reissuable_asset).build();

    let asset_uuid = "reissuable-uuid";

    // Initially, asset should have no reissuances
    let reissuances = client.get_asset_reissuances(asset_uuid).await.unwrap();
    assert_eq!(reissuances.len(), 0, "New asset should have no reissuances");

    // Perform a reissue
    let reissue_request = amp_rs::model::ReissueRequest {
        amount_to_reissue: 1_000_000_000,
    };
    let _reissue_response = client
        .reissue_request(asset_uuid, &reissue_request)
        .await
        .unwrap();
    let reissue_confirm = amp_rs::model::ReissueConfirmRequest {
        details: serde_json::json!({}),
        listissuances: vec![],
        reissuance_output: serde_json::json!({}),
    };
    let _confirm_response = client
        .reissue_confirm(asset_uuid, &reissue_confirm)
        .await
        .unwrap();

    // After reissue, asset should have reissuances
    let reissuances = client.get_asset_reissuances(&asset_uuid).await.unwrap();
    assert_eq!(
        reissuances.len(),
        1,
        "Asset should have one reissuance after reissue_confirm"
    );

    // Verify reissuance data
    let reissuance = &reissuances[0];
    assert!(!reissuance.txid.is_empty());
    assert_eq!(reissuance.vout, 0);
    assert!(!reissuance.destination_address.is_empty());
    assert_eq!(reissuance.reissuance_amount, 1_000_000_000);
    assert!(!reissuance.confirmed_in_block.is_empty());
    assert!(!reissuance.created.is_empty());
}

#[tokio::test]
async fn test_get_asset_reissuances_nonexistent_asset() {
    let client = MockApiClient::new();

    // Try to get reissuances for non-existent asset
    let result = client.get_asset_reissuances("nonexistent-uuid").await;
    assert!(result.is_err(), "Should error when asset doesn't exist");
}

#[tokio::test]
async fn test_get_asset_transactions() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    // Get transactions with default params
    let params = AssetTransactionParams::default();
    let transactions = client
        .get_asset_transactions(&asset_uuid, &params)
        .await
        .unwrap();

    // Should have at least the default issuance transaction
    assert!(
        !transactions.is_empty(),
        "Should have at least one transaction"
    );

    // Verify transaction structure
    let tx = &transactions[0];
    assert!(!tx.txid.is_empty(), "Transaction should have a txid");
    assert!(
        !tx.transaction_type().is_empty(),
        "Transaction should have a type"
    );
}

#[tokio::test]
async fn test_get_asset_transactions_with_filtering() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    // Filter by block height (the API doesn't support type filtering directly)
    let params = AssetTransactionParams {
        height_start: Some(1),
        ..Default::default()
    };
    let transactions = client
        .get_asset_transactions(&asset_uuid, &params)
        .await
        .unwrap();

    // Should have issuance transaction (default mock has blockheight 1)
    assert!(!transactions.is_empty());
    assert!(transactions.iter().all(|tx| tx.blockheight >= 1));
}

#[tokio::test]
async fn test_get_asset_transactions_with_pagination() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    // Get transactions with pagination
    let params = AssetTransactionParams {
        start: Some(0),
        count: Some(10),
        ..Default::default()
    };
    let transactions = client
        .get_asset_transactions(&asset_uuid, &params)
        .await
        .unwrap();

    assert!(transactions.len() <= 10, "Should respect count limit");
}

#[tokio::test]
async fn test_get_asset_transaction() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    // First get all transactions to find a valid txid
    let params = AssetTransactionParams::default();
    let transactions = client
        .get_asset_transactions(&asset_uuid, &params)
        .await
        .unwrap();

    assert!(
        !transactions.is_empty(),
        "Should have transactions to test with"
    );

    // Get specific transaction
    let txid = &transactions[0].txid;
    let tx = client
        .get_asset_transaction(&asset_uuid, txid)
        .await
        .unwrap();

    assert_eq!(tx.txid, *txid);
    assert!(!tx.transaction_type().is_empty());
}

#[tokio::test]
async fn test_get_asset_transaction_not_found() {
    let client = MockApiClient::new();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets[0].asset_uuid.clone();

    // Try to get non-existent transaction
    let result = client
        .get_asset_transaction(&asset_uuid, "nonexistent-txid")
        .await;

    assert!(result.is_err(), "Should error when transaction not found");
}

#[tokio::test]
async fn test_get_asset_transactions_nonexistent_asset() {
    let client = MockApiClient::new();

    // Try to get transactions for non-existent asset
    let params = AssetTransactionParams::default();
    let result = client
        .get_asset_transactions("nonexistent-uuid", &params)
        .await;

    assert!(result.is_err(), "Should error when asset doesn't exist");
}

#[tokio::test]
async fn test_with_asset_transaction_builder() {
    use amp_rs::model::{AssetTransactionInput, AssetTransactionOutput};

    let custom_tx = AssetTransaction {
        txid: "custom-txid-12345".to_string(),
        datetime: "2024-06-15T12:00:00Z".to_string(),
        blockheight: 500,
        is_issuance: false,
        is_reissuance: false,
        is_distribution: false,
        inputs: vec![AssetTransactionInput {
            asset_id: "test_asset_id".to_string(),
            vin: 0,
            prev_txid: "prev_txid".to_string(),
            prev_vout: 0,
            amount: 50_000,
            asset_blinder: "0".repeat(64),
            amount_blinder: "0".repeat(64),
            registered_user: Some(1),
            gaid: Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string()),
            is_treasury: false,
        }],
        outputs: vec![AssetTransactionOutput {
            asset_id: "test_asset_id".to_string(),
            vout: 0,
            amount: 50_000,
            asset_blinder: "0".repeat(64),
            amount_blinder: "0".repeat(64),
            registered_user: Some(2),
            gaid: Some("GAnotherGAID12345".to_string()),
            is_treasury: false,
            is_spent: false,
            is_burnt: false,
        }],
        unblinded_url: "https://example.com/tx".to_string(),
    };

    let client = MockApiClient::new()
        .with_asset_transaction("550e8400-e29b-41d4-a716-446655440000", custom_tx.clone())
        .build();

    // Should be able to get the custom transaction
    let tx = client
        .get_asset_transaction("550e8400-e29b-41d4-a716-446655440000", "custom-txid-12345")
        .await
        .unwrap();

    assert_eq!(tx.txid, "custom-txid-12345");
    assert_eq!(tx.transaction_type(), "transfer");
    assert_eq!(tx.total_output_amount(), 50_000);
    assert_eq!(tx.inputs[0].registered_user, Some(1));
}

#[tokio::test]
async fn test_get_asset_transactions_sorting() {
    use amp_rs::model::AssetTransactionOutput;

    let tx1 = AssetTransaction {
        txid: "txid-1".to_string(),
        datetime: "2024-01-01T00:00:00Z".to_string(),
        blockheight: 1,
        is_issuance: false,
        is_reissuance: false,
        is_distribution: false,
        inputs: vec![],
        outputs: vec![AssetTransactionOutput {
            asset_id: "test_asset".to_string(),
            vout: 0,
            amount: 100,
            asset_blinder: "0".repeat(64),
            amount_blinder: "0".repeat(64),
            registered_user: None,
            gaid: None,
            is_treasury: false,
            is_spent: false,
            is_burnt: false,
        }],
        unblinded_url: "https://example.com/tx1".to_string(),
    };

    let tx2 = AssetTransaction {
        txid: "txid-2".to_string(),
        datetime: "2024-01-02T00:00:00Z".to_string(),
        blockheight: 2,
        is_issuance: false,
        is_reissuance: false,
        is_distribution: false,
        inputs: vec![],
        outputs: vec![AssetTransactionOutput {
            asset_id: "test_asset".to_string(),
            vout: 0,
            amount: 200,
            asset_blinder: "0".repeat(64),
            amount_blinder: "0".repeat(64),
            registered_user: None,
            gaid: None,
            is_treasury: false,
            is_spent: false,
            is_burnt: false,
        }],
        unblinded_url: "https://example.com/tx2".to_string(),
    };

    let client = MockApiClient::new()
        .with_asset_transaction("550e8400-e29b-41d4-a716-446655440000", tx1)
        .with_asset_transaction("550e8400-e29b-41d4-a716-446655440000", tx2)
        .build();

    // Get with descending order
    let params = AssetTransactionParams {
        sortorder: Some("desc".to_string()),
        ..Default::default()
    };
    let transactions = client
        .get_asset_transactions("550e8400-e29b-41d4-a716-446655440000", &params)
        .await
        .unwrap();

    assert!(transactions.len() >= 2);
}

#[tokio::test]
async fn test_get_asset_lost_outputs() {
    let client = MockApiClient::new();

    // By default, should return empty lost outputs
    let lost_outputs = client
        .get_asset_lost_outputs("550e8400-e29b-41d4-a716-446655440000")
        .await
        .unwrap();

    assert!(lost_outputs.lost_outputs.is_empty());
    assert!(lost_outputs.reissuance_lost_outputs.is_empty());
}

#[tokio::test]
async fn test_get_asset_lost_outputs_with_data() {
    use amp_rs::model::{AssetLostOutputs, Outpoint};

    let lost_outputs = AssetLostOutputs {
        lost_outputs: vec![
            Outpoint {
                txid: "abc123".to_string(),
                vout: 0,
            },
            Outpoint {
                txid: "def456".to_string(),
                vout: 1,
            },
        ],
        reissuance_lost_outputs: vec![Outpoint {
            txid: "ghi789".to_string(),
            vout: 0,
        }],
    };

    let client = MockApiClient::new()
        .with_asset_lost_outputs("550e8400-e29b-41d4-a716-446655440000", lost_outputs);

    let result = client
        .get_asset_lost_outputs("550e8400-e29b-41d4-a716-446655440000")
        .await
        .unwrap();

    assert_eq!(result.lost_outputs.len(), 2);
    assert_eq!(result.reissuance_lost_outputs.len(), 1);
    assert_eq!(result.lost_outputs[0].txid, "abc123");
}

#[tokio::test]
async fn test_get_asset_lost_outputs_nonexistent_asset() {
    let client = MockApiClient::new();

    let result = client.get_asset_lost_outputs("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_asset_blinders() {
    use amp_rs::UpdateBlindersRequest;

    let client = MockApiClient::new();

    let request = UpdateBlindersRequest {
        txid: "abcd1234".to_string(),
        vout: 0,
        asset_blinder: "00112233".to_string(),
        amount_blinder: "44556677".to_string(),
    };

    // Should succeed for valid asset
    client
        .update_asset_blinders("550e8400-e29b-41d4-a716-446655440000", &request)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_update_asset_blinders_nonexistent_asset() {
    use amp_rs::UpdateBlindersRequest;

    let client = MockApiClient::new();

    let request = UpdateBlindersRequest {
        txid: "abcd1234".to_string(),
        vout: 0,
        asset_blinder: "00112233".to_string(),
        amount_blinder: "44556677".to_string(),
    };

    let result = client.update_asset_blinders("nonexistent", &request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_change_manager_password() {
    use amp_rs::model::Manager;
    use secrecy::Secret;

    let manager = Manager {
        username: "test_manager".to_string(),
        id: 1,
        is_locked: false,
        assets: vec![],
    };

    let client = MockApiClient::new().with_manager(manager);

    let new_password = Secret::new("new_password".to_string());
    let response = client
        .change_manager_password(1, new_password)
        .await
        .unwrap();

    assert_eq!(response.username, "test_manager");
    assert!(!response.token.expose_secret().is_empty());
}

#[tokio::test]
async fn test_change_manager_password_nonexistent_manager() {
    use secrecy::Secret;

    let client = MockApiClient::new();

    let new_password = Secret::new("new_password".to_string());
    let result = client.change_manager_password(999, new_password).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_with_manager_builder() {
    use amp_rs::model::Manager;

    let manager1 = Manager {
        username: "manager1".to_string(),
        id: 1,
        is_locked: false,
        assets: vec!["asset1".to_string()],
    };

    let manager2 = Manager {
        username: "manager2".to_string(),
        id: 2,
        is_locked: true,
        assets: vec![],
    };

    let client = MockApiClient::new()
        .with_manager(manager1)
        .with_manager(manager2);

    // Verify both managers are accessible
    use secrecy::Secret;
    let response1 = client
        .change_manager_password(1, Secret::new("pass1".to_string()))
        .await
        .unwrap();
    assert_eq!(response1.username, "manager1");

    let response2 = client
        .change_manager_password(2, Secret::new("pass2".to_string()))
        .await
        .unwrap();
    assert_eq!(response2.username, "manager2");
}
