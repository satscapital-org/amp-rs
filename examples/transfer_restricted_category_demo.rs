//! # Create Transfer-Restricted Asset with Category Management
//!
//! This example demonstrates the complete workflow for creating a transfer-restricted asset
//! with proper Elements wallet integration and category-based access control:
//!
//! 1. Derive confidential and unconfidential addresses from Elements wallet
//! 2. Issue a transfer-restricted asset with maximum circulation
//! 3. Wait for transaction confirmation (3 confirmations, up to 5 minutes)
//! 4. Add the issuance address to treasury addresses
//! 5. Authorize the asset for distribution
//! 6. Create a category with timestamp-based nonce
//! 7. Add the asset to the category
//! 8. Add a specified user to the category
//! 9. Display comprehensive information about the asset, category, and memberships
//!
//! ## Prerequisites
//!
//! - Running Elements node with RPC access
//! - Environment variables set for Elements RPC connection and AMP API
//! - AMP_TESTS=live for live API testing
//!
//! ## Usage
//!
//! ```bash
//! AMP_TESTS=live cargo run --example transfer_restricted_category_demo [USER_ID]
//! ```
//!
//! Examples:
//!   AMP_TESTS=live cargo run --example transfer_restricted_category_demo          # uses default user ID 2137
//!   AMP_TESTS=live cargo run --example transfer_restricted_category_demo 42       # uses user ID 42

use amp_rs::model::{CategoryAdd, IssuanceRequest};
use amp_rs::{AmpError, ApiClient, ElementsRpc};
use chrono::Utc;
use std::env;
use tokio::time::{sleep, Duration, Instant};

const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";
const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
const CONFIRMATION_CHECK_INTERVAL: Duration = Duration::from_secs(30); // 30 seconds
const MAX_ASSET_AMOUNT: i64 = 21_000_000_00000000; // 21 million with 8 decimal precision

#[tokio::main]
async fn main() -> Result<(), AmpError> {
    // Load environment variables from .env file first
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse optional CLI arg for user ID, default to 2137
    let user_id: i64 = env::args()
        .nth(1)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(2137);

    println!("🏭 Create Transfer-Restricted Asset with Category Management");
    println!("============================================================");

    // Check if we're running in live mode
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        return Err(AmpError::validation(
            "This example requires AMP_TESTS=live to be set. Set AMP_USERNAME, AMP_PASSWORD, and AMP_TESTS=live environment variables"
        ));
    }

    // Step 1: Derive confidential and unconfidential addresses from Elements wallet
    println!("\n1️⃣  Deriving confidential and unconfidential addresses from Elements wallet");
    println!("===========================================================================");

    let elements_rpc = ElementsRpc::from_env()
        .map_err(|e| AmpError::rpc(format!("Failed to create Elements RPC client: {}", e)))?;

    // Test Elements connectivity
    elements_rpc
        .get_network_info()
        .await
        .map_err(|e| AmpError::rpc(format!("Failed to connect to Elements node: {}", e)))?;

    println!("✅ Connected to Elements node");

    // Derive a new unconfidential address from the wallet
    let unconfidential_address = elements_rpc
        .get_new_address(WALLET_NAME, Some("bech32"))
        .await
        .map_err(|e| {
            AmpError::rpc(format!(
                "Failed to derive unconfidential address from wallet '{}': {}. Ensure the wallet exists and is loaded.",
                WALLET_NAME, e
            ))
        })?;

    // Derive the corresponding confidential address
    let confidential_address = elements_rpc
        .get_confidential_address(WALLET_NAME, &unconfidential_address)
        .await
        .map_err(|e| AmpError::rpc(format!("Failed to derive confidential address: {}", e)))?;

    println!("✅ Successfully derived address pair:");
    println!("   Unconfidential: {}", unconfidential_address);
    println!("   Confidential:   {}", confidential_address);

    // Step 2: Issue transfer-restricted asset with maximum amount
    println!("\n2️⃣  Issuing transfer-restricted asset with maximum circulation");
    println!("=============================================================");

    let client = ApiClient::new()
        .await
        .map_err(|e| AmpError::api(format!("Failed to create AMP API client: {}", e)))?;

    // Create unique asset name with timestamp
    let timestamp = Utc::now().timestamp();
    let asset_name = format!("TransferRestrictedDemo_{}", timestamp);
    let asset_ticker = format!("TRD{}", timestamp % 100000);

    let issuance_request = IssuanceRequest {
        name: asset_name.clone(),
        amount: MAX_ASSET_AMOUNT,
        destination_address: confidential_address.clone(),
        domain: "liquidtestnet.com".to_string(),
        ticker: asset_ticker.clone(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(true), // KEY: Enable transfer restrictions
    };

    println!("📋 Asset details:");
    println!("   Name: {}", asset_name);
    println!("   Ticker: {}", asset_ticker);
    println!(
        "   Amount: {} (21 million with 8 decimals)",
        MAX_ASSET_AMOUNT
    );
    println!("   Destination: {}", confidential_address);
    println!("   Transfer Restricted: true");

    let issuance_response = client
        .issue_asset(&issuance_request)
        .await
        .map_err(|e| AmpError::api(format!("Asset issuance was refused by AMP API: {}", e)))?;

    println!("✅ Asset issuance request accepted!");
    println!("   Asset UUID: {}", issuance_response.asset_uuid);
    println!("   Asset ID: {}", issuance_response.asset_id);
    println!("   Transaction ID: {}", issuance_response.txid);
    println!("   Asset VOut: {}", issuance_response.asset_vout);

    // Step 3: Wait for transaction confirmation (3 confirmations)
    println!("\n3️⃣  Waiting for transaction confirmation (3 confirmations)");
    println!("=========================================================");

    let txid = &issuance_response.txid;
    let start_time = Instant::now();

    println!(
        "🕐 Checking transaction {} every {} seconds...",
        txid,
        CONFIRMATION_CHECK_INTERVAL.as_secs()
    );
    println!(
        "⏰ Timeout after {} seconds",
        CONFIRMATION_TIMEOUT.as_secs()
    );

    loop {
        if start_time.elapsed() > CONFIRMATION_TIMEOUT {
            return Err(AmpError::timeout(format!(
                "Issuance transaction {} could not reach 3 confirmations within 5 minutes. \
                The transaction may still be pending in the mempool or the Elements node may be slow.",
                txid
            )));
        }

        match elements_rpc
            .get_transaction_from_wallet(WALLET_NAME, txid)
            .await
        {
            Ok(tx_detail) => {
                println!(
                    "📊 Transaction found with {} confirmations",
                    tx_detail.confirmations
                );

                if tx_detail.confirmations >= 3 {
                    println!("✅ Issuance transaction confirmed with sufficient confirmations!");
                    println!("   Confirmations: {}", tx_detail.confirmations);
                    if let Some(blockheight) = tx_detail.blockheight {
                        println!("   Block height: {}", blockheight);
                    }
                    break;
                } else {
                    println!(
                        "⏳ Transaction found but needs more confirmations ({}/3 confirmations)",
                        tx_detail.confirmations
                    );
                }
            }
            Err(e) => {
                println!("🔍 Transaction not yet visible in wallet: {}", e);
                println!("   This is normal for new transactions, continuing to wait...");
            }
        }

        println!(
            "   Waiting {} seconds before next check...",
            CONFIRMATION_CHECK_INTERVAL.as_secs()
        );
        sleep(CONFIRMATION_CHECK_INTERVAL).await;
    }

    // Step 4: Add issuance address to treasury addresses
    println!("\n4️⃣  Adding issuance address to treasury addresses");
    println!("================================================");

    let treasury_addresses = vec![confidential_address.clone()];

    client
        .add_asset_treasury_addresses(&issuance_response.asset_uuid, &treasury_addresses)
        .await
        .map_err(|e| {
            AmpError::api(format!(
                "Failed to add issuance address to treasury addresses: {}",
                e
            ))
        })?;

    println!("✅ Successfully added treasury address!");
    println!("   Address: {}", confidential_address);

    // Verify treasury addresses were added
    let current_treasury_addresses = client
        .get_asset_treasury_addresses(&issuance_response.asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to verify treasury addresses: {}", e)))?;

    println!(
        "📋 Current treasury addresses: {:?}",
        current_treasury_addresses
    );

    // Step 5: Authorize asset for distribution
    println!("\n5️⃣  Authorizing asset for distribution");
    println!("=====================================");

    let authorized_asset = client
        .register_asset_authorized(&issuance_response.asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to authorize asset for distribution: {}", e)))?;

    println!("✅ Asset successfully authorized for distribution!");
    println!("   Is Authorized: {}", authorized_asset.is_authorized);
    println!("   Is Registered: {}", authorized_asset.is_registered);

    // Step 6: Create a category with timestamp-based nonce
    println!("\n6️⃣  Creating category with timestamp nonce");
    println!("==========================================");

    let category_name = format!("Transfer Restriction Test Category {}", timestamp);
    let category_description = Some(format!(
        "Category created at {} for managing transfer-restricted asset {}",
        Utc::now().to_rfc3339(),
        asset_name
    ));

    let new_category = CategoryAdd {
        name: category_name.clone(),
        description: category_description.clone(),
    };

    let category = client
        .add_category(&new_category)
        .await
        .map_err(|e| AmpError::api(format!("Failed to create category: {}", e)))?;

    println!("✅ Category created successfully!");
    println!("   Category ID:  {}", category.id);
    println!("   Name:         {}", category.name);
    if let Some(desc) = &category.description {
        println!("   Description:  {}", desc);
    }

    // Step 7: Add the transfer-restricted asset to the category
    println!("\n7️⃣  Adding asset to category");
    println!("============================");

    let updated_category = client
        .add_asset_to_category(category.id, &issuance_response.asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to add asset to category: {}", e)))?;

    println!("✅ Asset added to category!");
    println!(
        "   Category '{}' now has {} asset(s)",
        updated_category.name,
        updated_category.assets.len()
    );

    // Step 8: Add the specified user to the category
    println!("\n8️⃣  Adding user {} to category", user_id);
    println!("==============================");

    let updated_category = client
        .add_registered_user_to_category(category.id, user_id)
        .await
        .map_err(|e| AmpError::api(format!("Failed to add user to category: {}", e)))?;

    println!("✅ User added to category!");
    println!(
        "   Category '{}' now has {} user(s)",
        updated_category.name,
        updated_category.registered_users.len()
    );

    // Step 9: Display comprehensive information
    println!("\n🎉 Setup Complete! Displaying Summary");
    println!("====================================\n");

    // Fetch fresh asset details
    let asset = client
        .get_asset(&issuance_response.asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to fetch asset details: {}", e)))?;

    println!("📋 ASSET INFORMATION");
    println!("   Asset UUID: {}", asset.asset_uuid);
    println!("   Asset ID: {}", asset.asset_id);
    println!("   Name: {}", asset.name);
    println!("   Ticker: {}", asset.ticker.as_deref().unwrap_or("N/A"));
    println!("   Domain: {}", asset.domain.as_deref().unwrap_or("N/A"));
    println!("   Precision: {}", asset.precision);
    println!("   Amount Issued: {} satoshis", MAX_ASSET_AMOUNT);
    println!(
        "   Treasury Address (Confidential): {}",
        confidential_address
    );
    println!(
        "   Treasury Address (Unconfidential): {}",
        unconfidential_address
    );
    println!("   Transaction ID: {}", issuance_response.txid);
    println!("   Transfer Restricted: {}", asset.transfer_restricted);
    println!("   Is Registered: {}", asset.is_registered);
    println!("   Is Authorized: {}", asset.is_authorized);
    println!("   Is Locked: {}", asset.is_locked);
    if let Some(reissuance_token) = &asset.reissuance_token_id {
        println!("   Reissuance Token ID: {}", reissuance_token);
    }
    println!();

    // Fetch fresh category details
    let category_details = client
        .get_category(category.id)
        .await
        .map_err(|e| AmpError::api(format!("Failed to fetch category details: {}", e)))?;

    println!("📁 CATEGORY INFORMATION");
    println!("   Category ID: {}", category_details.id);
    println!("   Name: {}", category_details.name);
    if let Some(desc) = &category_details.description {
        println!("   Description: {}", desc);
    }
    println!("   Total Assets: {}", category_details.assets.len());
    println!(
        "   Total Users: {}",
        category_details.registered_users.len()
    );
    println!();

    println!("🔗 CATEGORY MEMBERSHIPS");
    println!();
    println!("   Assets in category:");
    for (idx, asset_uuid) in category_details.assets.iter().enumerate() {
        match client.get_asset(asset_uuid).await {
            Ok(a) => {
                println!(
                    "     {}. {} ({}) - Transfer Restricted: {}",
                    idx + 1,
                    a.name,
                    a.ticker.as_deref().unwrap_or("N/A"),
                    a.transfer_restricted
                );
            }
            Err(e) => {
                println!("     {}. {} (error: {:?})", idx + 1, asset_uuid, e);
            }
        }
    }
    println!();

    println!("   Users in category:");
    for (idx, uid) in category_details.registered_users.iter().enumerate() {
        match client.get_registered_user(*uid).await {
            Ok(u) => {
                println!("     {}. User ID {}: {}", idx + 1, u.id, u.name);
                if let Some(gaid) = &u.gaid {
                    println!("        GAID: {}", gaid);
                }
            }
            Err(e) => {
                println!("     {}. User ID {} (error: {:?})", idx + 1, uid, e);
            }
        }
    }
    println!();

    println!("🚀 This asset is now ready for:");
    println!("   • Distribution tests (transfer-restricted)");
    println!("   • Assignment creation for category members");
    println!("   • Balance queries");
    println!("   • Transaction operations");
    println!();
    println!("💡 Use these values in other examples:");
    println!("   export TEST_ASSET_UUID={}", issuance_response.asset_uuid);
    println!("   export TEST_CATEGORY_ID={}", category.id);
    println!("   export TEST_USER_ID={}", user_id);

    Ok(())
}
