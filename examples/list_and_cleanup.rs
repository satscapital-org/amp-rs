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

    println!("🔍 AMP Resource Listing and Cleanup Tool");
    println!("========================================\n");

    // List all resources first
    list_all_resources(&client).await?;

    // Ask for confirmation before cleanup
    println!("\n⚠️  WARNING: The following operations will DELETE resources!");
    println!("Do you want to proceed with cleanup? (y/N): ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" || input.trim().to_lowercase() == "yes" {
        println!("\n🧹 Starting cleanup operations...\n");
        cleanup_all_resources(&client).await?;
    } else {
        println!("Cleanup cancelled.");
    }

    Ok(())
}

async fn list_all_resources(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    // List Managers
    println!("👥 MANAGERS");
    println!("-----------");
    match client.get_managers().await {
        Ok(managers) => {
            println!("📊 Total managers: {}", managers.len());
            for (i, manager) in managers.iter().enumerate() {
                println!(
                    "  {}. ID: {}, Username: {}, Locked: {}",
                    i + 1,
                    manager.id,
                    manager.username,
                    manager.is_locked
                );
            }
        }
        Err(e) => println!("❌ Failed to list managers: {}", e),
    }

    println!();

    // List Registered Users
    println!("👤 REGISTERED USERS");
    println!("-------------------");
    match client.get_registered_users().await {
        Ok(users) => {
            println!("📊 Total registered users: {}", users.len());
            for (i, user) in users.iter().enumerate() {
                println!(
                    "  {}. ID: {}, Name: {}, GAID: {:?}, Company: {}",
                    i + 1,
                    user.id,
                    user.name,
                    user.gaid,
                    user.is_company
                );
            }
        }
        Err(e) => println!("❌ Failed to list registered users: {}", e),
    }

    println!();

    // List Categories
    println!("📁 CATEGORIES");
    println!("-------------");
    match client.get_categories().await {
        Ok(categories) => {
            println!("📊 Total categories: {}", categories.len());
            for (i, category) in categories.iter().enumerate() {
                println!(
                    "  {}. ID: {}, Name: {}, Description: {:?}",
                    i + 1,
                    category.id,
                    category.name,
                    category.description
                );
            }
        }
        Err(e) => println!("❌ Failed to list categories: {}", e),
    }

    println!();

    // List Assets
    println!("💰 ASSETS");
    println!("---------");
    match client.get_assets().await {
        Ok(assets) => {
            println!("📊 Total assets: {}", assets.len());
            for (i, asset) in assets.iter().enumerate() {
                println!("  {}. Asset Details:", i + 1);
                println!("     Name: {}", asset.name);
                println!("     UUID: {}", asset.asset_uuid);
                println!("     Asset ID: {}", asset.asset_id);
                println!("     Issuer: {}", asset.issuer);
                println!("     Ticker: {:?}", asset.ticker);
                println!("     Precision: {}", asset.precision);
                println!("     Domain: {:?}", asset.domain);
                println!("     Pubkey: {:?}", asset.pubkey);
                println!("     Reissuance Token ID: {:?}", asset.reissuance_token_id);
                println!("     Requirements: {:?}", asset.requirements);
                println!("     Is Registered: {}", asset.is_registered);
                println!("     Is Authorized: {}", asset.is_authorized);
                println!("     Is Locked: {}", asset.is_locked);
                println!("     Transfer Restricted: {}", asset.transfer_restricted);
                println!("     Issuer Authorization Endpoint: {:?}", asset.issuer_authorization_endpoint);
                if i < assets.len() - 1 {
                    println!();
                }
            }
        }
        Err(e) => println!("❌ Failed to list assets: {}", e),
    }

    Ok(())
}

async fn cleanup_all_resources(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    // Delete all assets first (they may depend on other resources)
    delete_all_assets(client).await?;

    // Delete categories
    delete_all_categories(client).await?;

    // Delete registered users
    delete_all_registered_users(client).await?;

    // Unlock and revoke assets from managers
    revoke_all_manager_assets(client).await?;

    println!("\n✅ Complete cleanup finished!");
    println!("⚠️  WARNING: AMP environment is now completely clean!");
    println!("   You may need to recreate managers to continue using the API.");
    Ok(())
}

async fn delete_all_assets(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Deleting all assets...");

    match client.get_assets().await {
        Ok(assets) => {
            if assets.is_empty() {
                println!("   No assets to delete");
                return Ok(());
            }

            println!("   Found {} assets to delete", assets.len());
            let mut deleted_count = 0;
            let mut failed_count = 0;

            for asset in assets {
                print!(
                    "   Deleting asset '{}' (UUID: {})... ",
                    asset.name, asset.asset_uuid
                );
                match client.delete_asset(&asset.asset_uuid).await {
                    Ok(_) => {
                        println!("✅");
                        deleted_count += 1;
                    }
                    Err(e) => {
                        println!("❌ Error: {}", e);
                        failed_count += 1;
                    }
                }
            }

            println!(
                "   📊 Assets deleted: {}, Failed: {}",
                deleted_count, failed_count
            );
        }
        Err(e) => println!("   ❌ Failed to list assets for deletion: {}", e),
    }

    Ok(())
}

async fn delete_all_categories(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Deleting all categories...");

    match client.get_categories().await {
        Ok(categories) => {
            if categories.is_empty() {
                println!("   No categories to delete");
                return Ok(());
            }

            println!("   Found {} categories to delete", categories.len());
            let mut deleted_count = 0;
            let mut failed_count = 0;

            for category in categories {
                print!(
                    "   Deleting category '{}' (ID: {})... ",
                    category.name, category.id
                );
                match client.delete_category(category.id).await {
                    Ok(_) => {
                        println!("✅");
                        deleted_count += 1;
                    }
                    Err(e) => {
                        println!("❌ Error: {}", e);
                        failed_count += 1;
                    }
                }
            }

            println!(
                "   📊 Categories deleted: {}, Failed: {}",
                deleted_count, failed_count
            );
        }
        Err(e) => println!("   ❌ Failed to list categories for deletion: {}", e),
    }

    Ok(())
}

async fn delete_all_registered_users(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Deleting all registered users...");

    match client.get_registered_users().await {
        Ok(users) => {
            if users.is_empty() {
                println!("   No registered users to delete");
                return Ok(());
            }

            println!("   Found {} registered users to delete", users.len());
            let mut deleted_count = 0;
            let mut failed_count = 0;

            for user in users {
                print!("   Deleting user '{}' (ID: {})... ", user.name, user.id);
                match client.delete_registered_user(user.id).await {
                    Ok(_) => {
                        println!("✅");
                        deleted_count += 1;
                    }
                    Err(e) => {
                        println!("❌ Error: {}", e);
                        failed_count += 1;
                    }
                }
            }

            println!(
                "   📊 Users deleted: {}, Failed: {}",
                deleted_count, failed_count
            );
        }
        Err(e) => println!("   ❌ Failed to list registered users for deletion: {}", e),
    }

    Ok(())
}

async fn revoke_all_manager_assets(client: &ApiClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("🗑️  Unlocking managers and revoking their assets...");

    match client.get_managers().await {
        Ok(managers) => {
            if managers.is_empty() {
                println!("   No managers to process");
                return Ok(());
            }

            println!("   Found {} managers to process", managers.len());
            let mut unlocked_count = 0;
            let mut revoked_count = 0;
            let mut failed_count = 0;

            for manager in managers {
                print!(
                    "   Processing manager '{}' (ID: {})... ",
                    manager.username, manager.id
                );

                // First unlock if locked
                if manager.is_locked {
                    match client.unlock_manager(manager.id).await {
                        Ok(_) => {
                            print!("unlocked, ");
                            unlocked_count += 1;
                        }
                        Err(e) => {
                            println!("❌ Failed to unlock: {}", e);
                            failed_count += 1;
                            continue;
                        }
                    }
                }

                // Then revoke all assets from manager
                match client.revoke_manager(manager.id).await {
                    Ok(_) => {
                        println!("revoked all assets ✅");
                        revoked_count += 1;
                    }
                    Err(e) => {
                        println!("❌ Failed to revoke assets: {}", e);
                        failed_count += 1;
                    }
                }
            }

            println!(
                "   📊 Managers: {} unlocked, {} assets revoked, {} failed",
                unlocked_count, revoked_count, failed_count
            );
        }
        Err(e) => println!("   ❌ Failed to list managers for asset revocation: {}", e),
    }

    Ok(())
}
