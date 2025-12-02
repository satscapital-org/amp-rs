//! User Categories Example
//!
//! This example accepts a registered user ID from the command line (defaults to 2137)
//! and prints the categories that the user is a member of (Category ID and Name).
//!
//! Usage:
//!   cargo run --example user_categories [USER_ID]
//!
//! Examples:
//!   cargo run --example user_categories          # uses default user ID 2137
//!   cargo run --example user_categories 42       # uses user ID 42

use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables (AMP_USERNAME, AMP_PASSWORD, AMP_API_BASE_URL, etc.)
    // Per project conventions, prefer dotenvy over sourcing .env directly.
    dotenvy::dotenv().ok();

    // Parse optional CLI arg for user ID, default to 2137
    let user_id: i64 = env::args()
        .nth(1)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(2137);

    println!("Fetching categories for user ID {user_id}...");

    let client = ApiClient::new().await?;

    // Fetch the user to get the list of category IDs
    let user = match client.get_registered_user(user_id).await {
        Ok(u) => u,
        Err(e) => {
            eprintln!("Failed to fetch user {user_id}: {e:?}");
            std::process::exit(1);
        }
    };

    if user.categories.is_empty() {
        println!("User {user_id} is not a member of any categories.");
        return Ok(());
    }

    println!(
        "User {} is a member of {} categor{}:",
        user.id,
        user.categories.len(),
        if user.categories.len() == 1 {
            "y"
        } else {
            "ies"
        }
    );

    // For each category ID, fetch its details to display ID and Name
    for cat_id in user.categories {
        match client.get_category(cat_id).await {
            Ok(cat) => {
                println!("- ID: {:<6}  Name: {}", cat.id, cat.name);
            }
            Err(e) => {
                eprintln!("- ID: {:<6}  (error fetching name: {e:?})", cat_id);
            }
        }
    }

    Ok(())
}
