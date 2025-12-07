//! List Asset Transactions Example
//!
//! This example demonstrates how to retrieve and display all transactions for an asset.
//! Transactions include issuance, transfers, reissuance, burns, and other transaction types.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example list_asset_transactions
//! cargo run --example list_asset_transactions <asset_uuid>
//! ```
//!
//! Make sure to set up your `.env` file with `AMP_USERNAME` and `AMP_PASSWORD`

use amp_rs::{model::AssetTransactionParams, ApiClient};
use std::env;

fn print_transaction(index: usize, tx: &amp_rs::model::AssetTransaction) {
    println!("─── Transaction {} ───", index + 1);
    println!("  Transaction ID:     {}", tx.txid);
    println!("  Type:               {}", tx.transaction_type());
    println!("  Date/Time:          {}", tx.datetime);
    println!("  Block Height:       {}", tx.blockheight);

    // Print flags
    let mut flags = Vec::new();
    if tx.is_issuance {
        flags.push("issuance");
    }
    if tx.is_reissuance {
        flags.push("reissuance");
    }
    if tx.is_distribution {
        flags.push("distribution");
    }
    if !flags.is_empty() {
        println!("  Flags:              {}", flags.join(", "));
    }

    // Print inputs summary
    if !tx.inputs.is_empty() {
        println!("  Inputs:             {} input(s)", tx.inputs.len());
        for (i, input) in tx.inputs.iter().enumerate() {
            let gaid_str = input.gaid.as_deref().unwrap_or("-");
            let treasury_str = if input.is_treasury { " [treasury]" } else { "" };
            println!(
                "    [{i}] amount: {}, GAID: {gaid_str}{treasury_str}",
                input.amount
            );
        }
    }

    // Print outputs summary
    if !tx.outputs.is_empty() {
        println!("  Outputs:            {} output(s)", tx.outputs.len());
        for (i, output) in tx.outputs.iter().enumerate() {
            let gaid_str = output.gaid.as_deref().unwrap_or("-");
            let mut status = Vec::new();
            if output.is_treasury {
                status.push("treasury");
            }
            if output.is_spent {
                status.push("spent");
            }
            if output.is_burnt {
                status.push("burnt");
            }
            let status_str = if status.is_empty() {
                String::new()
            } else {
                format!(" [{}]", status.join(", "))
            };
            println!(
                "    [{i}] vout: {}, amount: {}, GAID: {gaid_str}{status_str}",
                output.vout, output.amount
            );
        }
    }

    // Print totals
    println!(
        "  Total Input:        {}",
        tx.total_input_amount()
    );
    println!(
        "  Total Output:       {}",
        tx.total_output_amount()
    );

    // Print explorer URL (truncated)
    let url_preview = if tx.unblinded_url.len() > 60 {
        format!("{}...", &tx.unblinded_url[..60])
    } else {
        tx.unblinded_url.clone()
    };
    println!("  Explorer URL:       {url_preview}");

    println!();
}

fn print_summary(transactions: &[amp_rs::model::AssetTransaction]) {
    println!("═══════════════════════════════════════════════════════════");
    println!("Transaction Summary:");
    println!("═══════════════════════════════════════════════════════════");

    let mut issuance_count = 0;
    let mut reissuance_count = 0;
    let mut distribution_count = 0;
    let mut transfer_count = 0;

    for tx in transactions {
        if tx.is_issuance {
            issuance_count += 1;
        } else if tx.is_reissuance {
            reissuance_count += 1;
        } else if tx.is_distribution {
            distribution_count += 1;
        } else {
            transfer_count += 1;
        }
    }

    println!("  {:<20} {:>5}", "Issuances:", issuance_count);
    println!("  {:<20} {:>5}", "Reissuances:", reissuance_count);
    println!("  {:<20} {:>5}", "Distributions:", distribution_count);
    println!("  {:<20} {:>5}", "Transfers:", transfer_count);
    println!("  {:<20} {:>5}", "Total:", transactions.len());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Get UUID from command line arguments or use default
    let args: Vec<String> = env::args().collect();
    let asset_uuid = if args.len() > 1 {
        args[1].clone()
    } else {
        "bc2d31af-60d0-4346-bfba-11b045f92dff".to_string()
    };

    println!("Fetching transactions for asset: {asset_uuid}\n");

    // Create API client
    let client = ApiClient::new().await?;

    // Get asset transactions with parameters to fetch all
    let params = AssetTransactionParams {
        count: Some(1000), // Fetch up to 1000 transactions
        sortorder: Some("desc".to_string()), // Most recent first
        ..Default::default()
    };

    let transactions = client.get_asset_transactions(&asset_uuid, &params).await?;

    if transactions.is_empty() {
        println!("No transactions found for this asset.");
        return Ok(());
    }

    println!("Found {} transactions:\n", transactions.len());
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                  ASSET TRANSACTIONS                                        ║");
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════╝");
    println!();

    // Display each transaction
    for (index, tx) in transactions.iter().enumerate() {
        print_transaction(index, tx);
    }

    // Summary statistics by transaction type
    print_summary(&transactions);

    Ok(())
}
