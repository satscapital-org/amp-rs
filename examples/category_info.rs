use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get category ID from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <category_id>", args[0]);
        eprintln!("Example: {} 1", args[0]);
        std::process::exit(1);
    }

    let category_id: i64 = match args[1].parse() {
        Ok(id) => id,
        Err(_) => {
            eprintln!("Error: category_id must be a valid integer");
            std::process::exit(1);
        }
    };

    // Create API client
    let client = ApiClient::new().await?;

    // Get category details
    println!("Fetching category details for ID: {}", category_id);
    let category = client.get_category(category_id).await?;

    // Display category information
    println!("\n=== Category Details ===");
    println!("Name: {}", category.name);
    println!("ID: {}", category.id);
    if let Some(desc) = &category.description {
        println!("Description: {}", desc);
    }

    // Display registered users
    println!(
        "\n=== Registered Users ({}) ===",
        category.registered_users.len()
    );
    if category.registered_users.is_empty() {
        println!("No users in this category.");
    } else {
        for user_id in &category.registered_users {
            // Fetch user details
            match client.get_registered_user(*user_id).await {
                Ok(user) => {
                    println!("  - {} (ID: {})", user.name, user.id);
                    if let Some(gaid) = &user.gaid {
                        println!("    GAID: {}", gaid);
                    }
                    println!(
                        "    Company: {}",
                        if user.is_company { "Yes" } else { "No" }
                    );
                }
                Err(e) => {
                    println!("  - User ID {} (error fetching details: {})", user_id, e);
                }
            }
        }
    }

    // Display assets
    println!("\n=== Assets ({}) ===", category.assets.len());
    if category.assets.is_empty() {
        println!("No assets in this category.");
    } else {
        for asset_uuid in &category.assets {
            // Fetch asset details
            match client.get_asset(asset_uuid).await {
                Ok(asset) => {
                    println!("  - {}", asset.name);
                    println!("    UUID: {}", asset.asset_uuid);
                    if let Some(ticker) = &asset.ticker {
                        println!("    Ticker: {}", ticker);
                    }
                    println!(
                        "    Registered: {}",
                        if asset.is_registered { "Yes" } else { "No" }
                    );
                    println!("    Locked: {}", if asset.is_locked { "Yes" } else { "No" });
                }
                Err(e) => {
                    println!(
                        "  - Asset UUID {} (error fetching details: {})",
                        asset_uuid, e
                    );
                }
            }
        }
    }

    Ok(())
}
