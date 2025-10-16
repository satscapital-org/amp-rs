// Example demonstrating transaction signing integration with ElementsRpc
use amp_rs::{ElementsRpc, AmpError};
use amp_rs::signer::{Signer, SignerError};
use async_trait::async_trait;

// Mock signer for demonstration purposes
struct MockSigner {
    should_succeed: bool,
}

#[async_trait]
impl Signer for MockSigner {
    async fn sign_transaction(&self, unsigned_tx: &str) -> Result<String, SignerError> {
        if self.should_succeed {
            // Simulate signing by appending signature data
            Ok(format!("{}deadbeefcafebabe1234567890abcdef", unsigned_tx))
        } else {
            Err(SignerError::Lwk("Mock signing failure for demonstration".to_string()))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), AmpError> {
    println!("Transaction Signing Integration Example");
    println!("=====================================");
    
    // Create ElementsRpc client
    let rpc = ElementsRpc::new(
        "http://localhost:18884".to_string(),
        "user".to_string(),
        "pass".to_string(),
    );
    
    println!("✓ ElementsRpc client created successfully");
    
    // Example unsigned transaction (minimal valid hex)
    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef";
    println!("📝 Unsigned transaction: {}...", &unsigned_tx[..32]);
    
    // Test with successful mock signer
    println!("\n🔐 Testing transaction signing with successful signer...");
    let success_signer = MockSigner { should_succeed: true };
    
    match rpc.sign_transaction(unsigned_tx, &success_signer).await {
        Ok(signed_tx) => {
            println!("✅ Transaction signed successfully!");
            println!("📝 Signed transaction: {}...", &signed_tx[..32]);
            println!("📏 Size increase: {} -> {} bytes", 
                     unsigned_tx.len() / 2, 
                     signed_tx.len() / 2);
        }
        Err(e) => {
            println!("❌ Signing failed: {}", e);
        }
    }
    
    // Test with failing mock signer
    println!("\n🔐 Testing transaction signing with failing signer...");
    let fail_signer = MockSigner { should_succeed: false };
    
    match rpc.sign_transaction(unsigned_tx, &fail_signer).await {
        Ok(_) => {
            println!("❌ Unexpected success - signer should have failed");
        }
        Err(e) => {
            println!("✅ Expected signing failure: {}", e);
            println!("🔍 Error type: {}", 
                     if e.to_string().contains("Signer error") { "AmpError::Signer" } else { "Other" });
        }
    }
    
    // Test validation features
    println!("\n🔍 Testing input validation...");
    
    // Test empty transaction
    match rpc.sign_transaction("", &success_signer).await {
        Ok(_) => println!("❌ Empty transaction should have failed"),
        Err(e) => println!("✅ Empty transaction rejected: {}", e),
    }
    
    // Test invalid hex
    match rpc.sign_transaction("invalid_hex_zz", &success_signer).await {
        Ok(_) => println!("❌ Invalid hex should have failed"),
        Err(e) => println!("✅ Invalid hex rejected: {}", e),
    }
    
    // Test odd length hex
    match rpc.sign_transaction("abc", &success_signer).await {
        Ok(_) => println!("❌ Odd length hex should have failed"),
        Err(e) => println!("✅ Odd length hex rejected: {}", e),
    }
    
    println!("\n🎯 Key Features Demonstrated:");
    println!("  • Transaction hex format validation");
    println!("  • Signer trait integration with async/await");
    println!("  • Comprehensive error handling and propagation");
    println!("  • Signed transaction structure validation");
    println!("  • Size and format checks for security");
    
    println!("\n🔧 Integration Points:");
    println!("  • ElementsRpc.sign_transaction() - Core signing method");
    println!("  • ElementsRpc.sign_and_broadcast_transaction() - Convenience method");
    println!("  • AmpError::Signer - Proper error type conversion");
    println!("  • Signer trait - Flexible signing backend support");
    
    println!("\n✨ Ready for integration with:");
    println!("  • LwkSoftwareSigner for testnet/regtest");
    println!("  • Hardware wallets (future implementations)");
    println!("  • Remote signing services (future implementations)");
    println!("  • Custom signing backends via Signer trait");
    
    Ok(())
}