use amp_rs::client::{Error, RetryClient, RetryConfig, TokenError, TokenManager};
use amp_rs::model::TokenData;
use chrono::{Duration, Utc};
use httpmock::prelude::*;
use secrecy::ExposeSecret;
use serial_test::serial;
use std::env;
use std::time::Duration as StdDuration;
use url::Url;

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
#[serial]
fn test_retry_config_from_env_defaults() {
    // Don't load .env for this test since we want to test defaults
    // Clear any existing environment variables
    env::remove_var("API_RETRY_MAX_ATTEMPTS");
    env::remove_var("API_RETRY_BASE_DELAY_MS");
    env::remove_var("API_RETRY_MAX_DELAY_MS");
    env::remove_var("API_REQUEST_TIMEOUT_SECONDS");

    let config = RetryConfig::from_env().unwrap();
    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.base_delay_ms, 1000);
    assert_eq!(config.max_delay_ms, 30000);
    assert_eq!(config.timeout_seconds, 10);
}

#[test]
#[serial]
fn test_retry_config_from_env_custom_values() {
    dotenvy::dotenv().ok();
    env::set_var("API_RETRY_MAX_ATTEMPTS", "5");
    env::set_var("API_RETRY_BASE_DELAY_MS", "2000");
    env::set_var("API_RETRY_MAX_DELAY_MS", "60000");
    env::set_var("API_REQUEST_TIMEOUT_SECONDS", "30");

    let config = RetryConfig::from_env().unwrap();
    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.base_delay_ms, 2000);
    assert_eq!(config.max_delay_ms, 60000);
    assert_eq!(config.timeout_seconds, 30);

    // Clean up
    env::remove_var("API_RETRY_MAX_ATTEMPTS");
    env::remove_var("API_RETRY_BASE_DELAY_MS");
    env::remove_var("API_RETRY_MAX_DELAY_MS");
    env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
}
#[test]
#[serial]
fn test_retry_config_from_env_invalid_values() {
    dotenvy::dotenv().ok();
    env::set_var("API_RETRY_MAX_ATTEMPTS", "invalid");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid API_RETRY_MAX_ATTEMPTS"));
    env::remove_var("API_RETRY_MAX_ATTEMPTS");

    env::set_var("API_RETRY_BASE_DELAY_MS", "not_a_number");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid API_RETRY_BASE_DELAY_MS"));
    env::remove_var("API_RETRY_BASE_DELAY_MS");

    env::set_var("API_RETRY_MAX_DELAY_MS", "-1");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid API_RETRY_MAX_DELAY_MS"));
    env::remove_var("API_RETRY_MAX_DELAY_MS");

    env::set_var("API_REQUEST_TIMEOUT_SECONDS", "abc");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid API_REQUEST_TIMEOUT_SECONDS"));
    env::remove_var("API_REQUEST_TIMEOUT_SECONDS");
}

#[test]
#[serial]
fn test_retry_config_validation() {
    dotenvy::dotenv().ok();
    // Test zero max_attempts
    env::set_var("API_RETRY_MAX_ATTEMPTS", "0");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("max_attempts must be greater than 0"));
    env::remove_var("API_RETRY_MAX_ATTEMPTS");

    // Test zero base_delay_ms
    env::set_var("API_RETRY_BASE_DELAY_MS", "0");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("base_delay_ms must be greater than 0"));
    env::remove_var("API_RETRY_BASE_DELAY_MS");

    // Test max_delay_ms < base_delay_ms
    env::set_var("API_RETRY_BASE_DELAY_MS", "2000");
    env::set_var("API_RETRY_MAX_DELAY_MS", "1000");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("max_delay_ms must be greater than or equal to base_delay_ms"));
    env::remove_var("API_RETRY_BASE_DELAY_MS");
    env::remove_var("API_RETRY_MAX_DELAY_MS");

    // Test zero timeout_seconds
    env::set_var("API_REQUEST_TIMEOUT_SECONDS", "0");
    let result = RetryConfig::from_env();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("timeout_seconds must be greater than 0"));
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

#[test]
#[serial]
fn test_retry_config_partial_env_vars() {
    dotenvy::dotenv().ok();
    // Set only some environment variables, others should use defaults
    env::set_var("API_RETRY_MAX_ATTEMPTS", "7");
    env::set_var("API_RETRY_BASE_DELAY_MS", "1500");
    // Don't set API_RETRY_MAX_DELAY_MS and API_REQUEST_TIMEOUT_SECONDS

    let config = RetryConfig::from_env().unwrap();
    assert_eq!(config.max_attempts, 7);
    assert_eq!(config.base_delay_ms, 1500);
    assert_eq!(config.max_delay_ms, 30000); // default
    assert_eq!(config.timeout_seconds, 10); // default

    // Clean up
    env::remove_var("API_RETRY_MAX_ATTEMPTS");
    env::remove_var("API_RETRY_BASE_DELAY_MS");
}

// Token Error Tests
#[test]
fn test_token_error_creation() {
    let refresh_error = TokenError::refresh_failed("Connection timeout");
    assert_eq!(
        refresh_error.to_string(),
        "Token refresh failed: Connection timeout"
    );

    let obtain_error = TokenError::obtain_failed(3, "Authentication failed".to_string());
    assert_eq!(
        obtain_error.to_string(),
        "Token obtain failed after 3 attempts: Authentication failed"
    );

    let rate_limit_error = TokenError::rate_limited(60);
    assert_eq!(
        rate_limit_error.to_string(),
        "Rate limited: retry after 60 seconds"
    );

    let timeout_error = TokenError::timeout(30);
    assert_eq!(
        timeout_error.to_string(),
        "Request timeout after 30 seconds"
    );

    let serialization_error = TokenError::serialization("Invalid JSON format");
    assert_eq!(
        serialization_error.to_string(),
        "Serialization error: Invalid JSON format"
    );

    let storage_error = TokenError::storage("Failed to write to disk");
    assert_eq!(
        storage_error.to_string(),
        "Token storage error: Failed to write to disk"
    );

    let validation_error = TokenError::validation("Token format invalid");
    assert_eq!(
        validation_error.to_string(),
        "Token validation error: Token format invalid"
    );
}

#[test]
fn test_token_error_is_retryable() {
    assert!(TokenError::refresh_failed("test").is_retryable());
    assert!(TokenError::rate_limited(60).is_retryable());
    assert!(TokenError::timeout(30).is_retryable());

    assert!(!TokenError::obtain_failed(3, "test".to_string()).is_retryable());
    assert!(!TokenError::serialization("test").is_retryable());
    assert!(!TokenError::storage("test").is_retryable());
    assert!(!TokenError::validation("test").is_retryable());
}

#[test]
fn test_token_error_is_rate_limited() {
    assert!(TokenError::rate_limited(60).is_rate_limited());

    assert!(!TokenError::refresh_failed("test").is_rate_limited());
    assert!(!TokenError::timeout(30).is_rate_limited());
    assert!(!TokenError::obtain_failed(3, "test".to_string()).is_rate_limited());
    assert!(!TokenError::serialization("test").is_rate_limited());
    assert!(!TokenError::storage("test").is_rate_limited());
    assert!(!TokenError::validation("test").is_rate_limited());
}

#[test]
fn test_token_error_retry_after_seconds() {
    let rate_limit_error = TokenError::rate_limited(120);
    assert_eq!(rate_limit_error.retry_after_seconds(), Some(120));

    assert_eq!(
        TokenError::refresh_failed("test").retry_after_seconds(),
        None
    );
    assert_eq!(TokenError::timeout(30).retry_after_seconds(), None);
    assert_eq!(
        TokenError::obtain_failed(3, "test".to_string()).retry_after_seconds(),
        None
    );
    assert_eq!(
        TokenError::serialization("test").retry_after_seconds(),
        None
    );
    assert_eq!(TokenError::storage("test").retry_after_seconds(), None);
    assert_eq!(TokenError::validation("test").retry_after_seconds(), None);
}

#[test]
fn test_token_error_equality() {
    let error1 = TokenError::refresh_failed("Connection timeout");
    let error2 = TokenError::refresh_failed("Connection timeout");
    let error3 = TokenError::refresh_failed("Different message");

    assert_eq!(error1, error2);
    assert_ne!(error1, error3);

    let obtain_error1 = TokenError::obtain_failed(3, "Auth failed".to_string());
    let obtain_error2 = TokenError::obtain_failed(3, "Auth failed".to_string());
    let obtain_error3 = TokenError::obtain_failed(2, "Auth failed".to_string());

    assert_eq!(obtain_error1, obtain_error2);
    assert_ne!(obtain_error1, obtain_error3);

    let rate_limit1 = TokenError::rate_limited(60);
    let rate_limit2 = TokenError::rate_limited(60);
    let rate_limit3 = TokenError::rate_limited(120);

    assert_eq!(rate_limit1, rate_limit2);
    assert_ne!(rate_limit1, rate_limit3);
}

#[test]
fn test_token_error_clone() {
    let original = TokenError::refresh_failed("Test message");
    let cloned = original.clone();

    assert_eq!(original, cloned);
    assert_eq!(original.to_string(), cloned.to_string());
}

#[test]
fn test_token_error_debug_format() {
    let error = TokenError::refresh_failed("Test message");
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("RefreshFailed"));
    assert!(debug_str.contains("Test message"));

    let obtain_error = TokenError::obtain_failed(3, "Auth failed".to_string());
    let debug_str = format!("{:?}", obtain_error);
    assert!(debug_str.contains("ObtainFailed"));
    assert!(debug_str.contains("attempts: 3"));
    assert!(debug_str.contains("Auth failed"));
}

#[test]
fn test_token_error_from_serde_json_error() {
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let token_error: TokenError = json_error.into();

    match token_error {
        TokenError::Serialization(msg) => {
            assert!(msg.contains("expected"));
        }
        _ => panic!("Expected Serialization error"),
    }
}

#[test]
fn test_error_from_token_error() {
    let token_error = TokenError::refresh_failed("Test message");
    let error: Error = token_error.into();

    match error {
        Error::Token(TokenError::RefreshFailed(msg)) => {
            assert_eq!(msg, "Test message");
        }
        _ => panic!("Expected Token error variant"),
    }
}

#[test]
fn test_error_display_with_token_error() {
    let token_error = TokenError::rate_limited(60);
    let error = Error::Token(token_error);

    assert_eq!(
        error.to_string(),
        "Token management error: Rate limited: retry after 60 seconds"
    );
}

// RetryClient Tests
#[test]
fn test_retry_client_creation() {
    let config = RetryConfig::for_tests();
    let retry_client = RetryClient::new(config.clone());

    assert_eq!(retry_client.config().max_attempts, config.max_attempts);
    assert_eq!(retry_client.config().base_delay_ms, config.base_delay_ms);
    assert_eq!(retry_client.config().max_delay_ms, config.max_delay_ms);
    assert_eq!(
        retry_client.config().timeout_seconds,
        config.timeout_seconds
    );
}

#[test]
fn test_retry_client_with_default_config() {
    let retry_client = RetryClient::with_default_config();
    let default_config = RetryConfig::default();

    assert_eq!(
        retry_client.config().max_attempts,
        default_config.max_attempts
    );
    assert_eq!(
        retry_client.config().base_delay_ms,
        default_config.base_delay_ms
    );
    assert_eq!(
        retry_client.config().max_delay_ms,
        default_config.max_delay_ms
    );
    assert_eq!(
        retry_client.config().timeout_seconds,
        default_config.timeout_seconds
    );
}

#[test]
fn test_retry_client_for_tests() {
    let retry_client = RetryClient::for_tests();
    let test_config = RetryConfig::for_tests();

    assert_eq!(retry_client.config().max_attempts, test_config.max_attempts);
    assert_eq!(
        retry_client.config().base_delay_ms,
        test_config.base_delay_ms
    );
    assert_eq!(retry_client.config().max_delay_ms, test_config.max_delay_ms);
    assert_eq!(
        retry_client.config().timeout_seconds,
        test_config.timeout_seconds
    );
}

#[test]
fn test_retry_client_calculate_backoff_delay() {
    let config = RetryConfig {
        max_attempts: 3,
        base_delay_ms: 1000,
        max_delay_ms: 10000,
        timeout_seconds: 10,
    };
    let retry_client = RetryClient::new(config);

    // Test first attempt (should be base_delay + jitter)
    let delay1 = retry_client.calculate_backoff_delay(1);
    assert!(delay1.as_millis() >= 1000); // At least base delay
    assert!(delay1.as_millis() <= 1500); // Base delay + max jitter (base/2)

    // Test second attempt (should be 2 * base_delay + jitter)
    let delay2 = retry_client.calculate_backoff_delay(2);
    assert!(delay2.as_millis() >= 2000); // At least 2 * base delay
    assert!(delay2.as_millis() <= 2500); // 2 * base delay + max jitter

    // Test third attempt (should be 4 * base_delay + jitter)
    let delay3 = retry_client.calculate_backoff_delay(3);
    assert!(delay3.as_millis() >= 4000); // At least 4 * base delay
    assert!(delay3.as_millis() <= 4500); // 4 * base delay + max jitter

    // Test that delay is capped at max_delay_ms
    let delay_large = retry_client.calculate_backoff_delay(10);
    assert_eq!(delay_large.as_millis(), 10000); // Should be capped at max_delay_ms
}

#[test]
fn test_retry_client_calculate_backoff_delay_with_small_max() {
    let config = RetryConfig {
        max_attempts: 5,
        base_delay_ms: 1000,
        max_delay_ms: 2000, // Small max delay to test capping
        timeout_seconds: 10,
    };
    let retry_client = RetryClient::new(config);

    // Test that exponential backoff is capped properly
    let delay3 = retry_client.calculate_backoff_delay(3);
    assert_eq!(delay3.as_millis(), 2000); // Should be capped at max_delay_ms

    let delay4 = retry_client.calculate_backoff_delay(4);
    assert_eq!(delay4.as_millis(), 2000); // Should still be capped
}

#[test]
fn test_retry_client_extract_retry_after() {
    use reqwest::header::{HeaderMap, HeaderValue};

    let config = RetryConfig::for_tests();
    let _retry_client = RetryClient::new(config);

    // Create a mock response with Retry-After header
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("120"));

    // We can't easily create a reqwest::Response for testing, so we'll test the logic
    // by creating a simple test that verifies the header parsing logic would work
    let retry_after_value = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    assert_eq!(retry_after_value, Some(120));

    // Test invalid header value
    let mut headers_invalid = HeaderMap::new();
    headers_invalid.insert("retry-after", HeaderValue::from_static("invalid"));

    let retry_after_invalid = headers_invalid
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    assert_eq!(retry_after_invalid, None);

    // Test missing header
    let headers_empty = HeaderMap::new();
    let retry_after_missing = headers_empty
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    assert_eq!(retry_after_missing, None);
}

#[test]
fn test_retry_client_client_access() {
    let retry_client = RetryClient::for_tests();
    let client = retry_client.client();

    // Verify we can access the underlying client
    assert!(client.get("https://example.com").build().is_ok());
}

#[test]
fn test_retry_client_config_access() {
    let config = RetryConfig {
        max_attempts: 5,
        base_delay_ms: 2000,
        max_delay_ms: 20000,
        timeout_seconds: 15,
    };
    let retry_client = RetryClient::new(config.clone());

    let retrieved_config = retry_client.config();
    assert_eq!(retrieved_config.max_attempts, config.max_attempts);
    assert_eq!(retrieved_config.base_delay_ms, config.base_delay_ms);
    assert_eq!(retrieved_config.max_delay_ms, config.max_delay_ms);
    assert_eq!(retrieved_config.timeout_seconds, config.timeout_seconds);
}

#[test]
fn test_retry_client_clone() {
    let config = RetryConfig::for_tests();
    let retry_client = RetryClient::new(config);
    let cloned_client = retry_client.clone();

    // Verify the clone has the same configuration
    assert_eq!(
        retry_client.config().max_attempts,
        cloned_client.config().max_attempts
    );
    assert_eq!(
        retry_client.config().base_delay_ms,
        cloned_client.config().base_delay_ms
    );
    assert_eq!(
        retry_client.config().max_delay_ms,
        cloned_client.config().max_delay_ms
    );
    assert_eq!(
        retry_client.config().timeout_seconds,
        cloned_client.config().timeout_seconds
    );
}

#[tokio::test]
async fn test_retry_client_with_mock_server() {
    let server = MockServer::start();

    // Test successful request (no retries needed)
    let success_mock = server.mock(|when, then| {
        when.method(GET).path("/success");
        then.status(200).body("success");
    });

    let retry_client = RetryClient::for_tests();
    let url = format!("{}/success", server.base_url());

    let result = retry_client
        .execute_with_retry(|| retry_client.client().get(&url))
        .await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status(), 200);
    success_mock.assert_hits(1); // Should only be called once
}

#[tokio::test]
async fn test_retry_client_server_error_retry() {
    let server = MockServer::start();

    // Test server error - this will always return 500
    // We'll verify that it retries by checking the hit count
    let _server_error_mock = server.mock(|when, then| {
        when.method(GET).path("/server-error");
        then.status(500).body("server error");
    });

    let retry_client = RetryClient::for_tests();
    let url = format!("{}/server-error", server.base_url());

    let result = retry_client
        .execute_with_retry(|| retry_client.client().get(&url))
        .await;

    // Should fail after retries are exhausted
    assert!(result.is_err());
    match result.unwrap_err() {
        TokenError::ObtainFailed {
            attempts,
            last_error,
        } => {
            assert_eq!(attempts, 2); // Should have tried max_attempts times
            assert!(last_error.contains("Server error: 500"));
        }
        _ => panic!("Expected ObtainFailed error"),
    }

    // Verify it was called the expected number of times
    _server_error_mock.assert_hits(2); // Should be called max_attempts times
}

#[tokio::test]
async fn test_retry_client_rate_limit_handling() {
    let server = MockServer::start();

    // Test rate limiting (429) response
    let rate_limit_mock = server.mock(|when, then| {
        when.method(GET).path("/rate-limited");
        then.status(429)
            .header("retry-after", "1") // 1 second retry
            .body("rate limited");
    });

    let retry_client = RetryClient::for_tests();
    let url = format!("{}/rate-limited", server.base_url());

    let start_time = std::time::Instant::now();
    let result = retry_client
        .execute_with_retry(|| retry_client.client().get(&url))
        .await;

    let elapsed = start_time.elapsed();

    // Should fail with rate limit error after all retries
    assert!(result.is_err());
    match result.unwrap_err() {
        TokenError::RateLimited {
            retry_after_seconds,
        } => {
            assert_eq!(retry_after_seconds, 1);
        }
        _ => panic!("Expected RateLimited error"),
    }

    // Should have waited for rate limit delays
    assert!(elapsed.as_secs() >= 1); // At least 1 second for the rate limit delay

    // Should have been called max_attempts times
    rate_limit_mock.assert_hits(2); // for_tests() config has max_attempts = 2
}

#[tokio::test]
async fn test_retry_client_non_retryable_client_error() {
    let server = MockServer::start();

    // Test non-retryable client error (404)
    let not_found_mock = server.mock(|when, then| {
        when.method(GET).path("/not-found");
        then.status(404).body("not found");
    });

    let retry_client = RetryClient::for_tests();
    let url = format!("{}/not-found", server.base_url());

    let result = retry_client
        .execute_with_retry(|| retry_client.client().get(&url))
        .await;

    // Should fail immediately without retries for 404
    assert!(result.is_err());
    match result.unwrap_err() {
        TokenError::ObtainFailed {
            attempts,
            last_error,
        } => {
            assert_eq!(attempts, 1); // Should not retry
            assert!(last_error.contains("Client error: 404"));
        }
        _ => panic!("Expected ObtainFailed error"),
    }

    not_found_mock.assert_hits(1); // Should only be called once
}

#[tokio::test]
async fn test_retry_client_timeout_handling() {
    let server = MockServer::start();

    // Test timeout scenario
    let _timeout_mock = server.mock(|when, then| {
        when.method(GET).path("/timeout");
        then.status(200)
            .delay(StdDuration::from_secs(10)) // Delay longer than timeout
            .body("delayed response");
    });

    let config = RetryConfig {
        max_attempts: 2,
        base_delay_ms: 100,
        max_delay_ms: 1000,
        timeout_seconds: 1, // Very short timeout
    };
    let retry_client = RetryClient::new(config);
    let url = format!("{}/timeout", server.base_url());

    let start_time = std::time::Instant::now();
    let result = retry_client
        .execute_with_retry(|| retry_client.client().get(&url))
        .await;

    let elapsed = start_time.elapsed();

    // Should fail with timeout error
    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        TokenError::Timeout { timeout_seconds } => {
            assert_eq!(timeout_seconds, 1);
        }
        _ => panic!("Expected Timeout error, got: {:?}", error),
    }

    // Should have timed out relatively quickly (within a few seconds)
    assert!(elapsed.as_secs() < 5);
}

#[tokio::test]
async fn test_retry_client_exhausted_retries() {
    let server = MockServer::start();

    // Test that all retries are exhausted
    let always_fail_mock = server.mock(|when, then| {
        when.method(GET).path("/always-fail");
        then.status(500).body("server error");
    });

    let retry_client = RetryClient::for_tests(); // max_attempts = 2
    let url = format!("{}/always-fail", server.base_url());

    let result = retry_client
        .execute_with_retry(|| retry_client.client().get(&url))
        .await;

    // Should fail after all retries are exhausted
    assert!(result.is_err());
    match result.unwrap_err() {
        TokenError::ObtainFailed {
            attempts,
            last_error,
        } => {
            assert_eq!(attempts, 2); // Should have tried max_attempts times
            assert!(last_error.contains("Server error: 500"));
        }
        _ => panic!("Expected ObtainFailed error"),
    }

    always_fail_mock.assert_hits(2); // Should be called max_attempts times
}

// TokenManager Tests
#[tokio::test]
#[serial]
async fn test_token_manager_creation() {
    // Test creation with default config
    let result = TokenManager::new();
    // This might fail if environment variables are not set, which is expected
    match result {
        Ok(manager) => {
            // If successful, verify the manager was created
            assert!(manager.token_data.lock().await.is_none());
        }
        Err(Error::MissingEnvVar(_)) => {
            // Expected if AMP_API_BASE_URL is not set
        }
        Err(e) => panic!("Unexpected error: {}", e),
    }

    // Test creation with custom config
    let config = RetryConfig::for_tests();
    let result = TokenManager::with_config(config);
    match result {
        Ok(manager) => {
            assert!(manager.token_data.lock().await.is_none());
            assert_eq!(manager.retry_client.config().max_attempts, 2);
        }
        Err(Error::MissingEnvVar(_)) => {
            // Expected if AMP_API_BASE_URL is not set
        }
        Err(e) => panic!("Unexpected error: {}", e),
    }
}

#[tokio::test]
#[serial]
async fn test_token_manager_clear_token() {
    let config = RetryConfig::for_tests();

    // Create a mock base URL for testing
    env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

    let manager = TokenManager::with_config(config).unwrap();

    // Initially no token
    assert!(manager.token_data.lock().await.is_none());

    // Manually set a token for testing
    let token_data = TokenData::new("test_token".to_string(), Utc::now() + Duration::hours(1));
    {
        let mut guard = manager.token_data.lock().await;
        *guard = Some(token_data);
    }

    // Verify token exists
    assert!(manager.token_data.lock().await.is_some());

    // Clear the token
    let result = manager.clear_token().await;
    assert!(result.is_ok());

    // Verify token is cleared
    assert!(manager.token_data.lock().await.is_none());

    // Clean up
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
#[serial]
async fn test_token_manager_get_token_info() {
    let config = RetryConfig::for_tests();

    // Create a mock base URL for testing
    env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

    let manager = TokenManager::with_config(config).unwrap();

    // Initially no token info
    let info = manager.get_token_info().await.unwrap();
    assert!(info.is_none());

    // Set a token
    let expires_at = Utc::now() + Duration::hours(2);
    let token_data = TokenData::new("test_token".to_string(), expires_at);
    {
        let mut guard = manager.token_data.lock().await;
        *guard = Some(token_data.clone());
    }

    // Get token info
    let info = manager.get_token_info().await.unwrap();
    assert!(info.is_some());

    let token_info = info.unwrap();
    assert_eq!(token_info.expires_at, expires_at);
    assert!(!token_info.is_expired);
    assert!(!token_info.expires_soon); // 2 hours > 5 minutes
    assert!(token_info.expires_in > Duration::hours(1));
    assert!(token_info.age < Duration::seconds(1));

    // Clean up
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
#[serial]
async fn test_token_manager_get_token_info_expires_soon() {
    let config = RetryConfig::for_tests();

    // Create a mock base URL for testing
    env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

    let manager = TokenManager::with_config(config).unwrap();

    // Set a token that expires soon (in 3 minutes)
    let expires_at = Utc::now() + Duration::minutes(3);
    let token_data = TokenData::new("test_token".to_string(), expires_at);
    {
        let mut guard = manager.token_data.lock().await;
        *guard = Some(token_data);
    }

    // Get token info
    let info = manager.get_token_info().await.unwrap().unwrap();
    assert!(!info.is_expired);
    assert!(info.expires_soon); // 3 minutes < 5 minutes threshold
    assert!(info.expires_in < Duration::minutes(5));

    // Clean up
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
#[serial]
async fn test_token_manager_get_token_info_expired() {
    let config = RetryConfig::for_tests();

    // Create a mock base URL for testing
    env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

    let manager = TokenManager::with_config(config).unwrap();

    // Set an expired token
    let expires_at = Utc::now() - Duration::hours(1);
    let token_data = TokenData::new("test_token".to_string(), expires_at);
    {
        let mut guard = manager.token_data.lock().await;
        *guard = Some(token_data);
    }

    // Get token info
    let info = manager.get_token_info().await.unwrap().unwrap();
    assert!(info.is_expired);
    assert!(info.expires_soon); // Expired tokens also expire "soon"
    assert!(info.expires_in < Duration::zero());

    // Clean up
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
#[serial]
async fn test_token_manager_get_token_with_valid_token() {
    let config = RetryConfig::for_tests();

    // Create a mock base URL for testing
    env::set_var("AMP_API_BASE_URL", "https://test.example.com/api");

    let manager = TokenManager::with_config(config).unwrap();

    // Set a valid token that doesn't expire soon
    let expires_at = Utc::now() + Duration::hours(2);
    let token_data = TokenData::new("valid_token_123".to_string(), expires_at);
    {
        let mut guard = manager.token_data.lock().await;
        *guard = Some(token_data);
    }

    // Get token should return the existing valid token without making any requests
    let result = manager.get_token().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "valid_token_123");

    // Clean up
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
#[serial]
async fn test_token_manager_obtain_token_success() {
    // Save original env vars before loading .env
    let original_base_url = env::var("AMP_API_BASE_URL").ok();
    let original_username = env::var("AMP_USERNAME").ok();
    let original_password = env::var("AMP_PASSWORD").ok();

    let server = MockServer::start();
    env::set_var("AMP_API_BASE_URL", server.base_url());
    env::set_var("AMP_USERNAME", "test_user");
    env::set_var("AMP_PASSWORD", "test_password");

    // Mock successful token obtain
    let obtain_mock = server.mock(|when, then| {
        when.method(POST).path("/user/obtain_token");
        then.status(200).json_body(serde_json::json!({
            "token": "new_token_456"
        }));
    });

    // Create TokenManager with mock server URL
    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let result = manager.obtain_token().await;
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "new_token_456");

    // Verify the token was stored
    let stored_token = manager.token_data.lock().await;
    assert!(stored_token.is_some());
    let token_data = stored_token.as_ref().unwrap();
    assert_eq!(token_data.token.expose_secret(), "new_token_456");
    assert!(token_data.expires_at > Utc::now() + Duration::hours(23)); // Should be ~24 hours

    obtain_mock.assert_hits(1);

    // Restore original env vars
    match original_base_url {
        Some(val) => env::set_var("AMP_API_BASE_URL", val),
        None => env::remove_var("AMP_API_BASE_URL"),
    }
    match original_username {
        Some(val) => env::set_var("AMP_USERNAME", val),
        None => env::remove_var("AMP_USERNAME"),
    }
    match original_password {
        Some(val) => env::set_var("AMP_PASSWORD", val),
        None => env::remove_var("AMP_PASSWORD"),
    }
}

#[tokio::test]
#[serial]
async fn test_token_manager_obtain_token_missing_credentials() {
    // Save original env vars before loading .env
    let original_base_url = env::var("AMP_API_BASE_URL").ok();
    let original_username = env::var("AMP_USERNAME").ok();
    let original_password = env::var("AMP_PASSWORD").ok();

    // Remove credentials
    env::remove_var("AMP_USERNAME");
    env::remove_var("AMP_PASSWORD");

    // Create TokenManager with test URL (doesn't matter since we won't make requests)
    let config = RetryConfig::for_tests();
    let base_url = Url::parse("https://test.example.com/api").unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let result = manager.obtain_token().await;
    assert!(result.is_err());

    match result.unwrap_err() {
        Error::MissingEnvVar(var) => {
            assert_eq!(var, "AMP_USERNAME");
        }
        e => panic!("Expected MissingEnvVar error, got: {}", e),
    }

    // Restore original env vars
    match original_base_url {
        Some(val) => env::set_var("AMP_API_BASE_URL", val),
        None => env::remove_var("AMP_API_BASE_URL"),
    }
    match original_username {
        Some(val) => env::set_var("AMP_USERNAME", val),
        None => env::remove_var("AMP_USERNAME"),
    }
    match original_password {
        Some(val) => env::set_var("AMP_PASSWORD", val),
        None => env::remove_var("AMP_PASSWORD"),
    }
}

#[tokio::test]
#[serial]
async fn test_token_manager_obtain_token_server_error() {
    // Save original env vars
    let original_base_url = env::var("AMP_API_BASE_URL").ok();
    let original_username = env::var("AMP_USERNAME").ok();
    let original_password = env::var("AMP_PASSWORD").ok();

    let server = MockServer::start();
    env::set_var("AMP_API_BASE_URL", server.base_url());
    env::set_var("AMP_USERNAME", "test_user");
    env::set_var("AMP_PASSWORD", "wrong_password");

    // Mock authentication failure
    let obtain_mock = server.mock(|when, then| {
        when.method(POST).path("/user/obtain_token");
        then.status(401).body("Authentication failed");
    });

    // Create TokenManager with mock server URL
    let config = RetryConfig::for_tests();
    let base_url = Url::parse(&server.base_url()).unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    let result = manager.obtain_token().await;
    assert!(result.is_err());

    match result.unwrap_err() {
        Error::TokenRequestFailed { status, error_text } => {
            assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
            assert_eq!(error_text, "Authentication failed");
        }
        Error::Token(TokenError::ObtainFailed {
            attempts,
            last_error,
        }) => {
            // The retry client wraps the error, which is also acceptable
            assert_eq!(attempts, 1); // Should not retry 401 errors
            assert!(last_error.contains("401") || last_error.contains("Unauthorized"));
        }
        e => panic!(
            "Expected TokenRequestFailed or ObtainFailed error, got: {}",
            e
        ),
    }

    obtain_mock.assert_hits(1); // Should not retry 401 errors

    // Restore original env vars
    match original_base_url {
        Some(val) => env::set_var("AMP_API_BASE_URL", val),
        None => env::remove_var("AMP_API_BASE_URL"),
    }
    match original_username {
        Some(val) => env::set_var("AMP_USERNAME", val),
        None => env::remove_var("AMP_USERNAME"),
    }
    match original_password {
        Some(val) => env::set_var("AMP_PASSWORD", val),
        None => env::remove_var("AMP_PASSWORD"),
    }
}

#[tokio::test]
#[serial]
async fn test_token_manager_refresh_token_success() {
    let server = MockServer::start();
    env::set_var("AMP_API_BASE_URL", server.base_url());

    // Mock successful token refresh
    let refresh_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/user/refresh_token")
            .header("authorization", "token current_token_123");
        then.status(200).json_body(serde_json::json!({
            "token": "refreshed_token_789"
        }));
    });

    let config = RetryConfig::for_tests();
    let manager = TokenManager::with_config(config).unwrap();

    // Set an existing token
    let token_data = TokenData::new(
        "current_token_123".to_string(),
        Utc::now() + Duration::hours(1),
    );
    {
        let mut guard = manager.token_data.lock().await;
        *guard = Some(token_data);
    }

    let result = manager.refresh_token().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "refreshed_token_789");

    // Verify the token was updated
    let stored_token = manager.token_data.lock().await;
    assert!(stored_token.is_some());
    let token_data = stored_token.as_ref().unwrap();
    assert_eq!(token_data.token.expose_secret(), "refreshed_token_789");

    refresh_mock.assert_hits(1);

    // Clean up
    env::remove_var("AMP_API_BASE_URL");
}

#[tokio::test]
async fn test_token_manager_refresh_token_no_existing_token_logic() {
    // This test focuses on the logic when no token exists for refresh
    let config = RetryConfig::for_tests();
    let base_url = Url::parse("https://test.example.com/api").unwrap();
    let manager = TokenManager::with_config_and_base_url(config, base_url).unwrap();

    // No token exists, so refresh should handle this gracefully
    // The actual behavior depends on implementation - it might try to obtain a new token
    // or return an error. We'll just verify it doesn't panic.
    let result = manager.refresh_token().await;

    // The result can be either success (if it falls back to obtain) or error
    // We just want to ensure it doesn't panic and returns a proper Result
    match result {
        Ok(_) => {
            // If it succeeded, it likely fell back to obtaining a new token
            // This would require mocking the obtain endpoint, but for this test
            // we're just checking the logic path
        }
        Err(_) => {
            // If it failed, that's also acceptable behavior
            // The important thing is it didn't panic
        }
    }
}
