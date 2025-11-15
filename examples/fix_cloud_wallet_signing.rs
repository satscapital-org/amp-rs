//! Fix cloud wallet signing by properly reimporting with private keys
//!
//! This script deletes the existing cloud wallet and recreates it from scratch
//! with a proper import of all private keys and blinding keys.

use amp_rs::ElementsRpc;
use dotenvy;
use std::env;

const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🔧 Fixing Cloud Wallet Signing Capability");
    println!("=========================================");
    println!("Wallet: {}", WALLET_NAME);
    println!();

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

    let local_rpc = ElementsRpc::new(local_url.clone(), local_user, local_password);
    let cloud_rpc = ElementsRpc::new(cloud_url.clone(), cloud_user, cloud_password);

    // Test connectivity
    println!("🔌 Testing Connectivity...");
    local_rpc.get_network_info().await?;
    println!("✅ Connected to local node");
    cloud_rpc.get_network_info().await?;
    println!("✅ Connected to cloud node");
    println!();

    // Step 1: Load local wallet
    println!("📂 Step 1: Loading wallet on local node...");
    match local_rpc.load_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet loaded"),
        Err(e) if e.to_string().contains("already loaded") => println!("✅ Wallet already loaded"),
        Err(e) => return Err(e.into()),
    }
    println!();

    // Step 2: Export wallet from local node
    println!("💾 Step 2: Exporting wallet from local node...");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let export_filename = format!("{}_export_{}.dat", WALLET_NAME, timestamp);
    let export_path = format!("/tmp/{}", export_filename);

    local_rpc.dump_wallet(WALLET_NAME, &export_path).await?;
    println!("✅ Wallet exported to: {}", export_path);
    println!();

    // Step 3: Unload cloud wallet if loaded
    println!("📤 Step 3: Unloading cloud wallet...");
    match cloud_rpc.unload_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet unloaded"),
        Err(e) if e.to_string().contains("not found") => {
            println!("ℹ️  Wallet not loaded");
        }
        Err(e) => {
            println!("⚠️  Error unloading: {}", e);
        }
    }
    println!();

    // Step 4: Delete cloud wallet directory
    println!("🗑️  Step 4: Deleting old cloud wallet...");
    println!("   ⚠️  WARNING: This will permanently delete the watch-only cloud wallet");
    println!("   The wallet will be recreated with proper private keys from the local backup.");
    println!();
    println!("   You need to manually delete the wallet directory on the cloud server:");
    println!(
        "   ssh cloud-server 'rm -rf ~/.elements/liquidv1/wallets/{}'",
        WALLET_NAME
    );
    println!();
    println!("   Press Enter when you've deleted the cloud wallet directory...");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    println!();

    // Step 5: Create fresh wallet on cloud
    println!("🏗️  Step 5: Creating fresh wallet on cloud node...");
    match cloud_rpc.create_elements_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet created"),
        Err(e) => {
            println!("❌ Failed to create wallet: {}", e);
            println!("   Make sure you deleted the wallet directory first!");
            return Err(e.into());
        }
    }
    println!();

    // Step 6: Import wallet to cloud node
    println!("📥 Step 6: Importing wallet with private keys to cloud node...");
    println!("   This will import ALL private keys and blinding keys");

    // Check if file needs to be copied
    if !cloud_url.contains("localhost") && !cloud_url.contains("127.0.0.1") {
        println!();
        println!("   ⚠️  Cloud server is remote. Manual file copy required:");
        println!("   scp {} user@cloud-server:/tmp/", export_path);
        println!();
        println!("   Press Enter after copying the file...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    }

    cloud_rpc.import_wallet(WALLET_NAME, &export_path).await?;
    println!("✅ Wallet imported successfully");
    println!("   All private keys and blinding keys are now on cloud node");
    println!();

    // Step 7: Clean up export file
    println!("🧹 Step 7: Cleaning up...");
    match std::fs::remove_file(&export_path) {
        Ok(()) => println!("✅ Removed temporary export file"),
        Err(e) => println!("⚠️  Could not remove export file: {}", e),
    }
    println!();

    // Step 8: Verify
    println!("✔️  Step 8: Verifying signing capability...");

    // Try to generate an address and dump its private key
    match cloud_rpc.get_new_address(WALLET_NAME, None).await {
        Ok(address) => match cloud_rpc.dump_private_key(WALLET_NAME, &address).await {
            Ok(_) => {
                println!("✅ SUCCESS! Cloud wallet can now sign transactions");
            }
            Err(e) => {
                println!("❌ FAILED: Cloud wallet still cannot access private keys");
                println!("   Error: {}", e);
                return Err(e.into());
            }
        },
        Err(e) => {
            println!("❌ Failed to generate test address: {}", e);
            return Err(e.into());
        }
    }
    println!();

    println!("🎉 Migration Complete!");
    println!("====================");
    println!("✅ Cloud wallet can now sign transactions");
    println!();
    println!("💡 Next steps:");
    println!(
        "  - Rescan blockchain: elements-cli -rpcwallet={} rescanblockchain",
        WALLET_NAME
    );
    println!("  - Test sending a transaction from the cloud wallet");

    Ok(())
}
