use amp_rs::signer::lwk::LwkSoftwareSigner;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Generating LWK address for funding...");
    
    // Use the same mnemonic index as the test (300)
    let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(300)?;
    println!("📝 Mnemonic (first 50 chars): {}...", &mnemonic[..50]);
    
    // Get the address from the signer (using default index 0)
    let address = signer.derive_address(Some(0))?;
    println!("🏠 LWK Address: {}", address);
    
    println!("\n💰 Send L-BTC to this address: {}", address);
    println!("   This is the address that the LWK signer will use in tests.");
    println!("   Note: The test also creates Elements wallets with different addresses,");
    println!("   but the LWK signer always uses this same address from the mnemonic.");
    
    Ok(())
}