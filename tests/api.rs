use amp_rs::mocks;
use amp_rs::ApiClient;
use httpmock::prelude::*;
use std::env;
use std::sync::Arc;
use url::Url;
use tokio::sync::OnceCell;

// Shared token manager for live tests to avoid token conflicts
static SHARED_TOKEN_MANAGER: OnceCell<Arc<amp_rs::client::TokenManager>> = OnceCell::const_new();

async fn get_shared_client() -> Result<ApiClient, amp_rs::client::Error> {
    let token_manager = SHARED_TOKEN_MANAGER.get_or_init(|| async {
        Arc::new(amp_rs::client::TokenManager::new().expect("Failed to create token manager"))
    }).await;
    
    ApiClient::with_token_manager(Arc::clone(token_manager))
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_changelog(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let changelog = client.get_changelog().await;

    assert!(changelog.is_ok());
    let changelog_val = changelog.unwrap();
    assert!(changelog_val.as_object().unwrap().len() > 0);
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let assets = client.get_assets().await;

    assert!(assets.is_ok());
    assert!(!assets.unwrap().is_empty());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_get_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
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
    // This test is ignored by default because it performs a state-changing operation
    // and requires a valid destination address.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Set the `DESTINATION_ADDRESS` environment variable to a valid Liquid address.
    // 3. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let destination_address =
        env::var("DESTINATION_ADDRESS").expect("DESTINATION_ADDRESS must be set for this test");

    let client = get_shared_client().await.unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset".to_string(),
        amount: 1000,
        destination_address,
        domain: "example.com".to_string(),
        ticker: "TSTA".to_string(),
        pubkey: "03...".to_string(), // Replace with a valid pubkey
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let result = client.issue_asset(&issuance_request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_issue_asset_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_issue_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset".to_string(),
        amount: 1000,
        destination_address: "destination_address".to_string(),
        domain: "example.com".to_string(),
        ticker: "TSTA".to_string(),
        pubkey: "03...".to_string(), // Replace with a valid pubkey
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
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_edit_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
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
    // 2. Set the `DESTINATION_ADDRESS` environment variable to a valid Liquid address.
    // 3. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let destination_address =
        env::var("DESTINATION_ADDRESS").expect("DESTINATION_ADDRESS must be set for this test");

    let client = get_shared_client().await.unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset to Delete".to_string(),
        amount: 1000,
        destination_address,
        domain: "example.com".to_string(),
        ticker: "TSTD".to_string(),
        pubkey: "03...".to_string(), // Replace with a valid pubkey
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_issue_asset(&server);
    mocks::mock_delete_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Test Asset to Delete".to_string(),
        amount: 1000,
        destination_address: "destination_address".to_string(),
        domain: "example.com".to_string(),
        ticker: "TSTD".to_string(),
        pubkey: "03...".to_string(), // Replace with a valid pubkey
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
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_registered_users(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let registered_users = client.get_registered_users().await;

    assert!(registered_users.is_ok());
    assert!(!registered_users.unwrap().is_empty());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_registered_users(&server);
    mocks::mock_get_registered_user(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_add_registered_user(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
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
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_categories(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let categories = client.get_categories().await;

    assert!(categories.is_ok());
    assert!(!categories.unwrap().is_empty());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_add_category_live() {
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
    println!("Cleaning up: deleting category with ID {}", created_category.id);
    let delete_result = client.delete_category(created_category.id).await;
    if let Err(e) = &delete_result {
        println!("Warning: Failed to delete category: {:?}", e);
    } else {
        println!("Successfully deleted test category");
    }
}

#[tokio::test]
async fn test_add_category_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_add_category(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let new_category = amp_rs::model::CategoryAdd {
        name: "Test Category".to_string(),
        description: Some("Test category description".to_string()),
    };

    let result = client.add_category(&new_category).await;
    assert!(result.is_ok());
    let added_category = result.unwrap();
    assert_eq!(added_category.id, 2);
    assert_eq!(added_category.name, "Test Category");
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_validate_gaid(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.validate_gaid(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_valid);
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.get_gaid_address(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.address.is_empty());
}

#[tokio::test]
async fn test_get_gaid_address_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_gaid_address(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.get_gaid_address(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.address.is_empty());
    assert_eq!(response.address, "mock_address");
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
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
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_managers(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let managers = client.get_managers().await;

    assert!(managers.is_ok());
    assert!(!managers.unwrap().is_empty());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}



#[tokio::test]
async fn test_create_manager_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_create_manager(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let new_manager = amp_rs::model::ManagerCreate {
        username: "test_manager".to_string(),
        password: "password".to_string(),
    };

    let result = client.create_manager(&new_manager).await;
    assert!(result.is_ok());
    let created_manager = result.unwrap();
    assert_eq!(created_manager.id, 2);
    assert_eq!(created_manager.username, "test_manager");
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_broadcast_transaction_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_broadcast_transaction(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.broadcast_transaction("mock_tx_hex").await;
    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.txid, "mock_txid");
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_create_asset_assignment_live() {
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

    // 1. Create an asset
    let destination_address =
        "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
            .to_string();
    let pubkey = "02963a059e1ab729b653b78360626657e40dfb0237b754007acd43e8e0141a1bb4".to_string();

    let issuance_request = amp_rs::model::IssuanceRequest {
        name: "Jules Test Asset".to_string(),
        amount: 1000,
        destination_address,
        domain: "jules.test".to_string(),
        ticker: "JTA".to_string(),
        pubkey,
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true),
    };

    let issued_asset = client.issue_asset(&issuance_request).await.unwrap();
    let asset_uuid = issued_asset.asset_uuid;

    // 2. Create a registered user
    let new_user = amp_rs::model::RegisteredUserAdd {
        name: "Test User for Assignment".to_string(),
        gaid: None,
        is_company: false,
    };
    let user = client.add_registered_user(&new_user).await.unwrap();
    let user_id = user.id;

    // 3. Create the assignment
    let request = amp_rs::model::CreateAssetAssignmentRequest {
        registered_user_id: user_id,
        amount: 100,
        is_locked: false,
        vesting_timestamp: None,
        comment: Some("Test assignment from Jules".to_string()),
    };

    let result = client.create_asset_assignment(&asset_uuid, &request).await;
    println!("{:?}", result);
    if let Err(e) = &result {
        println!("Error: {:?}", e);
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_asset_assignment_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_assets(&server);
    mocks::mock_get_registered_users(&server);
    mocks::mock_create_asset_assignment(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let assets = client.get_assets().await.unwrap();
    let asset_uuid = assets.first().unwrap().asset_uuid.clone();
    let users = client.get_registered_users().await.unwrap();
    let user_id = users.first().unwrap().id;

    let request = amp_rs::model::CreateAssetAssignmentRequest {
        registered_user_id: user_id,
        amount: 100,
        is_locked: false,
        vesting_timestamp: None,
        comment: Some("Test assignment from Jules".to_string()),
    };

    let result = client.create_asset_assignment(&asset_uuid, &request).await;
    assert!(result.is_ok());
    let assignment = result.unwrap();
    assert_eq!(assignment.id, 10);
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_get_broadcast_status_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_broadcast_status(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.get_broadcast_status("mock_txid").await;
    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.txid, "mock_txid");
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}
#[tokio::test]
async fn test_get_manager_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_manager(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.get_manager(1).await;
    assert!(result.is_ok());
    let manager = result.unwrap();
    assert_eq!(manager.id, 1);
    assert_eq!(manager.username, "mock_manager");
    assert_eq!(manager.assets.len(), 2);
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_manager_remove_asset_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_manager_remove_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.manager_remove_asset(1, "asset_uuid_1").await;
    assert!(result.is_ok());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_revoke_manager_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_manager(&server);
    mocks::mock_manager_remove_asset(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.revoke_manager(1).await;
    assert!(result.is_ok());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_get_current_manager_raw_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_get_current_manager_raw(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.get_current_manager_raw().await;
    assert!(result.is_ok());
    let manager_json = result.unwrap();
    assert_eq!(manager_json["id"], 1);
    assert_eq!(manager_json["username"], "current_manager");
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}

#[tokio::test]
async fn test_unlock_manager_mock() {
    dotenvy::from_filename_override(".env").ok();
    std::env::set_var("AMP_USERNAME", "mock_user");
    std::env::set_var("AMP_PASSWORD", "mock_pass");
    let server = MockServer::start();
    mocks::mock_obtain_token(&server);
    mocks::mock_unlock_manager(&server);

    let client = ApiClient::with_base_url(Url::parse(&server.base_url()).unwrap()).unwrap();
    let result = client.unlock_manager(1).await;
    assert!(result.is_ok());
    
    // Cleanup: reload .env file
    dotenvy::from_filename_override(".env").ok();
}