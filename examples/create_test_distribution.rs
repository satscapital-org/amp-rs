//! Create Test Distribution Example
//!
//! This example creates a test distribution from existing assignments
//! to demonstrate the cancellation functionality.

use amp_rs::ApiClient;
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🚀 Create Test Distribution Example");
    println!("===================================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();
    
    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!("✅ Connected to AMP API with {} strategy", client.get_strategy_type());

    // Target the specific asset UUID from the end-to-end test
    let asset_uuid = "bf03c7ce-8cce-400e-9c08-e5231b44036c";
    println!("\n🎯 Targeting test asset: {}", asset_uuid);

    // Check existing assignments
    println!("📋 Getting assignments for asset...");
    let assignments = client.get_asset_assignments(asset_uuid).await?;
    
    if assignments.is_empty() {
        println!("❌ No assignments found. Run create_test_assignments first.");
        return Ok(());
    }
    
    println!("✅ Found {} assignments", assignments.len());
    
    // Find assignments ready for distribution
    let ready_assignments: Vec<_> = assignments.iter()
        .filter(|a| a.ready_for_distribution && !a.is_distributed && a.distribution_uuid.is_none())
        .collect();
    
    if ready_assignments.is_empty() {
        println!("❌ No assignments ready for distribution found");
        return Ok(());
    }
    
    println!("📦 Found {} assignments ready for distribution", ready_assignments.len());
    for assignment in &ready_assignments {
        println!("   ID: {} - User: {} - Amount: {}", 
            assignment.id, assignment.registered_user, assignment.amount);
    }

    // Create a distribution using the ready assignments
    println!("\n🚀 Creating distribution...");

    // Convert assignments to AssetDistributionAssignment format
    let distribution_assignments: Vec<amp_rs::model::AssetDistributionAssignment> = ready_assignments
        .iter()
        .filter_map(|assignment| {
            if let Some(address) = &assignment.receiving_address {
                Some(amp_rs::model::AssetDistributionAssignment {
                    user_id: assignment.registered_user.to_string(),
                    address: address.clone(),
                    amount: assignment.amount as f64,
                })
            } else {
                println!("⚠️  Skipping assignment {} - no receiving address", assignment.id);
                None
            }
        })
        .collect();

    if distribution_assignments.is_empty() {
        println!("❌ No assignments with receiving addresses found");
        return Ok(());
    }

    match client.create_distribution(asset_uuid, distribution_assignments).await {
        Ok(distribution) => {
            println!("✅ Created distribution:");
            println!("   UUID: {}", distribution.distribution_uuid);
            println!("   Asset ID: {}", distribution.asset_id);
            println!("   Addresses: {}", distribution.map_address_amount.len());
            
            // Check assignments again to see if they're now linked
            println!("\n🔍 Checking updated assignments...");
            let updated_assignments = client.get_asset_assignments(asset_uuid).await?;
            for assignment in updated_assignments {
                if let Some(dist_uuid) = &assignment.distribution_uuid {
                    println!("   Assignment {} now linked to distribution {}", 
                        assignment.id, dist_uuid);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to create distribution: {}", e);
        }
    }

    println!("\n🎉 Test distribution creation completed!");
    println!("   You can now run the cancel example to test cleanup.");

    Ok(())
}