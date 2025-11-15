//! Fully automated wallet migration with private key support
//!
//! This script:
//! 1. Exports the wallet from local node
//! 2. Unloads the cloud wallet
//! 3. Uses SSH to delete the cloud wallet directory
//! 4. Uses SCP to copy the wallet file to cloud
//! 5. Creates a fresh wallet on cloud
//! 6. Imports the wallet with all private keys
//! 7. Verifies signing capability

use amp_rs::ElementsRpc;
use dotenvy;
use std::env;
use std::process::Command;

const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🚀 Automated Wallet Migration with Signing Support");
    println!("==================================================");
    println!("Wallet: {}", WALLET_NAME);
    println!();

    dotenvy::dotenv().ok();

    // Get environment variables
    let local_url = env::var("ELEMENTS_RPC_URL")
        .map_err(|_| "ELEMENTS_RPC_URL not set in environment")?;
    let local_user = env::var("ELEMENTS_RPC_USER")
        .map_err(|_| "ELEMENTS_RPC_USER not set in environment")?;
    let local_password = env::var("ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "ELEMENTS_RPC_PASSWORD not set in environment")?;

    let cloud_url = env::var("CLOUD_ELEMENTS_RPC_URL")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_URL not set in environment")?;
    let cloud_user = env::var("CLOUD_ELEMENTS_RPC_USER")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_USER not set in environment")?;
    let cloud_password = env::var("CLOUD_ELEMENTS_RPC_PASSWORD")
        .map_err(|_| "CLOUD_ELEMENTS_RPC_PASSWORD not set in environment")?;

    // Extract cloud server IP from URL
    let cloud_ip = cloud_url
        .split("://")
        .nth(1)
        .and_then(|s| s.split(':').next())
        .ok_or("Could not extract IP from CLOUD_ELEMENTS_RPC_URL")?;

    println!("📡 Connection Details:");
    println!("  Local:  {}", local_url);
    println!("  Cloud:  {} ({})", cloud_url, cloud_ip);
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

    // Step 1: Load and export wallet from local node
    println!("📂 Step 1: Exporting wallet from local node...");
    match local_rpc.load_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet loaded"),
        Err(e) if e.to_string().contains("already loaded") => println!("✅ Wallet already loaded"),
        Err(e) => return Err(e.into()),
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let export_filename = format!("{}_export_{}.dat", WALLET_NAME, timestamp);
    let export_path = format!("/tmp/{}", export_filename);
    
    local_rpc.dump_wallet(WALLET_NAME, &export_path).await?;
    println!("✅ Wallet exported to: {}", export_path);
    println!();

    // Step 2: Unload cloud wallet
    println!("📤 Step 2: Unloading cloud wallet...");
    match cloud_rpc.unload_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet unloaded"),
        Err(e) if e.to_string().contains("not found") => println!("ℹ️  Wallet not loaded"),
        Err(e) => println!("⚠️  Unload error (continuing anyway): {}", e),
    }
    println!();

    // Step 3: Delete wallet directory on cloud via Docker
    println!("🗑️  Step 3: Deleting old wallet directory on cloud server...");
    let ssh_delete = Command::new("ssh")
        .arg(format!("ubuntu@{}", cloud_ip))
        .arg(format!("sudo docker exec elements-testnet rm -rf /root/.elements/liquidtestnet/wallets/{}", WALLET_NAME))
        .output();

    match ssh_delete {
        Ok(output) => {
            if output.status.success() {
                println!("✅ Wallet directory deleted on cloud");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠️  SSH delete warning: {}", stderr);
                println!("   Continuing anyway...");
            }
        }
        Err(e) => {
            println!("⚠️  SSH delete failed: {}", e);
            println!("   You may need to manually delete: ~/.elements/liquidv1/wallets/{}", WALLET_NAME);
            println!("   Or the wallet may not exist yet, which is fine.");
        }
    }
    println!();

    // Step 4: Copy wallet file to cloud via SCP, then into Docker container
    println!("📦 Step 4: Copying wallet file to cloud server...");
    
    // First copy to host
    let scp_output = Command::new("scp")
        .arg(&export_path)
        .arg(format!("ubuntu@{}:/tmp/{}", cloud_ip, export_filename))
        .output();

    match scp_output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("SCP to host failed: {}", stderr).into());
            }
        }
        Err(e) => {
            return Err(format!("Failed to execute SCP: {}", e).into());
        }
    }
    println!("✅ Wallet file copied to host");
    
    // Then copy into Docker container
    let docker_cp = Command::new("ssh")
        .arg(format!("ubuntu@{}", cloud_ip))
        .arg(format!("sudo docker cp /tmp/{} elements-testnet:/tmp/{}", export_filename, export_filename))
        .output();
    
    match docker_cp {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Docker cp failed: {}", stderr).into());
            }
        }
        Err(e) => {
            return Err(format!("Failed to copy into Docker: {}", e).into());
        }
    }
    println!("✅ Wallet file copied into Docker container");
    println!();

    // Step 5: Create fresh wallet on cloud
    println!("🏗️  Step 5: Creating fresh wallet on cloud node...");
    match cloud_rpc.create_elements_wallet(WALLET_NAME).await {
        Ok(()) => println!("✅ Wallet created"),
        Err(e) => {
            println!("❌ Failed to create wallet: {}", e);
            println!("   Make sure the wallet directory was deleted!");
            return Err(e.into());
        }
    }
    println!();

    // Step 6: Import wallet with private keys
    println!("📥 Step 6: Importing wallet with private keys...");
    let cloud_import_path = format!("/tmp/{}", export_filename);
    cloud_rpc.import_wallet(WALLET_NAME, &cloud_import_path).await?;
    println!("✅ Wallet imported successfully");
    println!("   All private keys and blinding keys are now on cloud node");
    println!();

    // Step 7: Clean up files
    println!("🧹 Step 7: Cleaning up temporary files...");
    
    // Delete local export file
    match std::fs::remove_file(&export_path) {
        Ok(()) => println!("✅ Removed local export file"),
        Err(e) => println!("⚠️  Could not remove local file: {}", e),
    }

    // Delete cloud export file via Docker
    let ssh_cleanup = Command::new("ssh")
        .arg(format!("ubuntu@{}", cloud_ip))
        .arg(format!("sudo docker exec elements-testnet rm -f {}", cloud_import_path))
        .output();

    match ssh_cleanup {
        Ok(output) => {
            if output.status.success() {
                println!("✅ Removed cloud export file");
            } else {
                println!("⚠️  Could not remove cloud file (not critical)");
            }
        }
        Err(_) => {
            println!("⚠️  Could not remove cloud file (not critical)");
        }
    }
    println!();

    // Step 8: Verify signing capability
    println!("✔️  Step 8: Verifying signing capability...");
    match cloud_rpc.get_new_address(WALLET_NAME, None).await {
        Ok(address) => {
            match cloud_rpc.dump_private_key(WALLET_NAME, &address).await {
                Ok(_) => {
                    println!("✅ SUCCESS! Cloud wallet can sign transactions");
                }
                Err(e) => {
                    println!("❌ FAILED: Cloud wallet cannot access private keys");
                    println!("   Error: {}", e);
                    return Err(e.into());
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to generate test address: {}", e);
            return Err(e.into());
        }
    }
    println!();

    println!("🎉 Migration Complete!");
    println!("======================");
    println!("✅ Cloud wallet '{}' can now sign transactions", WALLET_NAME);
    println!();
    println!("💡 Next steps:");
    println!("  1. Rescan blockchain on cloud node:");
    println!("     ssh ubuntu@{} 'sudo docker exec elements-testnet elements-cli -rpcwallet={} rescanblockchain'", cloud_ip, WALLET_NAME);
    println!();
    println!("  2. Test sending a transaction from the cloud wallet");

    Ok(())
}
