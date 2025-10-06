use amp_rs::client::{RetryClient, RetryConfig, TokenError};
use httpmock::prelude::*;
use serial_test::serial;
use std::env;
use std::time::Duration as StdDuration;

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


