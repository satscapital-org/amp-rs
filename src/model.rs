use chrono::{DateTime, Duration, Utc};
use secrecy::{DebugSecret, Secret, SerializableSecret};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroize;

/// Request payload for AMP token acquisition
#[derive(Debug, Serialize)]
pub struct TokenRequest {
    pub username: String,
    pub password: String,
}

/// Response from AMP token acquisition
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Password(pub String);

impl Zeroize for Password {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl From<String> for Password {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl SerializableSecret for Password {}

impl DebugSecret for Password {}

#[derive(Debug, Serialize)]
pub struct ChangePasswordRequest {
    pub password: Secret<Password>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordResponse {
    pub username: String,
    pub password: Secret<Password>,
    pub token: Secret<String>,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Asset {
    pub name: String,
    pub asset_uuid: String,
    pub issuer: i64,
    pub asset_id: String,
    pub reissuance_token_id: Option<String>,
    pub requirements: Vec<i64>,
    pub ticker: Option<String>,
    pub precision: i64,
    pub domain: Option<String>,
    pub pubkey: Option<String>,
    pub is_registered: bool,
    pub is_authorized: bool,
    pub is_locked: bool,
    pub issuer_authorization_endpoint: Option<String>,
    pub transfer_restricted: bool,
}

#[derive(Debug, Serialize)]
pub struct IssuanceRequest {
    pub name: String,
    pub amount: i64,
    pub destination_address: String,
    pub domain: String,
    pub ticker: String,
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_confidential: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_reissuable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reissuance_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reissuance_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_restricted: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct IssuanceResponse {
    pub name: String,
    pub amount: i64,
    pub destination_address: String,
    pub domain: String,
    pub ticker: String,
    pub pubkey: String,
    pub is_confidential: bool,
    pub is_reissuable: bool,
    pub reissuance_amount: i64,
    pub reissuance_address: String,
    pub asset_id: String,
    pub reissuance_token_id: Option<String>,
    pub asset_uuid: String,
    pub txid: String,
    pub vin: i64,
    pub asset_vout: i64,
    pub reissuance_vout: Option<i64>,
    pub issuer_authorization_endpoint: Option<String>,
    pub transfer_restricted: bool,
    pub issuance_assetblinder: String,
    pub issuance_tokenblinder: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EditAssetRequest {
    pub issuer_authorization_endpoint: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisteredUserResponse {
    pub id: i64,
    #[serde(rename = "GAID")]
    pub gaid: Option<String>,
    pub is_company: bool,
    pub name: String,
    pub categories: Vec<i64>,
    pub creator: i64,
}

#[derive(Debug, Serialize)]
pub struct RegisteredUserAdd {
    pub name: String,
    #[serde(rename = "GAID")]
    pub gaid: Option<String>,
    pub is_company: bool,
}

#[derive(Debug, Serialize)]
pub struct RegisteredUserEdit {
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GaidRequest {
    pub gaid: String,
}

#[derive(Debug, Serialize)]
pub struct CategoriesRequest {
    pub categories: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub registered_users: Vec<i64>,
    pub assets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryAdd {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryEdit {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateGaidResponse {
    pub is_valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddressGaidResponse {
    pub address: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Manager {
    pub username: String,
    pub id: i64,
    pub is_locked: bool,
    pub assets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ManagerCreate {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Unconfirmed,
    Confirmed,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DistributionAssignment {
    pub registered_user: i64,
    pub amount: i64,
    pub vout: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Transaction {
    pub txid: String,
    pub transaction_status: Status,
    pub included_blockheight: i64,
    pub confirmed_datetime: String,
    pub assignments: Vec<DistributionAssignment>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Distribution {
    pub distribution_uuid: String,
    pub distribution_status: Status,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateAssetAssignmentRequest {
    pub registered_user: i64,
    pub amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vesting_timestamp: Option<i64>, // Unix timestamp in seconds, nullable
    #[serde(default = "default_ready_for_distribution")]
    pub ready_for_distribution: bool, // Defaults to false
}

const fn default_ready_for_distribution() -> bool {
    false
}

#[derive(Debug, Serialize)]
pub struct CreateAssetAssignmentRequestWrapper {
    pub assignments: Vec<CreateAssetAssignmentRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Assignment {
    pub id: i64,
    pub registered_user: i64,
    pub amount: i64,
    pub receiving_address: Option<String>,
    pub distribution_uuid: Option<String>,
    pub ready_for_distribution: bool,
    pub vesting_datetime: Option<String>,
    pub vesting_timestamp: Option<i64>,
    pub has_vested: bool,
    pub is_distributed: bool,
    pub creator: i64,
    #[serde(rename = "GAID")]
    pub gaid: Option<String>,
    // Legacy field for backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub investor: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisteredUserSummary {
    pub asset_uuid: String,
    pub asset_id: String,
    pub assignments: Vec<Assignment>,
    pub assignments_sum: i64,
    pub distributions: Vec<Distribution>,
    pub distributions_sum: i64,
    pub balance: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Activity {
    #[serde(rename = "type")]
    pub activity_type: String,
    pub datetime: String,
    pub description: String,
    pub txid: String,
    pub vout: i64,
    pub blockheight: i64,
    pub asset_blinder: String,
    pub amount_blinder: String,
    #[serde(rename = "registered user")]
    pub registered_user: Option<i64>,
    pub amount: i64,
}

#[derive(Debug, Serialize, Default)]
pub struct AssetActivityParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sortcolumn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sortorder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_stop: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Ownership {
    pub owner: String,
    pub amount: i64,
    #[serde(rename = "GAID")]
    pub gaid: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Outpoint {
    pub txid: String,
    pub vout: i64,
}

pub type LostOutputs = Vec<Outpoint>;

#[derive(Debug, Deserialize, Serialize)]
pub struct GaidBalanceEntry {
    pub asset_uuid: String,
    pub asset_id: String,
    pub balance: i64,
}

pub type Balance = Vec<GaidBalanceEntry>;

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetLostOutputs {
    pub lost_outputs: LostOutputs,
    pub reissuance_lost_outputs: LostOutputs,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetSummary {
    pub asset_id: String,
    pub reissuance_token_id: Option<String>,
    pub issued: i64,
    pub reissued: i64,
    pub assigned: i64,
    pub distributed: i64,
    pub burned: i64,
    pub blacklisted: i64,
    pub registered_users: i64,
    pub active_registered_users: i64,
    pub active_green_subaccounts: i64,
    #[serde(rename = "reissuance_tokens")]
    pub reissuance_tokens: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: i64,
    pub asset: String,
    pub amount: i64,
    pub registered_user: Option<i64>,
    pub gaid: Option<String>,
    pub blacklisted: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Reissuance {
    pub txid: String,
    pub vout: i64,
    pub destination_address: String,
    pub reissuance_amount: i64,
    pub confirmed_in_block: String,
    pub created: String,
}

#[derive(Debug, Serialize)]
pub struct ReissueRequest {
    pub amount_to_reissue: i64,
}

#[derive(Debug, Serialize)]
pub struct ReissueConfirmRequest {
    pub details: serde_json::Value,
    pub listissuances: Vec<serde_json::Value>,
    pub reissuance_output: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct BurnRequest {
    pub amount: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BurnCreate {
    pub command: String,
    pub min_supported_client_script_version: i64,
    pub base_url: String,
    pub asset_uuid: String,
    pub asset_id: String,
    pub amount: f64,
    pub utxos: Vec<Outpoint>,
}

#[derive(Debug, Serialize)]
pub struct BurnConfirmRequest {
    pub tx_data: serde_json::Value,
    pub change_data: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct SetAssetMemoRequest {
    pub memo: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    View,
    Receive,
    Transfer,
    Assign,
    Distribute,
    Reissue,
    Burn,
    Acquire,
    Manage,
    Permissions,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BroadcastResponse {
    pub txid: String,
    pub hex: String,
}

/// Enhanced token data structure with secure storage and timestamp tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    #[serde(with = "secret_serde")]
    pub token: Secret<String>,
    pub expires_at: DateTime<Utc>,
    pub obtained_at: DateTime<Utc>,
}

impl TokenData {
    /// Creates a new `TokenData` instance
    ///
    /// # Examples
    /// ```
    /// # use amp_rs::model::TokenData;
    /// # use chrono::{Utc, Duration};
    /// let expires_at = Utc::now() + Duration::hours(24);
    /// let token_data = TokenData::new("my_token".to_string(), expires_at);
    /// assert!(!token_data.is_expired());
    /// ```
    #[must_use]
    pub fn new(token: String, expires_at: DateTime<Utc>) -> Self {
        Self {
            token: Secret::new(token),
            expires_at,
            obtained_at: Utc::now(),
        }
    }

    /// Checks if the token is expired
    ///
    /// # Examples
    /// ```
    /// # use amp_rs::model::TokenData;
    /// # use chrono::{Utc, Duration};
    /// // Create an expired token
    /// let expires_at = Utc::now() - Duration::hours(1);
    /// let token_data = TokenData::new("expired_token".to_string(), expires_at);
    /// assert!(token_data.is_expired());
    /// 
    /// // Create a valid token
    /// let expires_at = Utc::now() + Duration::hours(1);
    /// let token_data = TokenData::new("valid_token".to_string(), expires_at);
    /// assert!(!token_data.is_expired());
    /// ```
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Checks if the token expires within the given threshold
    ///
    /// # Examples
    /// ```
    /// # use amp_rs::model::TokenData;
    /// # use chrono::{Utc, Duration};
    /// // Token expires in 30 minutes
    /// let expires_at = Utc::now() + Duration::minutes(30);
    /// let token_data = TokenData::new("token".to_string(), expires_at);
    /// 
    /// // Check if it expires within 1 hour
    /// assert!(token_data.expires_soon(Duration::hours(1)));
    /// 
    /// // Check if it expires within 15 minutes
    /// assert!(!token_data.expires_soon(Duration::minutes(15)));
    /// ```
    #[must_use]
    pub fn expires_soon(&self, threshold: Duration) -> bool {
        Utc::now() + threshold > self.expires_at
    }

    /// Returns the age of the token
    ///
    /// # Examples
    /// ```
    /// # use amp_rs::model::TokenData;
    /// # use chrono::{Utc, Duration};
    /// let expires_at = Utc::now() + Duration::hours(24);
    /// let token_data = TokenData::new("token".to_string(), expires_at);
    /// 
    /// // Token age should be very small (just created)
    /// let age = token_data.age();
    /// assert!(age < Duration::seconds(1));
    /// ```
    #[must_use]
    pub fn age(&self) -> Duration {
        Utc::now() - self.obtained_at
    }
}

/// Token information for debugging and monitoring
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub expires_at: DateTime<Utc>,
    pub obtained_at: DateTime<Utc>,
    pub expires_in: Duration,
    pub age: Duration,
    pub is_expired: bool,
    pub expires_soon: bool,
}

impl From<&TokenData> for TokenInfo {
    fn from(token_data: &TokenData) -> Self {
        let now = Utc::now();
        let expires_in = token_data.expires_at - now;
        let expires_soon_threshold = Duration::minutes(5);

        Self {
            expires_at: token_data.expires_at,
            obtained_at: token_data.obtained_at,
            expires_in,
            age: token_data.age(),
            is_expired: token_data.is_expired(),
            expires_soon: token_data.expires_soon(expires_soon_threshold),
        }
    }
}

/// Custom serialization module for Secret<String>
pub mod secret_serde {
    use super::{Deserialize, Deserializer, Secret, Serialize, Serializer};

    /// # Errors
    /// Returns an error if serialization fails
    pub fn serialize<S>(secret: &Secret<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use secrecy::ExposeSecret;
        secret.expose_secret().serialize(serializer)
    }

    /// # Errors
    /// Returns an error if deserialization fails
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Secret<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Secret::new(s))
    }
}
