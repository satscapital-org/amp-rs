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

    println!("🔧 Manager Operations Example");
    println!("=============================\n");

    // List all managers
    println!("📋 Listing all managers...");
    match client.get_managers().await {
        Ok(managers) => {
            println!("✅ Found {} managers:", managers.len());
            for manager in &managers {
                println!(
                    "   - ID: {}, Username: {}, Locked: {}, Assets: {}",
                    manager.id,
                    manager.username,
                    if manager.is_locked { "Yes" } else { "No" },
                    manager.assets.len()
                );
            }

            // If we have managers, demonstrate operations on the first one
            if let Some(first_manager) = managers.first() {
                let manager_id = first_manager.id;

                println!(
                    "\n🔍 Getting detailed info for manager ID {}...",
                    manager_id
                );
                match client.get_manager(manager_id).await {
                    Ok(manager) => {
                        println!("✅ Manager details:");
                        println!("   - ID: {}", manager.id);
                        println!("   - Username: {}", manager.username);
                        println!(
                            "   - Locked: {}",
                            if manager.is_locked { "Yes" } else { "No" }
                        );
                        println!("   - Assets: {:?}", manager.assets);

                        // If manager is locked, try to unlock
                        if manager.is_locked {
                            println!("\n🔓 Unlocking manager...");
                            match client.unlock_manager(manager_id).await {
                                Ok(_) => println!("✅ Manager unlocked successfully"),
                                Err(e) => println!("❌ Failed to unlock manager: {}", e),
                            }
                        }

                        // If manager has assets, demonstrate revoking them
                        if !manager.assets.is_empty() {
                            println!("\n🚫 Revoking all assets from manager...");
                            match client.revoke_manager(manager_id).await {
                                Ok(_) => println!("✅ All assets revoked successfully"),
                                Err(e) => println!("❌ Failed to revoke assets: {}", e),
                            }
                        } else {
                            println!("\n💡 Manager has no assets to revoke");
                        }
                    }
                    Err(e) => println!("❌ Failed to get manager details: {}", e),
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to get managers: {}", e);
        }
    }

    // Try to get current manager info
    println!("\n👤 Getting current manager info...");
    match client.get_current_manager_raw().await {
        Ok(manager_json) => {
            println!("✅ Current manager info:");
            println!("{}", serde_json::to_string_pretty(&manager_json)?);
        }
        Err(e) => {
            println!("❌ Failed to get current manager info: {}", e);
            println!(
                "💡 This might be expected if the current user doesn't have manager permissions"
            );
        }
    }

    Ok(())
}
