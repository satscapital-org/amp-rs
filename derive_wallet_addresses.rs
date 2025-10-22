use amp_rs::ElementsRpc;
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();
    env::set_var("AMP_TESTS", "live");

    // Initialize Elements RPC
    let elements_rpc = ElementsRpc::from_env()
        .map_err(|e| format!("Failed to create ElementsRpc from environment: {}", e))?;

    let wallet_name = "amp_elements_wallet_static_for_funding";

    println!("🔍 Deriving addresses from wallet for consistent usage");
    println!("   - Wallet: {}", wallet_name);

    // Check Elements node connectivity
    match elements_rpc.get_network_info().await {
        Ok(network_info) => {
            println!("✅ Connected to Elements node - Version: {}", network_info.version);
        }
        Err(e) => {
            println!("❌ Cannot connect to Elements node: {}", e);
            return Err(e.into());
        }
    }

    // Generate a new address from the wallet
    println!("\n🏠 Generating new address from wallet");
    let unconfidential_address = match elements_rpc.get_new_address(&wallet_name, Some("bech32")).await {
        Ok(address) => {
            println!("✅ Generated unconfidential address: {}", address);
            address
        }
        Err(e) => {
            println!("❌ Failed to generate address: {}", e);
            return Err(format!("Cannot generate address: {}", e).into());
        }
    };

    // Get the confidential version of the address
    println!("\n🔐 Getting confidential version of address");
    let confidential_address = match elements_rpc.get_confidential_address(&wallet_name, &unconfidential_address).await {
        Ok(address) => {
            println!("✅ Generated confidential address: {}", address);
            address
        }
        Err(e) => {
            println!("❌ Failed to get confidential address: {}", e);
            return Err(format!("Cannot get confidential address: {}", e).into());
        }
    };

    // Verify the wallet can see these addresses
    println!("\n🔍 Verifying wallet can see the generated addresses");
    match elements_rpc.list_unspent_for_wallet(&wallet_name, None).await {
        Ok(utxos) => {
            let unconf_utxos = utxos.iter().filter(|u| u.address == unconfidential_address).count();
            let conf_utxos = utxos.iter().filter(|u| u.address == confidential_address).count();
            
            println!("   - UTXOs at unconfidential address: {}", unconf_utxos);
            println!("   - UTXOs at confidential address: {}", conf_utxos);
            
            if unconf_utxos == 0 && conf_utxos == 0 {
                println!("   ⚠️  No UTXOs found at generated addresses (expected for new addresses)");
            }
        }
        Err(e) => {
            println!("   ⚠️  Could not check UTXOs: {}", e);
        }
    }

    println!("\n🎯 ADDRESSES FOR FAUCET FUNDING:");
    println!("=====================================");
    println!("Confidential Address (use for asset issuance):");
    println!("{}", confidential_address);
    println!();
    println!("Unconfidential Address (use for UTXO verification):");
    println!("{}", unconfidential_address);
    println!("=====================================");

    println!("\n📋 Next steps:");
    println!("1. Send tL-BTC to the confidential address using the faucet");
    println!("2. Update both asset creation scripts and tests to use these addresses");
    println!("3. Create assets using the confidential address");
    println!("4. Verify UTXOs using the unconfidential address");

    println!("\n🔧 Code updates needed:");
    println!("Update these constants in your scripts:");
    println!("const TREASURY_CONFIDENTIAL_ADDRESS: &str = \"{}\";", confidential_address);
    println!("const TREASURY_UNCONFIDENTIAL_ADDRESS: &str = \"{}\";", unconfidential_address);

    Ok(())
}