use amp_rs::client::{ApiClient, MockTokenStrategy, TokenEnvironment, TokenStrategy};
use serial_test::serial;
use std::env;
use url::Url;

/// Test helper to setup mock environment
async fn setup_mock_test() {
    // Force cleanup any existing token state
    let _ = ApiClient::force_cleanup_token_files().await;

    // Set mock environment variables
    env::set_var("AMP_USERNAME", "mock_user");
    env::set_var("AMP_PASSWORD", "mock_pass");
    env::remove_var("AMP_TESTS"); // Ensure not in live mode
    env::remove_var("AMP_TOKEN_PERSISTENCE"); // Disable persistence

    tracing::debug!("Mock test environment setup complete");
}

/// Test helper to cleanup mock test environment
async fn cleanup_mock_test() {
    // Cleanup any token files that might have been created
    let _ = ApiClient::force_cleanup_token_files().await;

    // Restore environment from .env file
    dotenvy::from_filename_override(".env").ok();

    tracing::debug!("Mock test environment cleanup complete");
}

/// Test helper to setup live test environment
async fn setup_live_test() {
    // Load real credentials from .env
    dotenvy::from_filename_override(".env").ok();

    // Ensure live test mode is set
    env::set_var("AMP_TESTS", "live");
    env::set_var("AMP_TOKEN_PERSISTENCE", "true");

    tracing::debug!("Live test environment setup complete");
}

#[tokio::test]
#[serial]
async fn test_api_client_with_mock_strategy() {
    setup_mock_test().await;

    let base_url = Url::parse("http://localhost:8080").unwrap();
    let mock_token = "test_mock_token_123".to_string();

    // Create ApiClient with explicit mock token
    let client = ApiClient::with_mock_token(base_url.clone(), mock_token.clone())
        .await
        .expect("Failed to create ApiClient with mock token");

    // Verify mock strategy is used
    assert_eq!(client.get_strategy_type(), "mock");
    assert!(!client.should_persist_tokens());

    // Verify token works without network
    let token = client.get_token().await.expect("Failed to get mock token");
    assert_eq!(token, mock_token);

    // Verify token can be retrieved multiple times
    let token2 = client
        .get_token()
        .await
        .expect("Failed to get mock token again");
    assert_eq!(token2, mock_token);

    // Verify clear_token works (should be no-op for mock)
    client
        .clear_token()
        .await
        .expect("Failed to clear mock token");

    // Token should still be available after clear (mock behavior)
    let token3 = client
        .get_token()
        .await
        .expect("Failed to get mock token after clear");
    assert_eq!(token3, mock_token);

    cleanup_mock_test().await;
}

#[tokio::test]
#[serial]
async fn test_api_client_automatic_strategy_selection_mock() {
    setup_mock_test().await;

    // Set base URL to mock environment
    env::set_var("AMP_API_BASE_URL", "http://localhost:8080");

    // Create ApiClient with automatic strategy selection
    let client = ApiClient::new().await.expect("Failed to create ApiClient");

    // Should automatically select mock strategy due to mock credentials
    assert_eq!(client.get_strategy_type(), "mock");
    assert!(!client.should_persist_tokens());

    cleanup_mock_test().await;
}

#[tokio::test]
#[serial]
async fn test_api_client_with_live_strategy() {
    setup_live_test().await;

    // Skip if no real credentials are available
    if env::var("AMP_USERNAME").unwrap_or_default().is_empty()
        || env::var("AMP_PASSWORD").unwrap_or_default().is_empty()
    {
        println!("Skipping live strategy test - no credentials available");
        return;
    }

    // Create ApiClient with automatic strategy selection
    let client = ApiClient::new().await.expect("Failed to create ApiClient");

    // Should automatically select live strategy
    assert_eq!(client.get_strategy_type(), "live");
    assert!(client.should_persist_tokens());

    // Note: We don't test actual token retrieval here to avoid network dependencies
    // That would be covered by live integration tests
}

#[tokio::test]
#[serial]
async fn test_token_environment_detection() {
    // Test mock environment detection
    env::set_var("AMP_USERNAME", "mock_user");
    env::set_var("AMP_PASSWORD", "mock_pass");
    env::remove_var("AMP_TESTS");

    let env_mock = TokenEnvironment::detect();
    assert_eq!(env_mock, TokenEnvironment::Mock);
    assert!(env_mock.is_mock());
    assert!(!env_mock.is_live());
    assert!(!env_mock.should_persist_tokens());

    // Test live environment detection
    env::set_var("AMP_USERNAME", "real_user");
    env::set_var("AMP_PASSWORD", "real_pass");
    env::set_var("AMP_TESTS", "live");

    let env_live = TokenEnvironment::detect();
    assert_eq!(env_live, TokenEnvironment::Live);
    assert!(!env_live.is_mock());
    assert!(env_live.is_live());
    assert!(env_live.should_persist_tokens());

    // Test mock URL detection
    env::set_var("AMP_USERNAME", "user");
    env::set_var("AMP_PASSWORD", "pass");
    env::set_var("AMP_API_BASE_URL", "http://localhost:8080");
    env::remove_var("AMP_TESTS");

    let env_mock_url = TokenEnvironment::detect();
    assert_eq!(env_mock_url, TokenEnvironment::Mock);

    // Cleanup
    env::remove_var("AMP_USERNAME");
    env::remove_var("AMP_PASSWORD");
    env::remove_var("AMP_TESTS");
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
#[serial]
async fn test_strategy_creation() {
    // Test mock strategy creation
    let mock_strategy = TokenEnvironment::Mock
        .create_strategy(Some("test_token".to_string()))
        .await
        .expect("Failed to create mock strategy");

    assert_eq!(mock_strategy.strategy_type(), "mock");
    assert!(!mock_strategy.should_persist());

    let token = mock_strategy
        .get_token()
        .await
        .expect("Failed to get token from mock strategy");
    assert_eq!(token, "test_token");

    // Test auto strategy creation with mock environment
    env::set_var("AMP_USERNAME", "mock_user");
    env::set_var("AMP_PASSWORD", "mock_pass");
    env::remove_var("AMP_TESTS");

    let auto_strategy = TokenEnvironment::create_auto_strategy(Some("auto_test_token".to_string()))
        .await
        .expect("Failed to create auto strategy");

    assert_eq!(auto_strategy.strategy_type(), "mock");

    let auto_token = auto_strategy
        .get_token()
        .await
        .expect("Failed to get token from auto strategy");
    assert_eq!(auto_token, "auto_test_token");

    // Cleanup
    env::remove_var("AMP_USERNAME");
    env::remove_var("AMP_PASSWORD");
}

#[tokio::test]
#[serial]
async fn test_api_client_strategy_methods() {
    setup_mock_test().await;

    let base_url = Url::parse("http://localhost:8080").unwrap();
    let client = ApiClient::with_mock_token(base_url, "test_token".to_string())
        .await
        .expect("Failed to create ApiClient");

    // Test strategy inspection methods
    assert_eq!(client.get_strategy_type(), "mock");
    assert!(!client.should_persist_tokens());

    // Test force refresh (should work with mock strategy)
    let refreshed_token = client
        .force_refresh()
        .await
        .expect("Failed to force refresh");
    assert_eq!(refreshed_token, "test_token");

    cleanup_mock_test().await;
}

#[tokio::test]
async fn test_api_client_force_cleanup() {
    // Test that force cleanup works without errors
    ApiClient::force_cleanup_token_files()
        .await
        .expect("Failed to force cleanup token files");

    // Should be idempotent
    ApiClient::force_cleanup_token_files()
        .await
        .expect("Failed to force cleanup token files again");
}

#[tokio::test]
async fn test_mock_strategy_isolation() {
    // Create multiple mock strategies with different tokens
    let strategy1 = MockTokenStrategy::new("token1".to_string());
    let strategy2 = MockTokenStrategy::new("token2".to_string());

    // Verify they are isolated
    let token1 = strategy1.get_token().await.expect("Failed to get token1");
    let token2 = strategy2.get_token().await.expect("Failed to get token2");

    assert_eq!(token1, "token1");
    assert_eq!(token2, "token2");
    assert_ne!(token1, token2);

    // Verify clear operations don't affect each other
    strategy1
        .clear_token()
        .await
        .expect("Failed to clear token1");

    let token1_after_clear = strategy1
        .get_token()
        .await
        .expect("Failed to get token1 after clear");
    let token2_after_clear = strategy2
        .get_token()
        .await
        .expect("Failed to get token2 after clear");

    assert_eq!(token1_after_clear, "token1"); // Mock clear is no-op
    assert_eq!(token2_after_clear, "token2"); // Should be unaffected
}
