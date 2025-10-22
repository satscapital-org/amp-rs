use amp_rs::{ApiClient, ElementsRpc};
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();
    env::set_var("AMP_TESTS", "live");

    // Initialize API client and Elements RPC
    let api_client = ApiClient::new().await?;
    let elements_rpc = ElementsRpc::from_env()
        .map_err(|e| format!("Failed to create ElementsRpc from environment: {}", e))?;

    // Treasury addresses - confidential required for issuance by AMP API, unconfidential for registration
    let treasury_confidential_address = "tlq1qq03w9tlxve4z5ypnqhq7m5ak3uwhxk637xjuyrqjt6u9u0qkzmmqfa9rrddjfeqemah3wpsk3lw76vz2xy3azq3xq9x5ptswl";
    let treasury_unconfidential_address = "tex1q7j33kkeyusva7mchqctglh0dxp9rzg73x07uxt";

    // Create asset with maximum circulation
    let timestamp = chrono::Utc::now().timestamp();
    let asset_name = format!("Test Distribution Asset {}", timestamp);
    let asset_ticker = format!("TDA{}", timestamp % 10000);

    println!("🚀 Creating new test asset");
    println!("   - Name: {}", asset_name);
    println!("   - Ticker: {}", asset_ticker);
    println!("   - Treasury Confidential Address (for issuance): {}", treasury_confidential_address);
    println!("   - Treasury Unconfidential Address (for registration): {}", treasury_unconfidential_address);
    println!("   - Max Circulation: 21,000,000 (with 8 decimal places)");

    let issuance_request = amp_rs::model::IssuanceRequest {
        name: asset_name.clone(),
        ticker: asset_ticker.clone(),
        domain: "test-distribution.example.com".to_string(),
        precision: Some(8),
        amount: 21_000_000_00_000_000, // 21M with 8 decimal places
        destination_address: treasury_confidential_address.to_string(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
        is_confidential: Some(true),
        is_reissuable: Some(false),
        reissuance_amount: None,
        reissuance_address: None,
        transfer_restricted: Some(false),
    };

    match api_client.issue_asset(&issuance_request).await {
        Ok(response) => {
            println!("✅ Asset created successfully!");
            println!("   - Asset UUID: {}", response.asset_uuid);
            println!("   - Transaction ID: {}", response.txid);
            println!("   - Circulation: 21,000,000.00000000 tokens");

            // Add treasury address to the asset
            println!("\n🏦 Adding treasury address to asset");
            match api_client.add_asset_treasury_addresses(&response.asset_uuid, &vec![treasury_confidential_address.to_string()]).await {
                Ok(_) => {
                    println!("✅ Treasury address added successfully");
                }
                Err(e) => {
                    println!("⚠️  Treasury address addition: {} (may already exist)", e);
                }
            }

            // Wait for blockchain confirmation before attempting authorization
            println!("\n⏳ Waiting for blockchain confirmation of asset issuance");
            println!("   Transaction ID: {}", response.txid);
            println!("   Checking blockchain directly for confirmation...");

            // First, verify Elements RPC connectivity
            match elements_rpc.get_network_info().await {
                Ok(network_info) => {
                    println!("✅ Connected to Elements node - Version: {}", network_info.version);
                }
                Err(e) => {
                    println!("❌ Cannot connect to Elements node: {}", e);
                    return Err(format!("Elements RPC not available for blockchain confirmation: {}", e).into());
                }
            }

            // Verify that the treasury address is in the wallet we're using
            let wallet_name = "amp_elements_wallet_static_for_funding";
            println!("🔍 Verifying treasury address is in wallet: {}", wallet_name);
            
            // Check if we can see the treasury address in the wallet
            match elements_rpc.list_unspent_for_wallet(wallet_name, None).await {
                Ok(existing_utxos) => {
                    let treasury_utxos: Vec<_> = existing_utxos.iter()
                        .filter(|utxo| utxo.address == treasury_unconfidential_address)
                        .collect();
                    
                    if treasury_utxos.is_empty() {
                        println!("⚠️  Treasury address {} not found in wallet {}", treasury_unconfidential_address, wallet_name);
                        println!("   This means the wallet cannot see transactions to this address");
                        println!("   Available addresses in wallet:");
                        
                        let mut unique_addresses: std::collections::HashSet<String> = std::collections::HashSet::new();
                        for utxo in &existing_utxos {
                            unique_addresses.insert(utxo.address.clone());
                        }
                        
                        for (i, addr) in unique_addresses.iter().take(5).enumerate() {
                            println!("     - {}", addr);
                        }
                        if unique_addresses.len() > 5 {
                            println!("     ... and {} more addresses", unique_addresses.len() - 5);
                        }
                        
                        println!("   ⚠️  WARNING: We won't be able to detect confirmations for this asset");
                        println!("   The asset was issued to: {}", treasury_confidential_address);
                        println!("   But wallet can only see: {}", treasury_unconfidential_address);
                        println!("   Proceeding anyway, but confirmation detection may fail...");
                    } else {
                        println!("✅ Treasury address found in wallet with {} existing UTXOs", treasury_utxos.len());
                        for (i, utxo) in treasury_utxos.iter().take(3).enumerate() {
                            println!("   Existing UTXO {}: {} {} (confirmations: {})", 
                                i + 1, utxo.amount, &utxo.asset[..8], utxo.confirmations.unwrap_or(0));
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Cannot check wallet contents: {}", e);
                    println!("   This may indicate wallet connectivity issues");
                    println!("   Proceeding anyway, but confirmation detection will likely fail...");
                }
            }

            // Wait for blockchain confirmation - check every 30 seconds for up to 5 minutes
            let mut confirmation_attempts = 0;
            let max_confirmation_attempts = 10; // 10 attempts * 30 seconds = 5 minutes max wait
            let mut blockchain_confirmed = false;
            let check_interval_secs = 30;

            while confirmation_attempts < max_confirmation_attempts {
                tokio::time::sleep(tokio::time::Duration::from_secs(check_interval_secs)).await;
                confirmation_attempts += 1;

                println!("   Checking blockchain confirmation (attempt {}/{})...", confirmation_attempts, max_confirmation_attempts);

                // Try to get the transaction from the blockchain to check confirmations
                // First try wallet-based approach, then fall back to other methods
                match elements_rpc.list_unspent_for_wallet("amp_elements_wallet_static_for_funding", None).await {
                    Ok(utxos) => {
                        // Look for UTXOs that match our asset issuance
                        // The transaction should create UTXOs at our treasury address
                        let treasury_utxos: Vec<_> = utxos.iter()
                            .filter(|utxo| utxo.address == treasury_unconfidential_address)
                            .collect();

                        if !treasury_utxos.is_empty() {
                            // Check if any UTXO has sufficient confirmations (at least 1)
                            let confirmed_utxos: Vec<_> = treasury_utxos.iter()
                                .filter(|utxo| utxo.confirmations.unwrap_or(0) >= 1)
                                .collect();

                            if !confirmed_utxos.is_empty() {
                                println!("✅ Transaction confirmed on blockchain!");
                                println!("   - Found {} confirmed UTXOs at treasury address", confirmed_utxos.len());
                                for (i, utxo) in confirmed_utxos.iter().take(3).enumerate() {
                                    println!("     UTXO {}: {} confirmations, amount: {}", 
                                        i + 1, utxo.confirmations.unwrap_or(0), utxo.amount);
                                }
                                blockchain_confirmed = true;
                                break;
                            } else {
                                println!("   Found {} UTXOs at treasury address but none confirmed yet", treasury_utxos.len());
                            }
                        } else {
                            println!("   No UTXOs found at treasury address yet, transaction may still be pending");
                        }

                        // Also check blockchain height to ensure we're making progress
                        match elements_rpc.get_blockchain_info().await {
                            Ok(blockchain_info) => {
                                println!("   Current blockchain height: {}", blockchain_info.blocks);
                            }
                            Err(e) => {
                                println!("   Could not get blockchain info: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("   Cannot check wallet UTXOs: {}", e);

                        // Fallback: just check if blockchain is progressing
                        match elements_rpc.get_blockchain_info().await {
                            Ok(blockchain_info) => {
                                println!("   Current blockchain height: {}", blockchain_info.blocks);
                                println!("   Cannot verify transaction directly, but blockchain is active");
                            }
                            Err(e2) => {
                                println!("   Cannot check blockchain info either: {}", e2);
                                return Err(format!("Cannot verify blockchain status: {}", e2).into());
                            }
                        }
                    }
                }

                if confirmation_attempts < max_confirmation_attempts {
                    println!("   Waiting {} seconds before next check...", check_interval_secs);
                }
            }

            if !blockchain_confirmed {
                println!("❌ Blockchain confirmation timed out after {} attempts ({} minutes)", 
                    max_confirmation_attempts, (max_confirmation_attempts * check_interval_secs) / 60);
                println!("   The transaction may still be pending or there may be network delays");
                println!("   Transaction ID: {}", response.txid);
                return Err("Blockchain confirmation timeout - no confirmed UTXOs found".into());
            }

            // Now that blockchain is confirmed, try to register asset as authorized
            println!("\n🔐 Registering asset as authorized for distribution");
            let mut authorization_attempts = 0;
            let max_authorization_attempts = 5;
            let mut asset_authorized = false;

            while authorization_attempts < max_authorization_attempts {
                authorization_attempts += 1;

                match api_client.register_asset_authorized(&response.asset_uuid).await {
                    Ok(authorized_asset) => {
                        println!("✅ Asset authorized for distribution!");
                        println!("   - Is Authorized: {}", authorized_asset.is_authorized);
                        asset_authorized = true;
                        break;
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        if error_msg.contains("already authorized") {
                            println!("✅ Asset already authorized for distribution");
                            asset_authorized = true;
                            break;
                        } else if authorization_attempts < max_authorization_attempts {
                            println!("   Authorization attempt {}/{} failed: {}", authorization_attempts, max_authorization_attempts, error_msg);
                            println!("   Waiting 10 seconds before retry...");
                            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                        } else {
                            println!("❌ Asset authorization failed after {} attempts: {}", max_authorization_attempts, e);
                        }
                    }
                }
            }

            if blockchain_confirmed && asset_authorized {
                println!("\n🎯 New asset ready for testing!");
                println!("   ✅ Asset UUID: {}", response.asset_uuid);
                println!("   ✅ Asset Name: {}", asset_name);
                println!("   ✅ Asset Ticker: {}", asset_ticker);
                println!("   ✅ Treasury Confidential Address: {}", treasury_confidential_address);
                println!("   ✅ Treasury Unconfidential Address: {}", treasury_unconfidential_address);
                println!("   ✅ Max Circulation: 21,000,000.00000000 tokens");
                println!("   ✅ Blockchain Confirmed: Yes");
                println!("   ✅ Authorized for Distribution: Yes");
                println!("\n🔄 You can now update the test to use this asset UUID:");
                println!("   {}", response.asset_uuid);
            } else {
                println!("\n❌ Asset creation incomplete!");
                if !blockchain_confirmed {
                    println!("   The asset was issued but blockchain confirmation timed out.");
                }
                if !asset_authorized {
                    println!("   The asset was confirmed but authorization failed.");
                }
                println!("   Asset UUID: {}", response.asset_uuid);
                println!("   Transaction ID: {}", response.txid);
                println!("   You may need to wait longer and manually authorize the asset.");
                return Err("Asset not fully confirmed and authorized".into());
            }
        }
        Err(e) => {
            println!("❌ Failed to create asset: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
