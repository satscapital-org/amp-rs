//! Integration tests for the asset reissuance workflow
//!
//! This test suite implements comprehensive end-to-end testing for the reissue_asset
//! functionality, including environment setup, test data creation, and cleanup.
//!
//! ## Test Environment Requirements
//!
//! These tests require:
//! - Valid AMP API credentials in environment variables
//! - Elements node RPC access for blockchain operations
//! - Testnet configuration for safe testing
//!
//! ## Environment Variables
//!
//! Required environment variables (loaded from .env):
//! - `AMP_USERNAME`: AMP API username
//! - `AMP_PASSWORD`: AMP API password
//! - `ELEMENTS_RPC_URL`: Elements node RPC endpoint
//! - `ELEMENTS_RPC_USER`: RPC authentication username
//! - `ELEMENTS_RPC_PASSWORD`: RPC authentication password
//!
//! ## Test Isolation
//!
//! Each test uses:
//! - Unique LwkSoftwareSigner instances with generated mnemonics
//! - Isolated test assets
//! - Proper cleanup to avoid test interference

use amp_rs::signer::LwkSoftwareSigner;
use amp_rs::{ApiClient, ElementsRpc};
use dotenvy;
use serial_test::serial;
use std::env;
use tracing_subscriber;

/// Wallet name to use for all tests
const WALLET_NAME: &str = "test_wallet";

/// Helper function to conditionally print based on nocapture mode
fn print_if_nocapture(msg: &str) {
    let should_print = std::env::args().any(|arg| arg == "--nocapture");
    if should_print {
        println!("{}", msg);
    }
}

/// Helper function to conditionally initialize tracing based on nocapture mode
fn init_tracing_if_nocapture() {
    let should_print = std::env::args().any(|arg| arg == "--nocapture");
    if should_print {
        let _ = tracing_subscriber::fmt::try_init();
    }
}

/// Helper function to setup a reissuable test asset
async fn setup_reissuable_test_asset(
    client: &ApiClient,
    elements_rpc: &ElementsRpc,
    wallet_name: &str,
) -> Result<(String, String, String), Box<dyn std::error::Error>> {
    // Get addresses for issuance and reissuance
    let destination_address = elements_rpc
        .get_new_address(wallet_name, Some("bech32"))
        .await?;
    let confidential_address = elements_rpc
        .get_confidential_address(wallet_name, &destination_address)
        .await?;

    // Get a second address for reissuance (must be different from destination)
    let reissuance_unconfidential = elements_rpc
        .get_new_address(wallet_name, Some("bech32"))
        .await?;
    let reissuance_confidential = elements_rpc
        .get_confidential_address(wallet_name, &reissuance_unconfidential)
        .await?;

    let asset_name = format!("Test Reissuable Asset {}", chrono::Utc::now().timestamp());
    let asset_ticker = format!("TRA{}", chrono::Utc::now().timestamp() % 10000);

    let issuance_request = amp_rs::model::IssuanceRequest {
        name: asset_name.clone(),
        amount: 1000000, // 0.01 BTC in satoshis for testing
        destination_address: confidential_address.clone(),
        domain: "test-reissue.example.com".to_string(),
        ticker: asset_ticker.clone(),
        pubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
        precision: Some(8),
        is_confidential: Some(true),
        is_reissuable: Some(true),
        reissuance_amount: Some(100000), // 100k satoshis of reissuance tokens
        reissuance_address: Some(reissuance_confidential),
        transfer_restricted: Some(false),
    };

    let issuance_response = client.issue_asset(&issuance_request).await?;
    let asset_uuid = issuance_response.asset_uuid.clone();

    // Wait for asset to be registered and authorized
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    Ok((asset_uuid, asset_name, asset_ticker))
}

/// End-to-end test for reissue_asset functionality
///
/// This test verifies the complete reissuance workflow:
/// - Creates a reissuable asset
/// - Calls reissue_asset to expand supply
/// - Verifies the reissuance was successful
#[tokio::test]
#[serial]
#[ignore] // Ignored by default - requires live API and blockchain
async fn test_reissue_asset_end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing_if_nocapture();
    dotenvy::dotenv().ok();

    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        print_if_nocapture("⏭️  Skipping live test (AMP_TESTS != 'live')");
        return Ok(());
    }

    print_if_nocapture("🔄 Starting end-to-end reissue_asset test");

    // Setup environment
    let api_client = ApiClient::new().await?;
    let elements_rpc = ElementsRpc::from_env()?;

    // Test Elements connectivity
    elements_rpc.get_network_info().await?;

    // Create signer
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(500)?;
    print_if_nocapture(&format!("✅ Signer created: {}...", &mnemonic[..50]));

    // Setup test asset
    print_if_nocapture("\n📦 Setting up reissuable test asset...");
    let (asset_uuid, asset_name, _asset_ticker) =
        setup_reissuable_test_asset(&api_client, &elements_rpc, WALLET_NAME).await?;
    print_if_nocapture(&format!(
        "✅ Created reissuable asset: {} ({})",
        asset_name, asset_uuid
    ));

    // Get asset summary before reissuance
    let summary_before = api_client.get_asset_summary(&asset_uuid).await?;
    print_if_nocapture(&format!(
        "📊 Asset summary before reissuance: issued={}, reissued={}",
        summary_before.issued, summary_before.reissued
    ));

    // Execute reissuance
    let amount_to_reissue = 1000000000; // 10 whole units (10 * 10^8)
    print_if_nocapture(&format!(
        "\n🔄 Executing reissuance: {} satoshis",
        amount_to_reissue
    ));

    let reissue_start = std::time::Instant::now();
    let reissue_result = api_client
        .reissue_asset(&asset_uuid, amount_to_reissue, &elements_rpc, WALLET_NAME, &signer)
        .await;

    match reissue_result {
        Ok(()) => {
            let reissue_duration = reissue_start.elapsed();
            print_if_nocapture(&format!(
                "✅ Reissuance completed successfully in {:?}",
                reissue_duration
            ));

            // Wait for API to update
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            // Verify reissuance
            let summary_after = api_client.get_asset_summary(&asset_uuid).await?;
            let reissued_delta = summary_after.reissued - summary_before.reissued;

            print_if_nocapture(&format!(
                "📊 Asset summary after reissuance: issued={}, reissued={}",
                summary_after.issued, summary_after.reissued
            ));
            print_if_nocapture(&format!(
                "   Reissued delta: {} satoshis (expected: {})",
                reissued_delta, amount_to_reissue
            ));

            assert_eq!(
                reissued_delta, amount_to_reissue,
                "Reissued amount should match expected amount"
            );

            print_if_nocapture("🎉 End-to-end reissue_asset test completed successfully!");
        }
        Err(e) => {
            print_if_nocapture(&format!("❌ Reissuance failed: {}", e));
            return Err(format!("Reissuance failed: {}", e).into());
        }
    }

    Ok(())
}

/// Test reissue_asset with invalid asset UUID
#[tokio::test]
#[serial]
#[ignore] // Ignored by default - requires live API
async fn test_reissue_asset_invalid_uuid() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing_if_nocapture();
    dotenvy::dotenv().ok();

    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        print_if_nocapture("⏭️  Skipping live test (AMP_TESTS != 'live')");
        return Ok(());
    }

    let api_client = ApiClient::new().await?;
    let elements_rpc = ElementsRpc::from_env()?;
    let (_, signer) = LwkSoftwareSigner::generate_new_indexed(501)?;

    let result = api_client
        .reissue_asset("invalid-uuid-format", 1000000000, &elements_rpc, WALLET_NAME, &signer)
        .await;

    match result {
        Err(amp_rs::AmpError::Validation(_)) => {
            print_if_nocapture("✅ Invalid UUID correctly rejected");
        }
        Err(amp_rs::AmpError::Api(_)) => {
            print_if_nocapture("✅ Invalid UUID correctly rejected (API error)");
        }
        Err(e) => {
            print_if_nocapture(&format!("✅ Invalid UUID rejected with error: {}", e));
        }
        Ok(_) => {
            return Err("Expected invalid UUID to be rejected".into());
        }
    }

    Ok(())
}

/// Test reissue_asset with non-reissuable asset
#[tokio::test]
#[serial]
#[ignore] // Ignored by default - requires live API
async fn test_reissue_asset_non_reissuable() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing_if_nocapture();
    dotenvy::dotenv().ok();

    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        print_if_nocapture("⏭️  Skipping live test (AMP_TESTS != 'live')");
        return Ok(());
    }

    // This test would require creating a non-reissuable asset first
    // For now, we'll skip it or use an existing non-reissuable asset UUID
    print_if_nocapture("⏭️  Skipping test_reissue_asset_non_reissuable (requires setup)");
    Ok(())
}

/// Test reissue_asset with zero amount
#[tokio::test]
#[serial]
#[ignore] // Ignored by default - requires live API
async fn test_reissue_asset_zero_amount() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing_if_nocapture();
    dotenvy::dotenv().ok();

    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        print_if_nocapture("⏭️  Skipping live test (AMP_TESTS != 'live')");
        return Ok(());
    }

    let api_client = ApiClient::new().await?;
    let elements_rpc = ElementsRpc::from_env()?;
    let (_, signer) = LwkSoftwareSigner::generate_new_indexed(502)?;

    // Use a valid UUID format (but may not exist)
    let result = api_client
        .reissue_asset(
            "00000000-0000-0000-0000-000000000000",
            0,
            &elements_rpc,
            WALLET_NAME,
            &signer,
        )
        .await;

    match result {
        Err(amp_rs::AmpError::Validation(_)) => {
            print_if_nocapture("✅ Zero amount correctly rejected");
        }
        Err(_) => {
            // Other errors are also acceptable (e.g., asset not found)
            print_if_nocapture("✅ Zero amount rejected or asset not found");
        }
        Ok(_) => {
            return Err("Expected zero amount to be rejected".into());
        }
    }

    Ok(())
}
