use amp_rs::signer::lwk::LwkSoftwareSigner;
use amp_rs::ElementsRpc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Generating test address for funding...");
    
    // Use the same mnemonic index as the test
    let (mnemonic, _signer) = LwkSoftwareSigner::generate_new_indexed(300)?;
    println!("📝 Mnemonic (first 50 chars): {}...", &mnemonic[..50]);
    
    // Create Elements RPC client
    let elements_rpc = ElementsRpc::from_env()?;
    
    // Create a wallet name for funding (fixed name for reuse)
    let wallet_name = "amp_elements_wallet_funding_static";
    
    // Create wallet
    match elements_rpc.create_wallet(wallet_name, false).await {
        Ok(_) => println!("✅ Created Elements wallet: {}", wallet_name),
        Err(e) if e.to_string().contains("already exists") => {
            println!("ℹ️  Wallet {} already exists", wallet_name);
        }
        Err(e) => return Err(e.into()),
    }
    
    // Generate address
    let address = elements_rpc.get_new_address(wallet_name, Some("bech32")).await?;
    println!("🏠 Unconfidential address: {}", address);
    
    // Get confidential version
    let confidential_address = elements_rpc.get_confidential_address(wallet_name, &address).await?;
    println!("🔐 Confidential address: {}", confidential_address);
    
    println!("\n💰 Send L-BTC to either address:");
    println!("   Unconfidential: {}", address);
    println!("   Confidential:   {}", confidential_address);
    
    Ok(())
}