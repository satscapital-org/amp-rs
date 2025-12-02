//! Tests for MockApiClient

use amp_rs::MockApiClient;
use amp_rs::model::{Asset, CreateAssetAssignmentRequest};

#[tokio::test]
async fn test_mock_client_creation() {
    let client = MockApiClient::new();
    
    // Should be able to get assets
    let assets = client.get_assets().await.unwrap();
    assert!(!assets.is_empty(), "Default client should have at least one asset");
    
    // Should be able to get users
    let users = client.get_registered_users().await.unwrap();
    assert!(!users.is_empty(), "Default client should have at least one user");
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
        destination_address: "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26".to_string(),
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
    
    let requests = vec![
        CreateAssetAssignmentRequest {
            registered_user: 1,
            amount: 1000,
            vesting_timestamp: None,
            ready_for_distribution: true,
        },
    ];
    
    let assignments = client.create_asset_assignments(&asset_uuid, &requests).await.unwrap();
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
    
    let assignments = vec![
        amp_rs::model::AssetDistributionAssignment {
            user_id: "1".to_string(),
            address: "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26".to_string(),
            amount: 100.0,
        },
    ];
    
    let response = client.create_distribution(&asset_uuid, assignments).await.unwrap();
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
    
    let client = MockApiClient::new()
        .with_asset(reissuable_asset)
        .build();
    
    let asset_uuid = "reissuable-uuid";
    
    // Initially, asset should have no reissuances
    let reissuances = client.get_asset_reissuances(asset_uuid).await.unwrap();
    assert_eq!(reissuances.len(), 0, "New asset should have no reissuances");
    
    // Perform a reissue
    let reissue_request = amp_rs::model::ReissueRequest {
        amount_to_reissue: 1_000_000_000,
    };
    let _reissue_response = client.reissue_request(asset_uuid, &reissue_request).await.unwrap();
    let reissue_confirm = amp_rs::model::ReissueConfirmRequest {
        details: serde_json::json!({}),
        listissuances: vec![],
        reissuance_output: serde_json::json!({}),
    };
    let _confirm_response = client.reissue_confirm(asset_uuid, &reissue_confirm).await.unwrap();
    
    // After reissue, asset should have reissuances
    let reissuances = client.get_asset_reissuances(&asset_uuid).await.unwrap();
    assert_eq!(reissuances.len(), 1, "Asset should have one reissuance after reissue_confirm");
    
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

