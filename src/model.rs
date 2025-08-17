use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize)]
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
