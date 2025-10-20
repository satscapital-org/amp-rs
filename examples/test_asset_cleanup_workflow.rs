//! Test Asset Cleanup Workflow Example
//!
//! This example demonstrates the complete workflow for finding and cleaning
//! "Test Distribution Asset" entries for repeatable testing.
//!
//! ## Workflow
//!
//! 1. Find test assets that need cleanup using find_test_distribution_assets
//! 2. Clean a specific asset using cancel_test_asset_distribution
//! 3. Verify the asset is ready for testing
//!
//! ## Usage
//!
//! ```bash
//! # Step 1: Find assets that need cleanup
//! cargo run --example find_test_distribution_assets
//! 
//! # Step 2: Edit cancel_test_asset_distribution.rs with the asset UUID
//! # Step 3: Clean the asset
//! cargo run --example cancel_test_asset_distribution
//! 
//! # Step 4: Verify cleanup (run this example)
//! cargo run --example test_asset_cleanup_workflow
//! ```
//!
//! ## Environment Variables
//!
//! This example uses dotenvy to load environment variables from .env:
//! - `AMP_USERNAME`: AMP API username
//! - `AMP_PASSWORD`: AMP API password

use amp_rs::ApiClient;
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🔄 Test Asset Cleanup Workflow");
    println!("===============================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();
    
    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!("✅ Connected to AMP API with {} strategy", client.get_strategy_type());

    // The asset we just cleaned
    let cleaned_asset_uuid = "93cffcb9-c1f5-4873-b5dc-f3ba1f29e3c2";
    
    println!("\n🎯 Verifying cleaned asset: {}", cleaned_asset_uuid);

    // Check the asset status
    let asset = client.get_asset(cleaned_asset_uuid).await?;
    println!("✅ Asset: {} ({})", asset.name, asset.ticker.as_ref().unwrap_or(&"".to_string()));

    let assignments = client.get_asset_assignments(cleaned_asset_uuid).await?;
    let distributions = client.get_asset_distributions(cleaned_asset_uuid).await?;

    println!("📊 Current Status:");
    println!("   - Assignments: {}", assignments.len());
    println!("   - Distributions: {}", distributions.len());

    if assignments.is_empty() && distributions.is_empty() {
        println!("✅ Asset is completely clean!");
        
        println!("\n🚀 Next Steps for Repeatable Testing:");
        println!("=====================================");
        
        println!("1. **UTXO Issue**: This asset currently has no UTXOs available");
        println!("   - This is why distributions fail with 'No spendable UTXOs found'");
        println!("   - The asset needs to have UTXOs in the treasury address");
        
        println!("\n2. **Solutions**:");
        println!("   a) **Import Treasury Address**: Ensure the treasury address is imported");
        println!("      as watch-only in the Elements node");
        println!("   b) **Check Issuance**: Verify the asset issuance transaction is confirmed");
        println!("   c) **Sync Elements Node**: Ensure the Elements node is fully synced");
        
        println!("\n3. **For Repeatable Testing**:");
        println!("   - Use this cleaned asset UUID: {}", cleaned_asset_uuid);
        println!("   - After fixing UTXO availability, this asset can be used in:");
        println!("     • test_end_to_end_distribution_workflow");
        println!("     • Any distribution tests that need a clean asset");
        
        println!("\n4. **Cleanup Process** (for future use):");
        println!("   - Run: cargo run --example find_test_distribution_assets");
        println!("   - Edit cancel_test_asset_distribution.rs with the asset UUID");
        println!("   - Run: cargo run --example cancel_test_asset_distribution");
        println!("   - Verify with this workflow example");
        
        println!("\n📋 **Asset Details for Testing**:");
        println!("   UUID: {}", cleaned_asset_uuid);
        println!("   Asset ID: {}", asset.asset_id);
        println!("   Name: {}", asset.name);
        println!("   Ticker: {}", asset.ticker.as_ref().unwrap_or(&"".to_string()));
        
    } else {
        println!("⚠️  Asset still needs cleanup:");
        println!("   - {} assignments remaining", assignments.len());
        println!("   - {} distributions remaining", distributions.len());
        println!("\n🔧 Run the cleanup process again:");
        println!("   cargo run --example cancel_test_asset_distribution");
    }

    println!("\n✅ Workflow complete!");
    Ok(())
}