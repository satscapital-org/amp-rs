//! Asset Summary Example
//!
//! This example demonstrates how to use the AMP client to:
//! 1. Fetch all assets using get_assets()
//! 2. Get detailed information for each asset using get_asset()
//! 3. Display a comprehensive summary of each asset's properties
//!
//! Usage: cargo run --example asset_summary
//!
//! Make sure to set up your .env file with AMP_USERNAME and AMP_PASSWORD

use amp_rs::ApiClient;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    let client = ApiClient::new().await?;

    println!("Fetching all assets...");
    let assets = client.get_assets().await?;

    if assets.is_empty() {
        println!("No assets found for the current credentials.");
        return Ok(());
    }

    println!(
        "Found {} assets. Fetching detailed information...\n",
        assets.len()
    );

    for (index, asset_summary) in assets.iter().enumerate() {
        println!("=== Asset {} of {} ===", index + 1, assets.len());

        // Get detailed asset information
        match client.get_asset(&asset_summary.asset_uuid).await {
            Ok(asset) => {
                println!("Name: {}", asset.name);
                println!("UUID: {}", asset.asset_uuid);
                println!("Asset ID: {}", asset.asset_id);
                println!("Ticker: {}", asset.ticker.as_deref().unwrap_or("N/A"));
                println!("Domain: {}", asset.domain.as_deref().unwrap_or("N/A"));
                println!("Precision: {}", asset.precision);
                println!("Is Registered: {}", asset.is_registered);
                println!("Is Authorized: {}", asset.is_authorized);
                println!("Is Locked: {}", asset.is_locked);
                println!("Transfer Restricted: {}", asset.transfer_restricted);

                if let Some(reissuance_token_id) = &asset.reissuance_token_id {
                    println!("Reissuance Token ID: {}", reissuance_token_id);
                }

                if let Some(pubkey) = &asset.pubkey {
                    println!("Public Key: {}", pubkey);
                }

                if let Some(endpoint) = &asset.issuer_authorization_endpoint {
                    println!("Authorization Endpoint: {}", endpoint);
                }

                println!("Requirements: {:?}", asset.requirements);
            }
            Err(e) => {
                println!(
                    "Failed to fetch details for asset {}: {:?}",
                    asset_summary.asset_uuid, e
                );
            }
        }

        println!(); // Empty line for readability
    }

    println!("Asset summary complete!");
    Ok(())
}
