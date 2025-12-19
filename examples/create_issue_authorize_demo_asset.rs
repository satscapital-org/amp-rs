//! # Create, Issue, and Authorize Asset Example
//!
//! This example demonstrates the complete workflow for creating a new asset for distribution tests:
//! 1. Derive both confidential and unconfidential addresses from the Elements wallet "amp_elements_wallet_static_for_funding"
//! 2. Issue an asset with maximum circulation to the confidential address
//! 3. Wait for transaction confirmation (3 confirmations, up to 5 minutes)
//! 4. Add the issuance address to the list of treasury addresses
//! 5. Authorize the asset for distribution
//! 6. Display success information
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
//! AMP_TESTS=live cargo run --example create_issue_authorize_asset
//! ```

use amp_rs::model::IssuanceRequest;
use amp_rs::{AmpError, ApiClient, ElementsRpc};
use dotenvy;
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

    println!("🏭 Create, Issue, and Authorize Asset Example");
    println!("=============================================");

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

    // Step 2: Issue asset with maximum amount
    println!("\n2️⃣  Issuing asset with maximum circulation");
    println!("=========================================");

    let client = ApiClient::new()
        .await
        .map_err(|e| AmpError::api(format!("Failed to create AMP API client: {}", e)))?;

    // Create unique asset name with timestamp
    let timestamp = chrono::Utc::now().timestamp();
    let asset_name = format!("SatsCapital_Demo_Asset{}", timestamp);
    let asset_ticker = format!("SCD{}", timestamp % 10000);

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
        transfer_restricted: Some(false), // Allow free transfers for distribution tests
    };

    println!("📋 Asset details:");
    println!("   Name: {}", asset_name);
    println!("   Ticker: {}", asset_ticker);
    println!(
        "   Amount: {} (21 million with 8 decimals)",
        MAX_ASSET_AMOUNT
    );
    println!("   Destination: {}", confidential_address);
    println!("   Transfer Restricted: false");

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

    // Step 6: Display success information
    println!("\n🎉 Asset Creation Complete!");
    println!("===========================");
    println!("✅ All steps completed successfully!");
    println!();
    println!("📋 Asset Information:");
    println!("   Asset UUID: {}", issuance_response.asset_uuid);
    println!("   Asset ID: {}", issuance_response.asset_id);
    println!("   Name: {}", asset_name);
    println!("   Ticker: {}", asset_ticker);
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
    println!("   Is Authorized: {}", authorized_asset.is_authorized);
    println!(
        "   Transfer Restricted: {}",
        authorized_asset.transfer_restricted
    );
    println!();
    println!("🚀 This asset is now ready for:");
    println!("   • Distribution tests");
    println!("   • Assignment creation");
    println!("   • Balance queries");
    println!("   • Transaction operations");
    println!();
    println!("💡 Use the Asset UUID in other examples:");
    println!("   export TEST_ASSET_UUID={}", issuance_response.asset_uuid);

    Ok(())
}
