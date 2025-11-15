//! Example demonstrating wallet migration from local Elements node to cloud Elements node
//!
//! This example exports the "amp_elements_wallet_static_for_funding" wallet from a local
//! Elements node and imports it into a cloud Elements node, including all private keys
//! and blinding keys.
//!
//! ## Prerequisites
//!
//! - Running local Elements node with RPC access
//! - Running cloud Elements node with RPC access
//! - The wallet "amp_elements_wallet_static_for_funding" must exist on the local node
//! - Environment variables set in .env:
//!   - ELEMENTS_RPC_URL (local node)
//!   - ELEMENTS_RPC_USER (local node)
//!   - ELEMENTS_RPC_PASSWORD (local node)
//!   - CLOUD_ELEMENTS_RPC_URL (cloud node)
//!   - CLOUD_ELEMENTS_RPC_USER (cloud node)
//!   - CLOUD_ELEMENTS_RPC_PASSWORD (cloud node)
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example migrate_test_wallet_to_cloud
//! ```

use amp_rs::ElementsRpc;
use dotenvy;
use std::env;

const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔄 Wallet Migration: Local → Cloud");
    println!("==================================");
    println!("Wallet: {}", WALLET_NAME);
    println!();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get environment variables for local node
    let local_url =
        env::var("ELEMENTS_RPC_URL").map_err(|_| "ELEMENTS_RPC_URL not set in environment")?;
    let local_user =
        env::var("ELEMENTS_RPC_USER").map_err(|_| "ELEMENTS_RPC_USER not set in environment")?;
    let local_password = env::var("ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "ELEMENTS_RPC_PASSWORD not set in environment")?;

    // Get environment variables for cloud node
    let cloud_url = env::var("CLOUD_ELEMENTS_RPC_URL")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_URL not set in environment")?;
    let cloud_user = env::var("CLOUD_ELEMENTS_RPC_USER")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_USER not set in environment")?;
    let cloud_password = env::var("CLOUD_ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_PASSWORD not set in environment")?;

    println!("📡 Connection Details:");
    println!("  Local:  {}", local_url);
    println!("  Cloud:  {}", cloud_url);
    println!();

    // Create RPC clients
    let local_rpc = ElementsRpc::new(local_url.clone(), local_user, local_password);
    let cloud_rpc = ElementsRpc::new(cloud_url.clone(), cloud_user, cloud_password);

    // Test connectivity to both nodes
    println!("🔌 Testing Connectivity...");
    match local_rpc.get_network_info().await {
        Ok(info) => {
            println!("✅ Connected to local node (version: {})", info.version);
        }
        Err(e) => {
            println!("❌ Failed to connect to local node: {}", e);
            return Err(e.into());
        }
    }

    match cloud_rpc.get_network_info().await {
        Ok(info) => {
            println!("✅ Connected to cloud node (version: {})", info.version);
        }
        Err(e) => {
            println!("❌ Failed to connect to cloud node: {}", e);
            return Err(e.into());
        }
    }
    println!();

    // Step 1: Load the wallet on local node
    println!("📂 Step 1: Loading wallet on local node...");
    match local_rpc.load_wallet(WALLET_NAME).await {
        Ok(()) => {
            println!("✅ Wallet loaded successfully");
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("already loaded") {
                println!("✅ Wallet already loaded");
            } else {
                println!("❌ Failed to load wallet: {}", e);
                return Err(e.into());
            }
        }
    }
    println!();

    // Step 2: Check if this is a descriptor wallet or legacy wallet
    println!("📋 Step 2: Checking wallet type...");
    let wallet_info = local_rpc.get_wallet_info(WALLET_NAME).await?;
    let is_descriptor_wallet = wallet_info
        .get("descriptors")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_descriptor_wallet {
        println!("✅ Descriptor wallet detected");
        println!(
            "   For descriptor wallets, use migrate_test_wallet_to_cloud_descriptors.rs instead"
        );
        println!("   That script will properly migrate the HD wallet structure and blinding keys.");
        return Ok(());
    } else {
        println!("✅ Legacy wallet detected");
        println!(
            "   Will use dumpwallet/importwallet for complete migration including blinding keys"
        );
    }
    println!();

    // Step 3: Export wallet using dumpwallet (includes master blinding key)
    println!("💾 Step 3: Exporting wallet from local node using dumpwallet...");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let export_filename = format!("{}_export_{}.dat", WALLET_NAME, timestamp);
    let export_path = format!("/tmp/{}", export_filename);

    match local_rpc.dump_wallet(WALLET_NAME, &export_path).await {
        Ok(()) => {
            println!("✅ Wallet exported to: {}", export_path);
        }
        Err(e) => {
            println!("❌ Failed to export wallet: {}", e);
            println!("   Note: The export path must be writable by the Elements node");
            return Err(e.into());
        }
    }
    println!();

    // Step 4: Handle file transfer for cloud import
    println!("📦 Step 4: Preparing wallet file for cloud import...");
    println!("   Local export file: {}", export_path);
    println!();

    // Check if local and cloud are the same server
    let same_server = local_url.contains("127.0.0.1") || local_url.contains("localhost");

    if same_server {
        println!("   ⚠️  Local and cloud appear to be different servers");
        println!("   The wallet file needs to be accessible to the cloud Elements node");
        println!();
        println!("   Manual steps required:");
        println!("   1. Copy the wallet file to the cloud server:");
        println!("      scp {} user@cloud-server:/tmp/", export_path);
        println!();
        println!("   2. Then run importwallet on the cloud node:");
        println!(
            "      elements-cli -rpcwallet={} importwallet /tmp/{}",
            WALLET_NAME, export_filename
        );
        println!();
        println!("   3. After import, rescan the blockchain:");
        println!(
            "      elements-cli -rpcwallet={} rescanblockchain",
            WALLET_NAME
        );
        println!();
        println!("Migration export complete. Manual import steps required on cloud server.");
        return Ok(());
    } else {
        println!("   ✅ Nodes appear to share filesystem, proceeding with import...");
        println!();
    }

    // Step 5: Create wallet on cloud node
    println!("🏗️  Step 5: Creating wallet on cloud node...");
    match cloud_rpc.create_elements_wallet(WALLET_NAME).await {
        Ok(()) => {
            println!("✅ Wallet created on cloud node");
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("already exists") || error_msg.contains("Database already exists")
            {
                println!("⚠️  Wallet already exists on cloud node");
                println!("   The import will add keys to the existing wallet");
            } else {
                println!("❌ Failed to create wallet on cloud node: {}", e);
                return Err(e.into());
            }
        }
    }
    println!();

    // Step 6: Import wallet to cloud node
    println!("📥 Step 6: Importing wallet to cloud node...");
    println!("   This will import all private keys and the master blinding key");
    match cloud_rpc.import_wallet(WALLET_NAME, &export_path).await {
        Ok(()) => {
            println!("✅ Wallet imported successfully");
            println!("   All keys (including blinding keys) are now available on cloud node");
        }
        Err(e) => {
            println!("❌ Failed to import wallet: {}", e);
            println!();
            println!("   Troubleshooting:");
            println!("   1. Ensure the wallet file exists at: {}", export_path);
            println!("   2. Ensure the cloud Elements node can read the file");
            println!("   3. Check file permissions");
            println!("   4. If nodes are on different servers, copy the file first:");
            println!("      scp {} user@cloud-server:/tmp/", export_path);
            return Err(e.into());
        }
    }
    println!();

    // Step 7: Clean up export file
    println!("🧹 Step 7: Cleaning up...");
    match std::fs::remove_file(&export_path) {
        Ok(()) => {
            println!("✅ Removed temporary export file");
        }
        Err(e) => {
            println!("⚠️  Could not remove export file: {}", e);
            println!("   You may want to manually delete: {}", export_path);
        }
    }
    println!();

    // Step 8: Verify migration
    println!("✔️  Step 8: Verifying migration...");

    let local_info = local_rpc.get_wallet_info(WALLET_NAME).await?;
    let cloud_info = cloud_rpc.get_wallet_info(WALLET_NAME).await?;

    println!("Local wallet:");
    println!("  Tx count: {:?}", local_info.get("txcount"));

    println!("Cloud wallet:");
    println!("  Tx count: {:?}", cloud_info.get("txcount"));
    println!();

    println!("🎉 Migration Complete!");
    println!("===================");
    println!(
        "✅ Wallet '{}' has been migrated from local to cloud node",
        WALLET_NAME
    );
    println!("✅ All private keys and master blinding key have been imported");
    println!("✅ Blinding keys are automatically derived from the master blinding key");
    println!();

    if cloud_info.get("txcount").and_then(|v| v.as_u64()) == Some(0) {
        println!("⚠️  Note: Cloud wallet shows 0 transactions");
        println!("   The cloud node needs to rescan the blockchain to see existing transactions.");
        println!();
        println!("   Run this command on the cloud node:");
        println!(
            "   elements-cli -rpcwallet={} rescanblockchain",
            WALLET_NAME
        );
        println!();
    }

    println!("💡 Next steps:");
    println!(
        "  - Rescan blockchain on cloud node: elements-cli -rpcwallet={} rescanblockchain",
        WALLET_NAME
    );
    println!("  - Verify addresses match between local and cloud");
    println!("  - Test generating a confidential address on cloud node");
    println!("  - Test transaction signing on cloud node");

    Ok(())
}
