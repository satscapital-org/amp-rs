use amp_rs::{ApiClient, model::EditAssetRequest};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get asset UUID from command line argument or use default
    let asset_uuid = env::args()
        .nth(1)
        .unwrap_or_else(|| "c0c24227-732b-4980-86b6-f2048ad21cd1".to_string());

    println!("Setting authorization endpoint for asset: {}", asset_uuid);

    // Initialize the API client
    let client = ApiClient::new().await?;

    // Get current asset details
    println!("\nFetching current asset details...");
    let asset = client.get_asset(&asset_uuid).await?;
    println!("Asset Name: {}", asset.name);
    println!("Ticker: {}", asset.ticker.as_deref().unwrap_or("N/A"));
    println!("Transfer Restricted: {}", asset.transfer_restricted);
    println!(
        "Current Auth Endpoint: {}",
        asset
            .issuer_authorization_endpoint
            .as_deref()
            .unwrap_or("(not set)")
    );

    // Set the new authorization endpoint
    let new_endpoint = "https://auth-test.duckdns.org";
    println!("\nSetting authorization endpoint to: {}", new_endpoint);

    let edit_request = EditAssetRequest {
        issuer_authorization_endpoint: new_endpoint.to_string(),
    };

    let updated_asset = client.edit_asset(&asset_uuid, &edit_request).await?;

    println!("\n✅ Asset updated successfully!");
    println!("Asset Name: {}", updated_asset.name);
    println!(
        "Authorization Endpoint: {}",
        updated_asset
            .issuer_authorization_endpoint
            .as_deref()
            .unwrap_or("(not set)")
    );

    Ok(())
}
