use amp_rs::ApiClient;
use std::env;

#[tokio::test]
async fn test_get_changelog() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let changelog = client.get_changelog().await;

    assert!(changelog.is_ok());
    let changelog_val = changelog.unwrap();
    assert!(changelog_val.as_object().unwrap().len() > 0);
}

#[tokio::test]
async fn test_get_assets() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let assets = client.get_assets().await;

    assert!(assets.is_ok());
}

#[tokio::test]
async fn test_get_asset() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_test) = assets.first() {
        let asset = client.get_asset(&asset_to_test.asset_uuid).await;
        assert!(asset.is_ok());
    } else {
        println!("Skipping test_get_asset because no assets were found.");
    }
}

#[tokio::test]
#[ignore]
async fn test_issue_asset() {
    // This test is ignored by default because it performs a state-changing operation
    // and requires a valid destination address.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Set the `DESTINATION_ADDRESS` environment variable to a valid Liquid address.
    // 3. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let destination_address = env::var("DESTINATION_ADDRESS")
        .expect("DESTINATION_ADDRESS must be set for this test");

    let client = ApiClient::new().unwrap();
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
#[ignore]
async fn test_edit_asset() {
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Make sure there is at least one asset available.
    // 3. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let assets = client.get_assets().await.unwrap();

    if let Some(asset_to_edit) = assets.first() {
        let edit_request = amp_rs::model::EditAssetRequest {
            issuer_authorization_endpoint: "https://example.com/authorize".to_string(),
        };
        let result = client.edit_asset(&asset_to_edit.asset_uuid, &edit_request).await;
        assert!(result.is_ok());
    } else {
        println!("Skipping test_edit_asset because no assets were found.");
    }
}

#[tokio::test]
#[ignore]
async fn test_delete_asset() {
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Set the `DESTINATION_ADDRESS` environment variable to a valid Liquid address.
    // 3. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let destination_address = env::var("DESTINATION_ADDRESS")
        .expect("DESTINATION_ADDRESS must be set for this test");

    let client = ApiClient::new().unwrap();
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
async fn test_get_registered_users() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let registered_users = client.get_registered_users().await;

    assert!(registered_users.is_ok());
}

#[tokio::test]
async fn test_get_registered_user() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let registered_users = client.get_registered_users().await.unwrap();

    if let Some(user_to_test) = registered_users.first() {
        let user = client.get_registered_user(user_to_test.id).await;
        assert!(user.is_ok());
    } else {
        println!("Skipping test_get_registered_user because no registered users were found.");
    }
}

#[tokio::test]
#[ignore]
async fn test_add_registered_user() {
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let new_user = amp_rs::model::RegisteredUserAdd {
        name: "Test User".to_string(),
        gaid: None,
        is_company: false,
    };

    let result = client.add_registered_user(&new_user).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_categories() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let categories = client.get_categories().await;

    assert!(categories.is_ok());
}

#[tokio::test]
#[ignore]
async fn test_add_category() {
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let new_category = amp_rs::model::CategoryAdd {
        name: "Test Category".to_string(),
        description: Some("Test category description".to_string()),
    };

    let result = client.add_category(&new_category).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_gaid() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.validate_gaid(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_valid);
}

#[tokio::test]
async fn test_get_gaid_address() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let gaid = "GAbYScu6jkWUND2jo3L4KJxyvo55d";
    let result = client.get_gaid_address(gaid).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(!response.address.is_empty());
}

#[tokio::test]
async fn test_get_managers() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let managers = client.get_managers().await;

    assert!(managers.is_ok());
}

#[tokio::test]
#[ignore]
async fn test_create_manager() {
    // This test is ignored by default because it performs a state-changing operation.
    // To run this test:
    // 1. Set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    // 2. Run `cargo test -- --ignored`.

    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let new_manager = amp_rs::model::ManagerCreate {
        username: "test_manager".to_string(),
        password: "password".to_string(),
    };

    let result = client.create_manager(&new_manager).await;
    assert!(result.is_ok());
}
