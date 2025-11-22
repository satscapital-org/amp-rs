//! Asset Summary Statistics Example
//!
//! This example demonstrates how to retrieve and display the complete AssetSummary
//! for an asset, which includes statistical information about issuance, distribution,
//! and token availability.
//!
//! Usage:
//!   cargo run --example asset_summary_stats
//!   cargo run --example asset_summary_stats <asset_uuid>
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

    println!("Fetching asset summary for: {}\n", asset_uuid);

    // Create API client
    let client = ApiClient::new().await?;

    // Get asset summary
    let summary = client.get_asset_summary(&asset_uuid).await?;

    // Get asset activities to count reissuance events
    let params = AssetActivityParams {
        count: Some(10000), // Get a large number to capture all activities
        ..Default::default()
    };
    let activities = client.get_asset_activities(&asset_uuid, &params).await?;

    // Count activities by type (same approach as list_asset_activities)
    let mut type_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for activity in &activities {
        *type_counts.entry(activity.activity_type.clone()).or_insert(0) += 1;
    }

    // Count reissuance activities - check for any activity type containing "reissue" or "Reissuance"
    let reissuance_count = type_counts
        .iter()
        .filter(|(activity_type, _)| {
            let activity_type_lower = activity_type.to_lowercase();
            activity_type_lower == "reissuance"
        })
        .map(|(_, count)| *count)
        .sum::<i64>();

    // Display complete asset summary
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║              ASSET SUMMARY STATISTICS                      ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    println!("Asset Identification:");
    println!("  Asset ID: {}", summary.asset_id);

    if let Some(reissuance_token_id) = &summary.reissuance_token_id {
        println!("  Reissuance Token ID: {}", reissuance_token_id);
    } else {
        println!("  Reissuance Token ID: None");
    }

    println!();
    println!("Supply & Distribution:");
    println!("  Total Issued:           {:>12}", summary.issued);
    println!("  Total Reissued:         {:>12}", summary.reissued);
    println!("  Total Assigned:         {:>12}", summary.assigned);
    println!("  Total Distributed:      {:>12}", summary.distributed);
    println!("  Total Burned:           {:>12}", summary.burned);
    println!("  Total Blacklisted:      {:>12}", summary.blacklisted);

    println!();
    let total_supply = summary.issued + summary.reissued;
    let available = total_supply - summary.distributed - summary.burned - summary.blacklisted;
    println!("Calculated Values:");
    println!("  Total Supply:           {:>12} (issued + reissued)", total_supply);
    println!("  Available:              {:>12} (total - distributed - burned - blacklisted)", available);

    println!();
    println!("User Statistics:");
    println!("  Total Registered Users:     {:>8}", summary.registered_users);
    println!("  Active Registered Users:    {:>8}", summary.active_registered_users);
    println!("  Active Green Subaccounts:   {:>8}", summary.active_green_subaccounts);

    println!();
    println!("Reissuance Capability:");
    println!("  Reissuance Tokens Available: {:>8}", summary.reissuance_tokens);
    println!("  Reissuance Events Count:     {:>8}", reissuance_count);

    let remaining_reissuances = summary.reissuance_tokens - reissuance_count;
    println!("  Remaining Reissuances:       {:>8} (available - used)", remaining_reissuances);

    if remaining_reissuances > 0 {
        println!("  ✓ This asset can be reissued {} more time(s)", remaining_reissuances);
    } else if summary.reissuance_tokens > 0 {
        println!("  ✗ This asset cannot be reissued (all tokens have been used)");
    } else {
        println!("  ✗ This asset cannot be reissued (no tokens available)");
    }

    Ok(())
}
