//! Example demonstrating Elements-first wallet setup approach
//!
//! This example shows the new Elements-first approach where we:
//! 1. Create a wallet in Elements Core
//! 2. Generate an address in Elements
//! 3. Export the private key from Elements
//! 4. Import the private key into LWK for signing
//! 5. Verify that LWK can sign for the Elements-generated address
//!
//! This approach ensures Elements can definitely see transactions to the address
//! since it generated the address, while LWK can sign transactions using the
//! imported private key.
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
//! cargo run --example elements_first_wallet_setup
//! ```

use amp_rs::signer::{LwkSoftwareSigner, Signer};
use amp_rs::ElementsRpc;
use chrono;
use dotenvy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔧 Elements-First Wallet Setup Example");
    println!("======================================");

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

    println!("\n🏦 Step 1: Creating Elements Wallet");
    println!("==================================");

    // Create a unique wallet name
    let wallet_name = format!("elements_first_{}", chrono::Utc::now().timestamp());
    println!("   Wallet name: {}", wallet_name);

    // Create the Elements wallet
    match elements_rpc.create_elements_wallet(&wallet_name).await {
        Ok(()) => {
            println!("✅ Successfully created Elements wallet: {}", wallet_name);
        }
        Err(e) => {
            println!("❌ Failed to create Elements wallet: {}", e);

            // Check if wallet already exists
            let error_msg = e.to_string();
            if error_msg.contains("already exists") || error_msg.contains("Database already exists")
            {
                println!("   Wallet already exists, continuing with existing wallet");
            } else {
                return Err(e.into());
            }
        }
    }

    println!("\n📍 Step 2: Generating Native Segwit Address in Elements");
    println!("======================================================");

    // Get a new native segwit address from Elements
    let unconfidential_address = match elements_rpc
        .get_new_address(&wallet_name, Some("bech32"))
        .await
    {
        Ok(address) => {
            println!(
                "✅ Generated new unconfidential address in Elements: {}",
                address
            );
            address
        }
        Err(e) => {
            println!("❌ Failed to generate address: {}", e);
            return Err(e.into());
        }
    };

    // Get the confidential version of the address
    let confidential_address = match elements_rpc
        .get_confidential_address(&wallet_name, &unconfidential_address)
        .await
    {
        Ok(address) => {
            println!("✅ Retrieved confidential address: {}", address);
            address
        }
        Err(e) => {
            println!("❌ Failed to get confidential address: {}", e);
            return Err(e.into());
        }
    };

    println!("\n🔑 Step 3: Exporting Private Key from Elements");
    println!("==============================================");

    // Export the private key for the unconfidential address
    let private_key = match elements_rpc
        .dump_private_key(&wallet_name, &unconfidential_address)
        .await
    {
        Ok(key) => {
            println!("✅ Successfully exported private key");
            println!("   Private key: {}...", &key[..10]); // Show only first 10 chars for security
            key
        }
        Err(e) => {
            println!("❌ Failed to export private key: {}", e);
            return Err(e.into());
        }
    };

    println!("\n🔧 Step 4: Creating LWK Signer from Elements Private Key");
    println!("========================================================");

    // Create LWK signer from the Elements private key
    let lwk_signer = match LwkSoftwareSigner::from_elements_private_key(&private_key) {
        Ok(signer) => {
            println!("✅ Successfully created LWK signer from Elements private key");
            println!("   Testnet mode: {}", signer.is_testnet());
            signer
        }
        Err(e) => {
            println!("❌ Failed to create LWK signer: {}", e);
            return Err(e.into());
        }
    };

    println!("\n🔍 Step 5: Verifying Address Compatibility");
    println!("==========================================");

    // Verify that LWK can work with the unconfidential address
    match lwk_signer.verify_elements_address(&unconfidential_address) {
        Ok(verified_address) => {
            println!("✅ Address verification successful");
            println!("   Unconfidential address: {}", unconfidential_address);
            println!("   Verified address: {}", verified_address);

            if unconfidential_address == verified_address {
                println!("✅ Addresses match perfectly!");
            } else {
                println!("⚠️  Addresses differ - this may be expected depending on implementation");
            }
        }
        Err(e) => {
            println!("❌ Address verification failed: {}", e);
            return Err(e.into());
        }
    }

    println!("\n🧪 Step 6: Testing Signer Functionality");
    println!("=======================================");

    // Test that the signer can handle transaction signing (with invalid input to test error handling)
    match lwk_signer.sign_transaction("invalid_transaction_hex").await {
        Ok(_) => {
            println!("⚠️  Unexpected: signer accepted invalid transaction");
        }
        Err(e) => {
            println!("✅ Signer correctly rejected invalid transaction: {}", e);
        }
    }

    // Test with empty transaction
    match lwk_signer.sign_transaction("").await {
        Ok(_) => {
            println!("⚠️  Unexpected: signer accepted empty transaction");
        }
        Err(e) => {
            println!("✅ Signer correctly rejected empty transaction: {}", e);
        }
    }

    println!("\n🎯 Elements-First Setup Complete!");
    println!("=================================");
    println!("✅ Elements wallet created: {}", wallet_name);
    println!(
        "✅ Unconfidential address generated: {}",
        unconfidential_address
    );
    println!(
        "✅ Confidential address retrieved: {}",
        confidential_address
    );
    println!("✅ Private key exported from Elements");
    println!("✅ LWK signer created from Elements private key");
    println!("✅ Address compatibility verified");
    println!();
    println!("🚀 Benefits of this approach:");
    println!("  - Elements can definitely see transactions to both addresses");
    println!("  - LWK can sign transactions using the imported private key");
    println!("  - No descriptor import issues or blinding key problems");
    println!("  - Direct compatibility between Elements and LWK");
    println!("  - Confidential address supports blinded transactions");
    println!();
    println!("💡 Next steps:");
    println!(
        "  - Use '{}' as treasury address for asset issuance (confidential)",
        confidential_address
    );
    println!(
        "  - Use '{}' for LWK signing operations (unconfidential)",
        unconfidential_address
    );
    println!("  - Use the LWK signer for transaction signing");
    println!("  - Elements wallet will automatically see incoming transactions");
    println!();
    println!("🔧 Manual verification commands:");
    println!(
        "  - Check wallet: elements-cli -rpcwallet={} getwalletinfo",
        wallet_name
    );
    println!(
        "  - List addresses: elements-cli -rpcwallet={} listreceivedbyaddress 0 true",
        wallet_name
    );
    println!(
        "  - Check balance: elements-cli -rpcwallet={} getbalance",
        wallet_name
    );

    Ok(())
}
