use secrecy::{DebugSecret, Secret, SerializableSecret};
use serde::{Deserialize, Serialize};
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
    pub name: String,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Assignment {
    pub id: i64,
    pub registered_user: i64,
    pub amount: i64,
    pub receiving_address: String,
    pub distribution_uuid: Option<String>,
    pub ready_for_distribution: bool,
    pub vesting_datetime: Option<String>,
    pub vesting_timestamp: Option<i64>,
    pub has_vested: bool,
    pub is_distributed: bool,
    pub creator: i64,
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
pub struct Balance {
    pub confirmed_balance: Vec<Ownership>,
    pub lost_outputs: LostOutputs,
    pub reissuance_lost_outputs: LostOutputs,
}

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

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetGroup {
    pub id: i64,
    pub name: String,
    pub assets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateAssetGroup {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateAssetGroup {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AddAssetToGroup {
    pub asset_uuid: String,
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
pub struct AssetPermission {
    pub id: i64,
    pub manager: i64,
    pub asset: Option<String>,
    pub asset_group: Option<i64>,
    pub permission: Permission,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Audit {
    pub id: i64,
    pub asset: String,
    pub audit_type: String,
    pub audit_status: String,
    pub created: String,
    pub updated: String,
    pub blockheight: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateAudit {
    pub asset_uuid: String,
    pub audit_type: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateAudit {
    pub audit_status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BroadcastResponse {
    pub txid: String,
    pub hex: String,
}

#[derive(Debug, Serialize)]
pub struct CreateAssetPermission {
    pub manager: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_group: Option<i64>,
    pub permission: Permission,
}

#[derive(Debug, Serialize)]
pub struct UpdateAssetPermission {
    pub manager: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_group: Option<i64>,
    pub permission: Permission,
}
