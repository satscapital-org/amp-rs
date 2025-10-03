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

    let client = ApiClient::new()?;

    println!("🧹 AMP Resource Cleanup Tool");
    println!("============================\n");

    // Show what will be cleaned up
    show_cleanup_preview(&client).await?;

    println!("⚠️  DANGER: This will COMPLETELY WIPE your AMP environment!");
    println!("⚠️  ALL resources will be permanently deleted!");
    println!("⚠️  Managers will remain but all other resources will be deleted!");
    println!("⚠️  This action cannot be undone!");
    println!("\n🚀 Proceeding with automatic cleanup...\n");

    // Perform cleanup automatically
    perform_cleanup(&client).await?;

    println!("\n✅ Cleanup completed!");
    Ok(())
}

async fn show_cleanup_preview(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Cleanup Preview:");
    println!("-------------------");

    // Preview assets
    match client.get_assets().await {
        Ok(assets) => {
            println!("💰 Assets to delete: {}", assets.len());
            if !assets.is_empty() {
                for asset in assets.iter().take(3) {
                    println!("   • {} ({:?})", asset.name, asset.ticker);
                }
                if assets.len() > 3 {
                    println!("   ... and {} more", assets.len() - 3);
                }
            }
        }
        Err(e) => println!("❌ Could not preview assets: {}", e),
    }

    // Preview categories
    match client.get_categories().await {
        Ok(categories) => {
            println!("📁 Categories to delete: {}", categories.len());
            if !categories.is_empty() {
                for category in categories.iter().take(3) {
                    println!("   • {}", category.name);
                }
                if categories.len() > 3 {
                    println!("   ... and {} more", categories.len() - 3);
                }
            }
        }
        Err(e) => println!("❌ Could not preview categories: {}", e),
    }

    // Preview registered users
    match client.get_registered_users().await {
        Ok(users) => {
            println!("👤 Registered users to delete: {}", users.len());
            if !users.is_empty() {
                for user in users.iter().take(3) {
                    println!("   • {} (ID: {})", user.name, user.id);
                }
                if users.len() > 3 {
                    println!("   ... and {} more", users.len() - 3);
                }
            }
        }
        Err(e) => println!("❌ Could not preview registered users: {}", e),
    }

    // Note: Managers cannot be deleted and will remain after cleanup

    println!();
    Ok(())
}



async fn perform_cleanup(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🧹 Starting complete cleanup...\n");

    // Delete assets first (they may have dependencies)
    delete_all_assets(client).await?;
    
    // Delete categories
    delete_all_categories(client).await?;
    
    // Delete registered users
    delete_all_registered_users(client).await?;

    println!("\n⚠️  WARNING: AMP environment is now clean!");
    println!("   Managers remain but all other resources have been deleted.");

    Ok(())
}

async fn delete_all_assets(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Deleting assets...");
    
    let assets = match client.get_assets().await {
        Ok(assets) => assets,
        Err(e) => {
            println!("   ❌ Failed to list assets: {}", e);
            return Ok(());
        }
    };

    if assets.is_empty() {
        println!("   ✅ No assets to delete");
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;

    for asset in assets {
        print!("   Deleting '{}' ({:?})... ", asset.name, asset.ticker);
        match client.delete_asset(&asset.asset_uuid).await {
            Ok(_) => {
                println!("✅");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                error_count += 1;
            }
        }
    }

    println!("   📊 Assets: {} deleted, {} errors", success_count, error_count);
    Ok(())
}

async fn delete_all_categories(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Deleting categories...");
    
    let categories = match client.get_categories().await {
        Ok(categories) => categories,
        Err(e) => {
            println!("   ❌ Failed to list categories: {}", e);
            return Ok(());
        }
    };

    if categories.is_empty() {
        println!("   ✅ No categories to delete");
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;

    for category in categories {
        print!("   Deleting '{}'... ", category.name);
        match client.delete_category(category.id).await {
            Ok(_) => {
                println!("✅");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                error_count += 1;
            }
        }
    }

    println!("   📊 Categories: {} deleted, {} errors", success_count, error_count);
    Ok(())
}

async fn delete_all_registered_users(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Deleting registered users...");
    
    let users = match client.get_registered_users().await {
        Ok(users) => users,
        Err(e) => {
            println!("   ❌ Failed to list registered users: {}", e);
            return Ok(());
        }
    };

    if users.is_empty() {
        println!("   ✅ No registered users to delete");
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;

    for user in users {
        print!("   Deleting user '{}'... ", user.name);
        match client.delete_registered_user(user.id).await {
            Ok(_) => {
                println!("✅");
                success_count += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                error_count += 1;
            }
        }
    }

    println!("   📊 Users: {} deleted, {} errors", success_count, error_count);
    Ok(())
}

