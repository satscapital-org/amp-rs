//! List Asset Activities Example
//!
//! This example demonstrates how to retrieve and display all activities for an asset.
//! Activities include issuance, reissuance, distribution, and other asset-related events.
//!
//! Usage:
//!   cargo run --example list_asset_activities
//!   cargo run --example list_asset_activities <asset_uuid>
//!
//! Make sure to set up your .env file with AMP_USERNAME and AMP_PASSWORD

use amp_rs::{ApiClient, model::AssetActivityParams};
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

    println!("Fetching activities for asset: {}\n", asset_uuid);

    // Create API client
    let client = ApiClient::new().await?;

    // Get asset activities with parameters to fetch all
    let params = AssetActivityParams {
        count: Some(1000), // Fetch up to 1000 activities
        sortcolumn: Some("datetime".to_string()),
        sortorder: Some("desc".to_string()), // Most recent first
        ..Default::default()
    };

    let activities = client.get_asset_activities(&asset_uuid, &params).await?;

    if activities.is_empty() {
        println!("No activities found for this asset.");
        return Ok(());
    }

    println!("Found {} activities:\n", activities.len());
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                  ASSET ACTIVITIES                                          ║");
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════╝");
    println!();

    // Display each activity
    for (index, activity) in activities.iter().enumerate() {
        println!("─── Activity {} ───", index + 1);
        println!("  Type:               {}", activity.activity_type);
        println!("  Date/Time:          {}", activity.datetime);
        println!("  Description:        {}", activity.description);
        println!("  Transaction ID:     {}", activity.txid);
        println!("  Output Index:       {}", activity.vout);
        println!("  Block Height:       {}", activity.blockheight);
        println!("  Amount:             {}", activity.amount);
        
        if let Some(registered_user) = activity.registered_user {
            println!("  Registered User:    {}", registered_user);
        }
        
        // Show blinding factors (truncated for readability)
        if !activity.asset_blinder.is_empty() {
            let blinder_preview = if activity.asset_blinder.len() > 16 {
                format!("{}...", &activity.asset_blinder[..16])
            } else {
                activity.asset_blinder.clone()
            };
            println!("  Asset Blinder:      {}", blinder_preview);
        }
        
        if !activity.amount_blinder.is_empty() {
            let blinder_preview = if activity.amount_blinder.len() > 16 {
                format!("{}...", &activity.amount_blinder[..16])
            } else {
                activity.amount_blinder.clone()
            };
            println!("  Amount Blinder:     {}", blinder_preview);
        }
        
        println!();
    }

    // Summary statistics by activity type
    println!("═══════════════════════════════════════════════════════════");
    println!("Activity Summary:");
    println!("═══════════════════════════════════════════════════════════");
    
    let mut type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for activity in &activities {
        *type_counts.entry(activity.activity_type.clone()).or_insert(0) += 1;
    }
    
    for (activity_type, count) in type_counts.iter() {
        println!("  {:<20} {:>5}", activity_type, count);
    }

    Ok(())
}
