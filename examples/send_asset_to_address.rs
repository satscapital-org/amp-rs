//! Send Asset to Address Example
//!
//! This example sends all of a specific asset from a source address to a destination address.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example send_asset_to_address
//! ```
//!
//! ## Environment Variables
//!
//! This example uses dotenvy to load environment variables from .env:
//! - `AMP_USERNAME`: AMP API username
//! - `AMP_PASSWORD`: AMP API password
//! - `ELEMENTS_RPC_URL`: Elements node RPC URL
//! - `ELEMENTS_RPC_USER`: Elements node RPC username
//! - `ELEMENTS_RPC_PASSWORD`: Elements node RPC password

use amp_rs::{ApiClient, ElementsRpc};
use dotenvy;
use std::collections::HashMap;
use std::env;

/// Asset UUID to transfer
const ASSET_UUID: &str = "bc2d31af-60d0-4346-bfba-11b045f92dff";

/// Source address (unconfidential address where funds are located)
const SOURCE_ADDRESS: &str = "8svsRZNydZFhZYLorxoSQ3UtR5X96SdBsf";

/// Destination address (confidential address to send to)
const DESTINATION_ADDRESS: &str = "vjTups9DKkNDyv6jxWy3ZYVXMvmjAdZ3NAyyhX18mMF8EmH66a64sbkTczYyxoq1cV5RTTwwUWCmfExj";

/// Wallet name containing the source address
const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("💸 Send Asset to Address Example");
    println!("==================================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();

    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!(
        "✅ Connected to AMP API with {} strategy",
        client.get_strategy_type()
    );

    // Create Elements RPC client
    println!("⚡ Creating Elements RPC client");
    let elements_rpc = ElementsRpc::from_env()?;
    println!("✅ Elements RPC client created");

    // Verify Elements node connectivity
    println!("🔍 Verifying Elements node connectivity");
    match elements_rpc.get_network_info().await {
        Ok(_network_info) => {
            println!("✅ Elements node connected successfully");
        }
        Err(e) => {
            println!("❌ Elements node connection failed: {}", e);
            return Err(format!("Elements node not available: {}", e).into());
        }
    }

    println!("\n📋 Transaction Details:");
    println!("  Asset UUID: {}", ASSET_UUID);
    println!("  Source Address: {}", SOURCE_ADDRESS);
    println!("  Destination Address: {}", DESTINATION_ADDRESS);
    println!("  Wallet: {}", WALLET_NAME);

    // Step 1: Get asset details to retrieve the asset_id
    println!("\n🪙 Step 1: Getting asset details from AMP");
    let asset = client.get_asset(ASSET_UUID).await?;
    println!("✅ Asset found:");
    println!("   Name: {}", asset.name);
    println!("   Asset ID: {}", asset.asset_id);
    println!(
        "   Ticker: {}",
        asset.ticker.as_deref().unwrap_or("N/A")
    );
    println!(
        "   Precision: {} decimal places",
        asset.precision
    );

    let asset_id = &asset.asset_id;

    // Step 2: List unspent outputs from the wallet for this asset
    println!("\n💰 Step 2: Listing unspent outputs for asset in wallet");
    let all_utxos = elements_rpc
        .list_unspent_for_wallet(WALLET_NAME, Some(asset_id))
        .await?;

    println!("✅ Found {} total UTXOs for this asset in wallet", all_utxos.len());

    // Filter UTXOs by source address
    let source_utxos: Vec<_> = all_utxos
        .iter()
        .filter(|utxo| utxo.address == SOURCE_ADDRESS)
        .collect();

    if source_utxos.is_empty() {
        println!("❌ No UTXOs found for address {} with asset {}", SOURCE_ADDRESS, asset_id);
        println!("   Available UTXOs are at the following addresses:");
        for utxo in &all_utxos {
            println!("     - {} (amount: {}, spendable: {})", utxo.address, utxo.amount, utxo.spendable);
        }
        return Err("No funds at source address".into());
    }

    println!("✅ Found {} UTXOs at source address:", source_utxos.len());
    let mut total_amount = 0.0;
    for (i, utxo) in source_utxos.iter().enumerate() {
        println!("   {}. TXID: {}:{}", i + 1, utxo.txid, utxo.vout);
        println!("      Amount: {} ({})", utxo.amount, if utxo.spendable { "spendable" } else { "not spendable" });
        println!("      Confirmations: {:?}", utxo.confirmations);
        total_amount += utxo.amount;
    }

    println!("\n💵 Total amount to send: {}", total_amount);

    if total_amount == 0.0 {
        println!("❌ No funds available to send");
        return Err("No funds available".into());
    }

    // Step 3: Prepare sendmany transaction
    println!("\n📤 Step 3: Preparing transaction with sendmany");

    // Create address amounts map (destination address -> amount)
    let mut address_amounts = HashMap::new();
    address_amounts.insert(DESTINATION_ADDRESS.to_string(), total_amount);

    // Create asset amounts map (destination address -> asset_id)
    let mut asset_amounts = HashMap::new();
    asset_amounts.insert(DESTINATION_ADDRESS.to_string(), asset_id.clone());

    println!("✅ Transaction prepared:");
    println!("   Sending: {} of asset {}", total_amount, asset_id);
    println!("   To: {}", DESTINATION_ADDRESS);

    // Step 4: Send the transaction
    println!("\n🚀 Step 4: Broadcasting transaction");
    let txid = elements_rpc
        .sendmany(
            WALLET_NAME,
            address_amounts,
            asset_amounts,
            Some(1),                        // min_conf: 1 to ensure we use confirmed UTXOs
            Some("Transfer all asset"),     // comment
            None,                           // subtract_fee_from: pay fee separately
            Some(false),                    // replaceable: false for final transaction
            Some(1),                        // conf_target: 1 block
            Some("UNSET"),                  // estimate_mode
        )
        .await?;

    println!("✅ Transaction broadcast successfully!");
    println!("   TXID: {}", txid);

    // Step 5: Wait for confirmations (optional)
    println!("\n⏳ Step 5: Waiting for confirmations (2 confirmations, 10-minute timeout)");
    match elements_rpc.wait_for_confirmations(&txid, Some(2), Some(10)).await {
        Ok(tx_detail) => {
            println!("✅ Transaction confirmed!");
            println!("   Confirmations: {}", tx_detail.confirmations);
            println!("   Block height: {:?}", tx_detail.blockheight);
            println!("   Block hash: {:?}", tx_detail.blockhash);
        }
        Err(e) => {
            println!("⚠️  Confirmation timeout: {}", e);
            println!("   Transaction may still be pending");
            println!("   You can check the status manually with TXID: {}", txid);
        }
    }

    // Summary
    println!("\n📊 Summary");
    println!("==========");
    println!("Asset UUID: {}", ASSET_UUID);
    println!("Asset ID: {}", asset_id);
    println!("Asset Name: {}", asset.name);
    println!("Source Address: {}", SOURCE_ADDRESS);
    println!("Destination Address: {}", DESTINATION_ADDRESS);
    println!("Amount Sent: {}", total_amount);
    println!("Transaction ID: {}", txid);
    println!("Wallet Used: {}", WALLET_NAME);

    println!("\n🎉 Asset transfer completed successfully!");

    Ok(())
}
