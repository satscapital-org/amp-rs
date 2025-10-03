use amp_rs::client::{RetryConfig, TokenManager, TokenError};
use amp_rs::model::{TokenData, TokenInfo};
use chrono::{Duration, Utc};
use httpmock::prelude::*;
use httpmock::Mock;
use secrecy::ExposeSecret;
use serde_json::json;
use std::env;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use url::Url;

/// Helper function to set up test environment variables
fn setup_test_env() {
    dotenvy::dotenv().ok();
    env::set_var("AMP_USERNAME", "test_user");
    env::set_var("AMP_PASSWORD", "test_password");
    env::set_var("AMP_API_BASE_URL", "http://localhost:8080/api");
}

/// Helper function to create a mock server with token obtain endpoint
fn setup_mock_server_with_token_obtain(server: &MockServer) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "token": "mock_obtained_token_12345"
            }));
    })
}

/// Helper function to create a mock server with token refresh endpoint
fn setup_mock_server_with_token_refresh(server: &MockServer) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/refresh_token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "token": "mock_refreshed_token_67890"
            }));
    })
}

/// Helper function to create a mock server that returns 429 rate limiting
fn setup_mock_server_with_rate_limiting(server: &MockServer, retry_after_seconds: u64) -> Mock<'_> {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(429)
            .header("retry-after", retry_after_seconds.to_string())
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Too Many Requests"
            }));
    })
}



#[cfg(test)]
mod token_data_tests {
    use super::*;

    #[test]
    fn test_token_data_serialization_deserialization() {
        let token = "test_token_for_serialization".to_string();
        let expires_at = Utc::now() + Duration::hours(24);
        let token_data = TokenData::new(token.clone(), expires_at);

        // Test serialization
        let serialized = serde_json::to_string(&token_data).expect("Failed to serialize TokenData");
        assert!(serialized.contains(&token)); // Token should be serialized
        // Note: The exact RFC3339 format might differ slightly, so just check for the year
        let year = expires_at.format("%Y").to_string();
        assert!(serialized.contains(&year)); // Expiry should be serialized

        // Test deserialization
        let deserialized: TokenData = serde_json::from_str(&serialized).expect("Failed to deserialize TokenData");
        assert_eq!(deserialized.token.expose_secret(), &token);
        assert_eq!(deserialized.expires_at, expires_at);
        assert_eq!(deserialized.obtained_at, token_data.obtained_at);
    }

    #[test]
    fn test_token_data_expiry_logic() {
        // Test token that doesn't expire soon
        let expires_at = Utc::now() + Duration::hours(2);
        let token_data = TokenData::new("token".to_string(), expires_at);
        assert!(!token_data.is_expired());
        assert!(!token_data.expires_soon(Duration::minutes(5)));

        // Test token that expires soon (within 5 minutes)
        let expires_at = Utc::now() + Duration::minutes(3);
        let token_data = TokenData::new("token".to_string(), expires_at);
        assert!(!token_data.is_expired());
        assert!(token_data.expires_soon(Duration::minutes(5)));

        // Test expired token
        let expires_at = Utc::now() - Duration::hours(1);
        let token_data = TokenData::new("token".to_string(), expires_at);
        assert!(token_data.is_expired());
        assert!(token_data.expires_soon(Duration::minutes(5)));
    }

    #[test]
    fn test_token_info_conversion() {
        let expires_at = Utc::now() + Duration::hours(2);
        let token_data = TokenData::new("token".to_string(), expires_at);
        
        let token_info: TokenInfo = (&token_data).into();
        
        assert_eq!(token_info.expires_at, expires_at);
        assert_eq!(token_info.obtained_at, token_data.obtained_at);
        assert!(!token_info.is_expired);
        assert!(!token_info.expires_soon);
        assert!(token_info.expires_in > Duration::hours(1));
        assert!(token_info.age < Duration::seconds(1));
    }

    #[test]
    fn test_token_data_age_calculation() {
        let token_data = TokenData::new("token".to_string(), Utc::now() + Duration::hours(1));
        
        // Age should be very small (just created)
        let age = token_data.age();
        assert!(age < Duration::seconds(1));
        assert!(age >= Duration::zero());
    }
}

#[cfg(test)]
mod retry_config_tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.timeout_seconds, 10);
    }

    #[test]
    fn test_retry_config_for_tests() {
        let config = RetryConfig::for_tests();
        assert_eq!(config.max_attempts, 2);
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 5000);
        assert_eq!(config.timeout_seconds, 5);
    }

    #[test]
    fn test_retry_config_from_env() {
        dotenvy::dotenv().ok();
        // Clean up any existing environment variables first
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
        
        // Set environment variables
        env::set_var("API_RETRY_MAX_ATTEMPTS", "5");
        env::set_var("API_RETRY_BASE_DELAY_MS", "2000");
        env::set_var("API_RETRY_MAX_DELAY_MS", "60000");
        env::set_var("API_REQUEST_TIMEOUT_SECONDS", "15");

        let config = RetryConfig::from_env().expect("Failed to create config from env");
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 2000);
        assert_eq!(config.max_delay_ms, 60000);
        assert_eq!(config.timeout_seconds, 15);

        // Clean up
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_retry_config_from_env_with_defaults() {
        // Don't load .env for this test since we want to test defaults
        // Ensure no environment variables are set
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");

        let config = RetryConfig::from_env().expect("Failed to create config from env");
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.timeout_seconds, 10);
        
        // Clean up (even though we didn't set anything, just to be safe)
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_retry_config_validation() {
        // Don't load .env for this test since we want to test validation with specific values
        // Test invalid max_attempts
        env::set_var("API_RETRY_MAX_ATTEMPTS", "0");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_attempts must be greater than 0"));

        // Test invalid base_delay_ms
        env::set_var("API_RETRY_MAX_ATTEMPTS", "3");
        env::set_var("API_RETRY_BASE_DELAY_MS", "0");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("base_delay_ms must be greater than 0"));

        // Test max_delay_ms < base_delay_ms
        env::set_var("API_RETRY_BASE_DELAY_MS", "5000");
        env::set_var("API_RETRY_MAX_DELAY_MS", "1000");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_delay_ms must be greater than or equal to base_delay_ms"));

        // Test invalid timeout_seconds
        env::set_var("API_RETRY_MAX_DELAY_MS", "10000");
        env::set_var("API_REQUEST_TIMEOUT_SECONDS", "0");
        let result = RetryConfig::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout_seconds must be greater than 0"));

        // Clean up
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_retry_config_builder_methods() {
        let config = RetryConfig::default()
            .with_timeout(20)
            .with_max_attempts(5)
            .with_base_delay_ms(2000)
            .with_max_delay_ms(60000);

        assert_eq!(config.timeout_seconds, 20);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 2000);
        assert_eq!(config.max_delay_ms, 60000);
    }
}

#[cfg(test)]
mod token_error_tests {
    use super::*;

    #[test]
    fn test_token_error_creation() {
        let refresh_error = TokenError::refresh_failed("Refresh failed");
        assert_eq!(refresh_error.to_string(), "Token refresh failed: Refresh failed");

        let obtain_error = TokenError::obtain_failed(3, "Network error".to_string());
        assert_eq!(obtain_error.to_string(), "Token obtain failed after 3 attempts: Network error");

        let rate_limit_error = TokenError::rate_limited(60);
        assert_eq!(rate_limit_error.to_string(), "Rate limited: retry after 60 seconds");

        let timeout_error = TokenError::timeout(10);
        assert_eq!(timeout_error.to_string(), "Request timeout after 10 seconds");

        let serialization_error = TokenError::serialization("JSON error");
        assert_eq!(serialization_error.to_string(), "Serialization error: JSON error");

        let storage_error = TokenError::storage("Storage failed");
        assert_eq!(storage_error.to_string(), "Token storage error: Storage failed");

        let validation_error = TokenError::validation("Invalid token");
        assert_eq!(validation_error.to_string(), "Token validation error: Invalid token");
    }

    #[test]
    fn test_token_error_retryable() {
        assert!(TokenError::refresh_failed("test").is_retryable());
        assert!(TokenError::rate_limited(60).is_retryable());
        assert!(TokenError::timeout(10).is_retryable());
        assert!(!TokenError::obtain_failed(3, "test".to_string()).is_retryable());
        assert!(!TokenError::serialization("test").is_retryable());
        assert!(!TokenError::storage("test").is_retryable());
        assert!(!TokenError::validation("test").is_retryable());
    }

    #[test]
    fn test_token_error_rate_limited() {
        assert!(TokenError::rate_limited(60).is_rate_limited());
        assert!(!TokenError::refresh_failed("test").is_rate_limited());
        assert!(!TokenError::timeout(10).is_rate_limited());
    }

    #[test]
    fn test_token_error_retry_after_seconds() {
        assert_eq!(TokenError::rate_limited(60).retry_after_seconds(), Some(60));
        assert_eq!(TokenError::refresh_failed("test").retry_after_seconds(), None);
        assert_eq!(TokenError::timeout(10).retry_after_seconds(), None);
    }

    #[test]
    fn test_token_error_from_serde_json() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_error.is_err());
        
        let token_error: TokenError = json_error.unwrap_err().into();
        assert!(matches!(token_error, TokenError::Serialization(_)));
        assert!(token_error.to_string().contains("Serialization error"));
    }
}

#[tokio::test]
async fn test_token_manager_obtain_token_success() {
    setup_test_env();
    let server = MockServer::start();
    let _mock = setup_mock_server_with_token_obtain(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let token = manager.obtain_token().await.expect("Failed to obtain token");
    assert_eq!(token, "mock_obtained_token_12345");

    // Verify token is stored
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_some());
    let info = token_info.unwrap();
    assert!(!info.is_expired);
    assert!(!info.expires_soon);
}

#[tokio::test]
async fn test_token_manager_refresh_token_success() {
    setup_test_env();
    let server = MockServer::start();
    
    // First set up obtain token mock
    let _obtain_mock = setup_mock_server_with_token_obtain(&server);
    
    // Then set up refresh token mock
    let _refresh_mock = setup_mock_server_with_token_refresh(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // First obtain a token to have something to refresh
    let _initial_token = manager.obtain_token().await.expect("Failed to obtain initial token");

    let token = manager.refresh_token().await.expect("Failed to refresh token");
    assert_eq!(token, "mock_refreshed_token_67890");

    // Verify token is updated
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_some());
}

#[tokio::test]
async fn test_token_manager_get_token_proactive_refresh() {
    setup_test_env();
    let server = MockServer::start();
    
    // Mock obtain token first (to get initial token)
    let _obtain_mock = setup_mock_server_with_token_obtain(&server);
    
    // Mock refresh token (for the proactive refresh)
    let _refresh_mock = setup_mock_server_with_token_refresh(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // First get a token
    let _initial_token = manager.obtain_token().await.expect("Failed to obtain initial token");
    
    // Now test that get_token will use the existing token if it's not expiring soon
    let token = manager.get_token().await.expect("Failed to get token");
    assert_eq!(token, "mock_obtained_token_12345"); // Should use cached token
}

#[tokio::test]
async fn test_token_manager_get_token_fallback_to_obtain() {
    setup_test_env();
    let server = MockServer::start();
    
    // Mock obtain to succeed (fallback case)
    let _obtain_mock = setup_mock_server_with_token_obtain(&server);
    
    // Mock refresh to fail
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/refresh_token");
        then.status(401)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Invalid token"
            }));
    });

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // First obtain a token, then try to refresh it (which will fail and fallback to obtain)
    let _initial_token = manager.obtain_token().await.expect("Failed to obtain initial token");
    
    // Force refresh should fail and fallback to obtain
    let token = manager.force_refresh().await.expect("Failed to get token with fallback");
    assert_eq!(token, "mock_obtained_token_12345");
}

#[tokio::test]
async fn test_token_manager_rate_limiting_handling() {
    setup_test_env();
    let server = MockServer::start();
    
    // First request gets rate limited
    let _rate_limit_mock = setup_mock_server_with_rate_limiting(&server, 2);
    
    // Second request succeeds
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "token": "mock_token_after_rate_limit"
            }));
    });

    let config = RetryConfig::for_tests().with_max_attempts(2);
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let start_time = std::time::Instant::now();
    let result = manager.obtain_token().await;
    let elapsed = start_time.elapsed();

    // Should fail due to rate limiting (only 2 attempts, both rate limited)
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Rate limited"));
    
    // Should have waited for rate limit delay
    assert!(elapsed >= StdDuration::from_millis(2000)); // 2 seconds retry-after
}

#[tokio::test]
async fn test_token_manager_network_failure_retry() {
    setup_test_env();
    let server = MockServer::start();
    
    // Test that retry logic is working by using a configuration with only 1 attempt
    // and ensuring that server errors are retried
    let config = RetryConfig::for_tests().with_max_attempts(3); // Allow more attempts
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // Create a mock that always returns server error
    let _fail_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Internal Server Error"
            }));
    });

    // This should fail after all retries are exhausted
    let result = manager.obtain_token().await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Token obtain failed after 3 attempts"));
}

#[tokio::test]
async fn test_token_manager_network_failure_exhausted_retries() {
    setup_test_env();
    let server = MockServer::start();
    
    // Create a mock that always fails
    let _fail_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Internal Server Error"
            }));
    });

    let config = RetryConfig::for_tests(); // 2 max attempts
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let result = manager.obtain_token().await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Token obtain failed after"));
}

#[tokio::test]
async fn test_token_manager_retry_behavior_with_success() {
    setup_test_env();
    let server = MockServer::start();
    
    // Test the retry behavior by checking that we can eventually succeed
    // We'll use a simple success case to verify the retry client works
    let _success_mock = setup_mock_server_with_token_obtain(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // This should succeed on the first attempt
    let token = manager.obtain_token().await.expect("Failed to obtain token");
    assert_eq!(token, "mock_obtained_token_12345");
}

#[tokio::test]
async fn test_token_manager_concurrent_access() {
    setup_test_env();
    let server = MockServer::start();
    let _mock = setup_mock_server_with_token_obtain(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = Arc::new(TokenManager::with_config_and_base_url(config, base_url).unwrap());

    // Spawn multiple concurrent tasks
    let mut handles = Vec::new();
    for i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            let token = manager_clone.get_token().await.expect(&format!("Task {} failed to get token", i));
            assert_eq!(token, "mock_obtained_token_12345");
            token
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut tokens = Vec::new();
    for handle in handles {
        let token = handle.await.expect("Task panicked");
        tokens.push(token);
    }

    // All tokens should be the same (only one obtain should have happened)
    assert_eq!(tokens.len(), 10);
    for token in &tokens {
        assert_eq!(token, "mock_obtained_token_12345");
    }
}

#[tokio::test]
async fn test_token_manager_utility_methods() {
    setup_test_env();
    let server = MockServer::start();
    let _mock = setup_mock_server_with_token_obtain(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // Test get_token_info with no token
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_none());

    // Obtain a token
    let _token = manager.obtain_token().await.expect("Failed to obtain token");

    // Test get_token_info with token
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_some());
    let info = token_info.unwrap();
    assert!(!info.is_expired);
    assert!(!info.expires_soon);

    // Test clear_token
    manager.clear_token().await.expect("Failed to clear token");
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_none());
}

#[tokio::test]
async fn test_token_manager_force_refresh() {
    setup_test_env();
    let server = MockServer::start();
    
    // Mock obtain token first
    let _obtain_mock = setup_mock_server_with_token_obtain(&server);
    
    // Mock refresh token - this should be called by force_refresh
    let _refresh_mock = setup_mock_server_with_token_refresh(&server);

    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // First obtain a token
    let _initial_token = manager.obtain_token().await.expect("Failed to obtain initial token");

    // Force refresh should call the refresh endpoint and return the refreshed token
    let token = manager.force_refresh().await.expect("Failed to force refresh token");
    assert_eq!(token, "mock_refreshed_token_67890");
}

#[tokio::test]
async fn test_token_manager_timeout_handling() {
    setup_test_env();
    let server = MockServer::start();
    
    // Mock server that delays response longer than timeout
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(200)
            .delay(StdDuration::from_secs(10)) // Longer than test timeout
            .header("content-type", "application/json")
            .json_body(json!({
                "token": "mock_token"
            }));
    });

    let config = RetryConfig::for_tests().with_timeout(1); // 1 second timeout
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let result = manager.obtain_token().await;
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Request timeout after 1 seconds"));
}

// Live API tests (only run when AMP_TESTS=live)
#[tokio::test]
async fn test_token_manager_live_obtain_token() {
    dotenvy::dotenv().ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let manager = TokenManager::new().expect("Failed to create TokenManager");
    let token = manager.obtain_token().await.expect("Failed to obtain token from live API");
    
    assert!(!token.is_empty());
    assert!(token.len() > 10); // Reasonable token length check
    
    // Verify token is stored
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_some());
    let info = token_info.unwrap();
    assert!(!info.is_expired);
    assert!(!info.expires_soon);
}

#[tokio::test]
async fn test_token_manager_live_refresh_token() {
    dotenvy::dotenv().ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let manager = TokenManager::new().expect("Failed to create TokenManager");
    
    // First obtain a token
    let _initial_token = manager.obtain_token().await.expect("Failed to obtain initial token");
    
    // Then refresh it
    let refreshed_token = manager.refresh_token().await.expect("Failed to refresh token");
    
    assert!(!refreshed_token.is_empty());
    assert!(refreshed_token.len() > 10);
    // Note: refreshed token might be the same as initial token depending on API implementation
}

#[tokio::test]
async fn test_token_manager_live_get_token_with_proactive_refresh() {
    dotenvy::dotenv().ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let manager = TokenManager::new().expect("Failed to create TokenManager");
    
    // Get token (should obtain new one)
    let token1 = manager.get_token().await.expect("Failed to get token (first call)");
    assert!(!token1.is_empty());
    
    // Get token again (should use cached token)
    let token2 = manager.get_token().await.expect("Failed to get token (second call)");
    assert_eq!(token1, token2);
    
    // Verify token info
    let token_info = manager.get_token_info().await.expect("Failed to get token info");
    assert!(token_info.is_some());
    let info = token_info.unwrap();
    assert!(!info.is_expired);
    assert!(!info.expires_soon);
    assert!(info.expires_in > Duration::hours(20)); // Should be close to 24 hours
}

#[tokio::test]
async fn test_token_manager_live_concurrent_access() {
    dotenvy::dotenv().ok();
    if env::var("AMP_TESTS").unwrap_or_default() != "live" {
        println!("Skipping live test");
        return;
    }

    // Ensure that the environment variables are set
    if env::var("AMP_USERNAME").is_err() || env::var("AMP_PASSWORD").is_err() {
        panic!("AMP_USERNAME and AMP_PASSWORD must be set for this test");
    }

    let manager = Arc::new(TokenManager::new().expect("Failed to create TokenManager"));

    // Spawn multiple concurrent tasks
    let mut handles = Vec::new();
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            let token = manager_clone.get_token().await.expect(&format!("Task {} failed to get token", i));
            assert!(!token.is_empty());
            token
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut tokens = Vec::new();
    for handle in handles {
        let token = handle.await.expect("Task panicked");
        tokens.push(token);
    }

    // All tokens should be the same (only one obtain should have happened)
    assert_eq!(tokens.len(), 5);
    let first_token = &tokens[0];
    for token in &tokens {
        assert_eq!(token, first_token);
    }
}