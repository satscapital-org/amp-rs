use amp_rs::ApiClient;
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
        "GAFeuScUmoDdj9CnFRiz5VGNKEFfW".to_string()
    };

    // Create API client
    let client = ApiClient::new().await?;

    // Look up the user associated with this GAID
    println!("Looking up user for GAID: {}", gaid);
    
    match client.get_gaid_registered_user(&gaid).await {
        Ok(user) => {
            println!("\n=== User Information ===");
            println!("User ID: {}", user.id);
            println!("Name: {}", user.name);
            println!("Is Company: {}", user.is_company);
            println!("GAID: {}", user.gaid.as_deref().unwrap_or("None"));
            println!("Creator ID: {}", user.creator);
            
            if !user.categories.is_empty() {
                println!("\nCategories: {:?}", user.categories);
            } else {
                println!("\nCategories: None");
            }
        }
        Err(e) => {
            eprintln!("Error: Failed to find user for GAID '{}': {}", gaid, e);
            std::process::exit(1);
        }
    }

    Ok(())
}
