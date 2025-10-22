use amp_rs::{ApiClient};
use dotenvy;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();
    env::set_var("AMP_TESTS", "live");

    // Initialize API client
    let api_client = ApiClient::new().await?;
    
    println!("🔍 Finding working assets with treasury balance");

    // Get all assets
    let assets = match api_client.get_assets().await {
        Ok(assets) => assets,
        Err(e) => {
            println!("❌ Failed to get assets: {}", e);
            return Err(e.into());
        }
    };

    println!("📊 Found {} total assets", assets.len());

    // Look for Test Distribution Assets
    let mut test_assets = Vec::new();
    for asset in &assets {
        if asset.name.contains("Test Distribution Asset") {
            test_assets.push(asset);
        }
    }

    println!("🎯 Found {} Test Distribution Assets", test_assets.len());

    // Test each asset to see if it has treasury balance
    for asset in &test_assets {
        println!("\n🧪 Testing asset: {} ({})", asset.name, asset.asset_uuid);
        println!("   - Ticker: {:?}", asset.ticker);

        // Check if asset is authorized
        match api_client.register_asset_authorized(&asset.asset_uuid).await {
            Ok(auth_info) => {
                println!("   ✅ Asset is authorized: {}", auth_info.is_authorized);
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("already authorized") {
                    println!("   ✅ Asset is already authorized");
                } else {
                    println!("   ❌ Asset authorization failed: {}", e);
                    continue;
                }
            }
        }

        // Try to create a test assignment to check treasury balance
        let test_user_id = 1352; // Known test user
        let test_assignment_request = amp_rs::model::CreateAssetAssignmentRequest {
            registered_user: test_user_id,
            amount: 1, // 1 satoshi test
            vesting_timestamp: None,
            ready_for_distribution: true,
        };

        match api_client.create_asset_assignments(&asset.asset_uuid, &vec![test_assignment_request]).await {
            Ok(assignments) => {
                println!("   ✅ Treasury balance available - test assignment created");
                println!("   - Assignment ID: {}", assignments[0].id);
                
                println!("\n🎯 WORKING ASSET FOUND!");
                println!("   Asset UUID: {}", asset.asset_uuid);
                println!("   Asset Name: {}", asset.name);
                println!("   Asset Ticker: {:?}", asset.ticker);
                println!("   ✅ Authorized: Yes");
                println!("   ✅ Treasury Balance: Available");
                println!("\n🔄 Use this asset UUID in your test:");
                println!("   {}", asset.asset_uuid);
                
                return Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("not enough in the treasury balance") {
                    println!("   ❌ Treasury balance insufficient: {}", e);
                } else if error_msg.contains("already created") {
                    println!("   ⚠️  Assignment already exists (treasury balance likely available)");
                    
                    println!("\n🎯 POTENTIALLY WORKING ASSET FOUND!");
                    println!("   Asset UUID: {}", asset.asset_uuid);
                    println!("   Asset Name: {}", asset.name);
                    println!("   Asset Ticker: {:?}", asset.ticker);
                    println!("   ✅ Authorized: Yes");
                    println!("   ⚠️  Treasury Balance: Likely available (assignment exists)");
                    println!("\n🔄 Try this asset UUID in your test:");
                    println!("   {}", asset.asset_uuid);
                } else {
                    println!("   ❌ Assignment test failed: {}", e);
                }
            }
        }
    }

    println!("\n❌ No working assets found with available treasury balance");
    println!("   You may need to wait for existing assets to synchronize");
    println!("   or contact support to increase the asset limit");

    Ok(())
}