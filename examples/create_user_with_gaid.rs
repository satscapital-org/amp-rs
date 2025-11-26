use amp_rs::{model::RegisteredUserAdd, ApiClient};
use std::env;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get GAID from command line arguments or use default
    let args: Vec<String> = env::args().collect();
    let gaid = if args.len() > 1 {
        args[1].clone()
    } else {
        "GAbJNiAdBqCSXJXVyjeRnT2dRZhbX".to_string()
    };

    let category_id = 48829;

    // Create API client
    let client = ApiClient::new().await?;

    // Step 1: Create the user
    println!("Creating user 'Test User'...");
    let new_user = RegisteredUserAdd {
        name: "Test User".to_string(),
        gaid: None,
        is_company: false,
    };

    let created_user = client.add_registered_user(&new_user).await?;
    println!(
        "✓ User created with ID: {} (Name: {})",
        created_user.id, created_user.name
    );

    // Step 2: Add GAID to the user
    println!("\nAdding GAID '{}' to user...", gaid);
    client
        .add_gaid_to_registered_user(created_user.id, &gaid)
        .await?;
    println!("✓ GAID added successfully");

    // Verify the GAID was added
    let gaids = client.get_registered_user_gaids(created_user.id).await?;
    println!("  Associated GAIDs: {:?}", gaids);

    // Step 3: Add user to category
    println!("\nAdding user to category {}...", category_id);
    let updated_category = client
        .add_registered_user_to_category(category_id, created_user.id)
        .await?;
    println!(
        "✓ User added to category '{}' (ID: {})",
        updated_category.name, updated_category.id
    );
    println!(
        "  Category now has {} user(s)",
        updated_category.registered_users.len()
    );

    println!("\n=== Summary ===");
    println!("User ID: {}", created_user.id);
    println!("User Name: {}", created_user.name);
    println!("GAID: {}", gaid);
    println!("Category: {} ({})", updated_category.name, category_id);

    Ok(())
}
