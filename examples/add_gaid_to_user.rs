//! Add GAID to Registered User Example
//!
//! This example demonstrates the complete workflow of adding a GAID to a registered user:
//! 1. Verify that the user exists and retrieve their information
//! 2. Display all GAIDs currently associated with the user
//! 3. Validate the GAID to be added
//! 4. Add the GAID to the user
//! 5. Report the results
//!
//! Usage:
//!   cargo run --example add_gaid_to_user
//!   cargo run --example add_gaid_to_user -- <GAID>

use amp_rs::ApiClient;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Configuration
    const USER_ID: i64 = 2137;
    const EXPECTED_NAME: &str = "Gregory Mikeska";
    const DEFAULT_GAID: &str = "GA3dRkMv4xTLrzRSci4mwvhKFRUsBN";

    // Get GAID from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let gaid_to_add = if args.len() > 1 {
        &args[1]
    } else {
        DEFAULT_GAID
    };

    println!("=== Add GAID to Registered User Example ===\n");
    println!("Using GAID: {}\n", gaid_to_add);

    // Create API client
    let client = ApiClient::new().await?;
    println!("✓ API client initialized\n");

    // Step 1: Verify user exists and check name
    println!("Step 1: Verifying user {} exists...", USER_ID);
    match client.get_registered_user(USER_ID).await {
        Ok(user) => {
            println!("✓ User found:");
            println!("  ID: {}", user.id);
            println!("  Name: {}", user.name);
            println!("  Is Company: {}", user.is_company);
            println!("  Creator: {}", user.creator);
            println!("  Categories: {:?}", user.categories);
            if let Some(gaid) = &user.gaid {
                println!("  Primary GAID: {}", gaid);
            } else {
                println!("  Primary GAID: None");
            }

            // Verify the name matches
            if user.name == EXPECTED_NAME {
                println!("✓ Name matches expected: \"{}\"", EXPECTED_NAME);
            } else {
                eprintln!("✗ Name mismatch!");
                eprintln!("  Expected: \"{}\"", EXPECTED_NAME);
                eprintln!("  Found: \"{}\"", user.name);
                return Err("User name does not match expected value".into());
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to get user {}: {:?}", USER_ID, e);
            return Err(e.into());
        }
    }
    println!();

    // Step 2: Show all GAIDs associated with the user
    println!("Step 2: Retrieving all GAIDs for user {}...", USER_ID);
    match client.get_registered_user_gaids(USER_ID).await {
        Ok(gaids) => {
            println!("✓ Found {} GAID(s) associated with user:", gaids.len());
            if gaids.is_empty() {
                println!("  (none)");
            } else {
                for (idx, gaid) in gaids.iter().enumerate() {
                    println!("  {}. {}", idx + 1, gaid);
                }
            }

            // Check if the GAID is already associated
            if gaids.contains(&gaid_to_add.to_string()) {
                println!(
                    "⚠ GAID \"{}\" is already associated with this user",
                    gaid_to_add
                );
                println!("  Attempting to add it again (this may succeed or fail depending on API behavior)");
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to get GAIDs for user {}: {:?}", USER_ID, e);
            return Err(e.into());
        }
    }
    println!();

    // Step 3: Validate the GAID
    println!("Step 3: Validating GAID \"{}\"...", gaid_to_add);
    match client.validate_gaid(gaid_to_add).await {
        Ok(validation) => {
            println!("✓ GAID validation result:");
            println!("  Valid: {}", validation.is_valid);
            if let Some(error) = &validation.error {
                println!("  Error: {}", error);
            }

            if !validation.is_valid {
                eprintln!("✗ GAID is invalid, cannot proceed with addition");
                return Err("Invalid GAID".into());
            }
            println!("✓ GAID is valid");
        }
        Err(e) => {
            eprintln!("✗ Failed to validate GAID: {:?}", e);
            return Err(e.into());
        }
    }
    println!();

    // Step 4: Add the GAID to the user
    println!(
        "Step 4: Adding GAID \"{}\" to user {}...",
        gaid_to_add, USER_ID
    );
    match client
        .add_gaid_to_registered_user(USER_ID, gaid_to_add)
        .await
    {
        Ok(_) => {
            println!("✓ Successfully added GAID to user!");
        }
        Err(e) => {
            eprintln!("✗ Failed to add GAID to user: {:?}", e);
            return Err(e.into());
        }
    }
    println!();

    // Step 5: Verify the GAID was added by retrieving the list again
    println!("Step 5: Verifying GAID was added...");
    match client.get_registered_user_gaids(USER_ID).await {
        Ok(gaids) => {
            println!("✓ User now has {} GAID(s):", gaids.len());
            for (idx, gaid) in gaids.iter().enumerate() {
                let marker = if gaid == gaid_to_add {
                    " ← newly added"
                } else {
                    ""
                };
                println!("  {}. {}{}", idx + 1, gaid, marker);
            }

            if gaids.contains(&gaid_to_add.to_string()) {
                println!(
                    "\n✓ Confirmed: GAID \"{}\" is now associated with user {}",
                    gaid_to_add, USER_ID
                );
            } else {
                eprintln!("\n⚠ Warning: GAID not found in user's GAID list after addition");
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to verify GAID addition: {:?}", e);
            return Err(e.into());
        }
    }

    println!("\n=== Example completed successfully ===");
    Ok(())
}
