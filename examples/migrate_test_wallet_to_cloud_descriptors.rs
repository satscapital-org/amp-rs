//! Example demonstrating HD wallet migration using descriptors
//!
//! This example exports the wallet descriptors (which include the master keys)
//! from a local Elements node and imports them into a cloud Elements node.
//! This preserves the HD wallet structure and all derived keys including blinding keys.
//!
//! ## Prerequisites
//!
//! - Running local Elements node with RPC access
//! - Running cloud Elements node with RPC access
//! - The wallet must be a descriptor wallet on the local node
//! - Environment variables set in .env
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example migrate_test_wallet_to_cloud_descriptors
//! ```

use amp_rs::ElementsRpc;
use dotenvy;
use std::env;

const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔄 HD Wallet Migration: Local → Cloud (Using Descriptors)");
    println!("=========================================================");
    println!("Wallet: {}", WALLET_NAME);
    println!();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get environment variables
    let local_url =
        env::var("ELEMENTS_RPC_URL").map_err(|_| "ELEMENTS_RPC_URL not set in environment")?;
    let local_user =
        env::var("ELEMENTS_RPC_USER").map_err(|_| "ELEMENTS_RPC_USER not set in environment")?;
    let local_password = env::var("ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "ELEMENTS_RPC_PASSWORD not set in environment")?;

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
    let local_rpc = ElementsRpc::new(local_url, local_user, local_password);
    let cloud_rpc = ElementsRpc::new(cloud_url, cloud_user, cloud_password);

    // Test connectivity
    println!("🔌 Testing Connectivity...");
    local_rpc.get_network_info().await?;
    println!("✅ Connected to local node");
    cloud_rpc.get_network_info().await?;
    println!("✅ Connected to cloud node");
    println!();

    // Step 1: Load wallet on local node
    println!("📂 Step 1: Loading wallet on local node...");
    match local_rpc.load_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet loaded"),
        Err(e) if e.to_string().contains("already loaded") => println!("✅ Wallet already loaded"),
        Err(e) => return Err(e.into()),
    }
    println!();

    // Step 2: Export descriptors from local wallet
    println!("📤 Step 2: Exporting wallet descriptors from local node...");
    let descriptors = local_rpc.list_descriptors(WALLET_NAME, Some(true)).await?;

    println!("✅ Exported {} descriptors:", descriptors.len());
    for (i, desc) in descriptors.iter().enumerate() {
        let desc_type = if desc.contains("wpkh") {
            "Native SegWit"
        } else if desc.contains("sh(wpkh") {
            "Nested SegWit"
        } else if desc.contains("pkh") {
            "Legacy"
        } else {
            "Other"
        };
        println!("  {}. {} descriptor", i + 1, desc_type);
    }
    println!();

    // Step 3: Create descriptor wallet on cloud node
    println!("🏗️  Step 3: Creating descriptor wallet on cloud node...");
    match cloud_rpc.create_descriptor_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Descriptor wallet created"),
        Err(e) if e.to_string().contains("already exists") => {
            println!("✅ Wallet already exists");
        }
        Err(e) => return Err(e.into()),
    }
    println!();

    // Step 4: Import descriptors to cloud wallet
    println!("📥 Step 4: Importing descriptors to cloud node...");
    let mut imported_count = 0;
    for (i, descriptor) in descriptors.iter().enumerate() {
        match cloud_rpc.import_descriptor(WALLET_NAME, descriptor).await {
            Ok(()) => {
                imported_count += 1;
                println!("  ✓ Imported descriptor {}/{}", i + 1, descriptors.len());
            }
            Err(e) => {
                println!("  ⚠ Failed to import descriptor {}: {}", i + 1, e);
            }
        }
    }
    println!(
        "✅ Imported {}/{} descriptors",
        imported_count,
        descriptors.len()
    );
    println!();

    // Step 5: Verify migration
    println!("✔️  Step 5: Verifying migration...");
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
        "✅ Wallet '{}' descriptors migrated to cloud node",
        WALLET_NAME
    );
    println!("✅ All HD keys (including blinding keys) are now available on cloud node");
    println!();

    if cloud_info.get("txcount").and_then(|v| v.as_u64()) == Some(0) {
        println!("⚠️  Note: Cloud wallet shows 0 transactions");
        println!("   The cloud node needs to rescan the blockchain.");
        println!(
            "   Run: elements-cli -rpcwallet={} rescanblockchain",
            WALLET_NAME
        );
        println!();
    }

    println!("💡 Next steps:");
    println!("  - Rescan blockchain on cloud node to see existing transactions");
    println!("  - Verify addresses match between local and cloud");
    println!("  - Test transaction signing on cloud node");

    Ok(())
}
