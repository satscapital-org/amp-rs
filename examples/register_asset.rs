//! Register Asset Example
//!
//! This example demonstrates how to register an asset with the Blockstream Asset Registry.
//! It takes an asset UUID as a command-line argument and calls the `register_asset` API endpoint.
//!
//! ## Usage
//!
//! ```bash
//! # Set environment variables for live API access
//! export AMP_USERNAME="your_username"
//! export AMP_PASSWORD="your_password"
//!
//! # Run the example with an asset UUID
//! cargo run --example register_asset -- 9bcd9987-9544-439f-80b3-6d76b072fd9b
//! ```
//!
//! ## Requirements
//!
//! - Valid AMP API credentials
//! - Asset UUID to register

use amp_rs::ApiClient;
use dotenvy;
use std::env;

/// Print usage information
fn print_usage() {
    println!("Usage:");
    println!("  cargo run --example register_asset -- <ASSET_UUID>");
    println!();
    println!("Arguments:");
    println!(
        "  ASSET_UUID    The UUID of the asset to register with the Blockstream Asset Registry"
    );
    println!();
    println!("Examples:");
    println!("  cargo run --example register_asset -- 9bcd9987-9544-439f-80b3-6d76b072fd9b");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    // Check for help flag
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        print_usage();
        return Ok(());
    }

    // Require asset UUID argument
    if args.len() < 2 {
        eprintln!("❌ Error: Asset UUID is required");
        eprintln!();
        print_usage();
        std::process::exit(1);
    }

    let asset_uuid = &args[1];

    println!("🚀 Starting Asset Registration Example");
    println!();
    println!("Configuration:");
    println!("  - Asset UUID: {}", asset_uuid);
    println!();

    // Load environment variables
    println!("📁 Loading environment variables");
    dotenvy::dotenv().ok();

    // Verify required environment variables
    let amp_username =
        env::var("AMP_USERNAME").map_err(|_| "AMP_USERNAME environment variable not set")?;
    let _amp_password =
        env::var("AMP_PASSWORD").map_err(|_| "AMP_PASSWORD environment variable not set")?;

    println!("✅ Environment variables loaded");
    println!("   - AMP Username: {}", amp_username);

    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating ApiClient");
    let api_client = ApiClient::new()
        .await
        .map_err(|e| format!("Failed to create ApiClient: {}", e))?;

    println!("✅ ApiClient created successfully");

    // Verify asset exists before attempting registration
    println!("🔍 Verifying asset exists");
    match api_client.get_asset(asset_uuid).await {
        Ok(asset_details) => {
            println!("✅ Asset verified successfully");
            println!("   - Name: {}", asset_details.name);
            println!("   - Ticker: {:?}", asset_details.ticker);
            println!("   - Domain: {:?}", asset_details.domain);
        }
        Err(e) => {
            eprintln!("❌ Failed to verify asset: {}", e);
            eprintln!("   Please check that the asset UUID is correct");
            return Err(e.into());
        }
    }

    // Register the asset
    println!();
    println!("📝 Registering asset with Blockstream Asset Registry");
    match api_client.register_asset(asset_uuid).await {
        Ok(response) => {
            println!("🎉 Asset registration successful!");
            println!();
            if let Some(message) = &response.message {
                println!("Message: {}", message);
            }
            if let Some(asset) = &response.asset_data {
                println!();
                println!("Asset Details:");
                println!("  - Name: {}", asset.name);
                println!("  - Asset UUID: {}", asset.asset_uuid);
                println!("  - Asset ID: {}", asset.asset_id);
                if let Some(ticker) = &asset.ticker {
                    println!("  - Ticker: {}", ticker);
                }
                if let Some(domain) = &asset.domain {
                    println!("  - Domain: {}", domain);
                }
                println!("  - Is Registered: {}", asset.is_registered);
                println!("  - Is Authorized: {}", asset.is_authorized);
                println!("  - Precision: {}", asset.precision);
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to register asset: {}", e);
            return Err(e.into());
        }
    }

    println!();
    println!("✅ Asset registration example completed successfully!");

    Ok(())
}
