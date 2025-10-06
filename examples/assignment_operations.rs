use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::from_filename_override(".env").ok();

    // Check if we're running in live mode
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("This example requires AMP_TESTS=live to be set");
        println!("Set AMP_USERNAME, AMP_PASSWORD, and AMP_TESTS=live environment variables");
        return Ok(());
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        eprintln!("AMP_USERNAME and AMP_PASSWORD must be set for this example");
        return Ok(());
    }

    let client = ApiClient::new()?;

    println!("Assignment Operations Example");
    println!("============================");

    // Get assets to work with
    let assets = client.get_assets().await?;
    if assets.is_empty() {
        println!("No assets found. Please create an asset first.");
        return Ok(());
    }

    let asset = &assets[0];
    println!("Working with asset: {} ({})", asset.name, asset.asset_uuid);

    // Get existing assignments for this asset
    let assignments = client.get_asset_assignments(&asset.asset_uuid).await?;
    println!("Found {} existing assignments", assignments.len());

    if let Some(assignment) = assignments.first() {
        let assignment_id = assignment.id.to_string();
        println!("Working with assignment ID: {}", assignment_id);

        // Demonstrate lock operation
        println!("\n1. Locking assignment...");
        match client
            .lock_asset_assignment(&asset.asset_uuid, &assignment_id)
            .await
        {
            Ok(locked_assignment) => {
                println!("✓ Assignment locked successfully");
                println!("  Assignment ID: {}", locked_assignment.id);
                println!("  Amount: {}", locked_assignment.amount);
            }
            Err(e) => println!("✗ Failed to lock assignment: {}", e),
        }

        // Demonstrate unlock operation
        println!("\n2. Unlocking assignment...");
        match client
            .unlock_asset_assignment(&asset.asset_uuid, &assignment_id)
            .await
        {
            Ok(unlocked_assignment) => {
                println!("✓ Assignment unlocked successfully");
                println!("  Assignment ID: {}", unlocked_assignment.id);
                println!("  Amount: {}", unlocked_assignment.amount);
            }
            Err(e) => println!("✗ Failed to unlock assignment: {}", e),
        }

        // Note: We're not demonstrating delete here as it's destructive
        println!("\n3. Delete operation available but not demonstrated (destructive)");
        println!("   Use client.delete_asset_assignment(&asset_uuid, &assignment_id) to delete");
    } else {
        println!("No assignments found for this asset. Create some assignments first.");
    }

    println!("\nExample completed!");
    Ok(())
}
