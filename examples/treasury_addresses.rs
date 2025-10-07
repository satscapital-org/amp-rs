use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::from_filename_override(".env").ok();

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        eprintln!("Error: AMP_USERNAME and AMP_PASSWORD must be set");
        std::process::exit(1);
    }

    // Create the API client
    let client = ApiClient::new().await?;

    // Get all assets
    println!("Fetching assets...");
    let assets = client.get_assets().await?;

    if assets.is_empty() {
        println!("No assets found.");
        return Ok(());
    }

    // Use the first asset for demonstration
    let asset = &assets[0];
    println!("Using asset: {} ({})", asset.name, asset.asset_uuid);

    // Get current treasury addresses
    println!("\nFetching current treasury addresses...");
    let current_addresses = client
        .get_asset_treasury_addresses(&asset.asset_uuid)
        .await?;
    println!("Current treasury addresses: {:?}", current_addresses);

    // Example treasury addresses to add (these are example Liquid addresses)
    let new_addresses = vec![
        "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
            .to_string(),
        "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw27"
            .to_string(),
    ];

    // Add treasury addresses
    println!("\nAdding treasury addresses...");
    client
        .add_asset_treasury_addresses(&asset.asset_uuid, &new_addresses)
        .await?;
    println!(
        "Successfully added {} treasury addresses",
        new_addresses.len()
    );

    // Fetch updated treasury addresses
    println!("\nFetching updated treasury addresses...");
    let updated_addresses = client
        .get_asset_treasury_addresses(&asset.asset_uuid)
        .await?;
    println!("Updated treasury addresses: {:?}", updated_addresses);

    println!("\nTreasury addresses management completed successfully!");

    Ok(())
}
