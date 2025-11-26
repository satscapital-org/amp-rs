use amp_rs::ApiClient;
use std::env;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get category ID from command line arguments or use default
    let args: Vec<String> = env::args().collect();
    let category_id: i64 = if args.len() > 1 {
        args[1].parse()?
    } else {
        48828
    };

    // Create API client
    let client = ApiClient::new().await?;

    // Get category information
    println!("Fetching category {}...\n", category_id);
    let category = client.get_category(category_id).await?;

    // Display category details
    println!("=== Category: {} ===", category.name);
    println!("ID: {}", category.id);
    if let Some(desc) = &category.description {
        println!("Description: {}", desc);
    }
    println!();

    // Display registered users
    println!("Registered Users ({}):", category.registered_users.len());
    if category.registered_users.is_empty() {
        println!("  (none)");
    } else {
        for user_id in &category.registered_users {
            // Fetch user details to show name
            match client.get_registered_user(*user_id).await {
                Ok(user) => println!("  • {} (ID: {})", user.name, user.id),
                Err(_) => println!("  • User ID: {}", user_id),
            }
        }
    }
    println!();

    // Display assets
    println!("Assets ({}):", category.assets.len());
    if category.assets.is_empty() {
        println!("  (none)");
    } else {
        for asset_uuid in &category.assets {
            // Fetch asset details to show name and ticker
            match client.get_asset(asset_uuid).await {
                Ok(asset) => {
                    let ticker = asset.ticker.as_deref().unwrap_or("N/A");
                    println!("  • {} ({}) - UUID: {}", asset.name, ticker, asset_uuid);
                }
                Err(_) => println!("  • Asset UUID: {}", asset_uuid),
            }
        }
    }

    Ok(())
}
