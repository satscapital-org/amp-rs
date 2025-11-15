//! Import Treasury Addresses to Cloud Wallet
//!
//! This script imports all treasury addresses from AMP assets into the cloud
//! Elements wallet so it can see and spend the asset UTXOs.

use amp_rs::{ApiClient, ElementsRpc};
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🔑 Import Treasury Addresses to Cloud Wallet");
    println!("============================================");
    println!();

    dotenvy::dotenv().ok();
    env::set_var("AMP_TESTS", "live");

    // Create AMP API client
    println!("🌐 Creating AMP API client...");
    let amp_client = ApiClient::new().await?;
    println!("✅ Connected to AMP API");

    // Create cloud Elements RPC client
    let cloud_url = env::var("CLOUD_ELEMENTS_RPC_URL")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_URL not set in environment")?;
    let cloud_user = env::var("CLOUD_ELEMENTS_RPC_USER")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_USER not set in environment")?;
    let cloud_password = env::var("CLOUD_ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_PASSWORD not set in environment")?;

    println!("📡 Cloud Node: {}", cloud_url);

    let cloud_rpc = ElementsRpc::new(cloud_url, cloud_user, cloud_password);

    // Test connection
    cloud_rpc.get_network_info().await?;
    println!("✅ Connected to cloud Elements node");
    println!();

    const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

    // Load the wallet
    println!("📂 Loading wallet: {}", WALLET_NAME);
    match cloud_rpc.load_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet loaded"),
        Err(e) if e.to_string().contains("already loaded") => println!("✅ Wallet already loaded"),
        Err(e) => return Err(e.into()),
    }
    println!();

    // Get all assets
    println!("📋 Fetching all assets from AMP...");
    let assets = amp_client.get_assets().await?;
    println!("✅ Found {} assets total", assets.len());
    
    // Filter for assets that have treasury addresses (issued assets)
    let issued_assets: Vec<_> = assets
        .into_iter()
        .filter(|asset| asset.pubkey.is_some() && !asset.pubkey.as_ref().unwrap().is_empty())
        .collect();
    
    println!("✅ Found {} issued assets with treasury addresses", issued_assets.len());
    println!();

    // Import each treasury address
    println!("🔑 Importing treasury addresses...");
    let mut imported_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;

    for (i, asset) in issued_assets.iter().enumerate() {
        println!("  [{}/{}] Processing asset: {} ({})", 
            i + 1, 
            issued_assets.len(), 
            asset.name, 
            asset.asset_uuid
        );

        let treasury_address = match &asset.pubkey {
            Some(addr) if !addr.is_empty() => addr,
            _ => {
                println!("    ⚠️  No treasury address");
                skipped_count += 1;
                continue;
            }
        };

        // Import the address as watch-only with a label
        let label = format!("AMP_Treasury_{}", asset.name);
        
        match cloud_rpc
            .import_address(WALLET_NAME, treasury_address, Some(&label), Some(false))
            .await
        {
            Ok(()) => {
                println!("    ✅ Imported: {}", treasury_address);
                imported_count += 1;
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("already") || error_msg.contains("duplicate") {
                    println!("    ✓ Already imported");
                    imported_count += 1;
                } else {
                    println!("    ❌ Failed: {}", e);
                    failed_count += 1;
                }
            }
        }
    }

    println!();
    println!("📈 Summary:");
    println!("  ✅ Addresses imported: {}", imported_count);
    if skipped_count > 0 {
        println!("  ⚠️  Addresses skipped (no address): {}", skipped_count);
    }
    if failed_count > 0 {
        println!("  ❌ Addresses failed: {}", failed_count);
    }
    println!();

    // Rescan the blockchain
    println!("🔄 Rescanning blockchain to detect UTXOs...");
    println!("   This may take several minutes...");
    
    match cloud_rpc.rescan_blockchain(WALLET_NAME, None).await {
        Ok(result) => {
            println!("✅ Rescan complete!");
            println!("   Scanned from block {} to {}", 
                result.get("start_height").and_then(|v| v.as_u64()).unwrap_or(0),
                result.get("stop_height").and_then(|v| v.as_u64()).unwrap_or(0)
            );
        }
        Err(e) => {
            println!("❌ Rescan failed: {}", e);
            println!("   You may need to manually rescan:");
            println!("   ssh ubuntu@<cloud-ip> 'sudo docker exec elements-testnet elements-cli -rpcuser=elements -rpcpassword=elementspass -rpcport=18891 -rpcwallet={} rescanblockchain'", WALLET_NAME);
        }
    }

    println!();
    println!("🎉 Treasury address import complete!");
    println!("   The cloud wallet should now be able to see and spend asset UTXOs.");

    Ok(())
}
