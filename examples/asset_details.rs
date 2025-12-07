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
    println!("Fetching asset details for UUID: {}\n", asset_uuid);
    let asset = client.get_asset(asset_uuid).await?;

    // Display all asset fields in human-readable format
    println!("╔════════════════════════════════════════════════════════════════════════");
    println!("║ ASSET DETAILS");
    println!("╠════════════════════════════════════════════════════════════════════════");
    println!("║");
    println!("║ Basic Information:");
    println!("║   Name:                 {}", asset.name);
    println!("║   Asset UUID:           {}", asset.asset_uuid);
    println!("║   Asset ID:             {}", asset.asset_id);
    println!(
        "║   Ticker:               {}",
        asset.ticker.as_ref().unwrap_or(&"(none)".to_string())
    );
    println!(
        "║   Precision:            {} decimal places",
        asset.precision
    );
    println!("║");
    println!("║ Issuer Information:");
    println!("║   Issuer ID:            {}", asset.issuer);
    println!(
        "║   Domain:               {}",
        asset.domain.as_ref().unwrap_or(&"(none)".to_string())
    );
    println!(
        "║   Public Key:           {}",
        asset
            .pubkey
            .as_ref()
            .map(|p| {
                if p.len() > 50 {
                    format!("{}...", &p[..50])
                } else {
                    p.clone()
                }
            })
            .unwrap_or("(none)".to_string())
    );
    println!("║");
    println!("║ Reissuance:");
    println!(
        "║   Reissuance Token ID:  {}",
        asset
            .reissuance_token_id
            .as_ref()
            .unwrap_or(&"(none)".to_string())
    );
    println!("║");
    println!("║ Status Flags:");
    println!(
        "║   Registered:           {}",
        if asset.is_registered {
            "✓ Yes"
        } else {
            "✗ No"
        }
    );
    println!(
        "║   Authorized:           {}",
        if asset.is_authorized {
            "✓ Yes"
        } else {
            "✗ No"
        }
    );
    println!(
        "║   Locked:               {}",
        if asset.is_locked { "✓ Yes" } else { "✗ No" }
    );
    println!(
        "║   Transfer Restricted:  {}",
        if asset.transfer_restricted {
            "✓ Yes"
        } else {
            "✗ No"
        }
    );
    println!("║");
    println!("║ Authorization:");
    println!(
        "║   Auth Endpoint:        {}",
        asset
            .issuer_authorization_endpoint
            .as_ref()
            .unwrap_or(&"(none)".to_string())
    );
    println!("║");
    println!("║ Requirements:");
    if asset.requirements.is_empty() {
        println!("║   No requirements");
    } else {
        println!("║   Requirement IDs:      {:?}", asset.requirements);
    }
    println!("║");
    println!("╚════════════════════════════════════════════════════════════════════════");

    // Also print the raw debug format for developers
    println!("\n\n=== Raw Debug Output ===");
    println!("{:#?}", asset);

    Ok(())
}
