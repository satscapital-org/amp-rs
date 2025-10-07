use amp_rs::ApiClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file like the tests do
    dotenvy::from_filename_override(".env").ok();
    
    println!("Environment variables:");
    println!("  AMP_TESTS: {:?}", env::var("AMP_TESTS"));
    println!("  AMP_USERNAME: {:?}", env::var("AMP_USERNAME").map(|_| "[REDACTED]"));
    println!("  AMP_PASSWORD: {:?}", env::var("AMP_PASSWORD").map(|_| "[REDACTED]"));
    println!("  AMP_API_BASE_URL: {:?}", env::var("AMP_API_BASE_URL"));
    
    // Check if token file exists
    let token_exists = tokio::fs::try_exists("token.json").await.unwrap_or(false);
    println!("  Token file exists: {}", token_exists);
    
    if token_exists {
        let token_content = tokio::fs::read_to_string("token.json").await?;
        println!("  Token file content: {}", token_content);
    }
    
    // Create client and see if it loads the token
    println!("\nCreating API client...");
    let client = ApiClient::new().await?;
    
    // Check token info
    let token_info = client.get_token_info().await?;
    match token_info {
        Some(info) => {
            println!("✅ Token loaded successfully!");
            println!("  Expires at: {}", info.expires_at);
            println!("  Is expired: {}", info.is_expired);
            println!("  Expires soon: {}", info.expires_soon);
        }
        None => {
            println!("❌ No token loaded");
        }
    }
    
    // Try to make an API call to see if the token works
    println!("\nTesting API call with loaded token...");
    match client.get_changelog().await {
        Ok(_) => {
            println!("✅ API call successful - token is valid!");
        }
        Err(e) => {
            println!("❌ API call failed: {:?}", e);
            println!("This suggests the token may be invalid or expired on the server side");
        }
    }
    
    Ok(())
}