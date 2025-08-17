use std::env;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use once_cell::sync::OnceCell;
use reqwest::header::AUTHORIZATION;
use reqwest::{Client, Url};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::model::{TokenRequest, TokenResponse};

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

    pub async fn get_assets(&self) -> Result<Vec<crate::model::Asset>, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("assets/");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get assets request failed with status {}: {}",
                status, error_text
            )));
        }

        let assets: Vec<crate::model::Asset> = response.json().await?;
        Ok(assets)
    }

    pub async fn get_asset(&self, asset_uuid: &str) -> Result<crate::model::Asset, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("assets").push(asset_uuid);

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get asset request failed with status {}: {}",
                status, error_text
            )));
        }

        let asset: crate::model::Asset = response.json().await?;
        Ok(asset)
    }

    pub async fn issue_asset(&self, issuance_request: &crate::model::IssuanceRequest) -> Result<crate::model::IssuanceResponse, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("assets").push("issue");

        let response = self.client
            .post(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .json(issuance_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Issue asset request failed with status {}: {}",
                status, error_text
            )));
        }

        let issuance_response: crate::model::IssuanceResponse = response.json().await?;
        Ok(issuance_response)
    }

    pub async fn edit_asset(&self, asset_uuid: &str, edit_asset_request: &crate::model::EditAssetRequest) -> Result<crate::model::Asset, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("assets").push(asset_uuid).push("edit");

        let response = self.client
            .put(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .json(edit_asset_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Edit asset request failed with status {}: {}",
                status, error_text
            )));
        }

        let asset: crate::model::Asset = response.json().await?;
        Ok(asset)
    }

    pub async fn delete_asset(&self, asset_uuid: &str) -> Result<(), Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("assets").push(asset_uuid).push("delete");

        let response = self.client
            .delete(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Delete asset request failed with status {}: {}",
                status, error_text
            )));
        }

        Ok(())
    }

    pub async fn get_registered_users(&self) -> Result<Vec<crate::model::RegisteredUserResponse>, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("registered_users/");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get registered users request failed with status {}: {}",
                status, error_text
            )));
        }

        let registered_users: Vec<crate::model::RegisteredUserResponse> = response.json().await?;
        Ok(registered_users)
    }

    pub async fn get_registered_user(&self, user_id: i64) -> Result<crate::model::RegisteredUserResponse, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("registered_users").push(&user_id.to_string());

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get registered user request failed with status {}: {}",
                status, error_text
            )));
        }

        let registered_user: crate::model::RegisteredUserResponse = response.json().await?;
        Ok(registered_user)
    }

    pub async fn add_registered_user(&self, new_user: &crate::model::RegisteredUserAdd) -> Result<crate::model::RegisteredUserResponse, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("registered_users").push("add");

        let response = self.client
            .post(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .json(new_user)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Add registered user request failed with status {}: {}",
                status, error_text
            )));
        }

        let registered_user: crate::model::RegisteredUserResponse = response.json().await?;
        Ok(registered_user)
    }

    pub async fn get_categories(&self) -> Result<Vec<crate::model::CategoryResponse>, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("categories");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get categories request failed with status {}: {}",
                status, error_text
            )));
        }

        let categories: Vec<crate::model::CategoryResponse> = response.json().await?;
        Ok(categories)
    }

    pub async fn add_category(&self, new_category: &crate::model::CategoryAdd) -> Result<crate::model::CategoryResponse, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("categories").push("add");

        let response = self.client
            .post(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .json(new_category)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Add category request failed with status {}: {}",
                status, error_text
            )));
        }

        let category: crate::model::CategoryResponse = response.json().await?;
        Ok(category)
    }

    pub async fn validate_gaid(&self, gaid: &str) -> Result<crate::model::ValidateGaidResponse, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("gaids").push(gaid).push("validate");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Validate GAID request failed with status {}: {}",
                status, error_text
            )));
        }

        let validate_gaid_response: crate::model::ValidateGaidResponse = response.json().await?;
        Ok(validate_gaid_response)
    }

    pub async fn get_gaid_address(&self, gaid: &str) -> Result<crate::model::AddressGaidResponse, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("gaids").push(gaid).push("address");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get GAID address request failed with status {}: {}",
                status, error_text
            )));
        }

        let address_gaid_response: crate::model::AddressGaidResponse = response.json().await?;
        Ok(address_gaid_response)
    }

    pub async fn get_managers(&self) -> Result<Vec<crate::model::Manager>, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("managers");

        let response = self.client
            .get(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Get managers request failed with status {}: {}",
                status, error_text
            )));
        }

        let managers: Vec<crate::model::Manager> = response.json().await?;
        Ok(managers)
    }

    pub async fn create_manager(&self, new_manager: &crate::model::ManagerCreate) -> Result<crate::model::Manager, Error> {
        let token = self.get_token().await?;

        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().push("managers").push("create");

        let response = self.client
            .post(url)
            .header(AUTHORIZATION, format!("token {}", token))
            .json(new_manager)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Create manager request failed with status {}: {}",
                status, error_text
            )));
        }

        let manager: crate::model::Manager = response.json().await?;
        Ok(manager)
    }
}

fn get_amp_api_base_url() -> Result<Url, Error> {
    let url_str = env::var("AMP_API_BASE_URL").unwrap_or_else(|_| "https://amp-test.blockstream.com/api".to_string());
    Url::parse(&url_str).map_err(Error::from)
}
