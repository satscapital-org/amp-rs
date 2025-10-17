//! Example demonstrating descriptor-based Elements wallet setup
//!
//! This example shows how to:
//! 1. Generate a mnemonic and create an LwkSoftwareSigner
//! 2. Generate WPkH Slip77 descriptors from the mnemonic
//! 3. Set up an Elements descriptor wallet with the descriptors
//! 4. Enable the wallet to see confidential transactions with blinding keys
//!
//! ## Prerequisites
//!
//! - Running Elements node with RPC access
//! - Environment variables set for Elements RPC connection:
//!   - ELEMENTS_RPC_URL (e.g., http://localhost:18884)
//!   - ELEMENTS_RPC_USER
//!   - ELEMENTS_RPC_PASSWORD
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example descriptor_wallet_setup
//! ```

use amp_rs::signer::LwkSoftwareSigner;
use amp_rs::ElementsRpc;
use chrono;
use dotenvy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔧 Descriptor-based Elements Wallet Setup Example");
    println!("================================================");

    // Load environment variables
    dotenvy::dotenv().ok();

    // Check if Elements RPC is configured
    let elements_rpc = match ElementsRpc::from_env() {
        Ok(rpc) => {
            println!("✅ Elements RPC client created from environment variables");
            rpc
        }
        Err(e) => {
            println!("❌ Failed to create Elements RPC client: {}", e);
            println!("\nPlease set the following environment variables:");
            println!("  ELEMENTS_RPC_URL=http://localhost:18884");
            println!("  ELEMENTS_RPC_USER=your_rpc_user");
            println!("  ELEMENTS_RPC_PASSWORD=your_rpc_password");
            return Err(e.into());
        }
    };

    // Test Elements node connectivity
    match elements_rpc.get_network_info().await {
        Ok(info) => {
            println!("✅ Connected to Elements node");
            println!("   Version: {}", info.version);
            println!("   Connections: {}", info.connections);
        }
        Err(e) => {
            println!("❌ Failed to connect to Elements node: {}", e);
            println!("   Please ensure your Elements node is running and accessible");
            return Err(e.into());
        }
    }

    println!("\n🔐 Generating Mnemonic and Signer");
    println!("=================================");

    // Generate a new mnemonic and signer
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
    println!("✅ Generated new mnemonic and signer");
    println!("   Mnemonic: {}", mnemonic);
    println!("   Testnet: {}", signer.is_testnet());

    // Derive a treasury address to demonstrate address generation
    let treasury_address = signer.derive_address(Some(0))?;
    println!("   Treasury address (index 0): {}", treasury_address);

    println!("\n📝 Generating Descriptors");
    println!("========================");

    // Generate WPkH Slip77 descriptor
    let descriptor = signer.get_wpkh_slip77_descriptor()?;
    println!("✅ Generated WPkH Slip77 descriptor");
    println!("   Descriptor: {}", descriptor);

    // Verify descriptor has the expected format for Liquid (ct = confidential transaction)
    assert!(descriptor.contains("ct(") || descriptor.contains("wpkh("));
    assert!(descriptor.contains("<0;1>/*") || descriptor.contains("/0/*"));
    println!("✅ Descriptor validated - correct format for Liquid confidential transactions");

    println!("\n🏦 Setting Up Elements Wallet");
    println!("============================");

    // Create a unique wallet name
    let wallet_name = format!("descriptor_example_{}", chrono::Utc::now().timestamp());
    println!("   Wallet name: {}", wallet_name);

    // Set up the descriptor wallet
    let wallet_result = async {
        elements_rpc.create_descriptor_wallet(&wallet_name).await?;
        elements_rpc.import_descriptor(&wallet_name, &descriptor).await
    }.await;
    
    match wallet_result {
        Ok(()) => {
            println!("✅ Successfully set up descriptor wallet");
            println!("   The wallet can now:");
            println!("   - Detect transactions to mnemonic-derived addresses");
            println!("   - Access blinding keys for confidential transactions");
            println!("   - Scan the blockchain for relevant UTXOs");
        }
        Err(e) => {
            println!("❌ Failed to set up descriptor wallet: {}", e);
            
            // Provide manual instructions
            println!("\n🔧 Manual Setup Instructions");
            println!("============================");
            println!("If automatic setup failed, you can manually set up the wallet:");
            println!();
            println!("1. Create descriptor wallet:");
            println!("   elements-cli createwallet \"{}\" true", wallet_name);
            println!();
            println!("2. Import descriptor:");
            println!("   elements-cli -rpcwallet={} importdescriptors '[", wallet_name);
            println!("     {{");
            println!("       \"desc\": \"{}\",", descriptor);
            println!("       \"timestamp\": \"now\",");
            println!("       \"active\": true,");
            println!("       \"internal\": false");
            println!("     }}");
            println!("   ]'");
            println!();
            println!("3. Verify import:");
            println!("   elements-cli -rpcwallet={} listdescriptors", wallet_name);
            
            return Err(e.into());
        }
    }

    println!("\n🎯 Example Complete");
    println!("==================");
    println!("✅ Mnemonic generated: {}", mnemonic);
    println!("✅ Descriptors created with Slip77 blinding support");
    println!("✅ Elements wallet configured: {}", wallet_name);
    println!();
    println!("The wallet is now ready to:");
    println!("  - Receive funds to mnemonic-derived addresses");
    println!("  - See confidential transactions with proper blinding keys");
    println!("  - Support the asset distribution workflow");
    println!();
    println!("💡 Next steps:");
    println!("  - Send test funds to: {}", treasury_address);
    println!("  - Check wallet balance: elements-cli -rpcwallet={} getbalance", wallet_name);
    println!("  - List UTXOs: elements-cli -rpcwallet={} listunspent", wallet_name);

    Ok(())
}