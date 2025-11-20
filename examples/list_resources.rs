use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file if it exists
    dotenvy::dotenv().ok();

    // Check for required environment variables
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        eprintln!("Error: AMP_USERNAME and AMP_PASSWORD environment variables must be set");
        eprintln!("You can set them in a .env file or as environment variables");
        std::process::exit(1);
    }

    let client = ApiClient::new().await?;

    println!("🔍 AMP Resource Summary");
    println!("======================\n");

    // List Managers with count
    print!("👥 Managers: ");
    match client.get_managers().await {
        Ok(managers) => {
            println!("{} found", managers.len());
            if !managers.is_empty() {
                println!("   Recent managers:");
                for manager in managers.iter().take(5) {
                    println!(
                        "   • {} (ID: {}, Locked: {})",
                        manager.username, manager.id, manager.is_locked
                    );
                }
                if managers.len() > 5 {
                    println!("   ... and {} more", managers.len() - 5);
                }
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!();

    // List Registered Users with count
    print!("👤 Registered Users: ");
    match client.get_registered_users().await {
        Ok(users) => {
            println!("{} found", users.len());
            if !users.is_empty() {
                println!("   Recent users:");
                for user in users.iter().take(5) {
                    println!(
                        "   • {} (ID: {}, GAID: {:?})",
                        user.name, user.id, user.gaid
                    );
                }
                if users.len() > 5 {
                    println!("   ... and {} more", users.len() - 5);
                }
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!();

    // List Categories with count
    print!("📁 Categories: ");
    match client.get_categories().await {
        Ok(categories) => {
            println!("{} found", categories.len());
            if !categories.is_empty() {
                println!("   Available categories:");
                for category in categories.iter().take(5) {
                    println!("   • {} (ID: {})", category.name, category.id);
                }
                if categories.len() > 5 {
                    println!("   ... and {} more", categories.len() - 5);
                }
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!();

    // List Assets with count
    print!("💰 Assets: ");
    match client.get_assets().await {
        Ok(assets) => {
            println!("{} found", assets.len());
            if !assets.is_empty() {
                println!("   All assets:");
                for asset in assets.iter() {
                    println!(
                        "   • {} - {}",
                        asset.name, asset.asset_uuid
                    );
                }
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }

    println!("\n✅ Resource summary complete!");
    Ok(())
}
