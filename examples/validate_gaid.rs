//! GAID Validation Example
//!
//! This example demonstrates how to validate a GAID (Global Asset ID) using the AMP API.
//! 
//! Usage:
//!   cargo run --example validate_gaid <GAID>
//!
//! Example:
//!   cargo run --example validate_gaid GAbYScu6jkWUND2jo3L4KJxyvo55d

use amp_rs::ApiClient;
use std::env;
use tokio;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get GAID from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <GAID>", args[0]);
        eprintln!("Example: {} GAbYScu6jkWUND2jo3L4KJxyvo55d", args[0]);
        std::process::exit(1);
    }

    let gaid = &args[1];
    println!("Validating GAID: {}", gaid);

    let client = ApiClient::new().await.expect("Failed to create API client");

    match client.validate_gaid(gaid).await {
        Ok(validation) => {
            println!("GAID validation result:");
            println!("  GAID: {}", gaid);
            println!("  Valid: {}", validation.is_valid);
            
            if let Some(error) = &validation.error {
                println!("  Error: {}", error);
            }

            if validation.is_valid {
                println!("✅ GAID is valid");
                std::process::exit(0);
            } else {
                println!("❌ GAID is invalid");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error validating GAID: {:?}", e);
            std::process::exit(1);
        }
    }
}