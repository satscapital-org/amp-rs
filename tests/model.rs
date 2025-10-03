use amp_rs::model::{TokenData, TokenInfo};
use chrono::{Duration, Utc};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};

#[test]
fn test_token_data_creation() {
    let token = "test_token_123".to_string();
    let expires_at = Utc::now() + Duration::hours(24);

    let token_data = TokenData::new(token.clone(), expires_at);

    assert_eq!(token_data.token.expose_secret(), &token);
    assert_eq!(token_data.expires_at, expires_at);
    assert!(token_data.obtained_at <= Utc::now());
    assert!(token_data.obtained_at > Utc::now() - Duration::seconds(1));
}

#[test]
fn test_token_data_is_expired() {
    // Test non-expired token
    let expires_at = Utc::now() + Duration::hours(1);
    let token_data = TokenData::new("token".to_string(), expires_at);
    assert!(!token_data.is_expired());

    // Test expired token
    let expires_at = Utc::now() - Duration::hours(1);
    let token_data = TokenData::new("token".to_string(), expires_at);
    assert!(token_data.is_expired());

    // Test token expiring right now (edge case)
    let expires_at = Utc::now();
    let token_data = TokenData::new("token".to_string(), expires_at);
    // This might be true or false depending on timing, but should not panic
    let _ = token_data.is_expired();
}

#[test]
fn test_token_data_expires_soon() {
    // Test token that expires soon (within 5 minutes)
    let expires_at = Utc::now() + Duration::minutes(3);
    let token_data = TokenData::new("token".to_string(), expires_at);
    assert!(token_data.expires_soon(Duration::minutes(5)));

    // Test token that doesn't expire soon
    let expires_at = Utc::now() + Duration::hours(1);
    let token_data = TokenData::new("token".to_string(), expires_at);
    assert!(!token_data.expires_soon(Duration::minutes(5)));

    // Test with custom threshold
    let expires_at = Utc::now() + Duration::minutes(8);
    let token_data = TokenData::new("token".to_string(), expires_at);
    assert!(token_data.expires_soon(Duration::minutes(10)));
    assert!(!token_data.expires_soon(Duration::minutes(5)));
}

#[test]
fn test_token_data_age() {
    let expires_at = Utc::now() + Duration::hours(24);
    let token_data = TokenData::new("token".to_string(), expires_at);

    let age = token_data.age();
    assert!(age >= Duration::zero());
    assert!(age < Duration::seconds(1)); // Should be very recent
}

#[test]
fn test_token_data_serialization() {
    let token = "secret_token_value".to_string();
    let expires_at = Utc::now() + Duration::hours(24);
    let token_data = TokenData::new(token.clone(), expires_at);

    // Test serialization
    let serialized = serde_json::to_string(&token_data).expect("Failed to serialize TokenData");
    assert!(serialized.contains(&token)); // Token should be serialized
    assert!(serialized.contains("expires_at")); // Field name should be present
    assert!(serialized.contains("obtained_at")); // Field name should be present

    // Test deserialization
    let deserialized: TokenData =
        serde_json::from_str(&serialized).expect("Failed to deserialize TokenData");
    assert_eq!(deserialized.token.expose_secret(), &token);
    assert_eq!(deserialized.expires_at, expires_at);
    assert_eq!(deserialized.obtained_at, token_data.obtained_at);
}

#[test]
fn test_token_data_serialization_roundtrip() {
    let original_token = TokenData::new(
        "test_token_roundtrip".to_string(),
        Utc::now() + Duration::hours(12),
    );

    // Serialize to JSON
    let json = serde_json::to_string(&original_token).expect("Serialization failed");

    // Deserialize back
    let restored_token: TokenData =
        serde_json::from_str(&json).expect("Deserialization failed");

    // Verify all fields match
    assert_eq!(
        original_token.token.expose_secret(),
        restored_token.token.expose_secret()
    );
    assert_eq!(original_token.expires_at, restored_token.expires_at);
    assert_eq!(original_token.obtained_at, restored_token.obtained_at);
}

#[test]
fn test_token_info_from_token_data() {
    let expires_at = Utc::now() + Duration::hours(2);
    let token_data = TokenData::new("token".to_string(), expires_at);

    let token_info = TokenInfo::from(&token_data);

    assert_eq!(token_info.expires_at, token_data.expires_at);
    assert_eq!(token_info.obtained_at, token_data.obtained_at);
    assert!(!token_info.is_expired);
    assert!(!token_info.expires_soon); // 2 hours > 5 minutes
    assert!(token_info.expires_in > Duration::hours(1));
    assert!(token_info.age < Duration::seconds(1));
}

#[test]
fn test_token_info_expires_soon() {
    // Token that expires in 3 minutes (should be flagged as expires_soon)
    let expires_at = Utc::now() + Duration::minutes(3);
    let token_data = TokenData::new("token".to_string(), expires_at);

    let token_info = TokenInfo::from(&token_data);

    assert!(!token_info.is_expired);
    assert!(token_info.expires_soon);
    assert!(token_info.expires_in < Duration::minutes(5));
}

#[test]
fn test_token_info_expired() {
    // Expired token
    let expires_at = Utc::now() - Duration::hours(1);
    let token_data = TokenData::new("token".to_string(), expires_at);

    let token_info = TokenInfo::from(&token_data);

    assert!(token_info.is_expired);
    assert!(token_info.expires_soon); // Expired tokens also expire "soon"
    assert!(token_info.expires_in < Duration::zero());
}

#[test]
fn test_secret_serde_module() {
    // Test the custom serialization module directly
    let secret = Secret::new("test_secret".to_string());

    // Create a simple struct to test the serde module
    #[derive(Serialize, Deserialize)]
    struct TestStruct {
        #[serde(with = "amp_rs::model::secret_serde")]
        secret_field: Secret<String>,
    }

    let test_struct = TestStruct {
        secret_field: secret,
    };

    let json = serde_json::to_string(&test_struct).expect("Serialization failed");
    assert!(json.contains("test_secret"));

    let restored: TestStruct = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(restored.secret_field.expose_secret(), "test_secret");
}