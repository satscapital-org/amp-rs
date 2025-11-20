use chrono::{DateTime, Duration, Utc};
use secrecy::{DebugSecret, Secret, SerializableSecret};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(test)]
use std::collections::HashMap;

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

#[derive(Debug, Deserialize, Serialize, Clone)]
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

/// Response from asset registration with the Blockstream Asset Registry
/// Response from asset registration with the Blockstream Asset Registry
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RegisterAssetResponse {
    /// Indicates whether the registration was successful
    pub success: bool,
    /// Optional message providing additional context
    pub message: Option<String>,
    /// The full asset data if registration was successful (HTTP 200 with asset data)
    #[serde(flatten)]
    pub asset_data: Option<Asset>,
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
    #[serde(rename = "GAID")]
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

/// Assignment for asset distribution workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDistributionAssignment {
    pub user_id: String,
    pub address: String,
    pub amount: f64,
}

/// Assignment for distribution creation API request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionAssignmentRequest {
    pub user_uuid: String,
    pub amount: f64,
    pub address: String,
}

/// Request payload for distribution creation API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDistributionRequest {
    pub assignments: Vec<DistributionAssignmentRequest>,
}

/// UTXO information from Elements node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unspent {
    pub txid: String,
    pub vout: u32,
    pub amount: f64,
    pub asset: String,
    pub address: String,
    pub spendable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scriptpubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redeemscript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witnessscript: Option<String>,
    /// Amount blinder for confidential transactions (Elements specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amountblinder: Option<String>,
    /// Asset blinder for confidential transactions (Elements specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assetblinder: Option<String>,
}

/// Transaction details from Elements node (full gettransaction response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetail {
    pub txid: String,
    pub confirmations: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockheight: Option<u64>,
    pub hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockhash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocktime: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timereceived: Option<i64>,
    /// The details field from gettransaction (array of transaction outputs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<serde_json::Value>>,
}

/// Transaction output detail from Elements gettransaction details array
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutputDetail {
    pub account: String,
    pub address: String,
    pub category: String,
    pub amount: f64,
    pub vout: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockhash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockindex: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocktime: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timereceived: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assetblinder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amountblinder: Option<String>,
}

/// Transaction input for raw transaction creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub txid: String,
    pub vout: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
}

/// Response from distribution creation API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionResponse {
    pub distribution_uuid: String,
    pub map_address_amount: std::collections::HashMap<String, f64>,
    pub map_address_asset: std::collections::HashMap<String, String>,
    pub asset_id: String,
}

/// Address information from listreceivedbyaddress RPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedByAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<std::collections::HashMap<String, f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txids: Option<Vec<String>>,
}

/// Transaction data for distribution confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionTxData {
    pub details: TransactionDetail,
    pub txid: String,
}

/// Transaction data for AMP API confirmation (matches Python implementation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmpTxData {
    /// The details field from gettransaction (array of transaction outputs)
    pub details: serde_json::Value,
    pub txid: String,
}

/// Request payload for distribution confirmation API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmDistributionRequest {
    pub tx_data: AmpTxData,
    pub change_data: Vec<Unspent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_distribution_assignment_creation() {
        let assignment = AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 100.5,
        };

        assert_eq!(assignment.user_id, "user123");
        assert_eq!(
            assignment.address,
            "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"
        );
        assert_eq!(assignment.amount, 100.5);
    }

    #[test]
    fn test_asset_distribution_assignment_serialization() {
        let assignment = AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 100.5,
        };

        // Test serialization
        let json = serde_json::to_string(&assignment).unwrap();
        assert!(json.contains("user123"));
        assert!(json.contains("lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"));
        assert!(json.contains("100.5"));

        // Test deserialization
        let deserialized: AssetDistributionAssignment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, assignment.user_id);
        assert_eq!(deserialized.address, assignment.address);
        assert_eq!(deserialized.amount, assignment.amount);
    }

    #[test]
    fn test_asset_distribution_assignment_clone() {
        let assignment = AssetDistributionAssignment {
            user_id: "user123".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            amount: 100.5,
        };

        let cloned = assignment.clone();
        assert_eq!(assignment.user_id, cloned.user_id);
        assert_eq!(assignment.address, cloned.address);
        assert_eq!(assignment.amount, cloned.amount);
    }

    #[test]
    fn test_unspent_creation_and_serialization() {
        let unspent = Unspent {
            txid: "abc123def456".to_string(),
            vout: 1,
            amount: 50.0,
            asset: "asset_id_hex".to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            spendable: true,
            confirmations: Some(6),
            scriptpubkey: Some("76a914...88ac".to_string()),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            assetblinder: Some(
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_string(),
            ),
        };

        // Test serialization
        let json = serde_json::to_string(&unspent).unwrap();
        assert!(json.contains("abc123def456"));
        assert!(json.contains("50.0"));
        assert!(json.contains("asset_id_hex"));

        // Test deserialization
        let deserialized: Unspent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.txid, unspent.txid);
        assert_eq!(deserialized.vout, unspent.vout);
        assert_eq!(deserialized.amount, unspent.amount);
        assert_eq!(deserialized.asset, unspent.asset);
        assert_eq!(deserialized.confirmations, unspent.confirmations);
    }

    #[test]
    fn test_transaction_detail_creation() {
        let tx_detail = TransactionDetail {
            txid: "def456abc123".to_string(),
            confirmations: 3,
            blockheight: Some(12345),
            hex: "020000000001...".to_string(),
            blockhash: Some("block_hash_hex".to_string()),
            details: Some(vec![]),
            blocktime: Some(1640995200),
            time: Some(1640995200),
            timereceived: Some(1640995180),
        };

        assert_eq!(tx_detail.txid, "def456abc123");
        assert_eq!(tx_detail.confirmations, 3);
        assert_eq!(tx_detail.blockheight, Some(12345));

        // Test serialization
        let json = serde_json::to_string(&tx_detail).unwrap();
        assert!(json.contains("def456abc123"));
        assert!(json.contains("\"confirmations\":3"));
    }

    #[test]
    fn test_tx_input_creation() {
        let tx_input = TxInput {
            txid: "input_txid_123".to_string(),
            vout: 2,
            sequence: Some(0xffffffff),
        };

        assert_eq!(tx_input.txid, "input_txid_123");
        assert_eq!(tx_input.vout, 2);
        assert_eq!(tx_input.sequence, Some(0xffffffff));

        // Test serialization
        let json = serde_json::to_string(&tx_input).unwrap();
        assert!(json.contains("input_txid_123"));
        assert!(json.contains("\"vout\":2"));
        assert!(json.contains("4294967295")); // 0xffffffff in decimal
    }

    #[test]
    fn test_distribution_response_creation() {
        let mut map_address_amount = HashMap::new();
        map_address_amount.insert("address1".to_string(), 100.0);
        map_address_amount.insert("address2".to_string(), 50.0);

        let mut map_address_asset = HashMap::new();
        map_address_asset.insert("address1".to_string(), "asset_id_1".to_string());
        map_address_asset.insert("address2".to_string(), "asset_id_1".to_string());

        let distribution_response = DistributionResponse {
            distribution_uuid: "dist_uuid_123".to_string(),
            map_address_amount,
            map_address_asset,
            asset_id: "main_asset_id".to_string(),
        };

        assert_eq!(distribution_response.distribution_uuid, "dist_uuid_123");
        assert_eq!(distribution_response.asset_id, "main_asset_id");
        assert_eq!(distribution_response.map_address_amount.len(), 2);
        assert_eq!(distribution_response.map_address_asset.len(), 2);

        // Test serialization
        let json = serde_json::to_string(&distribution_response).unwrap();
        assert!(json.contains("dist_uuid_123"));
        assert!(json.contains("main_asset_id"));
        assert!(json.contains("address1"));
        assert!(json.contains("100.0"));
    }

    #[test]
    fn test_distribution_assignment_request_creation() {
        let assignment_request = DistributionAssignmentRequest {
            user_uuid: "user_uuid_123".to_string(),
            amount: 150.0,
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        };

        assert_eq!(assignment_request.user_uuid, "user_uuid_123");
        assert_eq!(assignment_request.amount, 150.0);
        assert_eq!(
            assignment_request.address,
            "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"
        );

        // Test serialization
        let json = serde_json::to_string(&assignment_request).unwrap();
        assert!(json.contains("user_uuid_123"));
        assert!(json.contains("150.0"));
        assert!(json.contains("lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"));

        // Test deserialization
        let deserialized: DistributionAssignmentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_uuid, assignment_request.user_uuid);
        assert_eq!(deserialized.amount, assignment_request.amount);
        assert_eq!(deserialized.address, assignment_request.address);
    }

    #[test]
    fn test_create_distribution_request_creation() {
        let assignments = vec![
            DistributionAssignmentRequest {
                user_uuid: "user1".to_string(),
                amount: 100.0,
                address: "address1".to_string(),
            },
            DistributionAssignmentRequest {
                user_uuid: "user2".to_string(),
                amount: 50.0,
                address: "address2".to_string(),
            },
        ];

        let create_request = CreateDistributionRequest {
            assignments: assignments.clone(),
        };

        assert_eq!(create_request.assignments.len(), 2);
        assert_eq!(create_request.assignments[0].user_uuid, "user1");
        assert_eq!(create_request.assignments[1].amount, 50.0);

        // Test serialization
        let json = serde_json::to_string(&create_request).unwrap();
        assert!(json.contains("user1"));
        assert!(json.contains("user2"));
        assert!(json.contains("100.0"));
        assert!(json.contains("50.0"));
        assert!(json.contains("assignments"));

        // Test deserialization
        let deserialized: CreateDistributionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.assignments.len(), 2);
        assert_eq!(deserialized.assignments[0].user_uuid, "user1");
        assert_eq!(deserialized.assignments[1].user_uuid, "user2");
    }

    #[test]
    fn test_distribution_request_serialization_format() {
        let assignment = DistributionAssignmentRequest {
            user_uuid: "test_user".to_string(),
            amount: 123.45,
            address: "test_address".to_string(),
        };

        let request = CreateDistributionRequest {
            assignments: vec![assignment],
        };

        let json = serde_json::to_string(&request).unwrap();

        // Verify the JSON structure matches the API specification
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("assignments").is_some());
        let assignments_array = parsed["assignments"].as_array().unwrap();
        assert_eq!(assignments_array.len(), 1);

        let first_assignment = &assignments_array[0];
        assert_eq!(first_assignment["user_uuid"], "test_user");
        assert_eq!(first_assignment["amount"], 123.45);
        assert_eq!(first_assignment["address"], "test_address");
    }

    #[test]
    fn test_distribution_tx_data_creation() {
        let tx_detail = TransactionDetail {
            txid: "test_txid_123".to_string(),
            confirmations: 2,
            blockheight: Some(12345),
            hex: "020000000001...".to_string(),
            blockhash: Some("block_hash_hex".to_string()),
            blocktime: Some(1640995200),
            time: Some(1640995200),
            timereceived: Some(1640995180),
            details: Some(vec![]),
        };

        let tx_data = DistributionTxData {
            details: tx_detail.clone(),
            txid: "test_txid_123".to_string(),
        };

        assert_eq!(tx_data.txid, "test_txid_123");
        assert_eq!(tx_data.details.txid, "test_txid_123");
        assert_eq!(tx_data.details.confirmations, 2);

        // Test serialization
        let json = serde_json::to_string(&tx_data).unwrap();
        assert!(json.contains("test_txid_123"));
        assert!(json.contains("\"confirmations\":2"));

        // Test deserialization
        let deserialized: DistributionTxData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.txid, tx_data.txid);
        assert_eq!(
            deserialized.details.confirmations,
            tx_data.details.confirmations
        );
    }

    #[test]
    fn test_confirm_distribution_request_creation() {
        let _tx_detail = TransactionDetail {
            txid: "confirm_test_txid".to_string(),
            confirmations: 3,
            blockheight: Some(54321),
            hex: "020000000002...".to_string(),
            blockhash: Some("confirm_block_hash".to_string()),
            blocktime: Some(1640995300),
            time: Some(1640995300),
            timereceived: Some(1640995280),
            details: Some(vec![]),
        };

        let tx_data = AmpTxData {
            details: serde_json::json!([]),
            txid: "confirm_test_txid".to_string(),
        };

        let change_utxo = Unspent {
            txid: "change_txid_123".to_string(),
            vout: 1,
            amount: 25.0,
            asset: "change_asset_id".to_string(),
            address: "change_address".to_string(),
            spendable: true,
            confirmations: Some(3),
            scriptpubkey: Some("76a914...88ac".to_string()),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(
                "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            ),
            assetblinder: Some(
                "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            ),
        };

        let confirm_request = ConfirmDistributionRequest {
            tx_data,
            change_data: vec![change_utxo],
        };

        assert_eq!(confirm_request.tx_data.txid, "confirm_test_txid");
        assert_eq!(confirm_request.change_data.len(), 1);
        assert_eq!(confirm_request.change_data[0].txid, "change_txid_123");

        // Test serialization
        let json = serde_json::to_string(&confirm_request).unwrap();
        assert!(json.contains("confirm_test_txid"));
        assert!(json.contains("change_txid_123"));
        assert!(json.contains("tx_data"));
        assert!(json.contains("change_data"));

        // Test deserialization
        let deserialized: ConfirmDistributionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tx_data.txid, confirm_request.tx_data.txid);
        assert_eq!(deserialized.change_data.len(), 1);
        assert_eq!(deserialized.change_data[0].txid, "change_txid_123");
    }

    #[test]
    fn test_confirm_distribution_request_serialization_format() {
        let _tx_detail = TransactionDetail {
            txid: "format_test_txid".to_string(),
            confirmations: 2,
            blockheight: Some(98765),
            hex: "format_test_hex".to_string(),
            blockhash: None,
            blocktime: None,
            time: None,
            timereceived: None,
            details: Some(vec![]),
        };

        let tx_data = AmpTxData {
            details: serde_json::json!([]),
            txid: "format_test_txid".to_string(),
        };

        let confirm_request = ConfirmDistributionRequest {
            tx_data,
            change_data: vec![],
        };

        let json = serde_json::to_string(&confirm_request).unwrap();

        // Verify the JSON structure matches the API specification
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("tx_data").is_some());
        assert!(parsed.get("change_data").is_some());

        let tx_data_obj = &parsed["tx_data"];
        assert!(tx_data_obj.get("details").is_some());
        assert!(tx_data_obj.get("txid").is_some());
        assert_eq!(tx_data_obj["txid"], "format_test_txid");

        let change_data_array = parsed["change_data"].as_array().unwrap();
        assert_eq!(change_data_array.len(), 0);
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
