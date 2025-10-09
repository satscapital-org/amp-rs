use amp_rs::ApiClient;
use std::env;

// Protected resources that should not be deleted
const PROTECTED_CATEGORY_ID: i64 = 28273;
const PROTECTED_USER_IDS: &[i64] = &[1194, 1203];

// Test environment resources that should be preserved
const TEST_CATEGORY_NAME: &str = "Test Environment Category";
const TEST_USER_GAIDS: &[&str] = &[
    "GAbzSbgCZ6M6WU85rseKTrfehPsjt",
    "GAQzmXM7jVaKAwtHGXHENgn5KUUmL",
];
const TEST_ASSET_NAME: &str = "Test Environment Asset";

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

    // Preview assets and their assignments
    match client.get_assets().await {
        Ok(assets) => {
            let locked_count = assets.iter().filter(|a| a.is_locked).count();
            let test_assets_count = assets.iter().filter(|a| a.name == TEST_ASSET_NAME).count();
            println!(
                "💰 Assets to delete: {} ({} locked, {} test assets protected)",
                assets.len() - test_assets_count,
                locked_count,
                test_assets_count
            );
            if !assets.is_empty() {
                let mut total_assignments = 0;
                for asset in assets.iter().take(3) {
                    let assignment_count =
                        match client.get_asset_assignments(&asset.asset_uuid).await {
                            Ok(assignments) => {
                                total_assignments += assignments.len();
                                assignments.len()
                            }
                            Err(_) => 0,
                        };
                    let lock_status = if asset.is_locked { " 🔒" } else { "" };
                    println!(
                        "   • {} ({:?}) - {} assignments{}",
                        asset.name, asset.ticker, assignment_count, lock_status
                    );
                }
                if assets.len() > 3 {
                    // Count assignments for remaining assets
                    for asset in assets.iter().skip(3) {
                        if let Ok(assignments) =
                            client.get_asset_assignments(&asset.asset_uuid).await
                        {
                            total_assignments += assignments.len();
                        }
                    }
                    println!("   ... and {} more assets", assets.len() - 3);
                }
                println!("   📋 Total assignments to delete: {}", total_assignments);
                if locked_count > 0 {
                    println!(
                        "   🔓 {} locked assets will be unlocked before deletion",
                        locked_count
                    );
                }
            }
        }
        Err(e) => println!("❌ Could not preview assets: {}", e),
    }

    // Preview categories
    match client.get_categories().await {
        Ok(categories) => {
            let deletable_categories: Vec<_> = categories
                .iter()
                .filter(|cat| cat.id != PROTECTED_CATEGORY_ID && cat.name != TEST_CATEGORY_NAME)
                .collect();
            let test_categories_count = categories
                .iter()
                .filter(|cat| cat.name == TEST_CATEGORY_NAME)
                .count();
            println!(
                "📁 Categories to delete: {} (excluding protected category ID {} and {} test categories)",
                deletable_categories.len(),
                PROTECTED_CATEGORY_ID,
                test_categories_count
            );
            if !deletable_categories.is_empty() {
                for category in deletable_categories.iter().take(3) {
                    println!("   • {} (ID: {})", category.name, category.id);
                }
                if deletable_categories.len() > 3 {
                    println!("   ... and {} more", deletable_categories.len() - 3);
                }
            }
        }
        Err(e) => println!("❌ Could not preview categories: {}", e),
    }

    // Preview registered users
    match client.get_registered_users().await {
        Ok(users) => {
            let deletable_users: Vec<_> = users
                .iter()
                .filter(|user| {
                    !PROTECTED_USER_IDS.contains(&user.id)
                        && !TEST_USER_GAIDS
                            .iter()
                            .any(|&gaid| user.gaid.as_ref() == Some(&gaid.to_string()))
                })
                .collect();
            let test_users_count = users
                .iter()
                .filter(|user| {
                    TEST_USER_GAIDS
                        .iter()
                        .any(|&gaid| user.gaid.as_ref() == Some(&gaid.to_string()))
                })
                .count();
            println!(
                "👤 Registered users to delete: {} (excluding protected user IDs {:?} and {} test users)",
                deletable_users.len(),
                PROTECTED_USER_IDS,
                test_users_count
            );
            if !deletable_users.is_empty() {
                for user in deletable_users.iter().take(3) {
                    println!("   • {} (ID: {})", user.name, user.id);
                }
                if deletable_users.len() > 3 {
                    println!("   ... and {} more", deletable_users.len() - 3);
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
    println!("🗑️  Deleting assets and their assignments...");

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

    let mut asset_success_count = 0;
    let mut asset_error_count = 0;
    let mut total_assignments_deleted = 0;
    let mut total_assignment_errors = 0;
    let mut unlocked_count = 0;
    let mut unlock_error_count = 0;
    let mut protected_assets = 0;

    for asset in assets {
        // Skip test environment asset
        if asset.name == TEST_ASSET_NAME {
            println!(
                "   Skipping protected test asset '{}' (UUID: {})... 🛡️",
                asset.name, asset.asset_uuid
            );
            protected_assets += 1;
            continue;
        }
        println!(
            "   Processing asset '{}' ({:?})...",
            asset.name, asset.ticker
        );

        // Check if asset is locked and unlock it if necessary
        if asset.is_locked {
            print!("     Asset is locked, unlocking... ");
            match client.unlock_asset(&asset.asset_uuid).await {
                Ok(_) => {
                    println!("✅");
                    unlocked_count += 1;
                }
                Err(e) => {
                    println!("❌ {}", e);
                    unlock_error_count += 1;
                    // Continue with deletion attempt even if unlock fails
                }
            }
        } else {
            println!("     Asset is not locked");
        }

        // First, delete all assignments for this asset
        let (assignments_deleted, assignment_errors) =
            delete_asset_assignments(client, &asset.asset_uuid).await;
        total_assignments_deleted += assignments_deleted;
        total_assignment_errors += assignment_errors;

        // Then delete the asset itself
        print!("     Deleting asset... ");
        match client.delete_asset(&asset.asset_uuid).await {
            Ok(_) => {
                println!("✅");
                asset_success_count += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                asset_error_count += 1;
            }
        }
    }

    println!(
        "   📊 Assets: {} deleted, {} errors, {} protected",
        asset_success_count, asset_error_count, protected_assets
    );
    println!(
        "   📊 Assignments: {} deleted, {} errors",
        total_assignments_deleted, total_assignment_errors
    );
    println!(
        "   📊 Unlocked: {} assets, {} unlock errors",
        unlocked_count, unlock_error_count
    );
    Ok(())
}

async fn delete_asset_assignments(client: &ApiClient, asset_uuid: &str) -> (usize, usize) {
    let assignments = match client.get_asset_assignments(asset_uuid).await {
        Ok(assignments) => assignments,
        Err(e) => {
            println!("     ❌ Failed to list assignments: {}", e);
            return (0, 1);
        }
    };

    if assignments.is_empty() {
        println!("     ✅ No assignments to delete");
        return (0, 0);
    }

    println!("     Found {} assignments to delete", assignments.len());

    let mut success_count = 0;
    let mut error_count = 0;

    for assignment in assignments {
        let assignment_id = assignment.id.to_string();
        print!(
            "       Deleting assignment {} (user: {})... ",
            assignment_id, assignment.registered_user
        );

        match client
            .delete_asset_assignment(asset_uuid, &assignment_id)
            .await
        {
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

    (success_count, error_count)
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

    let deletable_categories: Vec<_> = categories
        .into_iter()
        .filter(|cat| cat.id != PROTECTED_CATEGORY_ID && cat.name != TEST_CATEGORY_NAME)
        .collect();

    if deletable_categories.is_empty() {
        println!(
            "   ✅ No categories to delete (protected category ID {} preserved)",
            PROTECTED_CATEGORY_ID
        );
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut protected_count = 0;

    for category in deletable_categories {
        if category.id == PROTECTED_CATEGORY_ID || category.name == TEST_CATEGORY_NAME {
            println!(
                "   Skipping protected category '{}' (ID: {})... 🛡️",
                category.name, category.id
            );
            protected_count += 1;
            continue;
        }

        print!("   Deleting '{}' (ID: {})... ", category.name, category.id);
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

    println!(
        "   📊 Categories: {} deleted, {} errors, {} protected",
        success_count, error_count, protected_count
    );
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

    let deletable_users: Vec<_> = users
        .into_iter()
        .filter(|user| {
            !PROTECTED_USER_IDS.contains(&user.id)
                && !TEST_USER_GAIDS
                    .iter()
                    .any(|&gaid| user.gaid.as_ref() == Some(&gaid.to_string()))
        })
        .collect();

    if deletable_users.is_empty() {
        println!(
            "   ✅ No registered users to delete (protected user IDs {:?} preserved)",
            PROTECTED_USER_IDS
        );
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut protected_count = 0;

    for user in deletable_users {
        if PROTECTED_USER_IDS.contains(&user.id)
            || TEST_USER_GAIDS
                .iter()
                .any(|&gaid| user.gaid.as_ref() == Some(&gaid.to_string()))
        {
            println!(
                "   Skipping protected user '{}' (ID: {}) with GAID: {}... 🛡️",
                user.name,
                user.id,
                user.gaid.as_ref().unwrap_or(&"None".to_string())
            );
            protected_count += 1;
            continue;
        }

        print!("   Deleting user '{}' (ID: {})... ", user.name, user.id);
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

    println!(
        "   📊 Users: {} deleted, {} errors, {} protected",
        success_count, error_count, protected_count
    );
    Ok(())
}
