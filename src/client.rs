use std::env;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use once_cell::sync::OnceCell;
use reqwest::header::AUTHORIZATION;
use reqwest::{Client, Method, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::Mutex;

use secrecy::ExposeSecret;
use secrecy::Secret;

use crate::model::{
    Activity, AddAssetToGroup, Asset, AssetActivityParams, AssetGroup, AssetPermission,
    AssetSummary, Assignment, Audit, Balance, BroadcastResponse, CategoryAdd, CategoryEdit,
    CategoryResponse, ChangePasswordRequest, ChangePasswordResponse, CreateAssetAssignmentRequest,
    CreateAssetGroup, CreateAssetPermission, CreateAudit, EditAssetRequest, IssuanceRequest,
    IssuanceResponse, Outpoint, Ownership, Password, TokenRequest, TokenResponse,
    UpdateAssetGroup, UpdateAssetPermission, UpdateAudit, Utxo,
};

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

#[allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
impl ApiClient {
    /// Creates a new API client with the base URL from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `AMP_API_BASE_URL` environment variable contains an invalid URL
    pub fn new() -> Result<Self, Error> {
        let base_url = get_amp_api_base_url()?;
        Ok(Self {
            client: Client::new(),
            base_url,
        })
    }

    /// Creates a new API client with the specified base URL.
    ///
    /// # Errors
    ///
    /// This function is infallible but returns `Result` for API consistency.
    pub fn with_base_url(base_url: Url) -> Result<Self, Error> {
        Ok(Self {
            client: Client::new(),
            base_url,
        })
    }

    /// Obtains a new authentication token from the AMP API.
    ///
    /// # Panics
    ///
    /// May panic if the base URL is malformed and cannot be segmented.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `AMP_USERNAME` or `AMP_PASSWORD` environment variables are not set
    /// - The HTTP request fails
    /// - The token request is rejected by the server
    /// - The response cannot be parsed
    pub async fn obtain_amp_token(&self) -> Result<String, Error> {
        // Get credentials from environment variables
        let username = env::var("AMP_USERNAME")
            .map_err(|_| Error::MissingEnvVar("AMP_USERNAME".to_string()))?;
        let password = env::var("AMP_PASSWORD")
            .map_err(|_| Error::MissingEnvVar("AMP_PASSWORD".to_string()))?;

        let request_payload = TokenRequest { username, password };

        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("user")
            .push("obtain_token");

        let response = self.client.post(url).json(&request_payload).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::TokenRequestFailed { status, error_text });
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))?;

        let token_storage = AMP_TOKEN.get_or_init(|| Arc::new(Mutex::new(None)));
        let expiry_storage = AMP_TOKEN_EXPIRY.get_or_init(|| Arc::new(Mutex::new(None)));

        {
            let mut token_guard = token_storage.lock().await;
            *token_guard = Some(token_response.token.clone());
            drop(token_guard);

            let mut expiry_guard = expiry_storage.lock().await;
            *expiry_guard = Some(Utc::now() + Duration::days(1));
        }

        tracing::info!("AMP authentication token obtained successfully");
        Ok(token_response.token)
    }

    /// Gets a valid authentication token, refreshing if necessary.
    ///
    /// # Panics
    ///
    /// May panic if no valid token exists after attempting to obtain one.
    ///
    /// # Errors
    ///
    /// Returns an error if token acquisition fails (see `obtain_amp_token`).
    pub async fn get_token(&self) -> Result<String, Error> {
        let token_storage = AMP_TOKEN.get_or_init(|| Arc::new(Mutex::new(None)));
        let expiry_storage = AMP_TOKEN_EXPIRY.get_or_init(|| Arc::new(Mutex::new(None)));

        let is_expired = {
            let expiry_guard = expiry_storage.lock().await;
            expiry_guard.is_none_or(|expiry| Utc::now() > expiry)
        };

        if is_expired {
            self.obtain_amp_token().await
        } else {
            let token_guard = token_storage.lock().await;
            Ok(token_guard.as_ref().unwrap().clone())
        }
    }

    async fn request_raw(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<reqwest::Response, Error> {
        let token = self.get_token().await?;
        let mut url = self.base_url.clone();
        url.path_segments_mut().unwrap().extend(path);

        let mut request_builder = self
            .client
            .request(method, url)
            .header(AUTHORIZATION, format!("token {token}"));

        if let Some(body) = body {
            request_builder = request_builder.json(&body);
        }

        let response = request_builder.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::RequestFailed(format!(
                "Request to {path:?} failed with status {status}: {error_text}"
            )));
        }

        Ok(response)
    }

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<T, Error> {
        let response = self.request_raw(method, path, body).await?;
        response
            .json()
            .await
            .map_err(|e| Error::ResponseParsingFailed(e.to_string()))
    }

    async fn request_empty(
        &self,
        method: Method,
        path: &[&str],
        body: Option<impl serde::Serialize>,
    ) -> Result<(), Error> {
        self.request_raw(method, path, body).await?;
        Ok(())
    }

    /// Gets the API changelog.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed as JSON
    pub async fn get_changelog(&self) -> Result<serde_json::Value, Error> {
        self.request_json(Method::GET, &["changelog"], None::<&()>)
            .await
    }

    /// Changes the user's password.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server rejects the password change
    /// - The response cannot be parsed
    pub async fn user_change_password(
        &self,
        password: Secret<String>,
    ) -> Result<ChangePasswordResponse, Error> {
        let request = ChangePasswordRequest {
            password: Secret::new(Password(password.expose_secret().clone())),
        };
        self.request_json(Method::POST, &["user", "change_password"], Some(request))
            .await
    }

    /// Gets a list of all assets.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The server returns an error status
    /// - The response cannot be parsed
    pub async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        self.request_json(Method::GET, &["assets"], None::<&()>)
            .await
    }

    /// Gets a specific asset by UUID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The asset does not exist
    /// - The response cannot be parsed
    pub async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid], None::<&()>)
            .await
    }

    /// Issues a new asset.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The issuance request is invalid
    /// - The response cannot be parsed
    pub async fn issue_asset(
        &self,
        issuance_request: &IssuanceRequest,
    ) -> Result<IssuanceResponse, Error> {
        self.request_json(Method::POST, &["assets", "issue"], Some(issuance_request))
            .await
    }

    /// Edits an existing asset.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - The HTTP request fails
    /// - The asset does not exist
    /// - The edit request is invalid
    /// - The response cannot be parsed
    pub async fn edit_asset(
        &self,
        asset_uuid: &str,
        edit_asset_request: &EditAssetRequest,
    ) -> Result<Asset, Error> {
        self.request_json(
            Method::PUT,
            &["assets", asset_uuid, "edit"],
            Some(edit_asset_request),
        )
        .await
    }

    pub async fn delete_asset(&self, asset_uuid: &str) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["assets", asset_uuid, "delete"],
            None::<&()>,
        )
        .await
    }

    pub async fn list_asset_permissions(&self) -> Result<Vec<AssetPermission>, Error> {
        self.request_json(Method::GET, &["asset_permissions"], None::<&()>)
            .await
    }

    pub async fn create_asset_permission(
        &self,
        create_asset_permission: &CreateAssetPermission,
    ) -> Result<AssetPermission, Error> {
        self.request_json(
            Method::POST,
            &["asset_permissions"],
            Some(create_asset_permission),
        )
        .await
    }

    pub async fn get_asset_permission(
        &self,
        asset_permission_id: i64,
    ) -> Result<AssetPermission, Error> {
        self.request_json(
            Method::GET,
            &["asset_permissions", &asset_permission_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn update_asset_permission(
        &self,
        asset_permission_id: i64,
        update_asset_permission: &UpdateAssetPermission,
    ) -> Result<AssetPermission, Error> {
        self.request_json(
            Method::PUT,
            &["asset_permissions", &asset_permission_id.to_string()],
            Some(update_asset_permission),
        )
        .await
    }

    pub async fn delete_asset_permission(&self, asset_permission_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["asset_permissions", &asset_permission_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn get_broadcast_status(&self, txid: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::GET, &["tx", "broadcast", txid], None::<&()>)
            .await
    }

    pub async fn broadcast_transaction(&self, tx_hex: &str) -> Result<BroadcastResponse, Error> {
        self.request_json(Method::POST, &["tx", "broadcast"], Some(tx_hex))
            .await
    }

    pub async fn list_audits(&self) -> Result<Vec<Audit>, Error> {
        self.request_json(Method::GET, &["audits"], None::<&()>)
            .await
    }

    pub async fn create_audit(&self, create_audit: &CreateAudit) -> Result<Audit, Error> {
        self.request_json(Method::POST, &["audits"], Some(create_audit))
            .await
    }

    pub async fn get_audit(&self, audit_id: i64) -> Result<Audit, Error> {
        self.request_json(Method::GET, &["audits", &audit_id.to_string()], None::<&()>)
            .await
    }

    pub async fn update_audit(
        &self,
        audit_id: i64,
        update_audit: &UpdateAudit,
    ) -> Result<Audit, Error> {
        self.request_json(
            Method::PUT,
            &["audits", &audit_id.to_string()],
            Some(update_audit),
        )
        .await
    }

    pub async fn delete_audit(&self, audit_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["audits", &audit_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn list_asset_groups(&self) -> Result<Vec<AssetGroup>, Error> {
        self.request_json(Method::GET, &["asset_groups"], None::<&()>)
            .await
    }

    pub async fn create_asset_group(
        &self,
        create_asset_group: &CreateAssetGroup,
    ) -> Result<AssetGroup, Error> {
        self.request_json(Method::POST, &["asset_groups"], Some(create_asset_group))
            .await
    }

    pub async fn get_asset_group(&self, asset_group_id: i64) -> Result<AssetGroup, Error> {
        self.request_json(
            Method::GET,
            &["asset_groups", &asset_group_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn update_asset_group(
        &self,
        asset_group_id: i64,
        update_asset_group: &UpdateAssetGroup,
    ) -> Result<AssetGroup, Error> {
        self.request_json(
            Method::PUT,
            &["asset_groups", &asset_group_id.to_string()],
            Some(update_asset_group),
        )
        .await
    }

    pub async fn delete_asset_group(&self, asset_group_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["asset_groups", &asset_group_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn add_asset_to_group(
        &self,
        asset_group_id: i64,
        add_asset_to_group: &AddAssetToGroup,
    ) -> Result<AssetGroup, Error> {
        self.request_json(
            Method::POST,
            &["asset_groups", &asset_group_id.to_string(), "assets"],
            Some(add_asset_to_group),
        )
        .await
    }

    pub async fn remove_asset_from_group(
        &self,
        asset_group_id: i64,
        asset_uuid: &str,
    ) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &[
                "asset_groups",
                &asset_group_id.to_string(),
                "assets",
                asset_uuid,
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn register_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "register"],
            None::<&()>,
        )
        .await
    }

    pub async fn register_asset_authorized(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "register-authorized"],
            None::<&()>,
        )
        .await
    }

    pub async fn lock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::PUT, &["assets", asset_uuid, "lock"], None::<&()>)
            .await
    }

    pub async fn unlock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.request_json(Method::PUT, &["assets", asset_uuid, "unlock"], None::<&()>)
            .await
    }

    pub async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "activities"],
            Some(params),
        )
        .await
    }

    pub async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error> {
        let mut path = vec!["assets", asset_uuid, "ownerships"];
        let height_str;
        if let Some(h) = height {
            height_str = h.to_string();
            path.push(&height_str);
        }
        self.request_json(Method::GET, &path, None::<&()>).await
    }

    pub async fn get_asset_balance(&self, asset_uuid: &str) -> Result<Balance, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "balance"], None::<&()>)
            .await
    }

    pub async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "summary"], None::<&()>)
            .await
    }

    pub async fn get_asset_utxos(&self, asset_uuid: &str) -> Result<Vec<Utxo>, Error> {
        self.request_json(Method::GET, &["assets", asset_uuid, "utxos"], None::<&()>)
            .await
    }

    pub async fn blacklist_asset_utxos(
        &self,
        asset_uuid: &str,
        utxos: &[Outpoint],
    ) -> Result<Vec<Utxo>, Error> {
        self.request_json(
            Method::POST,
            &["assets", asset_uuid, "utxos", "blacklist"],
            Some(utxos),
        )
        .await
    }

    pub async fn whitelist_asset_utxos(
        &self,
        asset_uuid: &str,
        utxos: &[Outpoint],
    ) -> Result<Vec<Utxo>, Error> {
        self.request_json(
            Method::POST,
            &["assets", asset_uuid, "utxos", "whitelist"],
            Some(utxos),
        )
        .await
    }

    pub async fn get_asset_treasury_addresses(
        &self,
        asset_uuid: &str,
    ) -> Result<Vec<String>, Error> {
        self.request_json(
            Method::GET,
            &["assets", asset_uuid, "treasury-addresses"],
            None::<&()>,
        )
        .await
    }

    pub async fn add_asset_treasury_addresses(
        &self,
        asset_uuid: &str,
        addresses: &[String],
    ) -> Result<(), Error> {
        self.request_empty(
            Method::POST,
            &["assets", asset_uuid, "treasury-addresses", "add"],
            Some(addresses),
        )
        .await
    }

    pub async fn delete_asset_treasury_addresses(
        &self,
        asset_uuid: &str,
        addresses: &[String],
    ) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["assets", asset_uuid, "treasury-addresses", "delete"],
            Some(addresses),
        )
        .await
    }

    pub async fn get_registered_users(
        &self,
    ) -> Result<Vec<crate::model::RegisteredUserResponse>, Error> {
        self.request_json(Method::GET, &["registered_users"], None::<&()>)
            .await
    }

    pub async fn get_registered_user(
        &self,
        user_id: i64,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(
            Method::GET,
            &["registered_users", &user_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn add_registered_user(
        &self,
        new_user: &crate::model::RegisteredUserAdd,
    ) -> Result<crate::model::RegisteredUserResponse, Error> {
        self.request_json(Method::POST, &["registered_users", "add"], Some(new_user))
            .await
    }

    pub async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        self.request_json(Method::GET, &["categories"], None::<&()>)
            .await
    }

    pub async fn add_category(
        &self,
        new_category: &CategoryAdd,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(Method::POST, &["categories", "add"], Some(new_category))
            .await
    }

    pub async fn get_category(&self, category_id: i64) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::GET,
            &["categories", &category_id.to_string()],
            None::<&()>,
        )
        .await
    }

    pub async fn edit_category(
        &self,
        category_id: i64,
        edit_category: &CategoryEdit,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &["categories", &category_id.to_string(), "edit"],
            Some(edit_category),
        )
        .await
    }

    pub async fn delete_category(&self, category_id: i64) -> Result<(), Error> {
        self.request_empty(
            Method::DELETE,
            &["categories", &category_id.to_string(), "delete"],
            None::<&()>,
        )
        .await
    }

    pub async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "registered_users",
                &user_id.to_string(),
                "add",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "registered_users",
                &user_id.to_string(),
                "remove",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "assets",
                asset_uuid,
                "add",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn remove_asset_from_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        self.request_json(
            Method::PUT,
            &[
                "categories",
                &category_id.to_string(),
                "assets",
                asset_uuid,
                "remove",
            ],
            None::<&()>,
        )
        .await
    }

    pub async fn validate_gaid(
        &self,
        gaid: &str,
    ) -> Result<crate::model::ValidateGaidResponse, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "validate"], None::<&()>)
            .await
    }

    pub async fn get_gaid_address(
        &self,
        gaid: &str,
    ) -> Result<crate::model::AddressGaidResponse, Error> {
        self.request_json(Method::GET, &["gaids", gaid, "address"], None::<&()>)
            .await
    }

    pub async fn get_managers(&self) -> Result<Vec<crate::model::Manager>, Error> {
        self.request_json(Method::GET, &["managers"], None::<&()>)
            .await
    }

    pub async fn create_manager(
        &self,
        new_manager: &crate::model::ManagerCreate,
    ) -> Result<crate::model::Manager, Error> {
        self.request_json(Method::POST, &["managers", "create"], Some(new_manager))
            .await
    }

    pub async fn create_asset_assignment(
        &self,
        asset_uuid: &str,
        request: &CreateAssetAssignmentRequest,
    ) -> Result<Assignment, Error> {
        self.request_json(
            Method::POST,
            &["assets", asset_uuid, "assignments"],
            Some(request),
        )
        .await
    }
}

fn get_amp_api_base_url() -> Result<Url, Error> {
    let url_str = env::var("AMP_API_BASE_URL")
        .unwrap_or_else(|_| "https://amp-test.blockstream.com/api".to_string());
    Url::parse(&url_str).map_err(Error::from)
}
