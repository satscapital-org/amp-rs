//! # Signer Usage Example
//!
//! This example demonstrates how to use the LwkSoftwareSigner for transaction signing
//! in testnet/regtest environments.
//!
//! ## ⚠️ SECURITY WARNING ⚠️
//!
//! This example is for TESTNET/REGTEST ONLY. Never use these patterns with real funds
//! or in production environments. The signer stores mnemonic phrases in plain text.

use amp_rs::signer::{LwkSoftwareSigner, Signer, SignerError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see signer operations
    tracing_subscriber::fmt::init();

    println!("🔐 LwkSoftwareSigner Usage Examples");
    println!("⚠️  TESTNET/REGTEST ONLY - Never use with real funds!");
    println!();

    // Example 1: Create signer from existing mnemonic
    println!("📝 Example 1: Creating signer from existing mnemonic");
    let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let signer1 = LwkSoftwareSigner::new(test_mnemonic)?;
    println!(
        "✅ Created signer from mnemonic (testnet: {})",
        signer1.is_testnet()
    );
    println!();

    // Example 2: Generate new signer with automatic mnemonic management
    println!("🎲 Example 2: Generating new signer with file persistence");
    let (mnemonic2, signer2) = LwkSoftwareSigner::generate_new()?;
    println!("✅ Generated signer with mnemonic: {}...", &mnemonic2[..50]);
    println!("   Mnemonic saved to mnemonic.local.json");
    println!();

    // Example 3: Indexed mnemonic access for test isolation
    println!("🔢 Example 3: Using indexed mnemonics for test isolation");
    let (mnemonic_alice, alice_signer) = LwkSoftwareSigner::generate_new_indexed(0)?;
    let (mnemonic_bob, bob_signer) = LwkSoftwareSigner::generate_new_indexed(1)?;
    let (mnemonic_charlie, charlie_signer) = LwkSoftwareSigner::generate_new_indexed(2)?;

    println!("✅ Alice (index 0): {}...", &mnemonic_alice[..30]);
    println!("✅ Bob (index 1): {}...", &mnemonic_bob[..30]);
    println!("✅ Charlie (index 2): {}...", &mnemonic_charlie[..30]);
    println!("   All signers configured for testnet");
    println!();

    // Example 4: Error handling demonstration
    println!("❌ Example 4: Error handling");

    // Invalid mnemonic
    match LwkSoftwareSigner::new("invalid mnemonic with wrong word count") {
        Ok(_) => println!("Unexpected success"),
        Err(SignerError::InvalidMnemonic(msg)) => {
            println!("✅ Caught invalid mnemonic error: {}", msg);
        }
        Err(e) => println!("Unexpected error: {}", e),
    }

    // Invalid transaction hex
    match signer1.sign_transaction("invalid_hex").await {
        Ok(_) => println!("Unexpected success"),
        Err(SignerError::HexParse(_)) => {
            println!("✅ Caught hex parsing error as expected");
        }
        Err(e) => println!("Unexpected error: {}", e),
    }

    // Empty transaction
    match signer1.sign_transaction("").await {
        Ok(_) => println!("Unexpected success"),
        Err(SignerError::InvalidTransaction(msg)) => {
            println!("✅ Caught empty transaction error: {}", msg);
        }
        Err(e) => println!("Unexpected error: {}", e),
    }
    println!();

    // Example 5: Demonstrate network configuration
    println!("🌐 Example 5: Network configuration verification");
    let signers = vec![
        &signer1,
        &signer2,
        &alice_signer,
        &bob_signer,
        &charlie_signer,
    ];
    for (i, signer) in signers.iter().enumerate() {
        println!("   Signer {}: testnet = {}", i + 1, signer.is_testnet());
    }
    println!("✅ All signers correctly configured for testnet");
    println!();

    // Example 6: Mock transaction signing (with invalid but properly formatted hex)
    println!("📝 Example 6: Transaction signing attempt");
    println!("   Note: This will fail because we're using mock transaction data");

    // This is a properly formatted hex string but not a valid Elements transaction
    let mock_tx_hex = "0200000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    match signer1.sign_transaction(mock_tx_hex).await {
        Ok(signed_tx) => {
            println!("✅ Transaction signed successfully!");
            println!("   Signed TX: {}...", &signed_tx[..64]);
        }
        Err(SignerError::InvalidTransaction(msg)) => {
            println!(
                "❌ Transaction signing failed (expected with mock data): {}",
                msg
            );
        }
        Err(e) => {
            println!("❌ Signing failed with error: {}", e);
        }
    }
    println!();

    // Example 7: Asset integration patterns
    asset_integration_example().await?;
    println!();

    println!("🎯 Summary:");
    println!("   - Created signers from existing and generated mnemonics");
    println!("   - Demonstrated indexed mnemonic access for test isolation");
    println!("   - Showed comprehensive error handling");
    println!("   - Verified testnet-only configuration");
    println!("   - Attempted transaction signing with mock data");
    println!("   - Demonstrated asset operation integration patterns");
    println!();
    println!("📁 Check mnemonic.local.json for persistent mnemonic storage");
    println!("⚠️  Remember: This is for TESTNET/REGTEST development only!");

    Ok(())
}

/// Helper function to demonstrate signer trait usage
async fn sign_with_trait(signer: &dyn Signer, tx_hex: &str) -> Result<String, SignerError> {
    // This function accepts any implementation of the Signer trait
    signer.sign_transaction(tx_hex).await
}

/// Example of using multiple signers in a test scenario
async fn multi_signer_test_scenario() -> Result<(), SignerError> {
    println!("🧪 Multi-signer test scenario");

    // Create signers for different test roles
    let (_, issuer_signer) = LwkSoftwareSigner::generate_new_indexed(10)?; // Issuer
    let (_, distributor_signer) = LwkSoftwareSigner::generate_new_indexed(11)?; // Distributor
    let (_, user_signer) = LwkSoftwareSigner::generate_new_indexed(12)?; // End user

    println!("✅ Created signers for issuer, distributor, and user roles");
    println!("   Each has a unique mnemonic for test isolation");

    // Verify all are testnet signers
    assert!(issuer_signer.is_testnet());
    assert!(distributor_signer.is_testnet());
    assert!(user_signer.is_testnet());

    println!("✅ All signers verified for testnet usage");

    // Demonstrate polymorphic usage with Signer trait
    let signers: Vec<&dyn Signer> = vec![&issuer_signer, &distributor_signer, &user_signer];

    println!("🔄 Testing polymorphic signer usage:");
    for (i, signer) in signers.iter().enumerate() {
        match sign_with_trait(*signer, "invalid_hex").await {
            Err(SignerError::HexParse(_)) => {
                println!("   ✅ Signer {} correctly rejected invalid hex", i + 1);
            }
            _ => println!("   ❌ Signer {} unexpected result", i + 1),
        }
    }

    Ok(())
}

/// Example showing integration patterns for asset operations
async fn asset_integration_example() -> Result<(), SignerError> {
    println!("🏭 Asset Integration Example");

    // Create role-based signers with consistent indices for testing
    let (_, asset_issuer) = LwkSoftwareSigner::generate_new_indexed(100)?;
    let (_, asset_distributor) = LwkSoftwareSigner::generate_new_indexed(101)?;
    let (_, end_user_a) = LwkSoftwareSigner::generate_new_indexed(102)?;
    let (_, _end_user_b) = LwkSoftwareSigner::generate_new_indexed(103)?;

    println!("✅ Created role-based signers:");
    println!("   - Asset Issuer (index 100)");
    println!("   - Asset Distributor (index 101)");
    println!("   - End User A (index 102)");
    println!("   - End User B (index 103)");

    // Simulate asset operation workflow
    println!("🔄 Simulating asset operation workflow:");

    // Step 1: Asset issuance (issuer signs)
    println!("   1. Asset Issuance - Issuer creates new asset");
    let mock_issuance_tx = "0200000000010001..."; // Mock unsigned issuance transaction
    match asset_issuer.sign_transaction(mock_issuance_tx).await {
        Err(SignerError::InvalidTransaction(_)) => {
            println!("      ✅ Issuer signing attempted (mock data rejected as expected)");
        }
        _ => println!("      ❓ Unexpected result with mock data"),
    }

    // Step 2: Asset distribution (distributor signs)
    println!("   2. Asset Distribution - Distributor sends to users");
    let mock_distribution_tx = "0200000000010002..."; // Mock unsigned distribution transaction
    match asset_distributor
        .sign_transaction(mock_distribution_tx)
        .await
    {
        Err(SignerError::InvalidTransaction(_)) => {
            println!("      ✅ Distributor signing attempted (mock data rejected as expected)");
        }
        _ => println!("      ❓ Unexpected result with mock data"),
    }

    // Step 3: User-to-user transfer (user A signs)
    println!("   3. User Transfer - User A sends to User B");
    let mock_transfer_tx = "0200000000010003..."; // Mock unsigned transfer transaction
    match end_user_a.sign_transaction(mock_transfer_tx).await {
        Err(SignerError::InvalidTransaction(_)) => {
            println!("      ✅ User A signing attempted (mock data rejected as expected)");
        }
        _ => println!("      ❓ Unexpected result with mock data"),
    }

    println!("✅ Asset integration workflow demonstrated");
    println!("   Note: Real integration would use valid transaction data from AMP API");

    Ok(())
}
