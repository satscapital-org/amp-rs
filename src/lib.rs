use std::env;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use once_cell::sync::OnceCell;
use reqwest::header::AUTHORIZATION;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

static AMP_TOKEN: OnceCell<Arc<Mutex<Option<String>>>> = OnceCell::new();
static AMP_TOKEN_EXPIRY: OnceCell<Arc<Mutex<Option<DateTime<Utc>>>>> = OnceCell::new();

#[derive(Error, Debug)]
pub enum Error {
    #[error("Missing {0} environment variable")]
    MissingEnvVar(String),
    #[error("AMP request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse AMP response: {0}")]
    ResponseParsingFailed(String),
    #[error("AMP token request failed with status {status}: {error_text}")]
    TokenRequestFailed {
        status: reqwest::StatusCode,
        error_text: String,
    },
    #[error("Failed to parse url: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

/// Request payload for AMP token acquisition
#[derive(Debug, Serialize)]
struct TokenRequest {
    username: String,
    password: String,
}

/// Response from AMP token acquisition
#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: Url,
}

impl ApiClient {
    pub fn new() -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        let client = Client::builder()
            .user_agent("amp-rs-client/0.1.0")
            .build()?;
        Ok(ApiClient {
            client,
            base_url,
        })
    }

    /// Obtains an API authentication token from Blockstream's AMP API
    ///
    /// This function retrieves credentials from environment variables `AMP_USERNAME` and `AMP_PASSWORD`,
    /// makes a POST request to the `/user/obtain_token` endpoint, and stores the token securely.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required environment variables are missing
    /// - The HTTP request fails
    /// - The API returns an error response
    /// - JSON parsing fails
    pub async fn obtain_amp_token(&self) -> Result<String, Error> {
        // Get credentials from environment variables
        let username = env::var("AMP_USERNAME")
            .map_err(|_| Error::MissingEnvVar("AMP_USERNAME".to_string()))?;
        let password = env::var("AMP_PASSWORD")
            .map_err(|_| Error::MissingEnvVar("AMP_PASSWORD".to_string()))?;

        // Prepare request payload
        let request_payload = TokenRequest { username, password };

        // Make POST request to obtain token
        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("user/obtain_token");

        let response = self.client
            .post(url)
            .json(&request_payload)
            .send()
            .await?;

        // Check if request was successful
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::TokenRequestFailed { status, error_text });
        }

        // Parse response
        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))?;

        // Store token securely
        let token_storage = AMP_TOKEN.get_or_init(|| Arc::new(Mutex::new(None)));
        let expiry_storage = AMP_TOKEN_EXPIRY.get_or_init(|| Arc::new(Mutex::new(None)));

        {
            let mut token_guard = token_storage.lock().await;
            *token_guard = Some(token_response.token.clone());
            drop(token_guard);

            let mut expiry_guard = expiry_storage.lock().await;
            // Set expiry to 1 day from now
            *expiry_guard = Some(Utc::now() + Duration::days(1));
        }

        tracing::info!("AMP authentication token obtained successfully");
        Ok(token_response.token)
    }

    pub async fn get_token(&self) -> Result<String, Error> {
        let token_storage = AMP_TOKEN.get_or_init(|| Arc::new(Mutex::new(None)));
        let expiry_storage = AMP_TOKEN_EXPIRY.get_or_init(|| Arc::new(Mutex::new(None)));

        let is_expired = {
            let expiry_guard = expiry_storage.lock().await;
            if let Some(expiry) = *expiry_guard {
                Utc::now() > expiry
            } else {
                true // No token or expiry, so it's "expired"
            }
        };

        if is_expired {
            self.obtain_amp_token().await
        } else {
            let token_guard = token_storage.lock().await;
            // The token must be Some if expiry is not None and not expired.
            Ok(token_guard.as_ref().unwrap().clone())
        }
    }

    pub async fn get_changelog(&self) -> Result<serde_json::Value, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("changelog");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Changelog request failed with status {}: {}",
                status, error_text
            )));
        }

        let changelog: serde_json::Value = response.json().await?;
        Ok(changelog)
    }
}

fn get_amp_api_base_url() -> Result<Url, Error> {
    let url_str = env::var("AMP_API_BASE_URL").unwrap_or_else(|_| "https://amp-test.blockstream.com/api".to_string());
    Url::parse(&url_str).map_err(Error::from)
}
