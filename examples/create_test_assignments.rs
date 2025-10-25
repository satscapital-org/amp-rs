//! Create Test Assignments Example
//!
//! This example creates test assignments for the test asset to demonstrate
//! the cancellation functionality.

use amp_rs::{model::CreateAssetAssignmentRequest, ApiClient};
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();

    println!("🚀 Create Test Assignments Example");
    println!("==================================");

    // Load environment variables from .env file
    println!("📁 Loading environment variables from .env file");
    dotenvy::dotenv().ok();

    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating AMP API client");
    let client = ApiClient::new().await?;
    println!(
        "✅ Connected to AMP API with {} strategy",
        client.get_strategy_type()
    );

    // Target the specific asset UUID from the end-to-end test
    let asset_uuid = "bf03c7ce-8cce-400e-9c08-e5231b44036c";
    println!("\n🎯 Targeting test asset: {}", asset_uuid);

    // Get registered users to assign to
    println!("👥 Getting registered users...");
    let users = client.get_registered_users().await?;

    if users.is_empty() {
        println!("❌ No registered users found");
        return Ok(());
    }

    println!("✅ Found {} registered users", users.len());

    // Use the first user for assignment
    let user = &users[0];
    println!("👤 Using user: {} (ID: {})", user.name, user.id);

    // Create a test assignment
    println!("\n📝 Creating test assignment...");
    let assignment_request = CreateAssetAssignmentRequest {
        registered_user: user.id,
        amount: 1000,
        vesting_timestamp: None,
        ready_for_distribution: true, // Make it ready so we can test distribution
    };

    match client
        .create_asset_assignments(asset_uuid, &vec![assignment_request])
        .await
    {
        Ok(assignments) => {
            println!("✅ Created {} assignment(s):", assignments.len());
            for assignment in assignments {
                println!("   ID: {}", assignment.id);
                println!("   User: {}", assignment.registered_user);
                println!("   Amount: {}", assignment.amount);
                println!(
                    "   Ready for distribution: {}",
                    assignment.ready_for_distribution
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to create assignment: {}", e);
        }
    }

    println!("\n🎉 Test assignment creation completed!");
    println!("   You can now run the cancel example to test cleanup.");

    Ok(())
}
