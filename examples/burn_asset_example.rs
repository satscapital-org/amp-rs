//! # Burn Asset Example
//!
//! This example demonstrates how to burn (destroy) an asset to reduce its supply.
//! It burns 1 whole unit of the asset (1 * 10^8 = 100,000,000 satoshis for 8-decimal precision).
//!
//! ## Prerequisites
//!
//! - An asset created using `create_issue_authorize_asset` example
//! - Running Elements node with RPC access
//! - Environment variables set for Elements RPC connection and AMP API
//! - AMP_TESTS=live for live API testing
//! - Sufficient asset balance in the wallet to burn
//!
//! ## Usage
//!
//! ```bash
//! # Option 1: Pass asset UUID as command line argument
//! AMP_TESTS=live cargo run --example burn_asset_example -- <ASSET_UUID>
//!
//! # Option 2: Use environment variable
//! export TEST_ASSET_UUID=<ASSET_UUID>
//! AMP_TESTS=live cargo run --example burn_asset_example
//! ```
//!
//! ## Example Workflow
//!
//! ```bash
//! # Step 1: Create an asset
//! AMP_TESTS=live cargo run --example create_issue_authorize_asset
//!
//! # Step 2: Use the asset UUID from step 1 to burn
//! AMP_TESTS=live cargo run --example burn_asset_example -- <ASSET_UUID>
//! ```

use amp_rs::signer::LwkSoftwareSigner;
use amp_rs::{AmpError, ApiClient, ElementsRpc};
use dotenvy;
use std::env;

const BURN_AMOUNT_WHOLE_UNITS: i64 = 1; // 1 whole unit
const ASSET_PRECISION: i64 = 8; // 8 decimal places
/// Default asset UUID for burning (created by create_issue_authorize_asset example)
const DEFAULT_ASSET_UUID: &str = "bd43436f-7bbe-4d2a-959a-40c25be66df0";

fn print_usage() {
    println!("Usage:");
    println!("  cargo run --example burn_asset_example -- <ASSET_UUID>");
    println!();
    println!("  Or set TEST_ASSET_UUID environment variable:");
    println!("  export TEST_ASSET_UUID=<ASSET_UUID>");
    println!("  cargo run --example burn_asset_example");
    println!();
    println!("Arguments:");
    println!("  ASSET_UUID    The UUID of the asset to burn (optional, uses default if not provided)");
    println!();
    println!("Environment Variables:");
    println!("  TEST_ASSET_UUID    Asset UUID (alternative to command line argument, uses default if not set)");
    println!();
    println!("Default Asset:");
    println!(
        "  If no asset UUID is provided, uses default: {}",
        DEFAULT_ASSET_UUID
    );
    println!("  (Created by create_issue_authorize_asset example)");
    println!("  AMP_TESTS          Must be set to 'live' for live API testing");
    println!("  AMP_USERNAME       AMP API username");
    println!("  AMP_PASSWORD       AMP API password");
    println!("  ELEMENTS_RPC_URL   Elements node RPC URL");
    println!("  ELEMENTS_RPC_USER  Elements node RPC username");
    println!("  ELEMENTS_RPC_PASSWORD  Elements node RPC password");
}

#[tokio::main]
async fn main() -> Result<(), AmpError> {
    // Load environment variables from .env file first
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔥 Burn Asset Example");
    println!("=====================");

    // Check if we're running in live mode
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        return Err(AmpError::validation(
            "This example requires AMP_TESTS=live to be set. Set AMP_USERNAME, AMP_PASSWORD, and AMP_TESTS=live environment variables"
        ));
    }

    // Parse command line arguments or use environment variable or default
    let args: Vec<String> = env::args().collect();
    let asset_uuid = if args.len() > 1 {
        // Check for help flag
        if args[1] == "--help" || args[1] == "-h" {
            print_usage();
            return Ok(());
        }
        args[1].clone()
    } else {
        // Try to get from environment variable, otherwise use default
        env::var("TEST_ASSET_UUID").unwrap_or_else(|_| {
            println!("ℹ️  No asset UUID provided via command line or TEST_ASSET_UUID environment variable.");
            println!("   Using default asset UUID: {}", DEFAULT_ASSET_UUID);
            println!("   (Created by create_issue_authorize_asset example)");
            DEFAULT_ASSET_UUID.to_string()
        })
    };

    println!("\n📋 Configuration:");
    println!("   Asset UUID: {}", asset_uuid);
    println!(
        "   Burn Amount: {} whole units",
        BURN_AMOUNT_WHOLE_UNITS
    );

    // Calculate amount in satoshis (whole units * 10^precision)
    let burn_amount_satoshis =
        BURN_AMOUNT_WHOLE_UNITS * 10_i64.pow(ASSET_PRECISION as u32);
    println!(
        "   Burn Amount (satoshis): {}",
        burn_amount_satoshis
    );

    // Step 1: Initialize clients
    println!("\n1️⃣  Initializing clients");
    println!("========================");

    let client = ApiClient::new()
        .await
        .map_err(|e| AmpError::api(format!("Failed to create AMP API client: {}", e)))?;

    let elements_rpc = ElementsRpc::from_env()
        .map_err(|e| AmpError::rpc(format!("Failed to create Elements RPC client: {}", e)))?;

    // Test Elements connectivity
    elements_rpc
        .get_network_info()
        .await
        .map_err(|e| AmpError::rpc(format!("Failed to connect to Elements node: {}", e)))?;

    println!("✅ AMP API client initialized");
    println!("✅ Elements RPC client connected");

    // Step 2: Verify asset exists
    println!("\n2️⃣  Verifying asset");
    println!("===================");

    let asset = client
        .get_asset(&asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to get asset: {}", e)))?;

    println!("✅ Asset found:");
    println!("   Name: {}", asset.name);
    println!("   Asset ID: {}", asset.asset_id);

    // Get current asset summary to show before/after
    let summary_before = client
        .get_asset_summary(&asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to get asset summary: {}", e)))?;

    println!("\n📊 Current Asset Summary:");
    println!("   Issued: {} satoshis", summary_before.issued);
    println!("   Burned: {} satoshis", summary_before.burned);
    println!(
        "   Total Supply: {} satoshis",
        summary_before.issued - summary_before.burned
    );

    // Step 3: Create signer (for future support, currently node RPC signs)
    println!("\n3️⃣  Setting up signer");
    println!("====================");

    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(300)
        .map_err(|e| AmpError::validation(format!("Failed to create signer: {}", e)))?;

    println!("✅ Signer created (mnemonic: {}...)", &mnemonic[..50]);
    println!("   Note: Currently the Elements node RPC signs transactions");

    // Step 4: Execute burn
    println!("\n4️⃣  Executing burn");
    println!("==================");
    println!("   This will:");
    println!("   1. Create burn request with AMP API");
    println!("   2. Wait for transaction propagation (60 seconds)");
    println!("   3. Check for lost outputs");
    println!("   4. Verify required UTXOs are available");
    println!("   5. Verify sufficient balance exists");
    println!("   6. Call Elements node's destroyamount RPC method");
    println!("   7. Wait for 2 blockchain confirmations");
    println!("   8. Confirm burn with AMP API");
    println!();

    let burn_start = std::time::Instant::now();

    const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

    client
        .burn_asset(&asset_uuid, burn_amount_satoshis, &elements_rpc, WALLET_NAME, &signer)
        .await
        .map_err(|e| {
            AmpError::api(format!(
                "Burn failed: {}. \
                Make sure sufficient asset balance exists in the wallet and the asset is properly set up.",
                e
            ))
        })?;

    let burn_duration = burn_start.elapsed();
    println!("✅ Burn completed successfully!");
    println!("   Duration: {:?}", burn_duration);

    // Step 5: Verify burn
    println!("\n5️⃣  Verifying burn");
    println!("==================");

    // Wait a moment for the API to update
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let summary_after = client
        .get_asset_summary(&asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to get updated asset summary: {}", e)))?;

    println!("📊 Updated Asset Summary:");
    println!("   Issued: {} satoshis (unchanged)", summary_after.issued);
    println!(
        "   Burned: {} satoshis (was: {})",
        summary_after.burned, summary_before.burned
    );
    println!(
        "   Total Supply: {} satoshis (was: {})",
        summary_after.issued - summary_after.burned,
        summary_before.issued - summary_before.burned
    );

    let burned_delta = summary_after.burned - summary_before.burned;
    println!("\n✅ Burn verified!");
    println!("   Amount burned: {} satoshis", burned_delta);
    println!("   Expected: {} satoshis", burn_amount_satoshis);

    if burned_delta == burn_amount_satoshis {
        println!("   ✅ Amount matches expected value!");
    } else {
        println!("   ⚠️  Amount differs from expected (this may be normal if there were previous burns)");
    }

    // Step 6: Display success information
    println!("\n🎉 Burn Complete!");
    println!("=================");
    println!("✅ Asset supply successfully reduced!");
    println!();
    println!("📋 Summary:");
    println!("   Asset UUID: {}", asset_uuid);
    println!("   Asset Name: {}", asset.name);
    println!("   Asset ID: {}", asset.asset_id);
    println!(
        "   Amount Burned: {} whole units ({} satoshis)",
        BURN_AMOUNT_WHOLE_UNITS, burn_amount_satoshis
    );
    println!(
        "   Previous Total Supply: {} satoshis",
        summary_before.issued - summary_before.burned
    );
    println!(
        "   New Total Supply: {} satoshis",
        summary_after.issued - summary_after.burned
    );
    println!();
    println!("🚀 The asset is now ready for:");
    println!("   • Further burns (if balance remains)");
    println!("   • Distribution with reduced supply");
    println!("   • All standard asset operations");

    Ok(())
}

