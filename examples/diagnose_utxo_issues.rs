//! UTXO Issue Diagnostic Tool
//!
//! This example diagnoses why specific assets have no UTXOs available by checking:
//! 1. Treasury address import status in Elements node
//! 2. Asset issuance transaction confirmation
//! 3. Previous UTXO spending history
//! 4. Elements node sync status
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example diagnose_utxo_issues
//! ```
//!
//! ## Environment Variables
//!
//! This example uses dotenvy to load environment variables from .env:
//! - `AMP_USERNAME`: AMP API username
//! - `AMP_PASSWORD`: AMP API password
//! - `ELEMENTS_RPC_URL`: Elements RPC endpoint
//! - `ELEMENTS_RPC_USER`: Elements RPC username
//! - `ELEMENTS_RPC_PASSWORD`: Elements RPC password

use amp_rs::ApiClient;
use dotenvy;
use serde_json::Value;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🔍 UTXO Issue Diagnostic Tool");
    println!("=============================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();

    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!(
        "✅ Connected to AMP API with {} strategy",
        client.get_strategy_type()
    );

    // Assets to diagnose
    let assets_to_check = vec![
        "93cffcb9-c1f5-4873-b5dc-f3ba1f29e3c2", // Previously cleaned asset
        "7750f273-53a9-4984-ad18-d38dd4435207", // Recently checked asset
    ];

    // Check Elements RPC availability
    let elements_available = check_elements_rpc_connection().await;

    if !elements_available {
        println!("❌ Elements RPC not available - limited diagnostics possible");
        println!("   Set ELEMENTS_RPC_URL, ELEMENTS_RPC_USER, and ELEMENTS_RPC_PASSWORD");
        return Ok(());
    }

    println!("\n🔍 Diagnosing {} assets...\n", assets_to_check.len());

    for (i, asset_uuid) in assets_to_check.iter().enumerate() {
        println!("{}. Asset: {}", i + 1, asset_uuid);
        println!("{}", "=".repeat(80));

        diagnose_asset(&client, asset_uuid).await?;

        if i < assets_to_check.len() - 1 {
            println!("\n");
        }
    }

    Ok(())
}

async fn check_elements_rpc_connection() -> bool {
    let rpc_url = env::var("ELEMENTS_RPC_URL");
    let rpc_user = env::var("ELEMENTS_RPC_USER");
    let rpc_password = env::var("ELEMENTS_RPC_PASSWORD");

    if rpc_url.is_err() || rpc_user.is_err() || rpc_password.is_err() {
        return false;
    }

    let client = reqwest::Client::new();
    let blockchain_info_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getblockchaininfo",
        "params": []
    });

    match client
        .post(rpc_url.unwrap())
        .basic_auth(rpc_user.unwrap(), Some(rpc_password.unwrap()))
        .json(&blockchain_info_request)
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

async fn diagnose_asset(
    client: &ApiClient,
    asset_uuid: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get asset details
    let asset = match client.get_asset(asset_uuid).await {
        Ok(asset) => {
            println!("✅ Asset Details:");
            println!("   Name: {}", asset.name);
            println!("   Asset ID: {}", asset.asset_id);
            println!("   Ticker: {:?}", asset.ticker);
            println!("   Issuer: {}", asset.issuer);
            println!("   Is Registered: {}", asset.is_registered);
            asset
        }
        Err(e) => {
            println!("❌ Failed to get asset details: {}", e);
            return Ok(());
        }
    };

    // Check Elements RPC diagnostics
    println!("\n🔧 Elements RPC Diagnostics:");

    let rpc_url = env::var("ELEMENTS_RPC_URL")?;
    let rpc_user = env::var("ELEMENTS_RPC_USER")?;
    let rpc_password = env::var("ELEMENTS_RPC_PASSWORD")?;

    let rpc_client = reqwest::Client::new();

    // 1. Check blockchain sync status
    println!("   📡 Checking blockchain sync status...");
    let sync_status =
        check_blockchain_sync(&rpc_client, &rpc_url, &rpc_user, &rpc_password).await?;

    // 2. Check if asset exists in Elements
    println!("   🔍 Checking asset existence in Elements...");
    let asset_exists = check_asset_in_elements(
        &rpc_client,
        &rpc_url,
        &rpc_user,
        &rpc_password,
        &asset.asset_id,
    )
    .await?;

    // 3. Check UTXOs for this asset
    println!("   💰 Checking UTXOs for asset...");
    let utxo_info = check_asset_utxos(
        &rpc_client,
        &rpc_url,
        &rpc_user,
        &rpc_password,
        &asset.asset_id,
    )
    .await?;

    // 4. Check issuance transaction
    println!("   📋 Checking issuance transaction...");
    let issuance_info = check_issuance_transaction(
        &rpc_client,
        &rpc_url,
        &rpc_user,
        &rpc_password,
        &asset.asset_id,
    )
    .await?;

    // 5. Check treasury address import status
    println!("   🏦 Checking treasury address status...");
    let treasury_info =
        check_treasury_address(&rpc_client, &rpc_url, &rpc_user, &rpc_password).await?;

    // Analyze and provide diagnosis
    println!("\n📊 Diagnosis:");
    analyze_findings(
        sync_status,
        asset_exists,
        utxo_info,
        issuance_info,
        treasury_info,
    );

    Ok(())
}

async fn check_blockchain_sync(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    password: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getblockchaininfo",
        "params": []
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    if let Ok(result) = response.json::<Value>().await {
        if let Some(result_data) = result.get("result") {
            let blocks = result_data
                .get("blocks")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let headers = result_data
                .get("headers")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let progress = result_data
                .get("verificationprogress")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            println!(
                "      Blocks: {}, Headers: {}, Progress: {:.2}%",
                blocks,
                headers,
                progress * 100.0
            );

            let is_synced = blocks == headers && progress > 0.99;
            if is_synced {
                println!("      ✅ Node is fully synced");
            } else {
                println!("      ⚠️  Node is not fully synced");
            }
            return Ok(is_synced);
        }
    }

    println!("      ❌ Failed to get blockchain info");
    Ok(false)
}

async fn check_asset_in_elements(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    password: &str,
    asset_id: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Try to get asset info using dumpassetlabels
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "dumpassetlabels",
        "params": []
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    if let Ok(result) = response.json::<Value>().await {
        if let Some(result_data) = result.get("result") {
            if let Some(labels) = result_data.as_object() {
                let asset_found = labels.contains_key(asset_id);
                if asset_found {
                    println!("      ✅ Asset found in Elements node");
                    if let Some(label) = labels.get(asset_id) {
                        println!("      Label: {}", label);
                    }
                } else {
                    println!("      ❌ Asset not found in Elements node");
                }
                return Ok(asset_found);
            }
        }
    }

    println!("      ❌ Failed to check asset labels");
    Ok(false)
}

async fn check_asset_utxos(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    password: &str,
    asset_id: &str,
) -> Result<(usize, f64), Box<dyn std::error::Error>> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "listunspent",
        "params": [0, 9999999, [], true, {"asset": asset_id}]
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    if let Ok(result) = response.json::<Value>().await {
        if let Some(utxos) = result.get("result").and_then(|r| r.as_array()) {
            let count = utxos.len();
            let total_amount: f64 = utxos
                .iter()
                .filter_map(|utxo| utxo.get("amount").and_then(|v| v.as_f64()))
                .sum();

            if count > 0 {
                println!(
                    "      ✅ Found {} UTXOs with total amount: {}",
                    count, total_amount
                );

                // Show first few UTXOs for debugging
                for (i, utxo) in utxos.iter().take(3).enumerate() {
                    if let (Some(txid), Some(vout), Some(amount)) = (
                        utxo.get("txid").and_then(|v| v.as_str()),
                        utxo.get("vout").and_then(|v| v.as_u64()),
                        utxo.get("amount").and_then(|v| v.as_f64()),
                    ) {
                        println!("         {}. {}:{} - {}", i + 1, txid, vout, amount);
                    }
                }
                if utxos.len() > 3 {
                    println!("         ... and {} more", utxos.len() - 3);
                }
            } else {
                println!("      ❌ No UTXOs found for this asset");
            }

            return Ok((count, total_amount));
        }
    }

    println!("      ❌ Failed to list unspent outputs");
    Ok((0, 0.0))
}

async fn check_issuance_transaction(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    password: &str,
    asset_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Try to find the issuance transaction by looking for transactions that created this asset
    // This is complex as we need to search through transactions

    // For now, let's try to get the asset registry info
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "listissuances",
        "params": [asset_id]
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    if let Ok(result) = response.json::<Value>().await {
        if let Some(issuances) = result.get("result").and_then(|r| r.as_array()) {
            if !issuances.is_empty() {
                println!(
                    "      ✅ Found {} issuance(s) for this asset",
                    issuances.len()
                );

                for (i, issuance) in issuances.iter().enumerate() {
                    if let Some(txid) = issuance.get("txid").and_then(|v| v.as_str()) {
                        println!("         {}. Issuance TXID: {}", i + 1, txid);

                        // Check if this transaction is confirmed
                        if let Ok(confirmations) =
                            check_transaction_confirmations(client, url, user, password, txid).await
                        {
                            if confirmations > 0 {
                                println!(
                                    "            ✅ Confirmed ({} confirmations)",
                                    confirmations
                                );
                            } else {
                                println!("            ⚠️  Unconfirmed");
                            }
                        }

                        return Ok(Some(txid.to_string()));
                    }
                }
            } else {
                println!("      ❌ No issuances found for this asset");
            }
        }
    } else {
        println!("      ❌ Failed to list issuances (method may not be available)");
    }

    Ok(None)
}

async fn check_transaction_confirmations(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    password: &str,
    txid: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "gettransaction",
        "params": [txid]
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    if let Ok(result) = response.json::<Value>().await {
        if let Some(tx_data) = result.get("result") {
            if let Some(confirmations) = tx_data.get("confirmations").and_then(|v| v.as_u64()) {
                return Ok(confirmations);
            }
        }
    }

    Ok(0)
}

async fn check_treasury_address(
    client: &reqwest::Client,
    url: &str,
    user: &str,
    password: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // List all addresses in the wallet to see if treasury addresses are imported
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "listaddressgroupings",
        "params": []
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    let mut addresses = Vec::new();

    if let Ok(result) = response.json::<Value>().await {
        if let Some(groupings) = result.get("result").and_then(|r| r.as_array()) {
            for group in groupings {
                if let Some(group_array) = group.as_array() {
                    for addr_info in group_array {
                        if let Some(addr_array) = addr_info.as_array() {
                            if let Some(address) = addr_array.get(0).and_then(|v| v.as_str()) {
                                addresses.push(address.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    println!("      Found {} addresses in wallet", addresses.len());

    // Also check watch-only addresses
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "listreceivedbyaddress",
        "params": [0, true, true]
    });

    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .json(&request)
        .send()
        .await?;

    let mut watch_only_count = 0;
    if let Ok(result) = response.json::<Value>().await {
        if let Some(received) = result.get("result").and_then(|r| r.as_array()) {
            watch_only_count = received.len();
        }
    }

    println!(
        "      Found {} addresses with received transactions (including watch-only)",
        watch_only_count
    );

    Ok(addresses)
}

fn analyze_findings(
    sync_status: bool,
    asset_exists: bool,
    utxo_info: (usize, f64),
    issuance_info: Option<String>,
    _treasury_info: Vec<String>,
) {
    let (utxo_count, utxo_amount) = utxo_info;

    println!("   🎯 Root Cause Analysis:");

    if !sync_status {
        println!("      🔴 CAUSE 4: Elements node not synced or missing transaction data");
        println!("         The Elements node is not fully synchronized with the network.");
        println!("         This could cause UTXOs to not be visible even if they exist.");
        println!("         SOLUTION: Wait for full synchronization or restart Elements node.");
    } else {
        println!("      ✅ Elements node is fully synced");
    }

    if !asset_exists {
        println!("      🔴 CAUSE 2: Asset issuance transaction not confirmed yet");
        println!("         The asset is not recognized by the Elements node.");
        println!("         This suggests the issuance transaction hasn't been processed.");
        println!("         SOLUTION: Check if issuance transaction is confirmed and visible.");
    } else {
        println!("      ✅ Asset exists in Elements node");
    }

    if utxo_count == 0 {
        if asset_exists && sync_status {
            if issuance_info.is_some() {
                println!("      🔴 CAUSE 3: UTXOs already spent in previous distributions");
                println!("         Asset exists and is issued, but no UTXOs remain.");
                println!("         All UTXOs may have been spent in previous distributions.");
                println!("         SOLUTION: Issue more of this asset or use a different asset.");
            } else {
                println!("      🔴 CAUSE 1: Treasury address not imported as watch-only in Elements node");
                println!("         Asset exists but no UTXOs are visible to this wallet.");
                println!("         The treasury address may not be imported as watch-only.");
                println!(
                    "         SOLUTION: Import treasury address with 'importaddress' command."
                );
            }
        }
    } else {
        println!(
            "      ✅ Asset has {} UTXOs with total amount: {}",
            utxo_count, utxo_amount
        );
        println!("      🤔 This is unexpected - the asset should be usable for distributions!");
    }

    println!("\n   📋 Summary:");
    if utxo_count > 0 {
        println!("      ✅ Asset appears ready for distributions");
    } else {
        println!("      ❌ Asset cannot be used for distributions until UTXO issues are resolved");
    }
}
