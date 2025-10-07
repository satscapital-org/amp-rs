use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file if it exists
    dotenvy::dotenv().ok();

    // Check for required environment variables
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        eprintln!("Error: AMP_USERNAME and AMP_PASSWORD environment variables must be set");
        std::process::exit(1);
    }

    let client = ApiClient::new().await?;

    println!("🔍 Manager Endpoint Testing");
    println!("============================\n");

    // Test /managers/me endpoint
    println!("🔍 Testing /managers/me endpoint...");
    match client.get_current_manager_raw().await {
        Ok(json_response) => {
            println!("✅ Successfully retrieved /managers/me response:");
            println!("{}", serde_json::to_string_pretty(&json_response)?);
        }
        Err(e) => {
            println!("❌ /managers/me failed: {}", e);
        }
    }

    println!("\n{}\n", "=".repeat(50));

    // Fallback: Show all managers for comparison
    println!("🔍 Fallback: Listing all managers from /managers endpoint...");
    match client.get_managers().await {
        Ok(managers) => {
            println!("✅ Successfully retrieved managers list:");
            for (i, manager) in managers.iter().enumerate() {
                println!(
                    "   Manager {}: {} (ID: {}, Locked: {})",
                    i + 1,
                    manager.username,
                    manager.id,
                    if manager.is_locked { "Yes" } else { "No" }
                );
            }

            if managers.len() == 1 {
                println!(
                    "\n💡 Since there's only one manager, this is likely the current manager."
                );
            }
        }
        Err(e) => {
            println!("❌ Failed to get managers list: {}", e);
        }
    }

    Ok(())
}
