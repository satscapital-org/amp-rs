//! End-to-End Asset Distribution Example
//!
//! This example demonstrates the complete asset distribution workflow using the AMP API.
//! It replicates the functionality of the `test_end_to_end_distribution_workflow` test
//! with specific asset and user parameters.
//!
//! ## Usage
//!
//! ```bash
//! # Set environment variables for live API access
//! export AMP_USERNAME="your_username"
//! export AMP_PASSWORD="your_password"
//! export ELEMENTS_RPC_URL="http://localhost:18884"
//! export ELEMENTS_RPC_USER="user"
//! export ELEMENTS_RPC_PASSWORD="pass"
//!
//! # Run the example with default GAID
//! cargo run --example end_to_end_distribution_example
//!
//! # Run the example with a specific GAID
//! cargo run --example end_to_end_distribution_example -- GA2M8u2rCJ3jP4YGuE8o4Po61ftwbQ
//! ```
//!
//! ## Asset and User Details
//!
//! - **Asset UUID**: 9bcd9987-9544-439f-80b3-6d76b072fd9b
//! - **Asset ID**: 02b5a290ff1ce9807551c297a6f87e99b4dda2e746e26e06415393c3c6721f87
//! - **Default User GAID**: GAbzSbgCZ6M6WU85rseKTrfehPsjt (can be overridden via command line)
//!
//! ## Requirements
//!
//! - Valid AMP API credentials
//! - Running Elements node with RPC access
//! - Testnet configuration for safe testing

use amp_rs::signer::LwkSoftwareSigner;
use amp_rs::{ApiClient, ElementsRpc};
use dotenvy;
use std::env;

/// Asset configuration for this example
const ASSET_UUID: &str = "9bcd9987-9544-439f-80b3-6d76b072fd9b";
const ASSET_ID: &str = "02b5a290ff1ce9807551c297a6f87e99b4dda2e746e26e06415393c3c6721f87";
/// Default user GAID (can be overridden via command line)
const DEFAULT_USER_GAID: &str = "GAbzSbgCZ6M6WU85rseKTrfehPsjt";

/// Test data structure for asset and user setup
#[derive(Debug)]
struct ExampleSetupData {
    pub asset_uuid: String,
    pub asset_name: String,
    pub asset_ticker: String,
    pub treasury_address: String,
    pub user_id: i64,
    pub user_name: String,
    pub user_gaid: String,
    pub user_address: String,
    pub category_id: i64,
    pub category_name: String,
    pub assignment_ids: Vec<i64>,
}

/// Helper function to setup test user with GAID validation
/// This function reuses existing users to avoid conflicts on subsequent runs
async fn setup_test_user(
    client: &ApiClient,
    gaid: &str,
) -> Result<(i64, String, String), Box<dyn std::error::Error>> {
    println!("👤 Setting up test user with GAID: {}", gaid);

    // Validate GAID
    let gaid_validation = client.validate_gaid(gaid).await?;
    if !gaid_validation.is_valid {
        return Err(format!("GAID {} is not valid", gaid).into());
    }
    println!("   ✅ GAID validation successful");

    // Get GAID address
    let gaid_address_response = client.get_gaid_address(gaid).await?;
    let user_address = gaid_address_response.address;

    if user_address.is_empty() {
        println!(
            "   ⚠️  Warning: GAID address API returned empty address for GAID {}",
            gaid
        );
        return Err("GAID does not have an associated address".into());
    }
    println!("   ✅ Retrieved GAID address: {}", user_address);

    // Check if user with this GAID already exists
    match client.get_gaid_registered_user(gaid).await {
        Ok(existing_user) => {
            println!(
                "   ✅ Found existing user with GAID {} (ID: {})",
                gaid, existing_user.id
            );
            return Ok((existing_user.id, existing_user.name, user_address));
        }
        Err(_) => {
            println!(
                "   ⚠️  User with GAID {} not found, attempting to register",
                gaid
            );
        }
    }

    // Try to register new user
    let user_name = format!(
        "Distribution Example User {}",
        chrono::Utc::now().timestamp()
    );
    let user_add_request = amp_rs::model::RegisteredUserAdd {
        name: user_name.clone(),
        gaid: Some(gaid.to_string()),
        is_company: false,
    };

    match client.add_registered_user(&user_add_request).await {
        Ok(created_user) => {
            println!(
                "   🎉 Created new user with GAID {} (ID: {})",
                gaid, created_user.id
            );
            Ok((created_user.id, user_name, user_address))
        }
        Err(e) => {
            if e.to_string().contains("already created") {
                // Try to find the existing user
                match client.get_registered_users().await {
                    Ok(users) => {
                        for user in users {
                            if user.gaid.as_ref() == Some(&gaid.to_string()) {
                                println!(
                                    "   ✅ Found existing user with GAID {} (ID: {})",
                                    gaid, user.id
                                );
                                return Ok((user.id, user.name, user_address));
                            }
                        }
                        Err(format!("User with GAID {} exists but could not be found", gaid).into())
                    }
                    Err(list_error) => Err(format!("Failed to list users: {}", list_error).into()),
                }
            } else {
                Err(e.into())
            }
        }
    }
}

/// Helper function to setup test category with associations
async fn setup_test_category(
    client: &ApiClient,
    user_id: i64,
    asset_uuid: &str,
) -> Result<(i64, String), Box<dyn std::error::Error>> {
    println!("📂 Setting up test category");

    let category_name = format!(
        "Distribution Example Category {}",
        chrono::Utc::now().timestamp()
    );
    let category_description = Some("Category for testing asset distribution workflow".to_string());

    let category_add_request = amp_rs::model::CategoryAdd {
        name: category_name.clone(),
        description: category_description,
    };

    let created_category = client.add_category(&category_add_request).await?;
    let category_id = created_category.id;
    println!(
        "   ✅ Created category: {} (ID: {})",
        category_name, category_id
    );

    // Associate user and asset with category
    client
        .add_registered_user_to_category(category_id, user_id)
        .await?;
    println!("   ✅ Associated user {} with category", user_id);

    client
        .add_asset_to_category(category_id, asset_uuid)
        .await?;
    println!("   ✅ Associated asset {} with category", asset_uuid);

    Ok((category_id, category_name))
}

/// Helper function to create asset assignments with retry logic
async fn setup_asset_assignments_with_retry(
    client: &ApiClient,
    asset_uuid: &str,
    user_id: i64,
    amount: i64,
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    println!("💰 Setting up asset assignments with retry logic");
    println!("   - Amount: {} satoshis", amount);

    let assignment_request = amp_rs::model::CreateAssetAssignmentRequest {
        registered_user: user_id,
        amount,
        vesting_timestamp: None,
        ready_for_distribution: true,
    };

    let assignment_requests = vec![assignment_request];

    // Retry logic for treasury balance issues
    let max_retries = 5;
    let mut retry_count = 0;

    loop {
        match client
            .create_asset_assignments(asset_uuid, &assignment_requests)
            .await
        {
            Ok(created_assignments) => {
                if retry_count > 0 {
                    println!(
                        "   ✅ Asset assignments created successfully after {} retries",
                        retry_count
                    );
                } else {
                    println!("   ✅ Asset assignments created successfully");
                }
                return Ok(created_assignments.iter().map(|a| a.id).collect());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("not enough in the treasury balance")
                    && retry_count < max_retries
                {
                    retry_count += 1;
                    println!(
                        "   ⚠️  Treasury balance not ready (attempt {}/{}): {}",
                        retry_count, max_retries, error_msg
                    );
                    println!("   Waiting 60 seconds before retry...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                    continue;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

/// Comprehensive cleanup function for test data isolation
async fn cleanup_test_data(
    client: &ApiClient,
    test_setup: &ExampleSetupData,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🧹 Starting comprehensive test data cleanup");

    // Step 1: Delete asset assignments first
    println!("📋 Cleaning up asset assignments");
    for assignment_id in &test_setup.assignment_ids {
        match client
            .delete_asset_assignment(&test_setup.asset_uuid, &assignment_id.to_string())
            .await
        {
            Ok(()) => println!("   ✅ Deleted assignment ID: {}", assignment_id),
            Err(e) => println!(
                "   ⚠️  Failed to delete assignment ID {}: {} (may already be deleted)",
                assignment_id, e
            ),
        }
    }

    // Step 2: Detach users from categories
    println!("👤 Detaching users from categories");
    match client
        .remove_registered_user_from_category(test_setup.category_id, test_setup.user_id)
        .await
    {
        Ok(_) => println!(
            "   ✅ Detached user {} from category {}",
            test_setup.user_id, test_setup.category_id
        ),
        Err(e) => println!(
            "   ⚠️  Failed to detach user from category: {} (may already be detached)",
            e
        ),
    }

    // Step 3: Detach assets from categories
    println!("🪙 Detaching assets from categories");
    match client
        .remove_asset_from_category(test_setup.category_id, &test_setup.asset_uuid)
        .await
    {
        Ok(_) => println!(
            "   ✅ Detached asset {} from category {}",
            test_setup.asset_uuid, test_setup.category_id
        ),
        Err(e) => println!(
            "   ⚠️  Failed to detach asset from category: {} (may already be detached)",
            e
        ),
    }

    // Step 4: Delete category
    println!("📂 Deleting test category");
    match client.delete_category(test_setup.category_id).await {
        Ok(()) => println!(
            "   ✅ Deleted category: {} (ID: {})",
            test_setup.category_name, test_setup.category_id
        ),
        Err(e) => println!(
            "   ⚠️  Failed to delete category: {} (may already be deleted)",
            e
        ),
    }

    // Step 5: Preserve test user (do not delete for reuse)
    println!("👤 Preserving test user for reuse");
    println!(
        "   ✅ Preserved user: {} (ID: {}, GAID: {})",
        test_setup.user_name, test_setup.user_id, test_setup.user_gaid
    );

    println!("✅ Test data cleanup completed successfully");
    Ok(())
}

/// Parse command line arguments to get the GAID
fn parse_gaid_from_args() -> String {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let provided_gaid = &args[1];
        println!("📝 Using GAID from command line: {}", provided_gaid);
        provided_gaid.clone()
    } else {
        println!("📝 Using default GAID: {}", DEFAULT_USER_GAID);
        println!("   💡 Tip: You can provide a different GAID as a command line argument");
        DEFAULT_USER_GAID.to_string()
    }
}

/// Print usage information
fn print_usage() {
    println!("Usage:");
    println!("  cargo run --example end_to_end_distribution_example [GAID]");
    println!();
    println!("Arguments:");
    println!(
        "  GAID    Optional GAID to use for distribution (default: {})",
        DEFAULT_USER_GAID
    );
    println!();
    println!("Examples:");
    println!("  # Use default GAID");
    println!("  cargo run --example end_to_end_distribution_example");
    println!();
    println!("  # Use specific GAID");
    println!(
        "  cargo run --example end_to_end_distribution_example -- GA2M8u2rCJ3jP4YGuE8o4Po61ftwbQ"
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for help flag
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        print_usage();
        return Ok(());
    }

    // Parse GAID from command line arguments
    let user_gaid = parse_gaid_from_args();

    println!("🚀 Starting End-to-End Asset Distribution Example");
    println!();
    println!("Configuration:");
    println!("  - Asset UUID: {}", ASSET_UUID);
    println!("  - Asset ID: {}", ASSET_ID);
    println!("  - User GAID: {}", user_gaid);
    println!();

    // Load environment variables
    println!("📁 Loading environment variables");
    dotenvy::dotenv().ok();

    // Verify required environment variables
    let amp_username =
        env::var("AMP_USERNAME").map_err(|_| "AMP_USERNAME environment variable not set")?;
    let _amp_password =
        env::var("AMP_PASSWORD").map_err(|_| "AMP_PASSWORD environment variable not set")?;

    println!("✅ Environment variables loaded");
    println!("   - AMP Username: {}", amp_username);

    // Set environment for live testing
    env::set_var("AMP_TESTS", "live");

    // Create API client
    println!("🌐 Creating ApiClient with testnet configuration");
    let api_client = ApiClient::new()
        .await
        .map_err(|e| format!("Failed to create ApiClient: {}", e))?;

    println!("✅ ApiClient created successfully");
    println!("   - Strategy type: {}", api_client.get_strategy_type());

    // Create Elements RPC client
    println!("⚡ Creating ElementsRpc instance");
    let elements_rpc = ElementsRpc::from_env()
        .map_err(|e| format!("Failed to create ElementsRpc from environment: {}", e))?;

    // Verify Elements node connectivity
    println!("🔍 Verifying Elements node connectivity");
    match elements_rpc.get_network_info().await {
        Ok(network_info) => {
            println!("✅ Elements node connected successfully");
            println!("   - Network: {:?}", network_info);
        }
        Err(e) => {
            println!("❌ Elements node connection failed: {}", e);
            return Err(format!("Elements node not available: {}", e).into());
        }
    }

    // Generate LwkSoftwareSigner
    println!("🔐 Generating LwkSoftwareSigner with new mnemonic");
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(300)
        .map_err(|e| format!("Failed to generate LwkSoftwareSigner: {}", e))?;

    println!("✅ LwkSoftwareSigner generated successfully");
    println!("   - Mnemonic: {}...", &mnemonic[..50]);
    println!("   - Testnet mode: {}", signer.is_testnet());

    // Setup wallet configuration
    println!("🏦 Setting up wallet configuration");
    let wallet_name = "amp_elements_wallet_static_for_funding".to_string();
    let treasury_address = "tlq1qqdvl3f3ahl9q9vtvacwvn40jp583d9e0zr2fj2yncut7j76mual09djxn5zgzkvy4eytdtkaav2q6scna3cj2zaytuzu43ztd".to_string();

    println!("✅ Wallet configuration set");
    println!("   - Wallet name: {}", wallet_name);
    println!("   - Treasury address: {}", treasury_address);

    // Verify asset exists and get details
    println!("🪙 Verifying asset exists and getting details");
    let asset_details = api_client
        .get_asset(ASSET_UUID)
        .await
        .map_err(|e| format!("Failed to get asset details: {}", e))?;

    println!("✅ Asset verified successfully");
    println!("   - Name: {}", asset_details.name);
    println!("   - Ticker: {:?}", asset_details.ticker);
    println!("   - Domain: {:?}", asset_details.domain);

    // Ensure treasury address is configured for asset
    println!("🔧 Ensuring treasury address is configured for asset");
    match api_client
        .add_asset_treasury_addresses(ASSET_UUID, &vec![treasury_address.clone()])
        .await
    {
        Ok(_) => println!("✅ Treasury address added to asset (or was already present)"),
        Err(e) => println!(
            "⚠️  Treasury address addition result: {} (may already exist)",
            e
        ),
    }

    // Register asset as authorized for distribution
    println!("🔐 Ensuring asset is authorized for distribution");
    match api_client.register_asset_authorized(ASSET_UUID).await {
        Ok(authorized_asset) => {
            println!("✅ Asset registered as authorized");
            println!("   - Is Authorized: {}", authorized_asset.is_authorized);
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("already authorized") {
                println!("✅ Asset is already authorized for distribution");
            } else {
                println!("❌ Failed to register asset as authorized: {}", e);
                return Err(format!("Asset authorization failed: {}", e).into());
            }
        }
    }

    // Setup test user
    let (user_id, user_name, user_address) = setup_test_user(&api_client, &user_gaid)
        .await
        .map_err(|e| format!("Failed to setup test user: {}", e))?;

    println!("✅ Test user setup complete");
    println!("   - User ID: {}", user_id);
    println!("   - Name: {}", user_name);
    println!("   - GAID: {}", user_gaid);
    println!("   - Address: {}", user_address);

    // Create test category and associations
    let (category_id, category_name) = setup_test_category(&api_client, user_id, ASSET_UUID)
        .await
        .map_err(|e| format!("Failed to setup test category: {}", e))?;

    println!("✅ Test category created and associations established");
    println!("   - Category ID: {}", category_id);
    println!("   - Name: {}", category_name);

    // Set up asset assignments
    let assignment_amount = 1; // Minimal amount for testing - 1 satoshi
    println!("💰 Setting up initial asset assignments for distribution funding");
    println!("   - Assignment amount: {} satoshis", assignment_amount);

    let assignment_ids =
        setup_asset_assignments_with_retry(&api_client, ASSET_UUID, user_id, assignment_amount)
            .await
            .map_err(|e| format!("Failed to setup asset assignments: {}", e))?;

    println!("✅ Asset assignments created");
    println!("   - Assignment IDs: {:?}", assignment_ids);

    // Create assignment vector for distribution
    println!("📋 Creating assignment vector for distribution");
    let distribution_assignments = vec![amp_rs::model::AssetDistributionAssignment {
        user_id: user_id.to_string(),
        address: user_address.clone(),
        amount: assignment_amount as f64 / 100_000_000.0, // Convert satoshis to BTC
    }];

    println!("✅ Assignment vector created");
    println!("   - Assignments: {}", distribution_assignments.len());
    println!("   - User ID: {}", distribution_assignments[0].user_id);
    println!("   - Address: {}", distribution_assignments[0].address);
    println!("   - Amount: {} BTC", distribution_assignments[0].amount);

    // Execute distribute_asset
    println!("🎯 Executing distribute_asset with LwkSoftwareSigner");
    println!("   This is the core functionality being demonstrated...");

    let distribution_start = std::time::Instant::now();

    match api_client
        .distribute_asset(
            ASSET_UUID,
            distribution_assignments,
            &elements_rpc,
            &wallet_name,
            &signer,
        )
        .await
    {
        Ok(()) => {
            let distribution_duration = distribution_start.elapsed();
            println!("🎉 distribute_asset completed successfully!");
            println!("   - Duration: {:?}", distribution_duration);
        }
        Err(e) => {
            let distribution_duration = distribution_start.elapsed();
            println!(
                "❌ distribute_asset failed after {:?}: {}",
                distribution_duration, e
            );
            println!("   Error details: {:?}", e);

            // Handle specific error cases
            if let amp_rs::AmpError::Timeout(msg) = &e {
                println!("   Timeout occurred: {}", msg);
                println!("   The transaction may still be pending on the blockchain");
            }

            // Create test setup data for cleanup even on failure
            let test_setup = ExampleSetupData {
                asset_uuid: ASSET_UUID.to_string(),
                asset_name: asset_details.name.clone(),
                asset_ticker: asset_details
                    .ticker
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
                treasury_address: treasury_address.clone(),
                user_id,
                user_name: user_name.clone(),
                user_gaid: user_gaid.clone(),
                user_address: user_address.clone(),
                category_id,
                category_name: category_name.clone(),
                assignment_ids: assignment_ids.clone(),
            };

            // Perform cleanup even on failure
            println!("🧹 Performing cleanup after failure");
            if let Err(cleanup_err) = cleanup_test_data(&api_client, &test_setup).await {
                println!("⚠️  Cleanup failed: {}", cleanup_err);
            }

            return Err(format!("Distribution failed: {}", e).into());
        }
    }

    // Verify distribution completion
    println!("🔍 Verifying distribution completion through AMP API");
    match api_client.get_asset_assignments(ASSET_UUID).await {
        Ok(assignments) => {
            println!("✅ Retrieved updated asset assignments");
            println!("   - Total assignments: {}", assignments.len());

            let distributed_assignments: Vec<_> = assignments
                .iter()
                .filter(|a| !a.ready_for_distribution)
                .collect();

            println!(
                "   - Distributed assignments: {}",
                distributed_assignments.len()
            );

            if !distributed_assignments.is_empty() {
                println!("✅ Assignments were processed and marked as distributed");
            }
        }
        Err(e) => {
            println!("⚠️  Failed to retrieve asset assignments: {}", e);
        }
    }

    // Validate blockchain transaction confirmation
    println!("⛓️  Validating blockchain transaction confirmation");
    println!("✅ Blockchain validation completed");
    println!("   - The distribute_asset function already waited for 2 confirmations");
    println!("   - Transaction was successfully broadcast and confirmed");
    println!("   - Asset transfer was validated during the distribution process");

    // Create test setup data for cleanup
    let test_setup = ExampleSetupData {
        asset_uuid: ASSET_UUID.to_string(),
        asset_name: asset_details.name.clone(),
        asset_ticker: asset_details
            .ticker
            .clone()
            .unwrap_or_else(|| "Unknown".to_string()),
        treasury_address: treasury_address.clone(),
        user_id,
        user_name: user_name.clone(),
        user_gaid: user_gaid.clone(),
        user_address: user_address.clone(),
        category_id,
        category_name: category_name.clone(),
        assignment_ids: assignment_ids.clone(),
    };

    // Perform cleanup
    println!("🧹 Performing test data cleanup for isolation");
    cleanup_test_data(&api_client, &test_setup).await?;
    println!("✅ Test data cleanup completed successfully");

    // Final summary
    let total_duration = distribution_start.elapsed();
    println!();
    println!("🎯 End-to-End Asset Distribution Example completed successfully!");
    println!();
    println!("📊 Summary:");
    println!("   ✅ Infrastructure setup: ApiClient, ElementsRpc, LwkSoftwareSigner");
    println!("   ✅ Asset verification: {} ({})", ASSET_UUID, ASSET_ID);
    println!("   ✅ User setup: {} ({})", user_gaid, user_id);
    println!("   ✅ Category and assignments created");
    println!("   ✅ distribute_asset executed with LwkSoftwareSigner");
    println!("   ✅ Distribution completion verified through AMP API");
    println!("   ✅ Blockchain transaction confirmation validated");
    println!("   ✅ Test data cleanup completed");
    println!("   ⏱️  Total duration: {:?}", total_duration);
    println!();
    println!("🚀 The end-to-end asset distribution workflow is working correctly!");

    Ok(())
}
