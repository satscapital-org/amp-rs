use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get asset UUID from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <asset_uuid>", args[0]);
        eprintln!("Example: {} 550e8400-e29b-41d4-a716-446655440000", args[0]);
        std::process::exit(1);
    }

    let asset_uuid = &args[1];

    // Create API client
    let client = ApiClient::new().await?;

    // Get asset details
    println!("Fetching asset details for UUID: {}", asset_uuid);
    let asset = client.get_asset(asset_uuid).await?;

    // Display asset name
    println!("\nAsset Name: {}", asset.name);
    if let Some(ticker) = &asset.ticker {
        println!("Ticker: {}", ticker);
    }

    // Get all categories
    println!("\nFetching categories...");
    let categories = client.get_categories().await?;

    // Filter categories that contain this asset
    let asset_categories: Vec<_> = categories
        .into_iter()
        .filter(|category| category.assets.contains(&asset.asset_uuid))
        .collect();

    // Display categories
    if asset_categories.is_empty() {
        println!("\nThis asset does not belong to any categories.");
    } else {
        println!(
            "\nThis asset belongs to {} category(ies):",
            asset_categories.len()
        );
        for category in asset_categories {
            println!("  - {} (ID: {})", category.name, category.id);
            if let Some(desc) = category.description {
                println!("    Description: {}", desc);
            }
        }
    }

    Ok(())
}
