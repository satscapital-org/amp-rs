//! Find Test Assets with Available Keys/UTXOs
//!
//! This example scans all "Test Distribution Asset" entries to find which ones
//! have keys or UTXOs available in the Elements node for distributions.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example find_assets_with_keys
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

    println!("🔍 Find Test Assets with Available Keys/UTXOs");
    println!("==============================================");

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

    // Check Elements RPC availability
    let elements_available = check_elements_rpc_connection().await;

    if !elements_available {
        println!("❌ Elements RPC not available - cannot check for keys/UTXOs");
        println!("   Set ELEMENTS_RPC_URL, ELEMENTS_RPC_USER, and ELEMENTS_RPC_PASSWORD");
        return Ok(());
    }

    println!("✅ Elements RPC connection verified");

    // Get all assets
    println!("\n📋 Getting all assets...");
    let assets = client.get_assets().await?;
    println!("✅ Found {} total assets", assets.len());

    // Filter for "Test Distribution Asset" entries
    let test_assets: Vec<_> = assets
        .iter()
        .filter(|asset| asset.name.contains("Test Distribution Asset"))
        .collect();

    if test_assets.is_empty() {
        println!("\n❌ No 'Test Distribution Asset' entries found");
        return Ok(());
    }

    println!(
        "\n🎯 Found {} 'Test Distribution Asset' entries",
        test_assets.len()
    );
    println!("Checking each for Elements node keys/UTXOs...\n");

    let mut assets_with_keys = Vec::new();
    let mut assets_without_keys = Vec::new();

    for (i, asset) in test_assets.iter().enumerate() {
        println!("{}. Checking: {} ({})", i + 1, asset.name, asset.asset_uuid);

        let utxo_info = check_asset_in_elements(&asset.asset_id, &asset.asset_uuid).await?;

        if utxo_info.has_utxos || utxo_info.in_elements {
            if utxo_info.has_utxos {
                println!(
                    "   ✅ HAS UTXOs: {} UTXOs, {} total amount",
                    utxo_info.utxo_count, utxo_info.total_amount
                );
            } else {
                println!("   ✅ IN ELEMENTS: Asset recognized but no UTXOs");
            }
            assets_with_keys.push((asset, utxo_info));
        } else {
            assets_without_keys.push(asset);
            println!("   ❌ No keys/UTXOs available");
        }

        // Check cleanup status
        let assignments = client
            .get_asset_assignments(&asset.asset_uuid)
            .await
            .unwrap_or_default();
        let distributions = client
            .get_asset_distributions(&asset.asset_uuid)
            .await
            .unwrap_or_default();

        if assignments.is_empty() && distributions.is_empty() {
            println!("   🧹 Clean (no assignments/distributions)");
        } else {
            println!(
                "   🔧 Needs cleanup ({} assignments, {} distributions)",
                assignments.len(),
                distributions.len()
            );
        }

        println!();
    }

    // Summary
    println!("📊 SUMMARY");
    println!("==========");

    if assets_with_keys.is_empty() {
        println!("❌ No test assets found with available keys/UTXOs in Elements node");
        println!(
            "   All {} test assets are missing from Elements or have no UTXOs",
            test_assets.len()
        );
    } else {
        println!(
            "✅ Found {} test assets with keys/UTXOs available:",
            assets_with_keys.len()
        );
        println!();

        for (i, (asset, utxo_info)) in assets_with_keys.iter().enumerate() {
            println!("{}. 🎯 USABLE ASSET:", i + 1);
            println!("   Name: {}", asset.name);
            println!("   UUID: {}", asset.asset_uuid);
            println!("   Asset ID: {}", asset.asset_id);
            println!("   Ticker: {:?}", asset.ticker);

            if utxo_info.has_utxos {
                println!(
                    "   💰 UTXOs: {} available, {} total amount",
                    utxo_info.utxo_count, utxo_info.total_amount
                );
                println!("   🚀 STATUS: READY FOR DISTRIBUTIONS");
            } else {
                println!("   🔍 STATUS: In Elements but no UTXOs (may need issuance)");
            }

            // Check if it needs cleanup
            let assignments = client
                .get_asset_assignments(&asset.asset_uuid)
                .await
                .unwrap_or_default();
            let distributions = client
                .get_asset_distributions(&asset.asset_uuid)
                .await
                .unwrap_or_default();

            if assignments.is_empty() && distributions.is_empty() {
                println!("   ✅ CLEAN: Ready to use immediately");
            } else {
                println!(
                    "   🧹 CLEANUP NEEDED: {} assignments, {} distributions",
                    assignments.len(),
                    distributions.len()
                );
                println!(
                    "      Run cleanup first: edit cancel_test_asset_distribution.rs with UUID {}",
                    asset.asset_uuid
                );
            }
            println!();
        }

        // Provide specific recommendations - check each asset individually
        let mut ready_assets = Vec::new();
        for (asset, utxo_info) in &assets_with_keys {
            if utxo_info.has_utxos {
                let assignments = client
                    .get_asset_assignments(&asset.asset_uuid)
                    .await
                    .unwrap_or_default();
                let distributions = client
                    .get_asset_distributions(&asset.asset_uuid)
                    .await
                    .unwrap_or_default();
                if assignments.is_empty() && distributions.is_empty() {
                    ready_assets.push((asset, utxo_info));
                }
            }
        }

        if !ready_assets.is_empty() {
            println!("🚀 IMMEDIATELY USABLE ASSETS:");
            for (asset, utxo_info) in ready_assets {
                println!("   • {} ({})", asset.name, asset.asset_uuid);
                println!(
                    "     UTXOs: {}, Amount: {}",
                    utxo_info.utxo_count, utxo_info.total_amount
                );
            }
        }
    }

    if !assets_without_keys.is_empty() {
        println!(
            "\n❌ Assets WITHOUT keys/UTXOs ({}):",
            assets_without_keys.len()
        );
        for asset in assets_without_keys.iter().take(5) {
            println!("   • {} ({})", asset.name, asset.asset_uuid);
        }
        if assets_without_keys.len() > 5 {
            println!("   ... and {} more", assets_without_keys.len() - 5);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct UtxoInfo {
    in_elements: bool,
    has_utxos: bool,
    utxo_count: usize,
    total_amount: f64,
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

async fn check_asset_in_elements(
    asset_id: &str,
    asset_uuid: &str,
) -> Result<UtxoInfo, Box<dyn std::error::Error>> {
    let rpc_url = env::var("ELEMENTS_RPC_URL")?;
    let rpc_user = env::var("ELEMENTS_RPC_USER")?;
    let rpc_password = env::var("ELEMENTS_RPC_PASSWORD")?;

    let client = reqwest::Client::new();

    // First check if asset exists in Elements using dumpassetlabels
    let asset_labels_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "dumpassetlabels",
        "params": []
    });

    let mut in_elements = false;

    if let Ok(response) = client
        .post(&rpc_url)
        .basic_auth(&rpc_user, Some(&rpc_password))
        .json(&asset_labels_request)
        .send()
        .await
    {
        if let Ok(result) = response.json::<Value>().await {
            if let Some(result_data) = result.get("result") {
                if let Some(labels) = result_data.as_object() {
                    in_elements = labels.contains_key(asset_id);
                }
            }
        }
    }

    // Check UTXOs regardless of whether asset is in labels (might be unlabeled)
    let utxo_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "listunspent",
        "params": [0, 9999999, [], true, {"asset": asset_id}]
    });

    let mut utxo_count = 0;
    let mut total_amount = 0.0;
    let mut has_utxos = false;

    if let Ok(response) = client
        .post(&rpc_url)
        .basic_auth(&rpc_user, Some(&rpc_password))
        .json(&utxo_request)
        .send()
        .await
    {
        if let Ok(result) = response.json::<Value>().await {
            if let Some(utxos) = result.get("result").and_then(|r| r.as_array()) {
                utxo_count = utxos.len();
                total_amount = utxos
                    .iter()
                    .filter_map(|utxo| utxo.get("amount").and_then(|v| v.as_f64()))
                    .sum();
                has_utxos = utxo_count > 0;

                // If we found UTXOs but asset wasn't in labels, it's still in Elements
                if has_utxos {
                    in_elements = true;
                }
            }
        }
    }

    // Also try using the asset UUID in case it's labeled differently
    if !in_elements && !has_utxos {
        let utxo_request_uuid = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "listunspent",
            "params": [0, 9999999, [], true, {"asset": asset_uuid}]
        });

        if let Ok(response) = client
            .post(&rpc_url)
            .basic_auth(&rpc_user, Some(&rpc_password))
            .json(&utxo_request_uuid)
            .send()
            .await
        {
            if let Ok(result) = response.json::<Value>().await {
                if let Some(utxos) = result.get("result").and_then(|r| r.as_array()) {
                    if !utxos.is_empty() {
                        utxo_count = utxos.len();
                        total_amount = utxos
                            .iter()
                            .filter_map(|utxo| utxo.get("amount").and_then(|v| v.as_f64()))
                            .sum();
                        has_utxos = true;
                        in_elements = true;
                    }
                }
            }
        }
    }

    Ok(UtxoInfo {
        in_elements,
        has_utxos,
        utxo_count,
        total_amount,
    })
}
