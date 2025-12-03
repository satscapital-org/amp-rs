use amp_rs::{model::Asset, ApiClient};
use std::env;

// Protected resources that should not be deleted
const PROTECTED_CATEGORY_ID: i64 = 28273;
const PROTECTED_USER_IDS: &[i64] = &[1194, 1203, 1880, 1936, 2137, 2148];

// Test environment resources that should be preserved
const TEST_CATEGORY_NAME: &str = "Test Environment Category";
const TEST_USER_GAIDS: &[&str] = &[
    "GAbzSbgCZ6M6WU85rseKTrfehPsjt",
    "GAQzmXM7jVaKAwtHGXHENgn5KUUmL",
    "GA42D48VRVzW8MxMEuWtRdJzDq4LBF",
    "GA2M8u2rCJ3jP4YGuE8o4Po61ftwbQ",
    "GA3haAs7uhdrbHNdZvuiXtuD1HCSAG",
    "GA4MAzHdepvHUxNQ4Z2a5F5nuNe1Dv",
    "GA4Bdf2hPtMajjT1uH5PvXPGgVAx2Z", // User 1194 - used in multiple live tests
    "GA4UwSzJb5EbyeCk2VDG4euhyhkiNX", // Used in test_delete_asset_live
    "GA2HsrczzwaFzdJiw5NJM8P4iWKQh1", // Used in multiple live tests
    "GA3tJqC58PwiCjp4tPkCjNkPnVzLqn", // Used in live tests
    "GAbYScu6jkWUND2jo3L4KJxyvo55d", // Used in multiple live tests
    "GA3DS3emT12zDF4RGywBvJqZfhefNp", // User 1203 - used in multiple live tests
    "GA44YYwPM8vuRMmjFL8i5kSqXhoTW2", // Used in mock and live tests
    // Additional protected user GAIDs
    "GA2FEky1FA1UqgAaZ8B4qT5vqrx1U4", // Graciela Villalta
    "GA3KL5bQweqDonjjX6FyaHPq5CAvTk", // Armando
    "GAMeYrKxFNqBTFsp4VfVF9FaRsM5x",  // Otto Boris
    "GAsBYxnpY8dxZyeyxhmu7TJmCdjgr",  // Bladimir Aly Henriquez
    "GAFnuw11Y1ZQVC5jeVztws4b4asgY",  // Diego CR
    "GA37Q8YmxDMExXny9915qzbWb458AD", // Bladimir Aly Henriquez (second GAID)
    "GA4MH22Y9aKPyDCriTpYRLDdsMNfZQ",
];
const TEST_ASSET_NAME: &str = "Test Environment Asset";
const PROTECTED_ASSET_UUIDS: &[&str] = &[
    "fff0928b-f78e-4a2c-bfa0-2c70bb72d545", // Distribution test asset - used in test_get_asset_distribution_live
    "9bcd9987-9544-439f-80b3-6d76b072fd9b", // Asset ID: 02b5a290ff1ce9807551c297a6f87e99b4dda2e746e26e06415393c3c6721f87
    "2982e097-918e-4113-a4f9-afb86bd03886", // Asset ID: 98912dd98abf410320ad2e6b4b2e9537f702d0d04ca575b22a7387233c0af9a2
    "b47d7afd-bc03-4850-83e4-e6a3428a13a0", // Asset ID: 04eebba5d2710d1e86aa6f12ef3ac8c2b20ba780d0d111c2358de2cbc89edcf6 Registered with Ticker
    "d9c37019-1287-4dbf-bcd9-f6a747f28fd6", // Asset ID: fbd776438be416a47a963d324b3f96b3239c88ed8814f8b4e5027b2e66c3fd78 Registered with Ticker
    "1d7245d2-7cbb-4092-9256-d9674c95684a", // New distribution asset (ID: 651f2acf48bd02d905e463cb1b57677e6459c0afc72c267114b28fc67a86c381)
    "716cb816-6cc7-469d-a41f-f4ed1c0d2dce", // Used in test_get_gaid_asset_balance_live and test_get_gaid_balance_live
    "5fd36bad-f0af-4b13-a0b5-fb1a91b751a4", // Asset referenced in live tests
    "49d36560-78be-4bef-aa62-bf64967d3634", // Asset referenced in live tests
    "550e8400-e29b-41d4-a716-446655440000", // Used in test_burn_confirm_live
];

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
            let test_assets_count = assets
                .iter()
                .filter(|a| {
                    a.name == TEST_ASSET_NAME
                        || PROTECTED_ASSET_UUIDS.contains(&a.asset_uuid.as_str())
                })
                .count();
            println!(
                "💰 Assets to delete: {} ({} locked, {} test assets protected)",
                assets.len() - test_assets_count,
                locked_count,
                test_assets_count
            );
            if !assets.is_empty() {
                let mut total_assignments = 0;
                let mut total_category_removals = 0;
                let mut total_distributions_to_cancel = 0;
                for asset in assets.iter().take(3) {
                    let assignment_count =
                        match client.get_asset_assignments(&asset.asset_uuid).await {
                            Ok(assignments) => {
                                total_assignments += assignments.len();
                                assignments.len()
                            }
                            Err(_) => 0,
                        };
                    let category_count = asset
                        .requirements
                        .iter()
                        .filter(|&&cat_id| cat_id != PROTECTED_CATEGORY_ID)
                        .count();
                    total_category_removals += category_count;

                    let distribution_count =
                        match client.get_asset_distributions(&asset.asset_uuid).await {
                            Ok(distributions) => {
                                let unconfirmed_count = distributions
                                    .iter()
                                    .filter(|d| {
                                        matches!(
                                            d.distribution_status,
                                            amp_rs::model::Status::Unconfirmed
                                        )
                                    })
                                    .count();
                                total_distributions_to_cancel += unconfirmed_count;
                                unconfirmed_count
                            }
                            Err(_) => 0,
                        };

                    let lock_status = if asset.is_locked { " 🔒" } else { "" };
                    let distribution_status = if distribution_count > 0 {
                        format!(" 📤{}", distribution_count)
                    } else {
                        "".to_string()
                    };
                    println!(
                        "   • {} ({:?}) - {} assignments, {} categories{}{}",
                        asset.name,
                        asset.ticker,
                        assignment_count,
                        category_count,
                        lock_status,
                        distribution_status
                    );
                }
                if assets.len() > 3 {
                    // Count assignments, categories, and distributions for remaining assets
                    for asset in assets.iter().skip(3) {
                        if let Ok(assignments) =
                            client.get_asset_assignments(&asset.asset_uuid).await
                        {
                            total_assignments += assignments.len();
                        }
                        let category_count = asset
                            .requirements
                            .iter()
                            .filter(|&&cat_id| cat_id != PROTECTED_CATEGORY_ID)
                            .count();
                        total_category_removals += category_count;

                        if let Ok(distributions) =
                            client.get_asset_distributions(&asset.asset_uuid).await
                        {
                            let unconfirmed_count = distributions
                                .iter()
                                .filter(|d| {
                                    matches!(
                                        d.distribution_status,
                                        amp_rs::model::Status::Unconfirmed
                                    )
                                })
                                .count();
                            total_distributions_to_cancel += unconfirmed_count;
                        }
                    }
                    println!("   ... and {} more assets", assets.len() - 3);
                }
                println!("   📋 Total assignments to delete: {}", total_assignments);
                println!("   📁 Total category removals: {}", total_category_removals);
                if total_distributions_to_cancel > 0 {
                    println!(
                        "   📤 Total in-progress distributions to cancel: {}",
                        total_distributions_to_cancel
                    );
                }
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
    // NOTE: This preview only checks primary GAIDs. The actual deletion will also check
    // secondary GAIDs via get_registered_user_gaids() for complete protection.
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
                "👤 Registered users to delete: ~{} (excluding protected user IDs and test users)",
                deletable_users.len()
            );
            println!(
                "   Note: {} users protected by primary GAID, {} by user ID",
                test_users_count,
                users.iter().filter(|u| PROTECTED_USER_IDS.contains(&u.id)).count()
            );
            println!("   (Secondary GAIDs will also be checked during deletion)");
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
    let mut total_categories_removed = 0;
    let mut total_category_removal_errors = 0;
    let mut total_distributions_cancelled = 0;
    let mut total_distribution_cancel_errors = 0;

    for asset in assets {
        // Skip test environment assets and protected UUIDs
        if asset.name == TEST_ASSET_NAME
            || PROTECTED_ASSET_UUIDS.contains(&asset.asset_uuid.as_str())
        {
            let protection_reason = if asset.name == TEST_ASSET_NAME {
                "test environment asset"
            } else {
                "protected UUID"
            };
            println!(
                "   Skipping protected {} '{}' (UUID: {})... 🛡️",
                protection_reason, asset.name, asset.asset_uuid
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

        // Check for and cancel any in-progress distributions first
        let (distributions_cancelled, distribution_cancel_errors) =
            cancel_asset_distributions(client, &asset.asset_uuid).await;
        total_distributions_cancelled += distributions_cancelled;
        total_distribution_cancel_errors += distribution_cancel_errors;

        // Remove asset from all categories first to avoid "Cannot delete an asset which has some requirements" error
        // The requirements field contains category IDs that the asset belongs to
        let (categories_removed, category_errors) =
            remove_asset_from_all_categories(client, &asset).await;
        total_categories_removed += categories_removed;
        total_category_removal_errors += category_errors;

        // Delete all assignments for this asset
        let (assignments_deleted, assignment_errors) =
            delete_asset_assignments(client, &asset.asset_uuid).await;
        total_assignments_deleted += assignments_deleted;
        total_assignment_errors += assignment_errors;

        // Finally delete the asset itself
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
        "   📊 Categories removed: {} removed, {} errors",
        total_categories_removed, total_category_removal_errors
    );
    println!(
        "   📊 Unlocked: {} assets, {} unlock errors",
        unlocked_count, unlock_error_count
    );
    println!(
        "   📊 Distributions: {} cancelled, {} errors",
        total_distributions_cancelled, total_distribution_cancel_errors
    );
    Ok(())
}

async fn remove_asset_from_all_categories(client: &ApiClient, asset: &Asset) -> (usize, usize) {
    if asset.requirements.is_empty() {
        println!("     ✅ Asset not in any categories");
        return (0, 0);
    }

    println!(
        "     Found {} categories to remove asset from",
        asset.requirements.len()
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for &category_id in &asset.requirements {
        // Skip protected category
        if category_id == PROTECTED_CATEGORY_ID {
            println!(
                "       Skipping protected category ID {}... 🛡️",
                category_id
            );
            continue;
        }

        print!("       Removing from category {}... ", category_id);
        match client
            .remove_asset_from_category(category_id, &asset.asset_uuid)
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

    if categories.is_empty() {
        println!("   ✅ No categories to delete");
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut protected_count = 0;

    for category in categories {
        // Check protection INSIDE the loop to ensure we never miss a protected category
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

    if users.is_empty() {
        println!("   ✅ No registered users to delete");
        return Ok(());
    }

    let mut success_count = 0;
    let mut error_count = 0;
    let mut protected_count = 0;

    for user in users {
        // Check protection by user ID first
        if PROTECTED_USER_IDS.contains(&user.id) {
            println!(
                "   Skipping protected user '{}' (ID: {}, protected by ID)... 🛡️",
                user.name, user.id
            );
            protected_count += 1;
            continue;
        }

        // Check if the primary GAID is protected
        let primary_gaid_protected = user.gaid.as_ref().map_or(false, |gaid| {
            TEST_USER_GAIDS.iter().any(|&protected| protected == gaid)
        });

        if primary_gaid_protected {
            println!(
                "   Skipping protected user '{}' (ID: {}, primary GAID: {})... 🛡️",
                user.name,
                user.id,
                user.gaid.as_ref().unwrap()
            );
            protected_count += 1;
            continue;
        }

        // CRITICAL: Check ALL GAIDs for this user, not just the primary one
        // A user might have a protected GAID as a secondary GAID
        let all_user_gaids = match client.get_registered_user_gaids(user.id).await {
            Ok(gaids) => gaids,
            Err(e) => {
                println!(
                    "   ⚠️  Could not fetch GAIDs for user '{}' (ID: {}): {} - SKIPPING for safety",
                    user.name, user.id, e
                );
                protected_count += 1; // Treat as protected if we can't verify
                continue;
            }
        };

        let has_protected_gaid = all_user_gaids.iter().any(|user_gaid| {
            TEST_USER_GAIDS.iter().any(|&protected| protected == user_gaid)
        });

        if has_protected_gaid {
            println!(
                "   Skipping protected user '{}' (ID: {}, has protected secondary GAID)... 🛡️",
                user.name, user.id
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
async fn cancel_asset_distributions(client: &ApiClient, asset_uuid: &str) -> (usize, usize) {
    let distributions = match client.get_asset_distributions(asset_uuid).await {
        Ok(distributions) => distributions,
        Err(e) => {
            println!("     ❌ Failed to list distributions: {}", e);
            return (0, 1);
        }
    };

    if distributions.is_empty() {
        println!("     ✅ No distributions to cancel");
        return (0, 0);
    }

    // Filter for unconfirmed (in-progress) distributions
    let unconfirmed_distributions: Vec<_> = distributions
        .iter()
        .filter(|d| matches!(d.distribution_status, amp_rs::model::Status::Unconfirmed))
        .collect();

    if unconfirmed_distributions.is_empty() {
        println!("     ✅ No in-progress distributions to cancel");
        return (0, 0);
    }

    println!(
        "     Found {} in-progress distributions to cancel",
        unconfirmed_distributions.len()
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for distribution in unconfirmed_distributions {
        print!(
            "       Cancelling distribution {}... ",
            distribution.distribution_uuid
        );

        match client
            .cancel_distribution(asset_uuid, &distribution.distribution_uuid)
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
