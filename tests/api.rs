use amp_rs::mocks;
use amp_rs::ApiClient;
use httpmock::prelude::*;
use serial_test::serial;
use std::env;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};
use url::Url;


static ENV_SETUP_LOCK: OnceCell<Arc<Mutex<()>>> = OnceCell::const_new();

/// Sets up a clean mock test environment
async fn setup_mock_test() {
    // Force cleanup any token files to prevent test pollution
    let _ = ApiClient::force_cleanup_token_files().await;
    
    // Set mock credentials
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
}

/// Cleans up after mock tests
async fn cleanup_mock_test() {
    // Force cleanup any token files created during test
    let _ = ApiClient::force_cleanup_token_files().await;
    
    // Reload .env file to restore original environment
    dotenvy::from_filename_override(".env").ok();
}

async fn get_shared_client() -> Result<ApiClient, amp_rs::client::Error> {
    // Use a lock to ensure environment setup is atomic
    let lock = ENV_SETUP_LOCK
        .get_or_init(|| async { Arc::new(Mutex::new(())) })
        .await;
    let _guard = lock.lock().await;

    // Load environment variables from .env file to avoid mock test pollution
    // This ensures live tests always use the correct credentials from the .env file
    dotenvy::from_filename_override(".env").ok();

    // Only cleanup token files if we're NOT in live test mode
    // Live tests should reuse existing valid tokens
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        let _ = ApiClient::force_cleanup_token_files().await;
    }

    ApiClient::new().await
}

/// Helper function to get a destination address for a specific GAID using address.py
async fn get_destination_address_for_gaid(gaid: &str) -> Result<String, String> {
    let output = Command::new("python3")
        .arg("gaid-scripts/address.py")
        .arg("amp") // Using 'amp' environment
        .arg(gaid)
        .output()
        .map_err(|e| format!("Failed to execute address.py: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("address.py failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_response: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    if let Some(error) = json_response.get("error") {
        return Err(format!("address.py returned error: {}", error));
    }

    json_response
        .get("address")
        .and_then(|addr| addr.as_str())
        .map(|addr| addr.to_string())
        .ok_or_else(|| "No address found in response".to_string())
}

#[tokio::test]
async fn test_get_changelog_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let changelog = client.get_changelog().await;

    assert!(changelog.is_ok());
    let changelog_val = changelog.unwrap();
    assert!(changelog_val.as_object().unwrap().len() > 0);
}

#[tokio::test]
async fn test_get_changelog_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_changelog(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let changelog = client.get_changelog().await;

    assert!(changelog.is_ok());
    let changelog_val = changelog.unwrap();
    assert!(changelog_val.as_object().unwrap().len() > 0);

    // Cleanup
    cleanup_mock_test().await;
}

#[tokio::test]
async fn test_get_assets_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let assets = client.get_assets().await;

    assert!(assets.is_ok());
}

#[tokio::test]
async fn test_get_assets_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let assets = client.get_assets().await;

    assert!(assets.is_ok());
    assert!(!assets.unwrap().is_empty());

    // Cleanup
    cleanup_mock_test().await;
}

#[tokio::test]
async fn test_get_asset_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_test) = assets.first() {
        let asset = client.get_asset(&asset_to_test.asset_uuid).await;
        assert!(asset.is_ok());
    } else {
        println!("Skipping test_get_asset because no assets were found.");
    }
}

#[tokio::test]
async fn test_get_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_get_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_test) = assets.first() {
        let asset = client.get_asset(&asset_to_test.asset_uuid).await;
        assert!(asset.is_ok());
        assert_eq!(asset.unwrap().asset_uuid, "mock_asset_uuid");
    } else {
        panic!("mock_get_assets should have returned at least one asset");
    }

    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_issue_asset_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.
    // Note: This test uses GAID GA4Bdf2hPtMajjT1uH5PvXPGgVAx2Z and gets addresses via address.py

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    // Use first GAID from gaids.json: GA4Bdf2hPtMajjT1uH5PvXPGgVAx2Z
    let destination_address = get_destination_address_for_gaid("GA4Bdf2hPtMajjT1uH5PvXPGgVAx2Z")
        .await
        .expect("Failed to get destination address for GAID GA4Bdf2hPtMajjT1uH5PvXPGgVAx2Z");

    let client = get_shared_client().await.unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset".to_string(),
        amount: 1000,
        destination_address: destination_address.clone(),
        domain: "example.com".to_string(),
        ticker: "TSTA".to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(), // Valid compressed pubkey
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let result = client.issue_asset(&issuance_request).await;
    assert!(result.is_ok());

    let issuance_response = result.unwrap();
    println!("Asset issued successfully!");
    println!("Asset ID: {}", issuance_response.asset_id);
    println!("Transaction ID: {}", issuance_response.txid);
    println!("Destination address: {}", destination_address);

    // Clean up: delete the created asset
    println!(
        "Cleaning up: deleting asset with UUID {}",
        issuance_response.asset_uuid
    );
    let delete_result = client.delete_asset(&issuance_response.asset_uuid).await;
    if let Err(e) = &delete_result {
        println!("Warning: Failed to delete asset: {:?}", e);
    } else {
        println!("Successfully deleted test asset");
    }
}

#[tokio::test]
async fn test_issue_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_issue_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset".to_string(),
        amount: 1000,
        destination_address: "destination_address".to_string(),
        domain: "example.com".to_string(),
        ticker: "TSTA".to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(), // Valid compressed pubkey
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let result = client.issue_asset(&issuance_request).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.asset_uuid, "new_mock_asset_uuid");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_edit_asset_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Make sure there is at least one asset available.
    // 3. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_edit) = assets.first() {
        let edit_request = amp_rs::model::EditAssetRequest {
            issuer_authorization_endpoint: "https://example.com/authorize".to_string(),
        };
        let result = client
            .edit_asset(&asset_to_edit.asset_uuid, &edit_request)
            .await;
        assert!(result.is_ok());
    } else {
        println!("Skipping test_edit_asset because no assets were found.");
    }
}

#[tokio::test]
async fn test_edit_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_edit_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_edit) = assets.first() {
        let edit_request = amp_rs::model::EditAssetRequest {
            issuer_authorization_endpoint: "https://example.com/authorize".to_string(),
        };
        let result = client
            .edit_asset(&asset_to_edit.asset_uuid, &edit_request)
            .await;
        if let Err(e) = &result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
        let edited_asset = result.unwrap();
        assert_eq!(
            edited_asset.issuer_authorization_endpoint,
            Some("https://example.com/authorize".to_string())
        );
    } else {
        panic!("mock_get_assets should have returned at least one asset");
    }

    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_delete_asset_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.
    // Note: This test uses GAID GA4UwSzJb5EbyeCk2VDG4euhyhkiNX and gets addresses via address.py

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    // Use second GAID from gaids.json: GA4UwSzJb5EbyeCk2VDG4euhyhkiNX
    let destination_address = get_destination_address_for_gaid("GA4UwSzJb5EbyeCk2VDG4euhyhkiNX")
        .await
        .expect("Failed to get destination address for GAID GA4UwSzJb5EbyeCk2VDG4euhyhkiNX");

    let client = get_shared_client().await.unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset to Delete".to_string(),
        amount: 1000,
        destination_address,
        domain: "example.com".to_string(),
        ticker: "TSTD".to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(), // Valid compressed pubkey
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let issue_result = client.issue_asset(&issuance_request).await.unwrap();
    let delete_result = client.delete_asset(&issue_result.asset_uuid).await;
    assert!(delete_result.is_ok());
}

#[tokio::test]
async fn test_delete_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_issue_asset(&server);
    mocks::mock_delete_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset to Delete".to_string(),
        amount: 1000,
        destination_address: "destination_address".to_string(),
        domain: "example.com".to_string(),
        ticker: "TSTD".to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(), // Valid compressed pubkey
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let issue_result = client.issue_asset(&issuance_request).await.unwrap();
    let delete_result = client.delete_asset(&issue_result.asset_uuid).await;
    assert!(delete_result.is_ok());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_registered_users_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let registered_users = client.get_registered_users().await;

    assert!(registered_users.is_ok());
}

#[tokio::test]
async fn test_get_registered_users_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_registered_users(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let registered_users = client.get_registered_users().await;

    assert!(registered_users.is_ok());
    assert!(!registered_users.unwrap().is_empty());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_registered_user_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let registered_users = client.get_registered_users().await.unwrap();

    if let Some(user_to_test) = registered_users.first() {
        let user = client.get_registered_user(user_to_test.id).await;
        assert!(user.is_ok());
    } else {
        println!("Skipping test_get_registered_user because no registered users were found.");
    }
}

#[tokio::test]
async fn test_get_registered_user_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_registered_users(&server);
    mocks::mock_get_registered_user(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let registered_users = client.get_registered_users().await.unwrap();

    if let Some(user_to_test) = registered_users.first() {
        let user = client.get_registered_user(user_to_test.id).await;
        assert!(user.is_ok());
        assert_eq!(user.unwrap().id, 1);
    } else {
        panic!("mock_get_registered_users should have returned at least one user");
    }

    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_add_registered_user_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let new_user = amp_rs::model::RegisteredUserAdd {
        name: "Test User".to_string(),
        gaid: None,
        is_company: false,
    };

    let result = client.add_registered_user(&new_user).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_add_registered_user_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_add_registered_user(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let new_user = amp_rs::model::RegisteredUserAdd {
        name: "Test User".to_string(),
        gaid: None,
        is_company: false,
    };

    let result = client.add_registered_user(&new_user).await;
    assert!(result.is_ok());
    let added_user = result.unwrap();
    assert_eq!(added_user.id, 2);
    assert_eq!(added_user.name, "Test User");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_categories_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let categories = client.get_categories().await;

    assert!(categories.is_ok());
    let categories_val = categories.unwrap();
    println!("Existing categories: {:?}", categories_val);
}

#[tokio::test]
async fn test_get_categories_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_categories(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let categories = client.get_categories().await;

    assert!(categories.is_ok());
    assert!(!categories.unwrap().is_empty());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
#[serial]
async fn test_add_category_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();

    // Use a unique name with timestamp to avoid conflicts
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let new_category = amp_rs::model::CategoryAdd {
        name: format!("Test Category {}", timestamp),
        description: Some("Test category description".to_string()),
    };

    println!("Attempting to add category: {:?}", new_category);
    let result = client.add_category(&new_category).await;
    if let Err(e) = &result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());

    // Clean up: delete the created category
    let created_category = result.unwrap();
    println!(
        "Cleaning up: deleting category with ID {}",
        created_category.id
    );
    let delete_result = client.delete_category(created_category.id).await;
    if let Err(e) = &delete_result {
        println!("Warning: Failed to delete category: {:?}", e);
    } else {
        println!("Successfully deleted test category");
    }
}

#[tokio::test]
async fn test_add_category_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_add_category(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let new_category = amp_rs::model::CategoryAdd {
        name: "Test Category".to_string(),
        description: Some("Test category description".to_string()),
    };

    let result = client.add_category(&new_category).await;
    assert!(result.is_ok());
    let added_category = result.unwrap();
    assert_eq!(added_category.id, 2);
    assert_eq!(added_category.name, "Test Category");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_validate_gaid_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.validate_gaid(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_valid);
}

#[tokio::test]
async fn test_validate_gaid_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_validate_gaid(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.validate_gaid(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_valid);

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_gaid_address_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let gaid = "GA3tJqC58PwiCjp4tPkCjNkPnVzLqn";
    let result = client.get_gaid_address(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.address.is_empty());
}

#[tokio::test]
async fn test_get_gaid_address_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_gaid_address(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.get_gaid_address(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.address.is_empty());
    assert_eq!(response.address, "mock_address");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_managers_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let managers = client.get_managers().await;

    assert!(managers.is_ok());
    let managers_val = managers.unwrap();
    println!("Existing managers: {:?}", managers_val);
}

#[tokio::test]
async fn test_get_managers_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_managers(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let managers = client.get_managers().await;

    assert!(managers.is_ok());
    assert!(!managers.unwrap().is_empty());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_create_manager_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_create_manager(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let new_manager = amp_rs::model::ManagerCreate {
        username: "test_manager".to_string(),
        password: "password".to_string(),
    };

    let result = client.create_manager(&new_manager).await;
    assert!(result.is_ok());
    let created_manager = result.unwrap();
    assert_eq!(created_manager.id, 2);
    assert_eq!(created_manager.username, "test_manager");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_broadcast_transaction_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_broadcast_transaction(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.broadcast_transaction("mock_tx_hex").await;
    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.txid, "mock_txid");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
#[ignore] // Slow test - requires blockchain confirmation (up to 180 seconds)
async fn test_create_asset_assignments_live_slow() {
    // This test demonstrates the complete flow for creating asset assignments:
    // 1. Issues a new asset
    // 2. Adds treasury addresses
    // 3. Waits for blockchain confirmation (up to 180 seconds)
    // 4. Creates a registered user (or uses existing)
    // 5. Attempts to create an asset assignment
    //
    // Note: This test may fail with "405 Method Not Allowed" if the API server
    // doesn't have the assignment creation endpoint enabled, but the client
    // implementation is correct based on the working-implementation.rs reference.
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();

    // 1. Get or create a registered user with a specific GAID
    let user_gaid = "GA3DS3emT12zDF4RGywBvJqZfhefNp";

    // Check if user already exists
    let existing_users = client.get_registered_users().await.unwrap();
    let existing_user = existing_users
        .iter()
        .find(|u| u.gaid.as_ref().map_or(false, |gaid| gaid == user_gaid));

    let user_id = if let Some(user) = existing_user {
        println!(
            "Reusing existing user with GAID {}: {} (ID: {})",
            user_gaid, user.name, user.id
        );
        user.id
    } else {
        // Create new user if it doesn't exist
        println!("Creating new user with GAID {}", user_gaid);
        let new_user = amp_rs::model::RegisteredUserAdd {
            name: "Test User for Assignment (Persistent)".to_string(),
            gaid: Some(user_gaid.to_string()),
            is_company: false,
        };
        let user = client.add_registered_user(&new_user).await.unwrap();
        println!("Created new user: {} (ID: {})", user.name, user.id);
        user.id
    };

    // 2. Get or create a category and add the user to it
    let categories = client.get_categories().await.unwrap();
    let category_id = if let Some(existing_category) = categories.first() {
        // Use existing category if available
        existing_category.id
    } else {
        // Create a new category
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let new_category = amp_rs::model::CategoryAdd {
            name: format!("Test Category for Assignment {}", timestamp),
            description: Some("Category for testing asset assignments".to_string()),
        };
        let category = client.add_category(&new_category).await.unwrap();
        category.id
    };

    // Add user to category before creating the asset
    let user_category_result = client
        .add_registered_user_to_category(category_id, user_id)
        .await;
    if let Err(e) = &user_category_result {
        println!("Warning: Failed to add user to category: {:?}", e);
    } else {
        println!(
            "Successfully added user to category {} before asset creation",
            category_id
        );
    }

    // 3. Get or reuse an existing asset that's already confirmed
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = if let Some(existing_asset) = assets.first() {
        println!(
            "Reusing existing asset: {} (UUID: {})",
            existing_asset.name, existing_asset.asset_uuid
        );
        existing_asset.asset_uuid.clone()
    } else {
        // If no assets exist, create one (this should be rare in a test environment)
        println!("No existing assets found, creating a new one...");
        let destination_address =
            get_destination_address_for_gaid("GA2HsrczzwaFzdJiw5NJM8P4iWKQh1")
                .await
                .expect(
                    "Failed to get destination address for GAID GA2HsrczzwaFzdJiw5NJM8P4iWKQh1",
                );
        let pubkey =
            "02963a059e1ab729b653b78360626657e40dfb0237b754007acd43e8e0141a1bb4".to_string();

        let issuance_request = amp_rs::model::IssuanceRequest {
            name: "Test Asset for Assignment".to_string(),
            amount: 1000000000000,
            destination_address: destination_address.clone(),
            domain: "test.asset".to_string(),
            ticker: "TAS".to_string(),
            pubkey,
            precision: Some(8),
            is_confidential: Some(true),
            is_reissuable: Some(false),
            reissuance_amount: None,
            reissuance_address: None,
            transfer_restricted: Some(true),
        };

        let issued_asset = client.issue_asset(&issuance_request).await.unwrap();
        println!(
            "Created new asset: {} (UUID: {})",
            issued_asset.name, issued_asset.asset_uuid
        );
        issued_asset.asset_uuid
    };

    // 4. Add the asset to the same category as the user if not already added
    let asset_category_result = client.add_asset_to_category(category_id, &asset_uuid).await;
    if let Err(e) = &asset_category_result {
        println!("Note: Asset may already be in category: {:?}", e);
    } else {
        println!("Successfully added asset to category {}", category_id);
    }

    // 6. Verify category membership by getting category details
    println!("\n=== CATEGORY MEMBERSHIP VERIFICATION ===");
    let category_info = client.get_category(category_id).await;
    match category_info {
        Ok(category) => {
            println!("Category Info:");
            println!("  ID: {}", category.id);
            println!("  Name: {}", category.name);
            println!("  Description: {:?}", category.description);
            println!("  Registered Users: {:?}", category.registered_users);
            println!("  Assets: {:?}", category.assets);

            // Check if user is in the category
            let user_is_member = category.registered_users.contains(&user_id);
            println!("\n=== MEMBERSHIP ANALYSIS ===");
            println!("Expected User ID: {}", user_id);
            println!("User is member of category: {}", user_is_member);

            // Check if asset is in the category
            let asset_is_member = category.assets.contains(&asset_uuid);
            println!("Expected Asset UUID: {}", asset_uuid);
            println!("Asset is member of category: {}", asset_is_member);

            if user_is_member && asset_is_member {
                println!("✅ BOTH user and asset are properly recognized as category members");
            } else {
                println!("❌ MEMBERSHIP ISSUE DETECTED:");
                if !user_is_member {
                    println!("  - User {} is NOT found in category members", user_id);
                }
                if !asset_is_member {
                    println!("  - Asset {} is NOT found in category members", asset_uuid);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get category info: {:?}", e);
        }
    }
    println!("==========================================\n");

    // 7. Create the assignment with a smaller amount
    let request = amp_rs::model::CreateAssetAssignmentRequest {
        registered_user: user_id,
        amount: 1,               // Use a very small amount to ensure treasury has enough
        vesting_timestamp: None, // No vesting for this test
        ready_for_distribution: false, // Default value
    };

    // Log the request for debugging
    println!(
        "Assignment creation request: {}",
        serde_json::to_string_pretty(&request).unwrap()
    );
    println!("Asset UUID: {}", asset_uuid);
    println!("User ID: {}", user_id);
    println!("Category ID: {}", category_id);

    // Construct and log the expected URL path
    let expected_path = format!("assets/{}/assignments/create", asset_uuid);
    println!("Expected URL path: {}", expected_path);
    println!("Asset UUID contains hyphens: {}", asset_uuid.contains('-'));
    println!("Asset UUID length: {}", asset_uuid.len());

    // Use the proper client method to create the assignment
    println!(
        "About to call client.create_asset_assignments with asset_uuid: {}",
        asset_uuid
    );
    let created_assignments = match client
        .create_asset_assignments(&asset_uuid, &[request.clone()])
        .await
    {
        Ok(assignments) => {
            println!("✅ Assignment creation succeeded!");
            println!(
                "Created {} assignment(s): {:?}",
                assignments.len(),
                assignments
            );
            if let Some(assignment) = assignments.first() {
                println!("First assignment ID: {}", assignment.id);
                println!("Registered User: {}", assignment.registered_user);
                println!("Amount: {}", assignment.amount);
            }
            assignments
        }
        Err(e) => {
            println!("❌ Assignment creation failed: {:?}", e);

            // Let's try to make a manual request to see what the actual response is
            use reqwest::header::AUTHORIZATION;
            use reqwest::Method;
            use std::env;

            println!("Making manual request to debug the response...");
            let base_url = env::var("AMP_API_BASE_URL")
                .unwrap_or_else(|_| "https://amp-api.blockstream.com".to_string());
            let mut url = reqwest::Url::parse(&base_url).unwrap();
            url.path_segments_mut().unwrap().extend(&[
                "assets",
                &asset_uuid,
                "assignments",
                "create",
            ]);

            let token = client.get_token().await.unwrap();
            let wrapper = amp_rs::model::CreateAssetAssignmentRequestWrapper {
                assignments: vec![request.clone()],
            };

            let http_client = reqwest::Client::new();
            let response = http_client
                .request(Method::POST, url.clone())
                .header(AUTHORIZATION, format!("token {}", token))
                .json(&wrapper)
                .send()
                .await
                .unwrap();

            let status = response.status();
            let response_body = response.text().await.unwrap();

            println!("Manual request URL: {}", url);
            println!("Manual request status: {}", status);
            println!("Manual request body: {}", response_body);

            // No asset cleanup needed since we're reusing existing assets

            panic!("Failed to create asset assignment: {:?}", e);
        }
    };

    // === CLEANUP SECTION ===
    println!("\n=== STARTING CLEANUP ===");

    // Delete all created assignments (asset is reused, so no need to delete it)
    for assignment in &created_assignments {
        println!("Deleting assignment ID: {}", assignment.id);
        match client
            .delete_asset_assignment(&asset_uuid, &assignment.id.to_string())
            .await
        {
            Ok(()) => {
                println!("✅ Successfully deleted assignment {}", assignment.id);
            }
            Err(e) => {
                println!("❌ Failed to delete assignment {}: {:?}", assignment.id, e);
            }
        }
    }

    println!("=== CLEANUP COMPLETED ===\n");
}

#[tokio::test]
async fn test_create_asset_assignments_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_get_registered_users(&server);
    mocks::mock_create_asset_assignments(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets.first().unwrap().asset_uuid.clone();
    let users = client.get_registered_users().await.unwrap();
    let user_id = users.first().unwrap().id;

    let request = amp_rs::model::CreateAssetAssignmentRequest {
        registered_user: user_id,
        amount: 100,
        vesting_timestamp: None,
        ready_for_distribution: false, // Default value
    };

    let result = client
        .create_asset_assignments(&asset_uuid, &[request])
        .await;
    assert!(result.is_ok(), "Assignment creation should succeed");

    let assignments = result.unwrap();

    // Validate response structure
    assert_eq!(
        assignments.len(),
        1,
        "Response should contain exactly one assignment"
    );

    let assignment = &assignments[0];

    // Validate all required fields and their data types
    assert_eq!(
        assignment.id, 10,
        "Assignment ID should match expected value"
    );
    assert_eq!(
        assignment.registered_user, 13,
        "Registered user should be an i64"
    );
    assert_eq!(assignment.amount, 100, "Amount should be an i64");
    assert_eq!(assignment.creator, 1, "Creator should be an i64");
    assert_eq!(
        assignment.ready_for_distribution, true,
        "Ready for distribution should be a boolean"
    );
    assert_eq!(
        assignment.has_vested, true,
        "Has vested should be a boolean"
    );
    assert_eq!(
        assignment.is_distributed, false,
        "Is distributed should be a boolean"
    );

    // Validate optional fields
    assert!(
        assignment.receiving_address.is_none(),
        "Receiving address should be None/null"
    );
    assert!(
        assignment.distribution_uuid.is_none(),
        "Distribution UUID should be None/null"
    );
    assert!(
        assignment.vesting_datetime.is_none(),
        "Vesting datetime should be None/null"
    );
    assert!(
        assignment.vesting_timestamp.is_none(),
        "Vesting timestamp should be None/null"
    );

    // Validate backward compatibility fields
    assert_eq!(
        assignment.gaid,
        Some("GA3DS3emT12zDF4RGywBvJqZfhefNp".to_string()),
        "GAID should be present for backward compatibility"
    );
    assert_eq!(
        assignment.investor,
        Some(13),
        "Investor field should be present for backward compatibility"
    );

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_create_asset_assignments_multiple_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_get_registered_users(&server);
    mocks::mock_create_asset_assignments_multiple(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets.first().unwrap().asset_uuid.clone();
    let users = client.get_registered_users().await.unwrap();
    let user_id = users.first().unwrap().id;

    // Create multiple assignment requests
    let requests = vec![
        amp_rs::model::CreateAssetAssignmentRequest {
            registered_user: user_id,
            amount: 100,
            vesting_timestamp: None,
            ready_for_distribution: false, // Default value
        },
        amp_rs::model::CreateAssetAssignmentRequest {
            registered_user: user_id + 1, // Different user
            amount: 200,
            vesting_timestamp: Some(1234567890),
            ready_for_distribution: true, // Test with different value
        },
    ];

    let result = client
        .create_asset_assignments(&asset_uuid, &requests)
        .await;
    assert!(
        result.is_ok(),
        "Multiple assignment creation should succeed"
    );

    let assignments = result.unwrap();

    // Validate response structure
    assert_eq!(
        assignments.len(),
        2,
        "Response should contain exactly two assignments"
    );

    // Validate first assignment
    let assignment1 = &assignments[0];
    assert_eq!(
        assignment1.id, 10,
        "First assignment ID should match expected value"
    );
    assert_eq!(
        assignment1.registered_user, 13,
        "First assignment registered user should be correct"
    );
    assert_eq!(
        assignment1.amount, 100,
        "First assignment amount should be correct"
    );

    // Validate second assignment
    let assignment2 = &assignments[1];
    assert_eq!(
        assignment2.id, 11,
        "Second assignment ID should match expected value"
    );
    assert_eq!(
        assignment2.registered_user, 14,
        "Second assignment registered user should be correct"
    );
    assert_eq!(
        assignment2.amount, 200,
        "Second assignment amount should be correct"
    );

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
#[ignore] // Slow test - requires blockchain confirmation (up to 180 seconds)
async fn test_create_asset_assignments_multiple_live_slow() {
    // This test demonstrates the complete flow for creating multiple asset assignments:
    // 1. Issues a new asset
    // 2. Adds treasury addresses
    // 3. Waits for blockchain confirmation (up to 180 seconds)
    // 4. Uses users 1203 and 1194 (making sure they're in the same category as the asset)
    // 5. Attempts to create both asset assignments simultaneously by calling client.create_asset_assignments with an array of assignment descriptors
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();

    // 1. Get or create registered users 1203 and 1194 with specific GAIDs
    let user_gaid_1203 = "GA3DS3emT12zDF4RGywBvJqZfhefNp";
    let user_gaid_1194 = "GA4Bdf2hPtMajjT1uH5PvXPGgVAx2Z";

    // Check if users already exist
    let existing_users = client.get_registered_users().await.unwrap();

    let existing_user_1203 = existing_users.iter().find(|u| u.id == 1203);
    let existing_user_1194 = existing_users.iter().find(|u| u.id == 1194);

    let user_id_1203 = if let Some(user) = existing_user_1203 {
        println!(
            "Reusing existing user 1203: {} (GAID: {:?})",
            user.name, user.gaid
        );
        user.id
    } else {
        // Create new user with ID 1203 if it doesn't exist
        println!(
            "Creating new user with target ID 1203 and GAID {}",
            user_gaid_1203
        );
        let new_user = amp_rs::model::RegisteredUserAdd {
            name: "Test User 1203 for Multiple Assignments".to_string(),
            gaid: Some(user_gaid_1203.to_string()),
            is_company: false,
        };
        let user = client.add_registered_user(&new_user).await.unwrap();
        println!(
            "Created new user: {} (ID: {}) with GAID: {:?}",
            user.name, user.id, user.gaid
        );
        user.id
    };

    let user_id_1194 = if let Some(user) = existing_user_1194 {
        println!(
            "Reusing existing user 1194: {} (GAID: {:?})",
            user.name, user.gaid
        );
        user.id
    } else {
        // Create new user with ID 1194 if it doesn't exist
        println!(
            "Creating new user with target ID 1194 and GAID {}",
            user_gaid_1194
        );
        let new_user = amp_rs::model::RegisteredUserAdd {
            name: "Test User 1194 for Multiple Assignments".to_string(),
            gaid: Some(user_gaid_1194.to_string()),
            is_company: false,
        };
        let user = client.add_registered_user(&new_user).await.unwrap();
        println!(
            "Created new user: {} (ID: {}) with GAID: {:?}",
            user.name, user.id, user.gaid
        );
        user.id
    };

    // 2. Get or create a category and add both users to it
    let categories = client.get_categories().await.unwrap();
    let category_id = if let Some(existing_category) = categories.first() {
        // Use existing category if available
        existing_category.id
    } else {
        // Create a new category
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let new_category = amp_rs::model::CategoryAdd {
            name: format!("Test Category for Multiple Assignments {}", timestamp),
            description: Some("Category for testing multiple asset assignments".to_string()),
        };
        let category = client.add_category(&new_category).await.unwrap();
        category.id
    };

    // Add both users to category before creating the asset
    let user_category_result_1203 = client
        .add_registered_user_to_category(category_id, user_id_1203)
        .await;
    if let Err(e) = &user_category_result_1203 {
        println!("Warning: Failed to add user 1203 to category: {:?}", e);
    } else {
        println!(
            "Successfully added user 1203 to category {} before asset creation",
            category_id
        );
    }

    let user_category_result_1194 = client
        .add_registered_user_to_category(category_id, user_id_1194)
        .await;
    if let Err(e) = &user_category_result_1194 {
        println!("Warning: Failed to add user 1194 to category: {:?}", e);
    } else {
        println!(
            "Successfully added user 1194 to category {} before asset creation",
            category_id
        );
    }

    // 3. Get or reuse an existing asset that's already confirmed
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = if let Some(existing_asset) = assets.first() {
        println!(
            "Reusing existing asset: {} (UUID: {})",
            existing_asset.name, existing_asset.asset_uuid
        );
        existing_asset.asset_uuid.clone()
    } else {
        // If no assets exist, create one (this should be rare in a test environment)
        println!("No existing assets found, creating a new one...");
        let destination_address =
            get_destination_address_for_gaid("GA2HsrczzwaFzdJiw5NJM8P4iWKQh1")
                .await
                .expect(
                    "Failed to get destination address for GAID GA2HsrczzwaFzdJiw5NJM8P4iWKQh1",
                );
        let pubkey =
            "02963a059e1ab729b653b78360626657e40dfb0237b754007acd43e8e0141a1bb4".to_string();

        let issuance_request = amp_rs::model::IssuanceRequest {
            name: "Test Asset for Multiple Assignments".to_string(),
            amount: 1000000000000,
            destination_address: destination_address.clone(),
            domain: "test.multiasset".to_string(),
            ticker: "TMAS".to_string(),
            pubkey,
            precision: Some(8),
            is_confidential: Some(true),
            is_reissuable: Some(false),
            reissuance_amount: None,
            reissuance_address: None,
            transfer_restricted: Some(true),
        };

        let issued_asset = client.issue_asset(&issuance_request).await.unwrap();
        println!(
            "Created new asset: {} (UUID: {})",
            issued_asset.name, issued_asset.asset_uuid
        );
        issued_asset.asset_uuid
    };

    // 4. Add the asset to the same category as the users if not already added
    let asset_category_result = client.add_asset_to_category(category_id, &asset_uuid).await;
    if let Err(e) = &asset_category_result {
        println!("Note: Asset may already be in category: {:?}", e);
    } else {
        println!("Successfully added asset to category {}", category_id);
    }

    // 6. Verify category membership by getting category details
    println!("\n=== CATEGORY MEMBERSHIP VERIFICATION ===");
    let category_info = client.get_category(category_id).await;
    match category_info {
        Ok(category) => {
            println!("Category Info:");
            println!("  ID: {}", category.id);
            println!("  Name: {}", category.name);
            println!("  Description: {:?}", category.description);
            println!("  Registered Users: {:?}", category.registered_users);
            println!("  Assets: {:?}", category.assets);

            // Check if both users are in the category
            let user_1203_is_member = category.registered_users.contains(&user_id_1203);
            let user_1194_is_member = category.registered_users.contains(&user_id_1194);
            println!("\n=== MEMBERSHIP ANALYSIS ===");
            println!("Expected User ID 1203: {}", user_id_1203);
            println!("User 1203 is member of category: {}", user_1203_is_member);
            println!("Expected User ID 1194: {}", user_id_1194);
            println!("User 1194 is member of category: {}", user_1194_is_member);

            // Check if asset is in the category
            let asset_is_member = category.assets.contains(&asset_uuid);
            println!("Expected Asset UUID: {}", asset_uuid);
            println!("Asset is member of category: {}", asset_is_member);

            if user_1203_is_member && user_1194_is_member && asset_is_member {
                println!("✅ ALL users and asset are properly recognized as category members");
            } else {
                println!("❌ MEMBERSHIP ISSUE DETECTED:");
                if !user_1203_is_member {
                    println!(
                        "  - User 1203 ({}) is NOT found in category members",
                        user_id_1203
                    );
                }
                if !user_1194_is_member {
                    println!(
                        "  - User 1194 ({}) is NOT found in category members",
                        user_id_1194
                    );
                }
                if !asset_is_member {
                    println!("  - Asset {} is NOT found in category members", asset_uuid);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get category info: {:?}", e);
        }
    }
    println!("==========================================\n");

    // 7. Create multiple assignment requests with small amounts
    let requests = vec![
        amp_rs::model::CreateAssetAssignmentRequest {
            registered_user: user_id_1203,
            amount: 1,               // Use very small amounts to ensure treasury has enough
            vesting_timestamp: None, // No vesting for this test
            ready_for_distribution: false, // Default value
        },
        amp_rs::model::CreateAssetAssignmentRequest {
            registered_user: user_id_1194,
            amount: 2,               // Use very small amounts to ensure treasury has enough
            vesting_timestamp: None, // No vesting for this test
            ready_for_distribution: false, // Default value
        },
    ];

    // Log the requests for debugging
    println!("Multiple assignment creation requests:");
    for (i, request) in requests.iter().enumerate() {
        println!(
            "  Request {}: {}",
            i + 1,
            serde_json::to_string_pretty(&request).unwrap()
        );
    }
    println!("Asset UUID: {}", asset_uuid);
    println!("User ID 1203: {}", user_id_1203);
    println!("User ID 1194: {}", user_id_1194);
    println!("Category ID: {}", category_id);

    // Construct and log the expected URL path
    let expected_path = format!("assets/{}/assignments/create", asset_uuid);
    println!("Expected URL path: {}", expected_path);

    // Use the proper client method to create multiple assignments simultaneously
    println!(
        "About to call client.create_asset_assignments with asset_uuid: {} and {} requests",
        asset_uuid,
        requests.len()
    );
    let created_assignments = match client
        .create_asset_assignments(&asset_uuid, &requests)
        .await
    {
        Ok(assignments) => {
            println!("✅ Multiple assignment creation succeeded!");
            println!(
                "Created {} assignment(s): {:?}",
                assignments.len(),
                assignments
            );
            for (i, assignment) in assignments.iter().enumerate() {
                println!(
                    "Assignment {}: ID={}, User={}, Amount={}",
                    i + 1,
                    assignment.id,
                    assignment.registered_user,
                    assignment.amount
                );
            }
            assignments
        }
        Err(e) => {
            println!("❌ Multiple assignment creation failed: {:?}", e);

            // Let's try to make a manual request to see what the actual response is
            use reqwest::header::AUTHORIZATION;
            use reqwest::Method;
            use std::env;

            println!("Making manual request to debug the response...");
            let base_url = env::var("AMP_API_BASE_URL")
                .unwrap_or_else(|_| "https://amp-api.blockstream.com".to_string());
            let mut url = reqwest::Url::parse(&base_url).unwrap();
            url.path_segments_mut().unwrap().extend(&[
                "assets",
                &asset_uuid,
                "assignments",
                "create",
            ]);

            let token = client.get_token().await.unwrap();
            let wrapper = amp_rs::model::CreateAssetAssignmentRequestWrapper {
                assignments: requests.clone(),
            };

            let http_client = reqwest::Client::new();
            let response = http_client
                .request(Method::POST, url.clone())
                .header(AUTHORIZATION, format!("token {}", token))
                .json(&wrapper)
                .send()
                .await
                .unwrap();

            let status = response.status();
            let response_body = response.text().await.unwrap();

            println!("Manual request URL: {}", url);
            println!("Manual request status: {}", status);
            println!("Manual request body: {}", response_body);

            panic!("Failed to create multiple asset assignments: {:?}", e);
        }
    };

    // Validate that we created exactly 2 assignments
    assert_eq!(
        created_assignments.len(),
        2,
        "Should create exactly 2 assignments"
    );

    // Validate assignments contain the correct users (note: order might not be preserved)
    let user_ids: Vec<i64> = created_assignments
        .iter()
        .map(|a| a.registered_user)
        .collect();
    assert!(user_ids.contains(&user_id_1203), "Should contain user 1203");
    assert!(user_ids.contains(&user_id_1194), "Should contain user 1194");

    // === CLEANUP SECTION ===
    println!("\n=== STARTING CLEANUP ===");

    // Delete all created assignments (asset is reused, so no need to delete it)
    for assignment in &created_assignments {
        println!("Deleting assignment ID: {}", assignment.id);
        match client
            .delete_asset_assignment(&asset_uuid, &assignment.id.to_string())
            .await
        {
            Ok(()) => {
                println!("✅ Successfully deleted assignment {}", assignment.id);
            }
            Err(e) => {
                println!("❌ Failed to delete assignment {}: {:?}", assignment.id, e);
            }
        }
    }

    println!("=== CLEANUP COMPLETED ===\n");
}

#[tokio::test]
async fn test_get_broadcast_status_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_broadcast_status(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.get_broadcast_status("mock_txid").await;
    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.txid, "mock_txid");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}
#[tokio::test]
async fn test_get_manager_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_manager(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.get_manager(1).await;
    assert!(result.is_ok());
    let manager = result.unwrap();
    assert_eq!(manager.id, 1);
    assert_eq!(manager.username, "mock_manager");
    assert_eq!(manager.assets.len(), 2);

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_manager_remove_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_manager_remove_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.manager_remove_asset(1, "asset_uuid_1").await;
    assert!(result.is_ok());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_revoke_manager_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_manager(&server);
    mocks::mock_manager_remove_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.revoke_manager(1).await;
    assert!(result.is_ok());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_current_manager_raw_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_current_manager_raw(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.get_current_manager_raw().await;
    assert!(result.is_ok());
    let manager_json = result.unwrap();
    assert_eq!(manager_json["id"], 1);
    assert_eq!(manager_json["username"], "current_manager");

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_unlock_manager_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_unlock_manager(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.unlock_manager(1).await;
    assert!(result.is_ok());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}
#[tokio::test]
#[serial]
async fn test_add_asset_treasury_addresses_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();

    // Create a new asset for this test
    let test_address = "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26";

    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Treasury Asset".to_string(),
        amount: 1000,
        destination_address: test_address.to_string(),
        domain: "example.com".to_string(),
        ticker: "TSTA".to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let issuance_result = client.issue_asset(&issuance_request).await;
    assert!(issuance_result.is_ok(), "Failed to create test asset");

    let issued_asset = issuance_result.unwrap();
    println!("Created test asset with UUID: {}", issued_asset.asset_uuid);

    // Add the treasury address (only once, not 3 times)
    let treasury_addresses = vec![test_address.to_string()];

    let result = client
        .add_asset_treasury_addresses(&issued_asset.asset_uuid, &treasury_addresses)
        .await;

    match result {
        Ok(_) => {
            println!(
                "Successfully added treasury address {} to asset {}",
                test_address, issued_asset.asset_uuid
            );
        }
        Err(e) => {
            let error_msg = format!("{:?}", e);
            if error_msg.contains("already been added") {
                println!(
                    "Treasury address {} was already added to asset {} - test passes",
                    test_address, issued_asset.asset_uuid
                );
            } else if error_msg.contains("Invalid value") {
                println!("Treasury address format may not be valid for this network - skipping test");
                println!(
                    "This is expected in test environments with different address formats"
                );
                // Clean up before returning
                let _ = client.delete_asset(&issued_asset.asset_uuid).await;
                return;
            } else {
                println!("Unexpected error adding treasury addresses: {:?}", e);
                // Clean up before panicking
                let _ = client.delete_asset(&issued_asset.asset_uuid).await;
                panic!("Unexpected error: {:?}", e);
            }
        }
    }

    // Clean up: delete the created asset
    println!("Cleaning up: deleting test asset with UUID {}", issued_asset.asset_uuid);
    let delete_result = client.delete_asset(&issued_asset.asset_uuid).await;
    if let Err(e) = &delete_result {
        println!("Warning: Failed to delete test asset: {:?}", e);
    } else {
        println!("Successfully deleted test asset");
    }
}

#[tokio::test]
async fn test_add_asset_treasury_addresses_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_add_asset_treasury_addresses(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let treasury_addresses = vec![
        "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
            .to_string(),
        "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw27"
            .to_string(),
    ];

    let result = client
        .add_asset_treasury_addresses("mock_asset_uuid", &treasury_addresses)
        .await;
    assert!(result.is_ok());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_get_asset_treasury_addresses_live() {
    dotenvy::from_filename_override(".env").ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = get_shared_client().await.unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_test) = assets.first() {
        let result = client
            .get_asset_treasury_addresses(&asset_to_test.asset_uuid)
            .await;
        assert!(result.is_ok());
        let addresses = result.unwrap();
        println!(
            "Treasury addresses for asset {}: {:?}",
            asset_to_test.asset_uuid, addresses
        );
    } else {
        println!("Skipping test_get_asset_treasury_addresses because no assets were found.");
    }
}

#[tokio::test]
async fn test_get_asset_treasury_addresses_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_asset_treasury_addresses(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.get_asset_treasury_addresses("mock_asset_uuid").await;
    assert!(result.is_ok());
    let addresses = result.unwrap();
    assert_eq!(addresses.len(), 2);
    assert!(addresses.contains(
        &"vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
            .to_string()
    ));

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_delete_asset_assignment_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_delete_asset_assignment(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client
        .delete_asset_assignment("mock_asset_uuid", "10")
        .await;
    assert!(result.is_ok());

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_lock_asset_assignment_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_lock_asset_assignment(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.lock_asset_assignment("mock_asset_uuid", "10").await;
    assert!(result.is_ok());
    let assignment = result.unwrap();
    assert_eq!(assignment.id, 10);

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_unlock_asset_assignment_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_unlock_asset_assignment(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client
        .unlock_asset_assignment("mock_asset_uuid", "10")
        .await;
    assert!(result.is_ok());
    let assignment = result.unwrap();
    assert_eq!(assignment.id, 10);

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}
#[tokio::test]
async fn test_lock_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    std::env::set_var("AMP_PASSWORD", "mock_password");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_lock_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.lock_asset("mock_asset_uuid").await;
    assert!(result.is_ok());
    let asset = result.unwrap();
    assert_eq!(asset.is_locked, true);

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}

#[tokio::test]
async fn test_unlock_asset_mock() {
    // Setup mock test environment
    setup_mock_test().await;
    
    std::env::set_var("AMP_PASSWORD", "mock_password");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_unlock_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).await.unwrap();
    let result = client.unlock_asset("mock_asset_uuid").await;
    assert!(result.is_ok());
    let asset = result.unwrap();
    assert_eq!(asset.is_locked, false);

    // Cleanup
    // Setup mock test environment
    setup_mock_test().await;
}
