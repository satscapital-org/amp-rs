//! Clean out all wallets from the cloud Elements node

use amp_rs::ElementsRpc;
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🧹 Cleaning Cloud Node Wallets");
    println!("==============================");
    println!();

    dotenvy::dotenv().ok();

    let cloud_url = env::var("CLOUD_ELEMENTS_RPC_URL")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_URL not set in environment")?;
    let cloud_user = env::var("CLOUD_ELEMENTS_RPC_USER")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_USER not set in environment")?;
    let cloud_password = env::var("CLOUD_ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_PASSWORD not set in environment")?;

    println!("📡 Cloud Node: {}", cloud_url);
    println!();

    let cloud_rpc = ElementsRpc::new(cloud_url, cloud_user, cloud_password);

    // Test connectivity
    match cloud_rpc.get_network_info().await {
        Ok(_) => println!("✅ Connected to cloud node"),
        Err(e) => {
            println!("❌ Failed to connect: {}", e);
            return Err(e.into());
        }
    }
    println!();

    // List all wallets
    println!("📋 Listing all wallets...");
    let wallets = cloud_rpc.list_wallets().await?;
    
    if wallets.is_empty() {
        println!("✅ No wallets found on cloud node");
        return Ok(());
    }

    println!("Found {} wallet(s):", wallets.len());
    for wallet in &wallets {
        println!("  - {}", wallet);
    }
    println!();

    // Unload and delete each wallet
    for wallet_name in &wallets {
        println!("🗑️  Processing wallet: {}", wallet_name);
        
        // Unload wallet
        match cloud_rpc.unload_wallet(wallet_name).await {
            Ok(()) => println!("  ✓ Unloaded"),
            Err(e) => println!("  ⚠️  Unload error: {}", e),
        }

        // Note: We can't delete wallet files via RPC, but unloading is the first step
        println!("  ℹ️  Wallet unloaded (files remain on disk)");
    }
    println!();

    println!("⚠️  Manual step required:");
    println!("SSH into the cloud server and delete wallet directories:");
    println!();
    for wallet_name in &wallets {
        println!("  rm -rf ~/.elements/liquidv1/wallets/{}", wallet_name);
    }
    println!();
    println!("Or delete all wallets at once:");
    println!("  rm -rf ~/.elements/liquidv1/wallets/*");
    println!();

    Ok(())
}
