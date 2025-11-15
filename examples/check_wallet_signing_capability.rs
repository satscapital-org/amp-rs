//! Diagnostic script to check wallet type and signing capability

use amp_rs::ElementsRpc;
use dotenvy;
use std::env;

const WALLET_NAME: &str = "amp_elements_wallet_static_for_funding";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🔍 Wallet Diagnostics");
    println!("====================");
    println!("Wallet: {}", WALLET_NAME);
    println!();

    dotenvy::dotenv().ok();

    // Check both local and cloud
    for (name, url_var, user_var, pass_var) in [
        (
            "Local",
            "ELEMENTS_RPC_URL",
            "ELEMENTS_RPC_USER",
            "ELEMENTS_RPC_PASSWORD",
        ),
        (
            "Cloud",
            "CLOUD_ELEMENTS_RPC_URL",
            "CLOUD_ELEMENTS_RPC_USER",
            "CLOUD_ELEMENTS_RPC_PASSWORD",
        ),
    ] {
        println!("📡 Checking {} Node", name);
        println!("-------------------");

        let url = match env::var(url_var) {
            Ok(u) => u,
            Err(_) => {
                println!("⚠️  {} not set, skipping\n", url_var);
                continue;
            }
        };

        let user = env::var(user_var)?;
        let password = env::var(pass_var)?;

        let rpc = ElementsRpc::new(url, user, password);

        // Load wallet
        match rpc.load_wallet(WALLET_NAME).await {
            Ok(()) => println!("✅ Wallet loaded"),
            Err(e) if e.to_string().contains("already loaded") => {
                println!("✅ Wallet already loaded")
            }
            Err(e) => {
                println!("❌ Failed to load wallet: {}", e);
                println!();
                continue;
            }
        }

        // Get wallet info
        let wallet_info = rpc.get_wallet_info(WALLET_NAME).await?;

        let is_descriptor = wallet_info
            .get("descriptors")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        println!(
            "Wallet type: {}",
            if is_descriptor {
                "Descriptor"
            } else {
                "Legacy"
            }
        );

        if is_descriptor {
            // Check descriptors
            match rpc.list_descriptors(WALLET_NAME, Some(false)).await {
                Ok(descriptors) => {
                    println!("Number of descriptors: {}", descriptors.len());

                    // Check if any descriptor contains private keys
                    let has_private = descriptors
                        .iter()
                        .any(|d| d.contains("prv") || d.contains("xprv"));
                    println!(
                        "Has private keys: {}",
                        if has_private {
                            "❌ NO (watch-only)"
                        } else {
                            "⚠️  Unknown"
                        }
                    );

                    // Try to list with private keys explicitly
                    match rpc.list_descriptors(WALLET_NAME, Some(true)).await {
                        Ok(private_descriptors) => {
                            let actually_has_private = private_descriptors
                                .iter()
                                .any(|d| d.contains("prv") || d.contains("xprv"));
                            println!(
                                "Private key check: {}",
                                if actually_has_private {
                                    "✅ YES"
                                } else {
                                    "❌ NO (watch-only)"
                                }
                            );
                        }
                        Err(e) => {
                            println!("Private key check: ❌ Failed - {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to list descriptors: {}", e);
                }
            }
        } else {
            // For legacy wallets, check if we can dump a private key
            // Try to get a new address first
            match rpc.get_new_address(WALLET_NAME, None).await {
                Ok(address) => match rpc.dump_private_key(WALLET_NAME, &address).await {
                    Ok(_) => println!("Private key access: ✅ YES (can sign)"),
                    Err(e) => println!("Private key access: ❌ NO - {}", e),
                },
                Err(e) => {
                    println!("❌ Failed to get address: {}", e);
                }
            }
        }

        println!();
    }

    Ok(())
}
