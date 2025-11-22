//! Reissuances Available Example
//!
//! This example demonstrates how to check the number of reissuance tokens available for an asset.
//! Reissuance tokens control the ability to issue additional units of an asset after initial creation.
//!
//! Usage:
//!   cargo run --example reissuances_available
//!   cargo run --example reissuances_available <asset_uuid>
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
        "84e282bf-16bf-40e2-9d4f-5b25415a906a".to_string()
    };

    println!("Checking reissuances available for asset: {}\n", asset_uuid);

    // Create API client
    let client = ApiClient::new().await?;

    // Get asset summary
    let summary = client.get_asset_summary(&asset_uuid).await?;

    // Display results
    println!("Asset Summary:");
    println!("  Asset ID: {}", summary.asset_id);
    println!("  Reissuance Tokens Available: {}", summary.reissuance_tokens);
    println!();
    println!("Additional Information:");
    println!("  Issued: {}", summary.issued);
    println!("  Reissued: {}", summary.reissued);
    println!("  Assigned: {}", summary.assigned);
    println!("  Distributed: {}", summary.distributed);
    println!("  Burned: {}", summary.burned);
    println!("  Blacklisted: {}", summary.blacklisted);

    if let Some(reissuance_token_id) = summary.reissuance_token_id {
        println!("  Reissuance Token ID: {}", reissuance_token_id);
    }

    Ok(())
}
