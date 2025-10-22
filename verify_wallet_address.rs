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
    let target_confidential_address = "tlq1qq0tdadpf5ua3hfufu9qglaegcl29f57f07qpa9a5zu8j2g0hz99lssjn9qpzz6k5vdu5970fjhpj5v239seaw09ws4adr39um";
    let target_unconfidential_address = "tex1qgffjsq3pdt2xx72zl85etse2x9gjcv7h9gh74t";

    println!("🔍 Verifying wallet contains target address");
    println!("   - Wallet: {}", wallet_name);
    println!("   - Target confidential address: {}", target_confidential_address);
    println!("   - Target unconfidential address: {}", target_unconfidential_address);

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

    // Skip address listing and go directly to UTXO verification
    println!("\n📋 Checking wallet UTXOs for target addresses");

    // Also check UTXOs to see what addresses have funds
    println!("\n💰 Checking UTXOs for address verification");
    match elements_rpc.list_unspent_for_wallet(&wallet_name, None).await {
        Ok(utxos) => {
            println!("✅ Found {} UTXOs in wallet", utxos.len());
            
            let mut confidential_utxos = 0;
            let mut unconfidential_utxos = 0;
            
            for utxo in &utxos {
                if utxo.address == target_confidential_address {
                    confidential_utxos += 1;
                }
                if utxo.address == target_unconfidential_address {
                    unconfidential_utxos += 1;
                }
            }
            
            println!("📊 UTXO address distribution:");
            println!("   - UTXOs at confidential address: {}", confidential_utxos);
            println!("   - UTXOs at unconfidential address: {}", unconfidential_utxos);
            
            if confidential_utxos > 0 || unconfidential_utxos > 0 {
                println!("✅ Wallet contains UTXOs at target addresses");
                
                // Show some example UTXOs
                println!("\n📋 Example UTXOs at target addresses:");
                for (i, utxo) in utxos.iter().enumerate() {
                    if utxo.address == target_confidential_address || utxo.address == target_unconfidential_address {
                        println!("   UTXO {}: {} {} at {}", 
                            i + 1, utxo.amount, &utxo.asset[..8], 
                            if utxo.address == target_confidential_address { "confidential" } else { "unconfidential" });
                        
                        if i >= 5 { // Show max 5 examples
                            break;
                        }
                    }
                }
            } else {
                println!("❌ No UTXOs found at target addresses");
                
                // Show what addresses do have UTXOs
                println!("\n📋 Addresses that do have UTXOs:");
                let mut unique_addresses: std::collections::HashSet<String> = std::collections::HashSet::new();
                for utxo in &utxos {
                    unique_addresses.insert(utxo.address.clone());
                }
                
                for (i, addr) in unique_addresses.iter().enumerate() {
                    println!("   Address {}: {}", i + 1, addr);
                    if i >= 10 { // Show max 10 addresses
                        println!("   ... and {} more addresses", unique_addresses.len() - 10);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to list UTXOs: {}", e);
            return Err(format!("Cannot list wallet UTXOs: {}", e).into());
        }
    }

    println!("\n🎯 Wallet address verification completed");
    
    Ok(())
}