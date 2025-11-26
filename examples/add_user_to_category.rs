use amp_rs::ApiClient;
use std::env;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get user ID from command line arguments or use default
    let args: Vec<String> = env::args().collect();
    let user_id: i64 = if args.len() > 1 {
        args[1].parse()?
    } else {
        2137
    };

    let category_id = 48829;

    // Create API client
    let client = ApiClient::new().await?;

    // Get user information
    println!("Fetching user {}...", user_id);
    let user = client.get_registered_user(user_id).await?;
    println!("✓ Found user: {} (ID: {})", user.name, user.id);

    // Add user to category
    println!("\nAdding user to category {}...", category_id);
    let updated_category = client
        .add_registered_user_to_category(category_id, user_id)
        .await?;

    println!(
        "✓ User added to category '{}' (ID: {})",
        updated_category.name, updated_category.id
    );
    println!(
        "  Category now has {} user(s)",
        updated_category.registered_users.len()
    );

    Ok(())
}
