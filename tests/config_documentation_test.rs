use std::env;
use amp_rs::client::RetryConfig;

#[cfg(test)]
mod config_documentation_tests {
    use super::*;

    #[test]
    fn test_documented_environment_variables() {
        dotenvy::dotenv().ok();
        // Test that all documented environment variables work as expected
        
        // Clean up any existing environment variables
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");

        // Test the example configuration from README.md
        env::set_var("API_RETRY_MAX_ATTEMPTS", "5");
        env::set_var("API_RETRY_BASE_DELAY_MS", "2000");
        env::set_var("API_RETRY_MAX_DELAY_MS", "60000");
        env::set_var("API_REQUEST_TIMEOUT_SECONDS", "30");

        let config = RetryConfig::from_env().expect("Should create config from documented env vars");
        
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
    fn test_documented_defaults() {
        // Don't load .env for this test since we want to test documented defaults
        // Test that documented defaults are correct
        
        // Clean up any existing environment variables
        env::remove_var("API_RETRY_MAX_ATTEMPTS");
        env::remove_var("API_RETRY_BASE_DELAY_MS");
        env::remove_var("API_RETRY_MAX_DELAY_MS");
        env::remove_var("API_REQUEST_TIMEOUT_SECONDS");

        let config = RetryConfig::from_env().expect("Should create config with defaults");
        
        // Verify documented defaults
        assert_eq!(config.max_attempts, 3, "Default max_attempts should be 3");
        assert_eq!(config.base_delay_ms, 1000, "Default base_delay_ms should be 1000");
        assert_eq!(config.max_delay_ms, 30000, "Default max_delay_ms should be 30000");
        assert_eq!(config.timeout_seconds, 10, "Default timeout_seconds should be 10");
    }

    #[test]
    fn test_security_dependencies_available() {
        // This test ensures that security dependencies are properly configured
        // by attempting to use them in a simple way
        
        use secrecy::{Secret, ExposeSecret};
        use zeroize::Zeroize;
        
        // Test secrecy crate
        let secret = Secret::new("test_secret".to_string());
        assert_eq!(secret.expose_secret(), "test_secret");
        
        // Test zeroize crate
        let mut sensitive_data = "sensitive".to_string();
        sensitive_data.zeroize();
        assert_eq!(sensitive_data, "");
    }
}