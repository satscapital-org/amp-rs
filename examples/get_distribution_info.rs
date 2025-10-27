//! Get Distribution Info Example
//!
//! This example demonstrates how to get information about a specific distribution
//! using the AMP API.
//!
//! Usage:
//!   cargo run --example get_distribution_info <ASSET_UUID> <DISTRIBUTION_UUID>
//!
//! Example:
//!   cargo run --example get_distribution_info asset-123 distribution-456

use amp_rs::ApiClient;
use std::env;
use tokio;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get asset UUID and distribution UUID from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <ASSET_UUID> <DISTRIBUTION_UUID>", args[0]);
        eprintln!("Example: {} asset-123 distribution-456", args[0]);
        std::process::exit(1);
    }

    let asset_uuid = &args[1];
    let distribution_uuid = &args[2];

    println!("Getting distribution info...");
    println!("  Asset UUID: {}", asset_uuid);
    println!("  Distribution UUID: {}", distribution_uuid);

    let client = ApiClient::new().await.expect("Failed to create API client");

    match client
        .get_asset_distribution(asset_uuid, distribution_uuid)
        .await
    {
        Ok(distribution) => {
            println!("\n✅ Distribution found:");
            println!("  UUID: {}", distribution.distribution_uuid);
            println!("  Status: {:?}", distribution.distribution_status);
            println!("  Transactions: {}", distribution.transactions.len());

            if !distribution.transactions.is_empty() {
                println!("\n📋 Transaction details:");
                for (i, tx) in distribution.transactions.iter().enumerate() {
                    println!("  {}. TXID: {}", i + 1, tx.txid);
                    println!("     Status: {:?}", tx.transaction_status);
                    println!("     Block Height: {}", tx.included_blockheight);
                    println!("     Confirmed: {}", tx.confirmed_datetime);
                    println!("     Assignments: {}", tx.assignments.len());
                }
            } else {
                println!("  No transactions associated with this distribution yet.");
            }
        }
        Err(e) => {
            eprintln!("❌ Error getting distribution info: {:?}", e);
            std::process::exit(1);
        }
    }
}
