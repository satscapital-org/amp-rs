//! List Asset Reissuances Example
//!
//! This example demonstrates how to retrieve and display the reissuance history for an asset.
//! It uses the `get_asset_reissuances` endpoint to fetch all reissuances performed on a specific asset.
//!
//! Usage:
//!   cargo run --example list_asset_reissuances
//!   cargo run --example list_asset_reissuances <asset_uuid>
//!
//! Make sure to set up your .env file with AMP_USERNAME and AMP_PASSWORD

use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get UUID from command line arguments or use default
    let args: Vec<String> = env::args().collect();
    let asset_uuid = if args.len() > 1 {
        args[1].clone()
    } else {
        // Default test asset UUID - replace with a valid asset UUID
        "84e282bf-16bf-40e2-9d4f-5b25415a906a".to_string()
    };

    println!("Fetching reissuance history for asset: {}\n", asset_uuid);

    // Create API client
    let client = ApiClient::new().await?;

    // Get asset reissuances
    let reissuances = client.get_asset_reissuances(&asset_uuid).await?;

    // Display results
    if reissuances.is_empty() {
        println!("No reissuances found for this asset.");
    } else {
        println!("Found {} reissuance(s):\n", reissuances.len());

        for (idx, reissuance) in reissuances.iter().enumerate() {
            println!("Reissuance #{}", idx + 1);
            println!("  Transaction ID: {}", reissuance.txid);
            println!("  Output Index (vout): {}", reissuance.vout);
            println!("  Destination Address: {}", reissuance.destination_address);
            println!("  Reissuance Amount: {}", reissuance.reissuance_amount);
            println!("  Confirmed in Block: {}", reissuance.confirmed_in_block);
            println!("  Created: {}", reissuance.created);
            println!();
        }

        // Summary statistics
        let total_reissued: i64 = reissuances.iter().map(|r| r.reissuance_amount).sum();
        println!("Summary:");
        println!("  Total Reissuances: {}", reissuances.len());
        println!("  Total Amount Reissued: {}", total_reissued);
    }

    Ok(())
}
