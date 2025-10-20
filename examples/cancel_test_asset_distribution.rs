//! Cancel Active Distribution Example
//!
//! This example demonstrates how to cancel an active (unconfirmed) distribution
//! for a specific test asset. It targets the asset used in the end-to-end
//! distribution workflow test.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example cancel_test_asset_distribution
//! ```
//!
//! ## Environment Variables
//!
//! This example uses dotenvy to load environment variables from .env:
//! - `AMP_USERNAME`: AMP API username
//! - `AMP_PASSWORD`: AMP API password

use amp_rs::{ApiClient, model::Status};
use dotenvy;
use std::env;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🚀 Cancel Test Asset Distribution Example");
    println!("==========================================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();
    
    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!("✅ Connected to AMP API with {} strategy", client.get_strategy_type());

    // Target the asset with UTXOs that we found
    let asset_uuid = "b9fc7bfc-b58f-4e1c-8299-e1d5d353f12d";
    println!("\n🎯 Targeting test asset: {}", asset_uuid);

    // Get all distributions for this asset
    println!("📋 Getting distributions for asset...");
    let distributions = client.get_asset_distributions(asset_uuid).await?;
    
    println!("✅ Found {} distributions for asset", distributions.len());

    // Also check assignments to understand the full picture
    println!("📋 Getting assignments for asset...");
    let assignments = client.get_asset_assignments(asset_uuid).await?;
    
    println!("✅ Found {} assignments for asset", assignments.len());

    if distributions.is_empty() && assignments.is_empty() {
        println!("ℹ️  No distributions or assignments found for this asset");
        println!("   This means the asset is completely clean and available for new distributions.");
        
        // Still run UTXO analysis for the test asset
        analyze_asset_utxos(&client, asset_uuid).await?;
        return Ok(());
    }

    if !assignments.is_empty() {
        println!("\n📊 Assignment Status:");
        for (i, assignment) in assignments.iter().enumerate() {
            let distribution_status = match &assignment.distribution_uuid {
                Some(uuid) => format!("🔗 Linked to distribution: {}", uuid),
                None => "🆓 Not linked to any distribution".to_string(),
            };
            println!("  {}. Assignment ID: {} - User: {} - Amount: {} - Ready: {} - Distributed: {}", 
                i + 1, 
                assignment.id,
                assignment.registered_user,
                assignment.amount,
                assignment.ready_for_distribution,
                assignment.is_distributed
            );
            println!("      {}", distribution_status);
        }
    }

    // Display all distributions
    println!("\n📊 Distribution Status:");
    for (i, distribution) in distributions.iter().enumerate() {
        let status_str = match distribution.distribution_status {
            Status::Unconfirmed => "🟡 UNCONFIRMED",
            Status::Confirmed => "🟢 CONFIRMED",
        };
        println!("  {}. {} - {} (Transactions: {})", 
            i + 1, 
            distribution.distribution_uuid, 
            status_str,
            distribution.transactions.len()
        );
    }

    // Find unconfirmed distributions to cancel
    let unconfirmed_distributions: Vec<_> = distributions.iter()
        .filter(|d| matches!(d.distribution_status, Status::Unconfirmed))
        .collect();

    // Also find distribution UUIDs from assignments that are linked to distributions
    let assignment_distribution_uuids: Vec<String> = assignments.iter()
        .filter_map(|a| a.distribution_uuid.clone())
        .collect::<std::collections::HashSet<_>>() // Remove duplicates
        .into_iter()
        .collect();

    if unconfirmed_distributions.is_empty() && assignment_distribution_uuids.is_empty() {
        println!("\n✅ No unconfirmed distributions found");
        
        // But we might still have assignments to clean up
        let assignments_to_delete: Vec<_> = assignments.iter()
            .filter(|a| {
                let should_delete = a.distribution_uuid.is_some() || 
                                   (a.ready_for_distribution && !a.is_distributed) ||
                                   !a.is_distributed;
                should_delete
            })
            .collect();
            
        if assignments_to_delete.is_empty() {
            println!("   All distributions are either confirmed or the asset is clean.");
            return Ok(());
        } else {
            println!("   But found {} assignments that need cleanup", assignments_to_delete.len());
        }
    }

    if !unconfirmed_distributions.is_empty() {
        println!("\n🔍 Found {} unconfirmed distribution(s) from distribution list:", unconfirmed_distributions.len());
        for distribution in &unconfirmed_distributions {
            println!("  - {}", distribution.distribution_uuid);
        }
    }

    if !assignment_distribution_uuids.is_empty() {
        println!("\n🔍 Found {} distribution(s) linked to assignments:", assignment_distribution_uuids.len());
        for uuid in &assignment_distribution_uuids {
            println!("  - {}", uuid);
        }
    }

    // Collect all distribution UUIDs to cancel (from both sources)
    let mut all_distribution_uuids = Vec::new();
    
    // Add unconfirmed distributions
    for distribution in unconfirmed_distributions {
        all_distribution_uuids.push(distribution.distribution_uuid.clone());
    }
    
    // Add distributions from assignments (if not already included)
    for uuid in assignment_distribution_uuids {
        if !all_distribution_uuids.contains(&uuid) {
            all_distribution_uuids.push(uuid);
        }
    }

    // Cancel each distribution
    println!("\n🗑️  Cancelling distributions...");
    let mut cancelled_count = 0;
    let mut failed_count = 0;

    for distribution_uuid in all_distribution_uuids {
        print!("  Cancelling {}... ", distribution_uuid);
        
        match client.cancel_distribution(asset_uuid, &distribution_uuid).await {
            Ok(()) => {
                println!("✅ Success");
                cancelled_count += 1;
            }
            Err(e) => {
                println!("❌ Failed: {}", e);
                failed_count += 1;
            }
        }
    }

    // Delete assignments that were linked to distributions or are ready for distribution
    println!("\n🗑️  Deleting assignments...");
    let mut assignments_deleted = 0;
    let mut assignment_delete_failed = 0;

    for assignment in assignments {
        // Delete assignments that are either:
        // 1. Linked to distributions (distribution_uuid is Some)
        // 2. Ready for distribution but not yet distributed
        // 3. Not distributed (covers test assignments)
        let should_delete = assignment.distribution_uuid.is_some() || 
                           (assignment.ready_for_distribution && !assignment.is_distributed) ||
                           !assignment.is_distributed;
        
        if should_delete {
            print!("  Deleting assignment {}... ", assignment.id);
            
            match client.delete_asset_assignment(asset_uuid, &assignment.id.to_string()).await {
                Ok(()) => {
                    println!("✅ Success");
                    assignments_deleted += 1;
                }
                Err(e) => {
                    println!("❌ Failed: {}", e);
                    assignment_delete_failed += 1;
                }
            }
        } else {
            println!("  Skipping assignment {} (already distributed and not linked)", assignment.id);
        }
    }

    // Summary
    println!("\n📈 Summary:");
    println!("  ✅ Distributions cancelled: {}", cancelled_count);
    if failed_count > 0 {
        println!("  ❌ Distribution cancellations failed: {}", failed_count);
    }
    println!("  ✅ Assignments deleted: {}", assignments_deleted);
    if assignment_delete_failed > 0 {
        println!("  ❌ Assignment deletions failed: {}", assignment_delete_failed);
    }
    println!("  📊 Total operations: {}", cancelled_count + failed_count + assignments_deleted + assignment_delete_failed);

    // Verify the cleanup worked by checking again
    if cancelled_count > 0 || assignments_deleted > 0 {
        println!("\n🔍 Verifying cleanup...");
        
        // Check distributions again
        let final_distributions = client.get_asset_distributions(asset_uuid).await?;
        println!("📋 Distributions remaining: {}", final_distributions.len());
        
        // Check assignments again  
        let final_assignments = client.get_asset_assignments(asset_uuid).await?;
        println!("📋 Assignments remaining: {}", final_assignments.len());
        
        if final_distributions.is_empty() && final_assignments.is_empty() {
            println!("\n🎉 Cleanup verified successful!");
            println!("   The test asset is completely clean and available for new distributions.");
        } else {
            println!("\n⚠️  Cleanup incomplete:");
            if !final_distributions.is_empty() {
                println!("   - {} distributions still exist", final_distributions.len());
                for dist in &final_distributions {
                    println!("     • {} ({})", dist.distribution_uuid, 
                        match dist.distribution_status {
                            Status::Unconfirmed => "UNCONFIRMED",
                            Status::Confirmed => "CONFIRMED",
                        });
                }
            }
            if !final_assignments.is_empty() {
                println!("   - {} assignments still exist", final_assignments.len());
                for assignment in &final_assignments {
                    println!("     • ID {} - User {} - Amount {} - Ready: {} - Distributed: {}", 
                        assignment.id,
                        assignment.registered_user,
                        assignment.amount,
                        assignment.ready_for_distribution,
                        assignment.is_distributed
                    );
                }
            }
        }
    } else if failed_count > 0 || assignment_delete_failed > 0 {
        println!("\n⚠️  Some operations failed.");
        println!("   Check the error messages above for details.");
    }

    // Analyze UTXOs for the cleaned asset
    analyze_asset_utxos(&client, asset_uuid).await?;

    Ok(())
}

/// Analyze UTXOs and outputs for a specific asset to diagnose distribution issues
async fn analyze_asset_utxos(client: &ApiClient, asset_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 UTXO Analysis for Asset: {}", asset_id);
    println!("{}", "=".repeat(80));
    
    // First, get the asset details
    println!("📋 Getting asset information...");
    match client.get_asset(asset_id).await {
        Ok(asset) => {
            println!("✅ Asset found:");
            println!("   Name: {}", asset.name);
            println!("   UUID: {}", asset.asset_uuid);
            println!("   Asset ID: {}", asset.asset_id);
            println!("   Issuer: {}", asset.issuer);
            println!("   Precision: {}", asset.precision);
            println!("   Ticker: {:?}", asset.ticker);
            println!("   Domain: {:?}", asset.domain);
            println!("   Is Registered: {}", asset.is_registered);
        }
        Err(e) => {
            println!("❌ Failed to get asset details: {}", e);
            println!("   This asset may not be registered in AMP or the asset ID may be incorrect.");
            println!("   Continuing with Elements RPC analysis...");
        }
    }

    // Get asset assignments to understand distribution state (only if asset exists in AMP)
    println!("\n📋 Getting asset assignments...");
    match client.get_asset_assignments(asset_id).await {
        Ok(assignments) => {
            println!("✅ Found {} assignments", assignments.len());
            for (i, assignment) in assignments.iter().enumerate() {
                println!("   {}. ID: {} - User: {} - Amount: {} - Ready: {} - Distributed: {}", 
                    i + 1,
                    assignment.id,
                    assignment.registered_user,
                    assignment.amount,
                    assignment.ready_for_distribution,
                    assignment.is_distributed
                );
                if let Some(dist_uuid) = &assignment.distribution_uuid {
                    println!("      🔗 Linked to distribution: {}", dist_uuid);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get assignments: {}", e);
            println!("   This is expected if the asset is not registered in AMP.");
        }
    }

    // Get asset distributions
    println!("\n📋 Getting asset distributions...");
    match client.get_asset_distributions(asset_id).await {
        Ok(distributions) => {
            println!("✅ Found {} distributions", distributions.len());
            for (i, distribution) in distributions.iter().enumerate() {
                let status_str = match distribution.distribution_status {
                    Status::Unconfirmed => "🟡 UNCONFIRMED",
                    Status::Confirmed => "🟢 CONFIRMED",
                };
                println!("   {}. {} - {} (Transactions: {})", 
                    i + 1, 
                    distribution.distribution_uuid, 
                    status_str,
                    distribution.transactions.len()
                );
                
                // Show transaction details
                for (j, tx) in distribution.transactions.iter().enumerate() {
                    println!("      Tx {}: {} (Status: {:?})", j + 1, tx.txid, tx.transaction_status);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get distributions: {}", e);
            println!("   This is expected if the asset is not registered in AMP.");
        }
    }

    // Try to get asset balance information
    println!("\n💰 Checking asset balance...");
    match client.get_asset_balance(asset_id).await {
        Ok(balance_entries) => {
            println!("✅ Asset balance information:");
            if balance_entries.is_empty() {
                println!("   No balance entries found");
            } else {
                for (i, entry) in balance_entries.iter().enumerate() {
                    println!("   Entry {}: Asset {} - Balance: {}", 
                        i + 1, entry.asset_id, entry.balance);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get asset balance: {}", e);
            println!("   This is expected if the asset is not registered in AMP.");
        }
    }

    // Check if we have Elements RPC access to analyze UTXOs directly
    println!("\n🔧 Elements RPC Analysis...");
    
    // Check if Elements RPC is configured
    if let (Ok(rpc_url), Ok(rpc_user), Ok(rpc_password)) = (
        std::env::var("ELEMENTS_RPC_URL"),
        std::env::var("ELEMENTS_RPC_USER"), 
        std::env::var("ELEMENTS_RPC_PASSWORD")
    ) {
        println!("✅ Elements RPC configured:");
        println!("   URL: {}", rpc_url);
        println!("   User: {}", rpc_user);
        
        // Try to create Elements RPC client and analyze UTXOs
        match analyze_elements_utxos(asset_id).await {
            Ok(()) => println!("✅ Elements UTXO analysis completed"),
            Err(e) => println!("❌ Elements UTXO analysis failed: {}", e),
        }
    } else {
        println!("⚠️  Elements RPC not configured - cannot analyze UTXOs directly");
        println!("   Set ELEMENTS_RPC_URL, ELEMENTS_RPC_USER, and ELEMENTS_RPC_PASSWORD");
    }

    println!("\n📊 UTXO Analysis Summary:");
    println!("   This analysis helps diagnose the 'No spendable UTXOs found' error.");
    println!("   Common causes:");
    println!("   1. Treasury address not imported as watch-only in Elements node");
    println!("   2. Asset issuance transaction not confirmed yet");
    println!("   3. UTXOs already spent in previous distributions");
    println!("   4. Elements node not synced or missing transaction data");

    Ok(())
}

/// Analyze UTXOs using Elements RPC directly
async fn analyze_elements_utxos(asset_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::Value;
    
    println!("🔍 Analyzing UTXOs via Elements RPC...");
    
    // Create a simple HTTP client to call Elements RPC
    let rpc_url = std::env::var("ELEMENTS_RPC_URL")?;
    let rpc_user = std::env::var("ELEMENTS_RPC_USER")?;
    let rpc_password = std::env::var("ELEMENTS_RPC_PASSWORD")?;
    
    let client = reqwest::Client::new();
    
    // First, try to get blockchain info to verify connection
    println!("📡 Testing Elements RPC connection...");
    let blockchain_info_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getblockchaininfo",
        "params": []
    });
    
    let response = client
        .post(&rpc_url)
        .basic_auth(&rpc_user, Some(&rpc_password))
        .json(&blockchain_info_request)
        .send()
        .await?;
    
    if response.status().is_success() {
        let result: Value = response.json().await?;
        if let Some(result_data) = result.get("result") {
            if let Some(blocks) = result_data.get("blocks") {
                println!("✅ Elements RPC connected - Current block height: {}", blocks);
            }
        }
    } else {
        println!("❌ Elements RPC connection failed: {}", response.status());
        return Ok(());
    }
    
    // Try to list unspent outputs for the asset
    println!("💰 Listing unspent outputs for asset {}...", asset_id);
    let listunspent_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "listunspent",
        "params": [0, 9999999, [], true, {"asset": asset_id}]
    });
    
    let response = client
        .post(&rpc_url)
        .basic_auth(&rpc_user, Some(&rpc_password))
        .json(&listunspent_request)
        .send()
        .await?;
    
    if response.status().is_success() {
        let result: Value = response.json().await?;
        if let Some(utxos) = result.get("result").and_then(|r| r.as_array()) {
            println!("✅ Found {} UTXOs for asset", utxos.len());
            
            if utxos.is_empty() {
                println!("⚠️  No UTXOs found for this asset!");
                println!("   This confirms the 'No spendable UTXOs found' error.");
            } else {
                let mut total_amount = 0.0;
                for (i, utxo) in utxos.iter().enumerate() {
                    if let (Some(txid), Some(vout), Some(amount), Some(address)) = (
                        utxo.get("txid").and_then(|v| v.as_str()),
                        utxo.get("vout").and_then(|v| v.as_u64()),
                        utxo.get("amount").and_then(|v| v.as_f64()),
                        utxo.get("address").and_then(|v| v.as_str())
                    ) {
                        println!("   {}. TXID: {}:{} - Amount: {} - Address: {}", 
                            i + 1, txid, vout, amount, address);
                        total_amount += amount;
                    }
                }
                println!("   💰 Total available: {}", total_amount);
            }
        }
    } else {
        println!("❌ Failed to list unspent outputs: {}", response.status());
    }
    
    // Try to get transaction details for the issuance
    println!("🔍 Checking issuance transaction...");
    // We would need the issuance TXID from the asset details to do this properly
    
    Ok(())
}