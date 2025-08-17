use amp_rs::ApiClient;
use tokio;

#[tokio::main]
async fn main() {
    let client = ApiClient::new().expect("Failed to create API client");

    println!("Fetching changelog...");
    let changelog = client.get_changelog().await;

    match changelog {
        Ok(log) => {
            println!("Changelog received:");
            println!("{}", serde_json::to_string_pretty(&log).unwrap());
        }
        Err(e) => {
            eprintln!("Error fetching changelog: {:?}", e);
        }
    }
}
