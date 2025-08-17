use amp_rs::ApiClient;
use std::env;

#[tokio::test]
async fn test_get_changelog() {
    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let client = ApiClient::new().unwrap();
    let changelog = client.get_changelog().await;

    assert!(changelog.is_ok());
    let changelog_val = changelog.unwrap();
    assert!(changelog_val.as_object().unwrap().len() > 0);
}
