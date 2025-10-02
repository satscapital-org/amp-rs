use amp_rs::ApiClient;
use std::fs;
use tokio;

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();
    
    let client = ApiClient::new().expect("Failed to create API client");

    println!("Fetching changelog...");
    let changelog = client.get_changelog().await;

    match changelog {
        Ok(log) => {
            println!("Changelog received, writing to file...");
            let json_pretty = serde_json::to_string_pretty(&log).unwrap();
            
            // Create markdown content
            let markdown_content = format!(
                "# AMP API Changelog\n\n```json\n{}\n```\n",
                json_pretty
            );
            
            // Write to file
            fs::write("amp_changelog.md", markdown_content)
                .expect("Failed to write changelog to file");
            
            println!("Changelog saved to amp_changelog.md");
        }
        Err(e) => {
            eprintln!("Error fetching changelog: {:?}", e);
        }
    }
}
