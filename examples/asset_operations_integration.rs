//! # Asset Operations Integration Example
//!
//! This example demonstrates how to integrate LwkSoftwareSigner with asset operations
//! in the amp-rust crate. It shows patterns for reissue_asset, distribute_asset,
//! and burn_asset functions using the Signer trait.
//!
//! ## ⚠️ SECURITY WARNING ⚠️
//!
//! This example is for TESTNET/REGTEST ONLY. Never use these patterns with real funds
//! or in production environments.

use amp_rs::signer::{LwkSoftwareSigner, Signer, SignerError};
use std::collections::HashMap;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🏭 Asset Operations Integration Examples");
    println!("⚠️  TESTNET/REGTEST ONLY - Never use with real funds!");
    println!();

    // Example 1: Single signer asset operations
    single_signer_operations().await?;

    // Example 2: Multi-signer asset operations
    multi_signer_operations().await?;

    // Example 3: Role-based asset management
    role_based_asset_management().await?;

    // Example 4: Test isolation patterns
    test_isolation_patterns().await?;

    println!("✅ All integration examples completed successfully!");

    Ok(())
}

/// Example 1: Single signer performing multiple asset operations
async fn single_signer_operations() -> Result<(), BoxError> {
    println!("📝 Example 1: Single Signer Asset Operations");

    // Create a signer for asset operations
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(0)?;
    println!("   Created signer with mnemonic: {}...", &mnemonic[..30]);

    // Mock asset operations (replace with actual AMP API calls)
    let asset_id = mock_issue_asset(&signer, "TestCoin", 1000000).await?;
    println!("   ✅ Issued asset: {}", asset_id);

    println!();
    Ok(())
}

/// Example 2: Multi-signer asset operations with different roles
async fn multi_signer_operations() -> Result<(), BoxError> {
    println!("👥 Example 2: Multi-Signer Asset Operations");

    // Create signers for different roles
    let (_, issuer_signer) = LwkSoftwareSigner::generate_new_indexed(10)?;
    let (_, distributor_signer) = LwkSoftwareSigner::generate_new_indexed(11)?;
    let (_, _treasury_signer) = LwkSoftwareSigner::generate_new_indexed(12)?;

    println!("   Created issuer, distributor, and treasury signers");

    // Issue asset with issuer signer
    let asset_id = mock_issue_asset(&issuer_signer, "MultiSigCoin", 2000000).await?;
    println!("   ✅ Issuer created asset: {}", asset_id);

    // Distribute asset with distributor signer
    let recipients = vec![
        ("user_address_1".to_string(), 10000),
        ("user_address_2".to_string(), 15000),
        ("user_address_3".to_string(), 20000),
    ];
    let distribute_tx = mock_distribute_asset(&distributor_signer, &asset_id, recipients).await?;
    println!("   ✅ Distributor sent assets: {}", distribute_tx);

    println!();
    Ok(())
}

/// Example 3: Role-based asset management with consistent indices
async fn role_based_asset_management() -> Result<(), BoxError> {
    println!("🎭 Example 3: Role-Based Asset Management");

    // Use consistent indices for roles (enables predictable testing)
    let roles = AssetRoles::new().await?;

    println!("   Created role-based signers with consistent indices");

    // Asset lifecycle with role separation
    let asset_id = mock_issue_asset(&roles.issuer, "RoleCoin", 5000000).await?;
    println!("   ✅ Issuer created asset: {}", asset_id);

    // Distribution to users
    let user_distributions = vec![
        (mock_get_address(&roles.user_a), 25000),
        (mock_get_address(&roles.user_b), 30000),
        (mock_get_address(&roles.user_c), 35000),
    ];
    let distribute_tx =
        mock_distribute_asset(&roles.distributor, &asset_id, user_distributions).await?;
    println!("   ✅ Distributed to users: {}", distribute_tx);

    println!();
    Ok(())
}

/// Example 4: Test isolation patterns for concurrent testing
async fn test_isolation_patterns() -> Result<(), BoxError> {
    println!("🧪 Example 4: Test Isolation Patterns");

    // Pattern 1: Index ranges for different test suites
    println!("   Pattern 1: Index-based test isolation");
    let _test_suite_a_signers = create_test_suite_signers(100, 3).await?;
    let _test_suite_b_signers = create_test_suite_signers(200, 3).await?;

    println!("   ✅ Created isolated signer sets for different test suites");

    // Pattern 2: Environment-specific signers
    println!("   Pattern 2: Environment-specific signers");
    let _regtest_signers = create_environment_signers("regtest", 1000).await?;
    let _testnet_signers = create_environment_signers("testnet", 2000).await?;

    println!("   ✅ Created environment-specific signer sets");

    // Pattern 3: Concurrent operations with isolation
    println!("   Pattern 3: Concurrent operations");
    let concurrent_results = run_concurrent_asset_operations(300, 5).await?;
    println!(
        "   ✅ Completed {} concurrent operations",
        concurrent_results.len()
    );

    println!();
    Ok(())
}

/// Struct to hold role-based signers with consistent indices
struct AssetRoles {
    issuer: LwkSoftwareSigner,
    distributor: LwkSoftwareSigner,
    user_a: LwkSoftwareSigner,
    user_b: LwkSoftwareSigner,
    user_c: LwkSoftwareSigner,
}

impl AssetRoles {
    async fn new() -> Result<Self, SignerError> {
        // Use consistent indices for each role (enables predictable testing)
        let (_, issuer) = LwkSoftwareSigner::generate_new_indexed(500)?;
        let (_, distributor) = LwkSoftwareSigner::generate_new_indexed(501)?;
        let (_, user_a) = LwkSoftwareSigner::generate_new_indexed(504)?;
        let (_, user_b) = LwkSoftwareSigner::generate_new_indexed(505)?;
        let (_, user_c) = LwkSoftwareSigner::generate_new_indexed(506)?;

        Ok(Self {
            issuer,
            distributor,
            user_a,
            user_b,
            user_c,
        })
    }
}

/// Create signers for a test suite using a specific index range
async fn create_test_suite_signers(
    start_index: usize,
    count: usize,
) -> Result<Vec<LwkSoftwareSigner>, SignerError> {
    let mut signers = Vec::new();

    for i in 0..count {
        let (_, signer) = LwkSoftwareSigner::generate_new_indexed(start_index + i)?;
        signers.push(signer);
    }

    Ok(signers)
}

/// Create environment-specific signers
async fn create_environment_signers(
    environment: &str,
    base_index: usize,
) -> Result<HashMap<String, LwkSoftwareSigner>, SignerError> {
    let mut signers = HashMap::new();

    let roles = vec!["issuer", "distributor", "user1", "user2", "treasury"];

    for (i, role) in roles.iter().enumerate() {
        let (_, signer) = LwkSoftwareSigner::generate_new_indexed(base_index + i)?;
        signers.insert(format!("{}_{}", environment, role), signer);
    }

    Ok(signers)
}

/// Run concurrent asset operations with isolated signers
async fn run_concurrent_asset_operations(
    base_index: usize,
    count: usize,
) -> Result<Vec<String>, BoxError> {
    let mut handles = Vec::new();

    for i in 0..count {
        let handle = tokio::spawn(async move {
            let (_, signer) = LwkSoftwareSigner::generate_new_indexed(base_index + i)
                .map_err(|e| Box::new(e) as BoxError)?;
            let asset_name = format!("ConcurrentAsset_{}", i);
            mock_issue_asset(&signer, &asset_name, 100000).await
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await??);
    }

    Ok(results)
}

// Mock functions for demonstration (replace with actual AMP API calls)

async fn mock_issue_asset(
    signer: &dyn Signer,
    asset_name: &str,
    amount: u64,
) -> Result<String, BoxError> {
    // In real implementation:
    // 1. Call AMP API to create issuance request
    // 2. Get unsigned transaction from response
    // 3. Sign transaction with signer
    // 4. Submit signed transaction

    let mock_unsigned_tx = "020000000001..."; // Mock unsigned transaction
    let _signed_tx = signer
        .sign_transaction(mock_unsigned_tx)
        .await
        .unwrap_or_else(|_| "mock_signed_tx".to_string());

    Ok(format!("asset_{}_{}", asset_name, amount))
}

async fn mock_distribute_asset(
    signer: &dyn Signer,
    asset_id: &str,
    recipients: Vec<(String, u64)>,
) -> Result<String, BoxError> {
    let mock_unsigned_tx = "020000000004...";
    let _signed_tx = signer
        .sign_transaction(mock_unsigned_tx)
        .await
        .unwrap_or_else(|_| "mock_distribute_tx".to_string());
    Ok(format!(
        "distribute_{}_{}_recipients",
        asset_id,
        recipients.len()
    ))
}

fn mock_get_address(signer: &LwkSoftwareSigner) -> String {
    // In real implementation, derive address from signer's public key
    format!("mock_address_{}", signer.is_testnet())
}
