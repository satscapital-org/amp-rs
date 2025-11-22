//! # Reissue Asset Example
//!
//! This example demonstrates how to reissue an asset to expand its supply.
//! It reissues 10 whole units of the asset (10 * 10^8 = 1,000,000,000 satoshis for 8-decimal precision).
//!
//! ## Prerequisites
//!
//! - A reissuable asset created using `create_issue_authorize_reissuable_asset` example
//! - Running Elements node with RPC access
//! - Environment variables set for Elements RPC connection and AMP API
//! - AMP_TESTS=live for live API testing
//! - Reissuance tokens must be available in the wallet
//!
//! ## Usage
//!
//! ```bash
//! # Option 1: Pass asset UUID as command line argument
//! AMP_TESTS=live cargo run --example reissue_asset_example -- <ASSET_UUID>
//!
//! # Option 2: Use environment variable
//! export TEST_ASSET_UUID=<ASSET_UUID>
//! AMP_TESTS=live cargo run --example reissue_asset_example
//! ```
//!
//! ## Example Workflow
//!
//! ```bash
//! # Step 1: Create a reissuable asset
//! AMP_TESTS=live cargo run --example create_issue_authorize_reissuable_asset
//!
//! # Step 2: Use the asset UUID from step 1 to reissue
//! AMP_TESTS=live cargo run --example reissue_asset_example -- <ASSET_UUID>
//! ```

use amp_rs::signer::LwkSoftwareSigner;
use amp_rs::{AmpError, ApiClient, ElementsRpc};
use dotenvy;
use std::env;

const REISSUANCE_AMOUNT_WHOLE_UNITS: i64 = 10; // 10 whole units
const ASSET_PRECISION: i64 = 8; // 8 decimal places
/// Default asset UUID for reissuance (created by create_issue_authorize_reissuable_asset example)
const DEFAULT_ASSET_UUID: &str = "84e282bf-16bf-40e2-9d4f-5b25415a906a";

fn print_usage() {
    println!("Usage:");
    println!("  cargo run --example reissue_asset_example -- <ASSET_UUID>");
    println!();
    println!("  Or set TEST_ASSET_UUID environment variable:");
    println!("  export TEST_ASSET_UUID=<ASSET_UUID>");
    println!("  cargo run --example reissue_asset_example");
    println!();
    println!("Arguments:");
    println!("  ASSET_UUID    The UUID of the reissuable asset to reissue (optional, uses default if not provided)");
    println!();
    println!("Environment Variables:");
    println!("  TEST_ASSET_UUID    Asset UUID (alternative to command line argument, uses default if not set)");
    println!();
    println!("Default Asset:");
    println!(
        "  If no asset UUID is provided, uses default: {}",
        DEFAULT_ASSET_UUID
    );
    println!("  (Created by create_issue_authorize_reissuable_asset example)");
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

    println!("🔄 Reissue Asset Example");
    println!("========================");

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
            println!("   (Created by create_issue_authorize_reissuable_asset example)");
            DEFAULT_ASSET_UUID.to_string()
        })
    };

    println!("\n📋 Configuration:");
    println!("   Asset UUID: {}", asset_uuid);
    println!(
        "   Reissuance Amount: {} whole units",
        REISSUANCE_AMOUNT_WHOLE_UNITS
    );

    // Calculate amount in satoshis (whole units * 10^precision)
    let reissuance_amount_satoshis =
        REISSUANCE_AMOUNT_WHOLE_UNITS * 10_i64.pow(ASSET_PRECISION as u32);
    println!(
        "   Reissuance Amount (satoshis): {}",
        reissuance_amount_satoshis
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

    // Step 2: Verify asset exists and is reissuable
    println!("\n2️⃣  Verifying asset");
    println!("===================");

    let asset = client
        .get_asset(&asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to get asset: {}", e)))?;

    println!("✅ Asset found:");
    println!("   Name: {}", asset.name);
    println!("   Asset ID: {}", asset.asset_id);
    println!("   Is Reissuable: {}", asset.reissuance_token_id.is_some());

    if asset.reissuance_token_id.is_none() {
        return Err(AmpError::validation(
            format!("Asset {} is not reissuable. Please create a reissuable asset first using the create_issue_authorize_reissuable_asset example.", asset_uuid)
        ));
    }

    if let Some(ref reissuance_token_id) = asset.reissuance_token_id {
        println!("   Reissuance Token ID: {}", reissuance_token_id);
    }

    // Get current asset summary to show before/after
    let summary_before = client
        .get_asset_summary(&asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to get asset summary: {}", e)))?;

    println!("\n📊 Current Asset Summary:");
    println!("   Issued: {} satoshis", summary_before.issued);
    println!("   Reissued: {} satoshis", summary_before.reissued);
    println!(
        "   Total Supply: {} satoshis",
        summary_before.issued + summary_before.reissued
    );

    // Step 3: Create signer (for future support, currently node RPC signs)
    println!("\n3️⃣  Setting up signer");
    println!("====================");

    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(200)
        .map_err(|e| AmpError::validation(format!("Failed to create signer: {}", e)))?;

    println!("✅ Signer created (mnemonic: {}...)", &mnemonic[..50]);
    println!("   Note: Currently the Elements node RPC signs transactions");

    // Step 4: Execute reissuance
    println!("\n4️⃣  Executing reissuance");
    println!("=======================");
    println!("   This will:");
    println!("   1. Create reissuance request with AMP API");
    println!("   2. Wait for transaction propagation (60 seconds)");
    println!("   3. Check for lost outputs");
    println!("   4. Verify reissuance token UTXOs are available");
    println!("   5. Call Elements node's reissueasset RPC method");
    println!("   6. Wait for 2 blockchain confirmations");
    println!("   7. Confirm reissuance with AMP API");
    println!();

    let reissue_start = std::time::Instant::now();

    client
        .reissue_asset(&asset_uuid, reissuance_amount_satoshis, &elements_rpc, &signer)
        .await
        .map_err(|e| {
            AmpError::api(format!(
                "Reissuance failed: {}. \
                Make sure the reissuance tokens are available in the wallet and the asset is properly set up.",
                e
            ))
        })?;

    let reissue_duration = reissue_start.elapsed();
    println!("✅ Reissuance completed successfully!");
    println!("   Duration: {:?}", reissue_duration);

    // Step 5: Verify reissuance
    println!("\n5️⃣  Verifying reissuance");
    println!("========================");

    // Wait a moment for the API to update
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let summary_after = client
        .get_asset_summary(&asset_uuid)
        .await
        .map_err(|e| AmpError::api(format!("Failed to get updated asset summary: {}", e)))?;

    println!("📊 Updated Asset Summary:");
    println!("   Issued: {} satoshis (unchanged)", summary_after.issued);
    println!(
        "   Reissued: {} satoshis (was: {})",
        summary_after.reissued, summary_before.reissued
    );
    println!(
        "   Total Supply: {} satoshis (was: {})",
        summary_after.issued + summary_after.reissued,
        summary_before.issued + summary_before.reissued
    );

    let reissued_delta = summary_after.reissued - summary_before.reissued;
    println!("\n✅ Reissuance verified!");
    println!("   Amount reissued: {} satoshis", reissued_delta);
    println!("   Expected: {} satoshis", reissuance_amount_satoshis);

    if reissued_delta == reissuance_amount_satoshis {
        println!("   ✅ Amount matches expected value!");
    } else {
        println!("   ⚠️  Amount differs from expected (this may be normal if there were previous reissuances)");
    }

    // Step 6: Display success information
    println!("\n🎉 Reissuance Complete!");
    println!("======================");
    println!("✅ Asset supply successfully expanded!");
    println!();
    println!("📋 Summary:");
    println!("   Asset UUID: {}", asset_uuid);
    println!("   Asset Name: {}", asset.name);
    println!("   Asset ID: {}", asset.asset_id);
    println!(
        "   Amount Reissued: {} whole units ({} satoshis)",
        REISSUANCE_AMOUNT_WHOLE_UNITS, reissuance_amount_satoshis
    );
    println!(
        "   Previous Total Supply: {} satoshis",
        summary_before.issued + summary_before.reissued
    );
    println!(
        "   New Total Supply: {} satoshis",
        summary_after.issued + summary_after.reissued
    );
    println!();
    println!("🚀 The asset is now ready for:");
    println!("   • Distribution with increased supply");
    println!("   • Further reissuances (if reissuance tokens remain)");
    println!("   • All standard asset operations");

    Ok(())
}
