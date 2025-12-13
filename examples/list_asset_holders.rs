use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get asset UUID from command line argument
    let asset_uuid = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --example list_asset_holders <ASSET_UUID>");
        eprintln!("Example: cargo run --example list_asset_holders c0c24227-732b-4980-86b6-f2048ad21cd1");
        std::process::exit(1);
    });

    println!("Fetching asset holders for asset: {}", asset_uuid);
    println!("=========================================\n");

    // Initialize the API client
    let client = ApiClient::new().await?;

    // Get asset details
    println!("Fetching asset details...");
    let asset = client.get_asset(&asset_uuid).await?;
    println!("Asset Name: {}", asset.name);
    println!("Ticker: {}", asset.ticker.as_deref().unwrap_or("N/A"));
    println!("Asset ID: {}", asset.asset_id);
    println!("Transfer Restricted: {}", asset.transfer_restricted);
    println!();

    // Get asset summary for circulation data
    println!("Fetching asset summary...");
    let summary = client.get_asset_summary(&asset_uuid).await?;
    let circulation = summary.issued + summary.reissued - summary.burned;
    println!("Asset Circulation: {} (issued: {}, reissued: {}, burned: {})", 
        circulation, summary.issued, summary.reissued, summary.burned);
    println!();

    // Get all ownerships (holders) for this asset
    println!("Fetching asset ownerships...");
    let ownerships = client.get_asset_ownerships(&asset_uuid, None).await?;

    // Display raw API response
    println!("\n=========================================");
    println!("RAW API RESPONSE:");
    println!("=========================================");
    println!("{}", serde_json::to_string_pretty(&ownerships)?);
    println!("=========================================\n");

    if ownerships.is_empty() {
        println!("No asset holders found for this asset.");
        return Ok(());
    }

    println!("Found {} asset holder(s):\n", ownerships.len());

    // Calculate total owned amount
    let mut total_amount = 0i64;

    // Display each holder
    for (idx, ownership) in ownerships.iter().enumerate() {
        println!("Holder #{}", idx + 1);
        
        if let Some(owner) = &ownership.owner {
            println!("  Owner: {}", owner);
        } else {
            println!("  Owner: (none)");
        }
        
        if let Some(gaid) = &ownership.gaid {
            println!("  GAID: {}", gaid);
        }
        
        println!("  Amount: {}", ownership.amount);

        total_amount += ownership.amount;
        println!();
    }

    // Summary statistics
    println!("=========================================");
    println!("Summary:");
    println!("  Total Holders: {}", ownerships.len());
    println!("  Total Amount Owned: {}", total_amount);
    println!("  Asset Circulation: {}", circulation);
    println!();
    
    // Validation
    if total_amount == circulation {
        println!("✓ VALIDATION PASSED: Ownerships sum matches circulation");
    } else {
        println!("✗ VALIDATION FAILED: Ownerships sum ({}) does NOT match circulation ({})", 
            total_amount, circulation);
        println!("  Difference: {}", circulation - total_amount);
    }

    Ok(())
}
