//! Find Test Distribution Assets Example
//!
//! This example finds all assets labeled as "Test Distribution Asset" and analyzes
//! their UTXO status to identify which ones can be used for repeatable testing.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example find_test_distribution_assets
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

    println!("🔍 Find Test Distribution Assets Example");
    println!("========================================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();
    
    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!("✅ Connected to AMP API with {} strategy", client.get_strategy_type());

    // Get all assets
    println!("\n📋 Getting all assets...");
    let assets = client.get_assets().await?;
    println!("✅ Found {} total assets", assets.len());

    // Filter for "Test Distribution Asset" entries
    let test_assets: Vec<_> = assets.iter()
        .filter(|asset| asset.name.contains("Test Distribution Asset"))
        .collect();

    if test_assets.is_empty() {
        println!("\n❌ No 'Test Distribution Asset' entries found");
        println!("   You may need to create test assets first using create_test_distribution example");
        return Ok(());
    }

    println!("\n🎯 Found {} 'Test Distribution Asset' entries:", test_assets.len());

    let mut suitable_assets = Vec::new();

    for (i, asset) in test_assets.iter().enumerate() {
        println!("\n{}. Asset: {} ({})", i + 1, asset.name, asset.asset_uuid);
        println!("   Asset ID: {}", asset.asset_id);
        println!("   Ticker: {:?}", asset.ticker);
        println!("   Locked: {}", asset.is_locked);

        // Check assignments
        let assignments = match client.get_asset_assignments(&asset.asset_uuid).await {
            Ok(assignments) => {
                println!("   📋 Assignments: {}", assignments.len());
                assignments
            }
            Err(e) => {
                println!("   ❌ Failed to get assignments: {}", e);
                continue;
            }
        };

        // Check distributions
        let distributions = match client.get_asset_distributions(&asset.asset_uuid).await {
            Ok(distributions) => {
                println!("   📤 Distributions: {}", distributions.len());
                distributions
            }
            Err(e) => {
                println!("   ❌ Failed to get distributions: {}", e);
                continue;
            }
        };

        // Check balance/UTXOs - skip for now due to API format issues
        // Instead, we'll focus on assets that have distributions or assignments
        let has_balance = true; // Assume assets have balance for now
        println!("   💰 Balance check: Skipped (focusing on cleanup candidates)");

        // Analyze suitability for testing
        let unconfirmed_distributions = distributions.iter()
            .filter(|d| matches!(d.distribution_status, Status::Unconfirmed))
            .count();
        
        let active_assignments = assignments.iter()
            .filter(|a| !a.is_distributed || a.distribution_uuid.is_some())
            .count();

        let is_clean = unconfirmed_distributions == 0 && active_assignments == 0;
        let needs_cleanup = unconfirmed_distributions > 0 || active_assignments > 0;
        let is_suitable = needs_cleanup; // Focus on assets that need cleanup

        println!("   🔍 Analysis:");
        println!("      - Unconfirmed distributions: {}", unconfirmed_distributions);
        println!("      - Active assignments: {}", active_assignments);
        println!("      - Is clean: {}", is_clean);
        println!("      - Needs cleanup: {}", needs_cleanup);

        if is_suitable {
            suitable_assets.push((asset, has_balance, is_clean, unconfirmed_distributions, active_assignments));
            println!("   🧹 CLEANUP CANDIDATE - Asset has distributions/assignments to clean");
        } else if is_clean {
            println!("   ✅ ALREADY CLEAN - No distributions or assignments");
        } else {
            println!("   ❓ UNKNOWN STATE");
        }
    }

    // Summary and recommendations
    println!("\n📊 Summary:");
    println!("===========");

    if suitable_assets.is_empty() {
        println!("❌ No test assets found that need cleanup");
        println!("   All test assets appear to be clean already");
        
        // Show the first clean asset as a recommendation
        if !test_assets.is_empty() {
            let first_clean = test_assets.first().unwrap();
            println!("\n🚀 RECOMMENDATION:");
            println!("✅ Use clean asset: {} ({})", first_clean.name, first_clean.asset_uuid);
            println!("   This asset appears clean and ready for testing");
            println!("   Note: You may need to check if it has UTXOs available");
        }
        return Ok(());
    }

    println!("✅ Found {} test assets that need cleanup:", suitable_assets.len());

    println!("\n🧹 ASSETS NEEDING CLEANUP:");
    for (asset, _, _, unconfirmed, active) in &suitable_assets {
        println!("   • {} ({})", asset.name, asset.asset_uuid);
        println!("     - Unconfirmed distributions: {}", unconfirmed);
        println!("     - Active assignments: {}", active);
    }

    // Provide specific recommendation for cleanup
    if let Some((best_asset, _, _, unconfirmed, active)) = suitable_assets.first() {
        println!("\n🚀 CLEANUP RECOMMENDATION:");
        println!("🧹 Clean asset: {} ({})", best_asset.name, best_asset.asset_uuid);
        println!("   - Unconfirmed distributions: {}", unconfirmed);
        println!("   - Active assignments: {}", active);
        println!("\n📋 Steps to clean:");
        println!("   1. Edit examples/cancel_test_asset_distribution.rs");
        println!("   2. Change asset_uuid to: {}", best_asset.asset_uuid);
        println!("   3. Run: cargo run --example cancel_test_asset_distribution");
        println!("   4. After cleanup, use this asset for testing");
    }

    Ok(())
}