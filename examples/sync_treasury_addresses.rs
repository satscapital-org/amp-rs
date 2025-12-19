//! Sync Treasury Addresses Example
//!
//! This example checks the wallet and the AMP service to determine if there are
//! outputs that haven't been added to the treasury addresses for the asset.
//! It automatically adds any missing addresses to the treasury address list.
//!
//! ## Usage
//!
//! ```bash
//! # Use default asset UUID
//! cargo run --example sync_treasury_addresses
//!
//! # Specify a different asset UUID
//! cargo run --example sync_treasury_addresses <asset-uuid>
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
use std::collections::HashSet;
use std::env;

/// Default asset UUID to check
const DEFAULT_ASSET_UUID: &str = "df6eaf0c-89c1-46b6-b688-84f2c1d3c4b";

/// Wallet name to check for outputs
const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🔄 Treasury Address Sync Example");
    println!("==================================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();

    // Get asset UUID from command line or use default
    let args: Vec<String> = env::args().collect();
    let asset_uuid = if args.len() > 1 {
        println!("📌 Using asset UUID from command line: {}", args[1]);
        &args[1]
    } else {
        println!("📌 Using default asset UUID: {}", DEFAULT_ASSET_UUID);
        DEFAULT_ASSET_UUID
    };

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

    println!("\n🎯 Processing asset: {}", asset_uuid);

    // Step 1: Get asset details to retrieve the asset_id
    println!("\n📋 Step 1: Getting asset details from AMP");
    let asset = client.get_asset(asset_uuid).await?;
    println!("✅ Asset found:");
    println!("   Name: {}", asset.name);
    println!("   Asset ID: {}", asset.asset_id);
    println!(
        "   Ticker: {}",
        asset.ticker.as_deref().unwrap_or("N/A")
    );
    println!("   Domain: {}", asset.domain.as_deref().unwrap_or("N/A"));

    let asset_id = &asset.asset_id;

    // Step 2: Get unspent outputs from the wallet for this asset
    println!("\n📋 Step 2: Listing unspent outputs from wallet '{}'", WALLET_NAME);
    let utxos = elements_rpc
        .list_unspent_for_wallet(WALLET_NAME, Some(asset_id))
        .await?;

    println!("✅ Found {} UTXOs for asset in wallet", utxos.len());

    if utxos.is_empty() {
        println!("ℹ️  No UTXOs found in wallet for this asset");
        println!("   This is normal if the asset hasn't been issued yet or all outputs have been spent.");
        return Ok(());
    }

    // Step 3: Extract unique addresses from UTXOs and convert to confidential addresses
    println!("\n📋 Step 3: Extracting addresses from UTXOs");
    let utxo_addresses: HashSet<String> = utxos.iter().map(|utxo| utxo.address.clone()).collect();

    println!("✅ Found {} unique addresses in wallet", utxo_addresses.len());
    println!("🔐 Converting to confidential addresses...");
    
    let mut wallet_addresses: HashSet<String> = HashSet::new();
    for address in utxo_addresses.iter() {
        match elements_rpc.get_confidential_address(WALLET_NAME, address).await {
            Ok(confidential_address) => {
                println!("   ✓ {} -> {}", address, confidential_address);
                wallet_addresses.insert(confidential_address);
            }
            Err(e) => {
                println!("   ⚠️  Failed to get confidential address for {}: {}", address, e);
                println!("      Using original address instead");
                wallet_addresses.insert(address.clone());
            }
        }
    }
    
    println!("✅ Resolved {} confidential addresses", wallet_addresses.len());

    // Step 4: Get current treasury addresses from AMP
    println!("\n📋 Step 4: Getting current treasury addresses from AMP");
    let treasury_addresses = client.get_asset_treasury_addresses(asset_uuid).await?;

    println!(
        "✅ Found {} treasury addresses in AMP:",
        treasury_addresses.len()
    );
    if treasury_addresses.is_empty() {
        println!("   (No treasury addresses configured)");
    } else {
        for (i, address) in treasury_addresses.iter().enumerate() {
            println!("   {}. {}", i + 1, address);
        }
    }

    // Step 4a: Check for unconfidential addresses that should be removed
    println!("\n📋 Step 4a: Checking for unconfidential addresses in treasury");
    let unconfidential_addresses_to_remove: Vec<String> = treasury_addresses
        .iter()
        .filter(|addr| utxo_addresses.contains(*addr))
        .cloned()
        .collect();

    if !unconfidential_addresses_to_remove.is_empty() {
        println!(
            "🔍 Found {} unconfidential addresses that need to be removed:",
            unconfidential_addresses_to_remove.len()
        );
        for (i, address) in unconfidential_addresses_to_remove.iter().enumerate() {
            println!("   {}. {}", i + 1, address);
        }

        println!("\n🗑️  Removing unconfidential addresses from treasury...");
        match client
            .delete_asset_treasury_addresses(asset_uuid, &unconfidential_addresses_to_remove)
            .await
        {
            Ok(()) => {
                println!("✅ Successfully removed {} unconfidential addresses", unconfidential_addresses_to_remove.len());
            }
            Err(e) => {
                println!("❌ Failed to remove unconfidential addresses: {}", e);
                println!("   Continuing with confidential address sync...");
            }
        }
    } else {
        println!("✅ No unconfidential addresses found in treasury");
    }

    // Refresh treasury addresses after potential removals
    let treasury_addresses = if !unconfidential_addresses_to_remove.is_empty() {
        client.get_asset_treasury_addresses(asset_uuid).await?
    } else {
        treasury_addresses
    };

    // Step 5: Find missing addresses (in wallet but not in treasury)
    println!("\n📋 Step 5: Identifying missing addresses");
    let treasury_set: HashSet<String> = treasury_addresses.into_iter().collect();
    let missing_addresses: Vec<String> = wallet_addresses
        .difference(&treasury_set)
        .cloned()
        .collect();

    if missing_addresses.is_empty() {
        println!("✅ All wallet addresses are already in the treasury address list!");
        
        if !unconfidential_addresses_to_remove.is_empty() {
            // Summary for removal-only case
            println!("\n📊 Summary");
            println!("==========");
            println!("Asset UUID: {}", asset_uuid);
            println!("Asset ID: {}", asset_id);
            println!("Wallet: {}", WALLET_NAME);
            println!("UTXOs found: {}", utxos.len());
            println!("Unique addresses in wallet: {}", wallet_addresses.len());
            println!("Unconfidential addresses removed: {}", unconfidential_addresses_to_remove.len());
            println!("Confidential addresses added: 0");
            println!(
                "Total treasury addresses: {}",
                client.get_asset_treasury_addresses(asset_uuid).await?.len()
            );
            println!("\n🎉 Treasury address sync completed successfully!");
        } else {
            println!("   No action needed.");
        }
        return Ok(());
    }

    println!(
        "🔍 Found {} addresses that need to be added to treasury:",
        missing_addresses.len()
    );
    for (i, address) in missing_addresses.iter().enumerate() {
        println!("   {}. {}", i + 1, address);
    }

    // Step 6: Add missing addresses to treasury
    println!("\n📋 Step 6: Adding missing addresses to treasury");
    match client
        .add_asset_treasury_addresses(asset_uuid, &missing_addresses)
        .await
    {
        Ok(()) => {
            println!("✅ Successfully added {} addresses to treasury", missing_addresses.len());
        }
        Err(e) => {
            println!("❌ Failed to add addresses to treasury: {}", e);
            return Err(e.into());
        }
    }

    // Step 7: Verify the update
    println!("\n📋 Step 7: Verifying treasury addresses after update");
    let updated_treasury_addresses = client.get_asset_treasury_addresses(asset_uuid).await?;

    println!(
        "✅ Treasury now has {} addresses",
        updated_treasury_addresses.len()
    );

    // Summary
    println!("\n📊 Summary");
    println!("==========");
    println!("Asset UUID: {}", asset_uuid);
    println!("Asset ID: {}", asset_id);
    println!("Wallet: {}", WALLET_NAME);
    println!("UTXOs found: {}", utxos.len());
    println!("Unique addresses in wallet: {}", wallet_addresses.len());
    println!("Unconfidential addresses removed: {}", unconfidential_addresses_to_remove.len());
    println!("Confidential addresses added: {}", missing_addresses.len());
    println!(
        "Total treasury addresses: {}",
        updated_treasury_addresses.len()
    );

    println!("\n🎉 Treasury address sync completed successfully!");

    Ok(())
}
