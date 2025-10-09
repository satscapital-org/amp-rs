use amp_rs::model::{
    CategoryAdd, CreateAssetAssignmentRequest, IssuanceRequest, RegisteredUserAdd,
};
use amp_rs::ApiClient;
use std::env;
use std::process::Command;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::from_filename_override(".env").ok();

    // Check if we're running in live mode
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("This example requires AMP_TESTS=live to be set");
        println!("Set AMP_USERNAME, AMP_PASSWORD, and AMP_TESTS=live environment variables");
        return Ok(());
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        eprintln!("AMP_USERNAME and AMP_PASSWORD must be set for this example");
        return Ok(());
    }

    let client = ApiClient::new().await?;

    println!("🏗️  Test Environment Setup");
    println!("==========================\n");

    // Target GAID for the test
    let target_gaid = "GAQzmXM7jVaKAwtHGXHENgn5KUUmL";
    let test_gaid = "GAbzSbgCZ6M6WU85rseKTrfehPsjt";

    println!("🎯 Setting up test environment for GAID: {}", test_gaid);
    println!("📍 Target address GAID: {}", target_gaid);

    // Step 1: Create or find a test category
    println!("\n1️⃣  Setting up test category...");
    let category_name = "Test Environment Category";

    // Check if category already exists
    let categories = client.get_categories().await?;
    let existing_category = categories.iter().find(|cat| cat.name == category_name);

    let category_id = if let Some(category) = existing_category {
        println!(
            "   ✅ Found existing category: {} (ID: {})",
            category.name, category.id
        );
        category.id
    } else {
        let new_category = CategoryAdd {
            name: category_name.to_string(),
            description: Some("Category for test environment setup".to_string()),
        };

        let created_category = client.add_category(&new_category).await?;
        println!(
            "   ✅ Created new category: {} (ID: {})",
            created_category.name, created_category.id
        );
        created_category.id
    };

    // Step 2: Create or find a test user with the target GAID
    println!("\n2️⃣  Setting up test user with GAID {}...", test_gaid);

    // Check if user with this GAID already exists
    let users = client.get_registered_users().await?;
    let existing_user = users
        .iter()
        .find(|user| user.gaid.as_ref() == Some(&test_gaid.to_string()));

    let user_id = if let Some(user) = existing_user {
        println!(
            "   ✅ Found existing user: {} (ID: {}) with GAID: {}",
            user.name,
            user.id,
            user.gaid.as_ref().unwrap_or(&"None".to_string())
        );
        user.id
    } else {
        // Try to create the user, but handle the case where it already exists
        let new_user = RegisteredUserAdd {
            name: "Test Environment User".to_string(),
            gaid: Some(test_gaid.to_string()),
            is_company: false,
        };

        match client.add_registered_user(&new_user).await {
            Ok(created_user) => {
                println!(
                    "   ✅ Created new user: {} (ID: {}) with GAID: {}",
                    created_user.name,
                    created_user.id,
                    created_user.gaid.as_ref().unwrap_or(&"None".to_string())
                );
                created_user.id
            }
            Err(e) if e.to_string().contains("GAID was already created") => {
                println!(
                    "   ⚠️  User with GAID {} already exists, searching again...",
                    test_gaid
                );
                // Refresh the user list and search again
                let users = client.get_registered_users().await?;

                if let Some(user) = users
                    .iter()
                    .find(|user| user.gaid.as_ref() == Some(&test_gaid.to_string()))
                {
                    println!(
                        "   ✅ Found existing user: {} (ID: {}) with GAID: {}",
                        user.name,
                        user.id,
                        user.gaid.as_ref().unwrap_or(&"None".to_string())
                    );
                    user.id
                } else {
                    return Err(format!("User with GAID {} exists but cannot be found in user list. This GAID may have been used before and cannot be reused.", test_gaid).into());
                }
            }
            Err(e) => return Err(e.into()),
        }
    };

    // Step 3: Assign user to category (optional - may not be required for GAID balance tests)
    println!("\n3️⃣  Assigning user to category...");
    match client
        .add_categories_to_registered_user(user_id, &[category_id])
        .await
    {
        Ok(_) => println!("   ✅ User assigned to category successfully"),
        Err(e) => {
            println!("   ⚠️  User assignment to category failed: {}", e);
            println!("   💡 This may not be required for the GAID balance tests to work");
        }
    }

    // Step 4: Create a test asset
    println!("\n4️⃣  Creating test asset...");

    // Use the provided destination address directly
    let destination_address =
        "vjTwqhz69nh7xHhtsHnx7mezsJV95EYHPqxshuoVXEMS5sqVzok57YVWYKDLcanqdSq54oTNhNM1NuTB"
            .to_string();

    println!("   📍 Using destination address: {}", destination_address);

    let asset_name = "Test Environment Asset";
    let asset_ticker = "TENV";

    // Check if asset with this name already exists
    let assets = client.get_assets().await?;
    let existing_asset = assets.iter().find(|asset| asset.name == asset_name);

    let asset_uuid = if let Some(asset) = existing_asset {
        println!(
            "   ✅ Found existing asset: {} (UUID: {})",
            asset.name, asset.asset_uuid
        );
        asset.asset_uuid.clone()
    } else {
        let issuance_request = IssuanceRequest {
            name: asset_name.to_string(),
            amount: 100000, // 0.001 BTC in satoshis
            destination_address,
            domain: "test.example.com".to_string(),
            ticker: asset_ticker.to_string(),
            pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                .to_string(),
            precision: Some(8),
            is_confidential: Some(true),
            is_reissuable: Some(false),
            reissuance_amount: None,
            reissuance_address: None,
            transfer_restricted: Some(true),
        };

        let issuance_response = client.issue_asset(&issuance_request).await?;
        println!(
            "   ✅ Created new asset: {} (UUID: {})",
            issuance_response.name, issuance_response.asset_uuid
        );
        println!("   📋 Asset ID: {}", issuance_response.asset_id);
        println!("   🔗 Transaction ID: {}", issuance_response.txid);
        issuance_response.asset_uuid
    };

    // Step 5: Assign asset to category
    println!("\n5️⃣  Assigning asset to category...");
    match client.add_asset_to_category(category_id, &asset_uuid).await {
        Ok(_) => println!("   ✅ Asset assigned to category successfully"),
        Err(e) => println!(
            "   ⚠️  Asset assignment to category failed (may already be assigned): {}",
            e
        ),
    }

    // Step 6: Asset is already issued to the destination address
    println!("\n6️⃣  Asset issuance complete...");
    println!("   ✅ Asset issued directly to destination address with 100000 satoshis (0.001 BTC)");

    // Step 7: Summary
    println!("\n🎉 Test Environment Setup Complete!");
    println!("=====================================");
    println!("📋 Summary:");
    println!("   • Category: {} (ID: {})", category_name, category_id);
    println!(
        "   • User: Test Environment User (ID: {}) with GAID: {}",
        user_id, test_gaid
    );
    println!("   • Asset: {} (UUID: {})", asset_name, asset_uuid);
    println!("   • Issuance: 100000 satoshis issued directly to destination address");
    println!("\n✅ The test environment is now ready for:");
    println!("   • test_get_gaid_balance_live");
    println!("   • test_get_gaid_asset_balance_live");
    println!(
        "\n💡 Run tests with: AMP_TESTS=live cargo test test_get_gaid_balance_live -- --ignored"
    );

    Ok(())
}
