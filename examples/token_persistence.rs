use amp_rs::{ApiClient, Error};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing for better debugging
    tracing_subscriber::fmt::init();

    // Enable token persistence for this example
    env::set_var("AMP_TOKEN_PERSISTENCE", "true");

    println!("🔐 Token Persistence Example");
    println!("============================");

    // Create API client
    let client = ApiClient::new().await?;

    // First run: Get token (will obtain from API and persist to disk)
    println!("\n📥 First token request (will obtain from API):");
    let token1 = client.get_token().await?;
    println!("✅ Token obtained: {}...", &token1[..20]);

    // Check token info
    if let Some(token_info) = client.get_token_info().await? {
        println!("📊 Token expires at: {}", token_info.expires_at);
        println!("📊 Token age: {:?}", token_info.age);
        println!("📊 Expires in: {:?}", token_info.expires_in);
        println!("📊 Is expired: {}", token_info.is_expired);
        println!("📊 Expires soon: {}", token_info.expires_soon);
    }

    // Second run: Get token (should load from disk if still valid)
    println!("\n📥 Second token request (should load from disk):");
    let token2 = client.get_token().await?;
    println!("✅ Token retrieved: {}...", &token2[..20]);

    // Verify tokens are the same (loaded from disk)
    if token1 == token2 {
        println!("✅ Token successfully loaded from disk!");
    } else {
        println!("⚠️  Different token - may have been refreshed");
    }

    // Demonstrate force refresh
    println!("\n🔄 Force refresh token:");
    let token3 = client.force_refresh().await?;
    println!("✅ Token refreshed: {}...", &token3[..20]);

    // Clear token to demonstrate cleanup
    println!("\n🧹 Clearing token:");
    client.clear_token().await?;
    println!("✅ Token cleared from memory and disk");

    // Final token request (will obtain fresh token)
    println!("\n📥 Final token request (will obtain fresh token):");
    let token4 = client.get_token().await?;
    println!("✅ Fresh token obtained: {}...", &token4[..20]);

    println!("\n🎉 Token persistence example completed successfully!");
    Ok(())
}