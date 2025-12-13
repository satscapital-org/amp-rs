//! Mock API Client for Testing
//!
//! This module provides a `MockApiClient` that implements the same interface as `ApiClient`
//! but returns configurable mock responses without making actual HTTP requests.
//! This is useful for integration testing in consuming applications.
//!
//! # Examples
//!
//! ```rust,no_run
//! use amp_rs::MockApiClient;
//! use amp_rs::model::{Asset, RegisteredUserResponse};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a mock client with default data
//! let client = MockApiClient::new();
//!
//! // Get mock assets
//! let assets = client.get_assets().await?;
//! assert!(!assets.is_empty());
//!
//! // Create a mock client with custom configuration
//! let custom_asset = Asset {
//!     name: "Custom Asset".to_string(),
//!     asset_uuid: "custom-uuid-123".to_string(),
//!     issuer: 1,
//!     asset_id: "asset-id-123".to_string(),
//!     reissuance_token_id: None,
//!     requirements: vec![],
//!     ticker: Some("CUST".to_string()),
//!     precision: 8,
//!     domain: Some("example.com".to_string()),
//!     pubkey: None,
//!     is_registered: true,
//!     is_authorized: true,
//!     is_locked: false,
//!     issuer_authorization_endpoint: None,
//!     transfer_restricted: false,
//! };
//!
//! let custom_user = RegisteredUserResponse {
//!     id: 1,
//!     gaid: Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string()),
//!     is_company: false,
//!     name: "Test User".to_string(),
//!     categories: vec![],
//!     creator: 1,
//! };
//!
//! let client = MockApiClient::new()
//!     .with_asset(custom_asset)
//!     .with_user(custom_user);
//!
//! // Use the configured client
//! let assets = client.get_assets().await?;
//! assert!(assets.iter().any(|a| a.asset_uuid == "custom-uuid-123"));
//! # Ok(())
//! # }
//! ```

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::unused_async,
    clippy::significant_drop_tightening,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::redundant_clone,
    clippy::needless_pass_by_value,
    clippy::uninlined_format_args,
    clippy::unnecessary_cast,
    clippy::map_unwrap_or,
    clippy::assigning_clones,
    clippy::used_underscore_binding
)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use secrecy::ExposeSecret;

use crate::client::{AmpError, Error};
use crate::model::{
    Activity, AddressGaidResponse, Asset, AssetActivityParams, AssetSummary, Assignment, Balance, BroadcastResponse,
    CategoryResponse, CreateAssetAssignmentRequest, Distribution, EditAssetRequest,
    GaidBalanceEntry, IssuanceRequest, IssuanceResponse, Ownership, RegisterAssetResponse,
    RegisteredUserResponse, Reissuance, ValidateGaidResponse,
};

/// Mock API Client that provides the same interface as ApiClient
/// but returns configurable mock responses.
#[derive(Debug, Clone)]
pub struct MockApiClient {
    inner: Arc<MockApiClientInner>,
}

#[derive(Debug)]
struct MockApiClientInner {
    assets: Mutex<HashMap<String, Asset>>,
    users: Mutex<HashMap<i64, RegisteredUserResponse>>,
    user_gaids: Mutex<HashMap<i64, Vec<String>>>,
    categories: Mutex<HashMap<i64, CategoryResponse>>,
    gaid_validations: Mutex<HashMap<String, bool>>,
    gaid_addresses: Mutex<HashMap<String, String>>,
    gaid_balances: Mutex<HashMap<String, Vec<GaidBalanceEntry>>>,
    asset_summaries: Mutex<HashMap<String, AssetSummary>>,
    asset_assignments: Mutex<HashMap<String, Vec<Assignment>>>,
    asset_transactions: Mutex<HashMap<String, Vec<crate::model::AssetTransaction>>>,
    asset_lost_outputs: Mutex<HashMap<String, crate::model::AssetLostOutputs>>,
    distributions: Mutex<HashMap<String, Distribution>>,
    managers: Mutex<HashMap<i64, crate::model::Manager>>,
    next_user_id: AtomicI64,
    next_category_id: AtomicI64,
    next_asset_uuid: AtomicU64,
    next_assignment_id: AtomicI64,
    next_distribution_uuid: AtomicU64,
}

impl Default for MockApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockApiClient {
    /// Creates a new MockApiClient with default mock data.
    ///
    /// # Examples
    /// ```rust
    /// use amp_rs::MockApiClient;
    ///
    /// let client = MockApiClient::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let inner = Arc::new(MockApiClientInner {
            assets: Mutex::new(HashMap::new()),
            users: Mutex::new(HashMap::new()),
            user_gaids: Mutex::new(HashMap::new()),
            categories: Mutex::new(HashMap::new()),
            gaid_validations: Mutex::new(HashMap::new()),
            gaid_addresses: Mutex::new(HashMap::new()),
            gaid_balances: Mutex::new(HashMap::new()),
            asset_summaries: Mutex::new(HashMap::new()),
            asset_assignments: Mutex::new(HashMap::new()),
            asset_transactions: Mutex::new(HashMap::new()),
            asset_lost_outputs: Mutex::new(HashMap::new()),
            distributions: Mutex::new(HashMap::new()),
            managers: Mutex::new(HashMap::new()),
            next_user_id: AtomicI64::new(1),
            next_category_id: AtomicI64::new(1),
            next_asset_uuid: AtomicU64::new(1),
            next_assignment_id: AtomicI64::new(1),
            next_distribution_uuid: AtomicU64::new(1),
        });

        let client = Self { inner };
        client.initialize_default_data();
        client
    }

    /// Initializes default mock data for out-of-box usage
    fn initialize_default_data(&self) {
        // Create a default asset
        let asset_uuid = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let asset = Asset {
            name: "Mock Asset".to_string(),
            asset_uuid: asset_uuid.clone(),
            issuer: 1,
            asset_id: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            reissuance_token_id: None,
            requirements: vec![],
            ticker: Some("MOCK".to_string()),
            precision: 8,
            domain: Some("mock.com".to_string()),
            pubkey: Some(
                "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(),
            ),
            is_registered: true,
            is_authorized: true,
            is_locked: false,
            issuer_authorization_endpoint: None,
            transfer_restricted: false,
        };

        self.inner
            .assets
            .lock()
            .unwrap()
            .insert(asset_uuid.clone(), asset.clone());

        // Create default asset summary
        let summary = AssetSummary {
            asset_id: asset.asset_id.clone(),
            reissuance_token_id: None,
            issued: 1_000_000_000_000,
            reissued: 0,
            assigned: 0,
            distributed: 0,
            burned: 0,
            blacklisted: 0,
            registered_users: 0,
            active_registered_users: 0,
            active_green_subaccounts: 0,
            reissuance_tokens: 0,
        };
        self.inner
            .asset_summaries
            .lock()
            .unwrap()
            .insert(asset_uuid.clone(), summary);

        // Create a default user
        let user_id = 1;
        let user = RegisteredUserResponse {
            id: user_id,
            gaid: Some("GAbYScu6jkWUND2jo3L4KJxyvo55d".to_string()),
            is_company: false,
            name: "Mock User".to_string(),
            categories: vec![],
            creator: 1,
        };
        let user_clone = RegisteredUserResponse {
            id: user.id,
            gaid: user.gaid.clone(),
            is_company: user.is_company,
            name: user.name.clone(),
            categories: user.categories.clone(),
            creator: user.creator,
        };
        self.inner.users.lock().unwrap().insert(user_id, user_clone);

        // Add GAID for user
        if let Some(ref gaid) = user.gaid {
            self.inner
                .user_gaids
                .lock()
                .unwrap()
                .insert(user_id, vec![gaid.clone()]);
            self.inner
                .gaid_validations
                .lock()
                .unwrap()
                .insert(gaid.clone(), true);
            self.inner.gaid_addresses.lock().unwrap().insert(
                gaid.clone(),
                "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
                    .to_string(),
            );
            self.inner.gaid_balances.lock().unwrap().insert(
                gaid.clone(),
                vec![GaidBalanceEntry {
                    asset_uuid: asset_uuid.clone(),
                    asset_id: asset.asset_id.clone(),
                    balance: 100_000_000,
                }],
            );
        }

        // Create a default category
        let category_id = 1;
        let category = CategoryResponse {
            id: category_id,
            name: "Mock Category".to_string(),
            description: Some("A mock category for testing".to_string()),
            registered_users: vec![user_id],
            assets: vec![asset_uuid],
        };
        self.inner
            .categories
            .lock()
            .unwrap()
            .insert(category_id, category);
    }

    /// Builder method to add an asset to the mock client
    #[must_use]
    pub fn with_asset(self, asset: Asset) -> Self {
        let asset_uuid = asset.asset_uuid.clone();
        self.inner
            .assets
            .lock()
            .unwrap()
            .insert(asset_uuid.clone(), asset.clone());
        // Also create a default summary for this asset
        let summary = AssetSummary {
            asset_id: asset.asset_id.clone(),
            reissuance_token_id: asset.reissuance_token_id.clone(),
            issued: 1_000_000_000_000,
            reissued: 0,
            assigned: 0,
            distributed: 0,
            burned: 0,
            blacklisted: 0,
            registered_users: 0,
            active_registered_users: 0,
            active_green_subaccounts: 0,
            reissuance_tokens: asset.reissuance_token_id.as_ref().map_or(0, |_| 100_000),
        };
        self.inner
            .asset_summaries
            .lock()
            .unwrap()
            .insert(asset_uuid, summary);
        self
    }

    /// Builder method to add a user to the mock client
    #[must_use]
    pub fn with_user(self, user: RegisteredUserResponse) -> Self {
        let user_id = user.id;
        let user_clone = RegisteredUserResponse {
            id: user.id,
            gaid: user.gaid.clone(),
            is_company: user.is_company,
            name: user.name.clone(),
            categories: user.categories.clone(),
            creator: user.creator,
        };
        self.inner.users.lock().unwrap().insert(user_id, user_clone);
        if let Some(ref gaid) = user.gaid {
            self.inner
                .user_gaids
                .lock()
                .unwrap()
                .entry(user_id)
                .or_default()
                .push(gaid.clone());
            self.inner
                .gaid_validations
                .lock()
                .unwrap()
                .insert(gaid.clone(), true);
        }
        self
    }

    /// Builder method to set GAID validation status
    #[must_use]
    pub fn with_gaid_validation(self, gaid: &str, is_valid: bool) -> Self {
        self.inner
            .gaid_validations
            .lock()
            .unwrap()
            .insert(gaid.to_string(), is_valid);
        self
    }

    /// Builder method to set GAID address
    #[must_use]
    pub fn with_gaid_address(self, gaid: &str, address: &str) -> Self {
        self.inner
            .gaid_addresses
            .lock()
            .unwrap()
            .insert(gaid.to_string(), address.to_string());
        self
    }

    /// Builder method to set GAID balance
    #[must_use]
    pub fn with_gaid_balance(self, gaid: &str, balance: Vec<GaidBalanceEntry>) -> Self {
        self.inner
            .gaid_balances
            .lock()
            .unwrap()
            .insert(gaid.to_string(), balance);
        self
    }

    /// Builder method to add a category
    #[must_use]
    pub fn with_category(self, category: CategoryResponse) -> Self {
        let category_id = category.id;
        self.inner
            .categories
            .lock()
            .unwrap()
            .insert(category_id, category);
        self
    }

    /// Finalizes the builder and returns the `MockApiClient`
    #[must_use]
    pub const fn build(self) -> Self {
        self
    }

    // Token methods to match ApiClient interface
    /// Gets a mock authentication token
    pub async fn get_token(&self) -> Result<String, Error> {
        Ok("mock_token".to_string())
    }

    /// Gets token info (always None for mock)
    pub async fn get_token_info(&self) -> Result<Option<crate::model::TokenInfo>, Error> {
        Ok(None)
    }

    /// Clears token (no-op for mock)
    pub async fn clear_token(&self) -> Result<(), Error> {
        Ok(())
    }

    /// Force refresh (returns mock token)
    pub async fn force_refresh(&self) -> Result<String, Error> {
        Ok("mock_token".to_string())
    }

    // Asset methods

    /// Gets all assets
    pub async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        let assets = self.inner.assets.lock().unwrap();
        Ok(assets.values().cloned().collect())
    }

    /// Gets a specific asset by UUID
    pub async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        let assets = self.inner.assets.lock().unwrap();
        assets
            .get(asset_uuid)
            .cloned()
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))
    }

    /// Issues a new asset
    pub async fn issue_asset(&self, request: &IssuanceRequest) -> Result<IssuanceResponse, Error> {
        let next_id = self.inner.next_asset_uuid.fetch_add(1, Ordering::SeqCst);
        let asset_uuid = format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            next_id as u32,
            (next_id >> 32) as u16,
            (next_id >> 48) as u16,
            ((next_id >> 16) & 0xffff) as u16,
            next_id as u64
        );

        let asset = Asset {
            name: request.name.clone(),
            asset_uuid: asset_uuid.clone(),
            issuer: 1,
            asset_id: format!("{:064x}", next_id),
            reissuance_token_id: if request.is_reissuable.unwrap_or(false) {
                Some(format!("{:064x}", next_id + 1000))
            } else {
                None
            },
            requirements: vec![],
            ticker: Some(request.ticker.clone()),
            precision: request.precision.unwrap_or(8),
            domain: Some(request.domain.clone()),
            pubkey: Some(request.pubkey.clone()),
            is_registered: false,
            is_authorized: false,
            is_locked: false,
            issuer_authorization_endpoint: None,
            transfer_restricted: request.transfer_restricted.unwrap_or(false),
        };

        // Store the asset
        self.inner
            .assets
            .lock()
            .unwrap()
            .insert(asset_uuid.clone(), asset.clone());

        // Create response
        let response = IssuanceResponse {
            name: request.name.clone(),
            amount: request.amount,
            destination_address: request.destination_address.clone(),
            domain: request.domain.clone(),
            ticker: request.ticker.clone(),
            pubkey: request.pubkey.clone(),
            is_confidential: request.is_confidential.unwrap_or(true),
            is_reissuable: request.is_reissuable.unwrap_or(false),
            reissuance_amount: request.reissuance_amount.unwrap_or(0),
            reissuance_address: request.reissuance_address.clone().unwrap_or_default(),
            asset_id: asset.asset_id.clone(),
            reissuance_token_id: asset.reissuance_token_id.clone(),
            asset_uuid: asset_uuid.clone(),
            txid: format!("{:064x}", next_id + 2000),
            vin: 0,
            asset_vout: 0,
            reissuance_vout: if request.is_reissuable.unwrap_or(false) {
                Some(1)
            } else {
                None
            },
            issuer_authorization_endpoint: None,
            transfer_restricted: request.transfer_restricted.unwrap_or(false),
            issuance_assetblinder: format!("{:064x}", next_id + 3000),
            issuance_tokenblinder: if request.is_reissuable.unwrap_or(false) {
                Some(format!("{:064x}", next_id + 4000))
            } else {
                None
            },
        };

        // Create default summary
        let summary = AssetSummary {
            asset_id: asset.asset_id.clone(),
            reissuance_token_id: asset.reissuance_token_id.clone(),
            issued: request.amount,
            reissued: 0,
            assigned: 0,
            distributed: 0,
            burned: 0,
            blacklisted: 0,
            registered_users: 0,
            active_registered_users: 0,
            active_green_subaccounts: 0,
            reissuance_tokens: asset
                .reissuance_token_id
                .as_ref()
                .map(|_| 100_000)
                .unwrap_or(0),
        };
        self.inner
            .asset_summaries
            .lock()
            .unwrap()
            .insert(asset_uuid, summary);

        Ok(response)
    }

    /// Edits an existing asset
    pub async fn edit_asset(
        &self,
        asset_uuid: &str,
        _request: &EditAssetRequest,
    ) -> Result<Asset, Error> {
        let mut assets = self.inner.assets.lock().unwrap();
        assets
            .get_mut(asset_uuid)
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))
            .map(|asset| {
                asset.issuer_authorization_endpoint =
                    Some("https://example.com/authorize".to_string());
                asset.clone()
            })
    }

    /// Registers an asset
    pub async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error> {
        let mut assets = self.inner.assets.lock().unwrap();
        let asset = assets
            .get_mut(asset_uuid)
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))?;

        asset.is_registered = true;

        Ok(RegisterAssetResponse {
            success: true,
            message: Some("Asset registered successfully".to_string()),
            asset_data: Some(asset.clone()),
        })
    }

    /// Deletes an asset
    pub async fn delete_asset(&self, asset_uuid: &str) -> Result<(), Error> {
        let mut assets = self.inner.assets.lock().unwrap();
        assets
            .remove(asset_uuid)
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))
            .map(|_| ())
    }

    /// Locks an asset
    pub async fn lock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        let mut assets = self.inner.assets.lock().unwrap();
        let asset = assets
            .get_mut(asset_uuid)
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))?;

        asset.is_locked = true;
        Ok(asset.clone())
    }

    /// Unlocks an asset
    pub async fn unlock_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        let mut assets = self.inner.assets.lock().unwrap();
        let asset = assets
            .get_mut(asset_uuid)
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))?;

        asset.is_locked = false;
        Ok(asset.clone())
    }

    /// Gets asset summary
    pub async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        let summaries = self.inner.asset_summaries.lock().unwrap();
        summaries
            .get(asset_uuid)
            .map(|s| AssetSummary {
                asset_id: s.asset_id.clone(),
                reissuance_token_id: s.reissuance_token_id.clone(),
                issued: s.issued,
                reissued: s.reissued,
                assigned: s.assigned,
                distributed: s.distributed,
                burned: s.burned,
                blacklisted: s.blacklisted,
                registered_users: s.registered_users,
                active_registered_users: s.active_registered_users,
                active_green_subaccounts: s.active_green_subaccounts,
                reissuance_tokens: s.reissuance_tokens,
            })
            .ok_or_else(|| Error::RequestFailed(format!("Asset summary not found: {}", asset_uuid)))
    }

    /// Gets asset balance
    pub async fn get_asset_balance(&self, _asset_uuid: &str) -> Result<Balance, Error> {
        Ok(vec![]) // Empty balance by default
    }

    /// Gets asset memo
    pub async fn get_asset_memo(&self, _asset_uuid: &str) -> Result<String, Error> {
        Ok("Mock asset memo".to_string())
    }

    /// Sets asset memo
    pub async fn set_asset_memo(&self, _asset_uuid: &str, _memo: &str) -> Result<(), Error> {
        Ok(())
    }

    // User methods

    /// Gets all registered users
    pub async fn get_registered_users(&self) -> Result<Vec<RegisteredUserResponse>, Error> {
        let users = self.inner.users.lock().unwrap();
        Ok(users
            .values()
            .map(|u| RegisteredUserResponse {
                id: u.id,
                gaid: u.gaid.clone(),
                is_company: u.is_company,
                name: u.name.clone(),
                categories: u.categories.clone(),
                creator: u.creator,
            })
            .collect())
    }

    /// Gets a specific registered user by ID
    pub async fn get_registered_user(&self, user_id: i64) -> Result<RegisteredUserResponse, Error> {
        let users = self.inner.users.lock().unwrap();
        users
            .get(&user_id)
            .map(|u| RegisteredUserResponse {
                id: u.id,
                gaid: u.gaid.clone(),
                is_company: u.is_company,
                name: u.name.clone(),
                categories: u.categories.clone(),
                creator: u.creator,
            })
            .ok_or_else(|| Error::RequestFailed(format!("User not found: {}", user_id)))
    }

    /// Adds a registered user
    pub async fn add_registered_user(
        &self,
        request: &crate::model::RegisteredUserAdd,
    ) -> Result<RegisteredUserResponse, Error> {
        let user_id = self.inner.next_user_id.fetch_add(1, Ordering::SeqCst);
        let user = RegisteredUserResponse {
            id: user_id,
            gaid: request.gaid.clone(),
            is_company: request.is_company,
            name: request.name.clone(),
            categories: vec![],
            creator: 1,
        };

        let user_clone = RegisteredUserResponse {
            id: user.id,
            gaid: user.gaid.clone(),
            is_company: user.is_company,
            name: user.name.clone(),
            categories: user.categories.clone(),
            creator: user.creator,
        };
        self.inner.users.lock().unwrap().insert(user_id, user_clone);
        if let Some(ref gaid) = user.gaid {
            self.inner
                .user_gaids
                .lock()
                .unwrap()
                .entry(user_id)
                .or_default()
                .push(gaid.clone());
        }

        Ok(RegisteredUserResponse {
            id: user.id,
            gaid: user.gaid.clone(),
            is_company: user.is_company,
            name: user.name.clone(),
            categories: user.categories.clone(),
            creator: user.creator,
        })
    }

    /// Deletes a registered user
    pub async fn delete_registered_user(&self, user_id: i64) -> Result<(), Error> {
        let mut users = self.inner.users.lock().unwrap();
        users
            .remove(&user_id)
            .ok_or_else(|| Error::RequestFailed(format!("User not found: {}", user_id)))
            .map(|_| ())
    }

    /// Edits a registered user
    pub async fn edit_registered_user(
        &self,
        user_id: i64,
        request: &crate::model::RegisteredUserEdit,
    ) -> Result<RegisteredUserResponse, Error> {
        let mut users = self.inner.users.lock().unwrap();
        let user = users
            .get_mut(&user_id)
            .ok_or_else(|| Error::RequestFailed(format!("User not found: {}", user_id)))?;

        if let Some(ref name) = request.name {
            user.name = name.clone();
        }

        Ok(RegisteredUserResponse {
            id: user.id,
            gaid: user.gaid.clone(),
            is_company: user.is_company,
            name: user.name.clone(),
            categories: user.categories.clone(),
            creator: user.creator,
        })
    }

    /// Gets GAIDs for a registered user
    pub async fn get_registered_user_gaids(&self, user_id: i64) -> Result<Vec<String>, Error> {
        let user_gaids = self.inner.user_gaids.lock().unwrap();
        Ok(user_gaids.get(&user_id).cloned().unwrap_or_default())
    }

    /// Adds a GAID to a registered user
    pub async fn add_gaid_to_registered_user(
        &self,
        user_id: i64,
        request: &crate::model::GaidRequest,
    ) -> Result<(), Error> {
        let _users = self.inner.users.lock().unwrap();
        if !_users.contains_key(&user_id) {
            return Err(Error::RequestFailed(format!("User not found: {}", user_id)));
        }

        let mut user_gaids = self.inner.user_gaids.lock().unwrap();
        user_gaids
            .entry(user_id)
            .or_default()
            .push(request.gaid.clone());
        self.inner
            .gaid_validations
            .lock()
            .unwrap()
            .insert(request.gaid.clone(), true);
        Ok(())
    }

    // GAID methods

    /// Validates a GAID
    pub async fn validate_gaid(&self, gaid: &str) -> Result<ValidateGaidResponse, Error> {
        let validations = self.inner.gaid_validations.lock().unwrap();
        let is_valid = validations.get(gaid).copied().unwrap_or(false);
        Ok(ValidateGaidResponse {
            is_valid,
            error: if is_valid {
                None
            } else {
                Some("Invalid GAID".to_string())
            },
        })
    }

    /// Gets the address for a GAID
    pub async fn get_gaid_address(&self, gaid: &str) -> Result<AddressGaidResponse, Error> {
        let addresses = self.inner.gaid_addresses.lock().unwrap();
        let address = addresses.get(gaid).cloned().unwrap_or_else(|| {
            "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26"
                .to_string()
        });

        Ok(AddressGaidResponse {
            address,
            error: None,
        })
    }

    /// Gets the balance for a GAID
    pub async fn get_gaid_balance(&self, gaid: &str) -> Result<Balance, Error> {
        let balances = self.inner.gaid_balances.lock().unwrap();
        Ok(balances
            .get(gaid)
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| GaidBalanceEntry {
                        asset_uuid: e.asset_uuid.clone(),
                        asset_id: e.asset_id.clone(),
                        balance: e.balance,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Gets the asset balance for a specific GAID and asset
    pub async fn get_gaid_asset_balance(
        &self,
        gaid: &str,
        asset_uuid: &str,
    ) -> Result<GaidBalanceEntry, Error> {
        let balances = self.inner.gaid_balances.lock().unwrap();
        let balance = balances
            .get(gaid)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|e| e.asset_uuid == asset_uuid)
                    .map(|e| GaidBalanceEntry {
                        asset_uuid: e.asset_uuid.clone(),
                        asset_id: e.asset_id.clone(),
                        balance: e.balance,
                    })
            })
            .ok_or_else(|| {
                Error::RequestFailed(format!(
                    "Balance not found for GAID {} and asset {}",
                    gaid, asset_uuid
                ))
            })?;

        Ok(balance)
    }

    /// Gets the registered user for a GAID
    pub async fn get_gaid_registered_user(
        &self,
        gaid: &str,
    ) -> Result<RegisteredUserResponse, Error> {
        let user_gaids = self.inner.user_gaids.lock().unwrap();
        let users = self.inner.users.lock().unwrap();

        for (user_id, gaids) in user_gaids.iter() {
            if gaids.contains(&gaid.to_string()) {
                return users
                    .get(user_id)
                    .map(|u| RegisteredUserResponse {
                        id: u.id,
                        gaid: u.gaid.clone(),
                        is_company: u.is_company,
                        name: u.name.clone(),
                        categories: u.categories.clone(),
                        creator: u.creator,
                    })
                    .ok_or_else(|| Error::RequestFailed("User not found".to_string()));
            }
        }

        Err(Error::RequestFailed(format!("GAID not found: {}", gaid)))
    }

    // Category methods

    /// Gets all categories
    pub async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        let categories = self.inner.categories.lock().unwrap();
        Ok(categories
            .values()
            .map(|c| CategoryResponse {
                id: c.id,
                name: c.name.clone(),
                description: c.description.clone(),
                registered_users: c.registered_users.clone(),
                assets: c.assets.clone(),
            })
            .collect())
    }

    /// Gets a specific category by ID
    pub async fn get_category(&self, category_id: i64) -> Result<CategoryResponse, Error> {
        let categories = self.inner.categories.lock().unwrap();
        categories
            .get(&category_id)
            .map(|c| CategoryResponse {
                id: c.id,
                name: c.name.clone(),
                description: c.description.clone(),
                registered_users: c.registered_users.clone(),
                assets: c.assets.clone(),
            })
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))
    }

    /// Adds a category
    pub async fn add_category(
        &self,
        request: &crate::model::CategoryAdd,
    ) -> Result<CategoryResponse, Error> {
        let category_id = self.inner.next_category_id.fetch_add(1, Ordering::SeqCst);
        let category = CategoryResponse {
            id: category_id,
            name: request.name.clone(),
            description: request.description.clone(),
            registered_users: vec![],
            assets: vec![],
        };

        let category_clone = CategoryResponse {
            id: category.id,
            name: category.name.clone(),
            description: category.description.clone(),
            registered_users: category.registered_users.clone(),
            assets: category.assets.clone(),
        };
        self.inner
            .categories
            .lock()
            .unwrap()
            .insert(category_id, category_clone);
        Ok(CategoryResponse {
            id: category.id,
            name: category.name.clone(),
            description: category.description.clone(),
            registered_users: category.registered_users.clone(),
            assets: category.assets.clone(),
        })
    }

    /// Edits a category
    pub async fn edit_category(
        &self,
        category_id: i64,
        request: &crate::model::CategoryEdit,
    ) -> Result<CategoryResponse, Error> {
        let mut categories = self.inner.categories.lock().unwrap();
        let category = categories
            .get_mut(&category_id)
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))?;

        if let Some(ref name) = request.name {
            category.name = name.clone();
        }
        if request.description.is_some() {
            category.description = request.description.clone();
        }

        Ok(CategoryResponse {
            id: category.id,
            name: category.name.clone(),
            description: category.description.clone(),
            registered_users: category.registered_users.clone(),
            assets: category.assets.clone(),
        })
    }

    /// Deletes a category
    pub async fn delete_category(&self, category_id: i64) -> Result<(), Error> {
        let mut categories = self.inner.categories.lock().unwrap();
        categories
            .remove(&category_id)
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))
            .map(|_| ())
    }

    /// Adds a registered user to a category
    pub async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<(), Error> {
        let mut categories = self.inner.categories.lock().unwrap();
        let category = categories
            .get_mut(&category_id)
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))?;

        if !category.registered_users.contains(&user_id) {
            category.registered_users.push(user_id);
        }
        Ok(())
    }

    /// Removes a registered user from a category
    pub async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<(), Error> {
        let mut categories = self.inner.categories.lock().unwrap();
        let category = categories
            .get_mut(&category_id)
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))?;

        category.registered_users.retain(|&id| id != user_id);
        Ok(())
    }

    /// Adds an asset to a category
    pub async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        let mut categories = self.inner.categories.lock().unwrap();
        let category = categories
            .get_mut(&category_id)
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))?;

        if !category.assets.contains(&asset_uuid.to_string()) {
            category.assets.push(asset_uuid.to_string());
        }
        Ok(CategoryResponse {
            id: category.id,
            name: category.name.clone(),
            description: category.description.clone(),
            registered_users: category.registered_users.clone(),
            assets: category.assets.clone(),
        })
    }

    /// Removes an asset from a category
    pub async fn remove_asset_from_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        let mut categories = self.inner.categories.lock().unwrap();
        let category = categories
            .get_mut(&category_id)
            .ok_or_else(|| Error::RequestFailed(format!("Category not found: {}", category_id)))?;

        category.assets.retain(|uuid| uuid != asset_uuid);
        Ok(CategoryResponse {
            id: category.id,
            name: category.name.clone(),
            description: category.description.clone(),
            registered_users: category.registered_users.clone(),
            assets: category.assets.clone(),
        })
    }

    // Assignment methods

    /// Gets all assignments for an asset
    pub async fn get_asset_assignments(&self, asset_uuid: &str) -> Result<Vec<Assignment>, Error> {
        let assignments = self.inner.asset_assignments.lock().unwrap();
        Ok(assignments
            .get(asset_uuid)
            .map(|v| {
                v.iter()
                    .map(|a| Assignment {
                        id: a.id,
                        registered_user: a.registered_user,
                        amount: a.amount,
                        receiving_address: a.receiving_address.clone(),
                        distribution_uuid: a.distribution_uuid.clone(),
                        ready_for_distribution: a.ready_for_distribution,
                        vesting_datetime: a.vesting_datetime.clone(),
                        vesting_timestamp: a.vesting_timestamp,
                        has_vested: a.has_vested,
                        is_distributed: a.is_distributed,
                        creator: a.creator,
                        gaid: a.gaid.clone(),
                        investor: a.investor,
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Gets a specific assignment
    pub async fn get_asset_assignment(
        &self,
        asset_uuid: &str,
        assignment_id: &str,
    ) -> Result<Assignment, Error> {
        let assignments = self.inner.asset_assignments.lock().unwrap();
        let assignment_id_num = assignment_id.parse::<i64>().map_err(|_| {
            Error::RequestFailed(format!("Invalid assignment ID: {}", assignment_id))
        })?;

        assignments
            .get(asset_uuid)
            .and_then(|assigns| {
                assigns
                    .iter()
                    .find(|a| a.id == assignment_id_num)
                    .map(|a| Assignment {
                        id: a.id,
                        registered_user: a.registered_user,
                        amount: a.amount,
                        receiving_address: a.receiving_address.clone(),
                        distribution_uuid: a.distribution_uuid.clone(),
                        ready_for_distribution: a.ready_for_distribution,
                        vesting_datetime: a.vesting_datetime.clone(),
                        vesting_timestamp: a.vesting_timestamp,
                        has_vested: a.has_vested,
                        is_distributed: a.is_distributed,
                        creator: a.creator,
                        gaid: a.gaid.clone(),
                        investor: a.investor,
                    })
            })
            .ok_or_else(|| Error::RequestFailed(format!("Assignment not found: {}", assignment_id)))
    }

    /// Creates asset assignments
    pub async fn create_asset_assignments(
        &self,
        asset_uuid: &str,
        requests: &[CreateAssetAssignmentRequest],
    ) -> Result<Vec<Assignment>, Error> {
        // Verify asset exists
        let _asset = self.get_asset(asset_uuid).await?;

        let mut assignments_map = self.inner.asset_assignments.lock().unwrap();
        let assignments = assignments_map.entry(asset_uuid.to_string()).or_default();

        let mut created = Vec::new();
        for request in requests {
            let assignment_id = self.inner.next_assignment_id.fetch_add(1, Ordering::SeqCst);
            let assignment = Assignment {
                id: assignment_id,
                registered_user: request.registered_user,
                amount: request.amount,
                receiving_address: None,
                distribution_uuid: None,
                ready_for_distribution: request.ready_for_distribution,
                vesting_datetime: request.vesting_timestamp.map(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .unwrap_or_default()
                        .to_rfc3339()
                }),
                vesting_timestamp: request.vesting_timestamp,
                has_vested: request
                    .vesting_timestamp
                    .is_none_or(|ts| chrono::Utc::now().timestamp() >= ts),
                is_distributed: false,
                creator: 1,
                gaid: None,
                investor: Some(request.registered_user),
            };
            let assignment_clone = Assignment {
                id: assignment.id,
                registered_user: assignment.registered_user,
                amount: assignment.amount,
                receiving_address: assignment.receiving_address.clone(),
                distribution_uuid: assignment.distribution_uuid.clone(),
                ready_for_distribution: assignment.ready_for_distribution,
                vesting_datetime: assignment.vesting_datetime.clone(),
                vesting_timestamp: assignment.vesting_timestamp,
                has_vested: assignment.has_vested,
                is_distributed: assignment.is_distributed,
                creator: assignment.creator,
                gaid: assignment.gaid.clone(),
                investor: assignment.investor,
            };
            created.push(Assignment {
                id: assignment.id,
                registered_user: assignment.registered_user,
                amount: assignment.amount,
                receiving_address: assignment.receiving_address.clone(),
                distribution_uuid: assignment.distribution_uuid.clone(),
                ready_for_distribution: assignment.ready_for_distribution,
                vesting_datetime: assignment.vesting_datetime.clone(),
                vesting_timestamp: assignment.vesting_timestamp,
                has_vested: assignment.has_vested,
                is_distributed: assignment.is_distributed,
                creator: assignment.creator,
                gaid: assignment.gaid.clone(),
                investor: assignment.investor,
            });
            assignments.push(assignment_clone);
        }

        Ok(created)
    }

    // Distribution methods

    /// Creates a distribution
    pub async fn create_distribution(
        &self,
        asset_uuid: &str,
        assignments: Vec<crate::model::AssetDistributionAssignment>,
    ) -> Result<crate::model::DistributionResponse, AmpError> {
        use crate::model::DistributionResponse;
        use AmpError;

        // Verify asset exists
        let asset = self
            .get_asset(asset_uuid)
            .await
            .map_err(|e| AmpError::api(format!("Asset not found: {}", e)))?;

        if assignments.is_empty() {
            return Err(AmpError::validation("Assignments cannot be empty"));
        }

        let next_id = self
            .inner
            .next_distribution_uuid
            .fetch_add(1, Ordering::SeqCst);
        let distribution_uuid = format!(
            "dist-{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            next_id as u32,
            (next_id >> 32) as u16,
            (next_id >> 48) as u16,
            ((next_id >> 16) & 0xffff) as u16,
            next_id as u64
        );

        let mut map_address_amount = std::collections::HashMap::new();
        let mut map_address_asset = std::collections::HashMap::new();

        for assignment in &assignments {
            map_address_amount.insert(assignment.address.clone(), assignment.amount);
            map_address_asset.insert(assignment.address.clone(), asset.asset_id.clone());
        }

        Ok(DistributionResponse {
            distribution_uuid,
            map_address_amount,
            map_address_asset,
            asset_id: asset.asset_id,
        })
    }

    /// Confirms a distribution
    pub async fn confirm_distribution(
        &self,
        _asset_uuid: &str,
        distribution_uuid: &str,
        _tx_data: crate::model::AmpTxData,
        _change_data: Vec<crate::model::Unspent>,
    ) -> Result<(), AmpError> {
        // Store distribution
        let mut distributions = self.inner.distributions.lock().unwrap();
        // Create a basic distribution record
        use crate::model::{Distribution, DistributionAssignment, Status, Transaction};
        let distribution = Distribution {
            distribution_uuid: distribution_uuid.to_string(),
            distribution_status: Status::Confirmed,
            transactions: vec![Transaction {
                txid: "mock_txid".to_string(),
                transaction_status: Status::Confirmed,
                included_blockheight: 100,
                confirmed_datetime: chrono::Utc::now().to_rfc3339(),
                assignments: vec![DistributionAssignment {
                    registered_user: 1,
                    amount: 100,
                    vout: 0,
                }],
            }],
        };
        distributions.insert(distribution_uuid.to_string(), distribution);
        Ok(())
    }

    /// Gets asset distributions
    pub async fn get_asset_distributions(
        &self,
        _asset_uuid: &str,
    ) -> Result<Vec<Distribution>, AmpError> {
        // For now, return empty list - can be extended to track distributions per asset
        Ok(vec![])
    }

    /// Gets a specific distribution
    pub async fn get_asset_distribution(
        &self,
        _asset_uuid: &str,
        distribution_uuid: &str,
    ) -> Result<Distribution, AmpError> {
        let distributions = self.inner.distributions.lock().unwrap();
        distributions
            .get(distribution_uuid)
            .map(|d| Distribution {
                distribution_uuid: d.distribution_uuid.clone(),
                distribution_status: match d.distribution_status {
                    crate::model::Status::Unconfirmed => crate::model::Status::Unconfirmed,
                    crate::model::Status::Confirmed => crate::model::Status::Confirmed,
                },
                transactions: d
                    .transactions
                    .iter()
                    .map(|t| crate::model::Transaction {
                        txid: t.txid.clone(),
                        transaction_status: match t.transaction_status {
                            crate::model::Status::Unconfirmed => crate::model::Status::Unconfirmed,
                            crate::model::Status::Confirmed => crate::model::Status::Confirmed,
                        },
                        included_blockheight: t.included_blockheight,
                        confirmed_datetime: t.confirmed_datetime.clone(),
                        assignments: t
                            .assignments
                            .iter()
                            .map(|a| crate::model::DistributionAssignment {
                                registered_user: a.registered_user,
                                amount: a.amount,
                                vout: a.vout,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .ok_or_else(|| AmpError::api(format!("Distribution not found: {}", distribution_uuid)))
    }

    // Reissue methods

    /// Creates a reissue request
    pub async fn reissue_request(
        &self,
        asset_uuid: &str,
        request: &crate::model::ReissueRequest,
    ) -> Result<crate::model::ReissueRequestResponse, AmpError> {
        use crate::model::{Outpoint, ReissueRequestResponse};

        let asset = self
            .get_asset(asset_uuid)
            .await
            .map_err(|e| AmpError::api(format!("Asset not found: {}", e)))?;

        if asset.reissuance_token_id.is_none() {
            return Err(AmpError::validation("Asset is not reissuable"));
        }

        Ok(ReissueRequestResponse {
            command: "reissue".to_string(),
            min_supported_client_script_version: 2,
            base_url: "https://amp-test.blockstream.com/api".to_string(),
            asset_uuid: asset_uuid.to_string(),
            asset_id: asset.asset_id.clone(),
            amount: request.amount_to_reissue as f64,
            reissuance_utxos: vec![Outpoint {
                txid: format!("{:064x}", 12345),
                vout: 0,
            }],
        })
    }

    /// Confirms a reissue
    pub async fn reissue_confirm(
        &self,
        asset_uuid: &str,
        _request: &crate::model::ReissueConfirmRequest,
    ) -> Result<crate::model::ReissueResponse, AmpError> {
        use crate::model::ReissueResponse;

        // Update asset summary
        let mut summaries = self.inner.asset_summaries.lock().unwrap();
        if let Some(summary) = summaries.get_mut(asset_uuid) {
            summary.reissued += 1_000_000_000; // Add some reissued amount
        }

        Ok(ReissueResponse {
            txid: format!("{:064x}", 54321),
            vin: 1,
            reissuance_amount: 1_000_000_000,
        })
    }

    /// Gets asset reissuances
    pub async fn get_asset_reissuances(
        &self,
        asset_uuid: &str,
    ) -> Result<Vec<crate::model::Reissuance>, AmpError> {
        use crate::model::Reissuance;

        // Check if asset exists
        let _asset = self
            .get_asset(asset_uuid)
            .await
            .map_err(|e| AmpError::api(format!("Asset not found: {}", e)))?;

        // Check if asset has been reissued by looking at summary
        let summaries = self.inner.asset_summaries.lock().unwrap();
        let has_reissuances = summaries
            .get(asset_uuid)
            .map(|s| s.reissued > 0)
            .unwrap_or(false);

        // If asset has reissuances, return mock data
        if has_reissuances {
            Ok(vec![Reissuance {
                txid: "abc123def456789012345678901234567890123456789012345678901234".to_string(),
                vout: 0,
                destination_address:
                    "lq1qqwxyz1234567890abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqr".to_string(),
                reissuance_amount: 1_000_000_000,
                confirmed_in_block:
                    "block_hash_1234567890abcdef1234567890abcdef1234567890abcdef12345678"
                        .to_string(),
                created: "2024-01-15T10:30:00Z".to_string(),
            }])
        } else {
            // No reissuances yet
            Ok(vec![])
        }
    }

    // Transaction methods

    /// Gets all transactions for an asset.
    ///
    /// Returns a list of transactions associated with the specified asset,
    /// including mock data for transfers, issuances, and other transaction types.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to retrieve transactions for
    /// * `params` - Query parameters for filtering and pagination
    ///
    /// # Returns
    /// A vector of `AssetTransaction` objects
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use amp_rs::MockApiClient;
    /// # use amp_rs::model::AssetTransactionParams;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MockApiClient::new();
    /// let params = AssetTransactionParams::default();
    /// let txs = client.get_asset_transactions("asset-uuid", &params).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_transactions(
        &self,
        asset_uuid: &str,
        params: &crate::model::AssetTransactionParams,
    ) -> Result<Vec<crate::model::AssetTransaction>, Error> {
        use crate::model::{AssetTransaction, AssetTransactionOutput};

        // Check if asset exists
        let asset = self.get_asset(asset_uuid).await?;

        // Get any stored transactions for this asset
        let transactions = self.inner.asset_transactions.lock().unwrap();
        let mut result = transactions.get(asset_uuid).cloned().unwrap_or_default();

        // If no stored transactions, generate some mock data based on asset state
        if result.is_empty() {
            // Create a default issuance transaction
            let issuance_tx = AssetTransaction {
                txid: format!("{:064x}", 1000),
                datetime: "2024-01-01T00:00:00Z".to_string(),
                blockheight: 1,
                is_issuance: true,
                is_reissuance: false,
                is_distribution: false,
                inputs: vec![],
                outputs: vec![AssetTransactionOutput {
                    asset_id: asset.asset_id.clone(),
                    vout: 0,
                    amount: 1_000_000_000_000,
                    asset_blinder: format!("{:064x}", 2000),
                    amount_blinder: format!("{:064x}", 2001),
                    registered_user: None,
                    gaid: None,
                    is_treasury: true,
                    is_spent: false,
                    is_burnt: false,
                }],
                unblinded_url: "https://blockstream.info/liquidtestnet/tx/mock".to_string(),
            };
            result.push(issuance_tx);
        }

        // Apply filtering based on params
        if let Some(height_start) = params.height_start {
            result.retain(|tx| tx.blockheight >= height_start);
        }

        if let Some(height_stop) = params.height_stop {
            result.retain(|tx| tx.blockheight <= height_stop);
        }

        // Apply sorting
        if let Some(ref sortorder) = params.sortorder {
            if sortorder == "desc" {
                result.reverse();
            }
        }

        // Apply pagination
        let start = usize::try_from(params.start.unwrap_or(0)).unwrap_or(0);
        let count = usize::try_from(params.count.unwrap_or(100)).unwrap_or(100);

        if start < result.len() {
            result = result.into_iter().skip(start).take(count).collect();
        } else {
            result = vec![];
        }

        Ok(result)
    }

    /// Gets a specific transaction for an asset by transaction ID.
    ///
    /// Returns detailed information about a specific transaction associated
    /// with the specified asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `txid` - The transaction ID to retrieve
    ///
    /// # Returns
    /// An `AssetTransaction` object with detailed transaction information
    ///
    /// # Errors
    /// Returns an error if the asset or transaction is not found
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use amp_rs::MockApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MockApiClient::new();
    /// let tx = client.get_asset_transaction("asset-uuid", "txid-123").await?;
    /// println!("Transaction type: {}", tx.transaction_type());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_transaction(
        &self,
        asset_uuid: &str,
        txid: &str,
    ) -> Result<crate::model::AssetTransaction, Error> {
        use crate::model::{AssetTransaction, AssetTransactionOutput};

        // Check if asset exists
        let asset = self.get_asset(asset_uuid).await?;

        // Look for the transaction in stored transactions
        let transactions = self.inner.asset_transactions.lock().unwrap();
        if let Some(asset_txs) = transactions.get(asset_uuid) {
            if let Some(tx) = asset_txs.iter().find(|tx| tx.txid == txid) {
                return Ok(tx.clone());
            }
        }

        // If not found, check if it matches the default issuance txid pattern
        let issuance_txid = format!("{:064x}", 1000);
        if txid == issuance_txid {
            return Ok(AssetTransaction {
                txid: issuance_txid,
                datetime: "2024-01-01T00:00:00Z".to_string(),
                blockheight: 1,
                is_issuance: true,
                is_reissuance: false,
                is_distribution: false,
                inputs: vec![],
                outputs: vec![AssetTransactionOutput {
                    asset_id: asset.asset_id,
                    vout: 0,
                    amount: 1_000_000_000_000,
                    asset_blinder: format!("{:064x}", 2000),
                    amount_blinder: format!("{:064x}", 2001),
                    registered_user: None,
                    gaid: None,
                    is_treasury: true,
                    is_spent: false,
                    is_burnt: false,
                }],
                unblinded_url: "https://blockstream.info/liquidtestnet/tx/mock".to_string(),
            });
        }

        Err(Error::RequestFailed(format!(
            "Transaction not found: {} for asset {}",
            txid, asset_uuid
        )))
    }

    /// Builder method to add a transaction to an asset.
    ///
    /// This allows configuring mock transactions for testing purposes.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to add the transaction to
    /// * `transaction` - The transaction to add
    ///
    /// # Returns
    /// Self for method chaining
    #[must_use]
    pub fn with_asset_transaction(
        self,
        asset_uuid: &str,
        transaction: crate::model::AssetTransaction,
    ) -> Self {
        self.inner
            .asset_transactions
            .lock()
            .unwrap()
            .entry(asset_uuid.to_string())
            .or_default()
            .push(transaction);
        self
    }

    /// Gets the lost outputs for a specific asset.
    ///
    /// Returns outputs that cannot be tracked by the AMP API, typically due to
    /// missing blinder information.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset to query
    ///
    /// # Returns
    /// Returns an `AssetLostOutputs` struct with lost outputs and reissuance lost outputs
    ///
    /// # Doc Test Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::MockApiClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MockApiClient::new();
    ///
    /// // By default, returns empty lost outputs
    /// let lost_outputs = client.get_asset_lost_outputs("550e8400-e29b-41d4-a716-446655440000").await?;
    /// assert!(lost_outputs.lost_outputs.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_asset_lost_outputs(
        &self,
        asset_uuid: &str,
    ) -> Result<crate::model::AssetLostOutputs, Error> {
        use crate::model::AssetLostOutputs;

        // Check if asset exists
        let _ = self.get_asset(asset_uuid).await?;

        // Check if lost outputs are configured
        let lost_outputs_map = self.inner.asset_lost_outputs.lock().unwrap();
        if let Some(lost_outputs) = lost_outputs_map.get(asset_uuid) {
            return Ok(lost_outputs.clone());
        }

        // Return empty lost outputs by default
        Ok(AssetLostOutputs {
            lost_outputs: vec![],
            reissuance_lost_outputs: vec![],
        })
    }

    /// Builder method to configure lost outputs for an asset.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `lost_outputs` - The lost outputs configuration
    ///
    /// # Returns
    /// Self for method chaining
    #[must_use]
    pub fn with_asset_lost_outputs(
        self,
        asset_uuid: &str,
        lost_outputs: crate::model::AssetLostOutputs,
    ) -> Self {
        self.inner
            .asset_lost_outputs
            .lock()
            .unwrap()
            .insert(asset_uuid.to_string(), lost_outputs);
        self
    }

    /// Updates blinder keys for a specific asset output.
    ///
    /// This mock implementation verifies the asset exists and the request is valid.
    ///
    /// # Arguments
    /// * `asset_uuid` - The UUID of the asset
    /// * `request` - The blinder update request
    ///
    /// # Doc Test Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::{MockApiClient, UpdateBlindersRequest};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MockApiClient::new();
    ///
    /// let request = UpdateBlindersRequest {
    ///     txid: "abcd1234".to_string(),
    ///     vout: 0,
    ///     asset_blinder: "00112233".to_string(),
    ///     amount_blinder: "44556677".to_string(),
    /// };
    ///
    /// client.update_asset_blinders("550e8400-e29b-41d4-a716-446655440000", &request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_asset_blinders(
        &self,
        asset_uuid: &str,
        _request: &crate::model::UpdateBlindersRequest,
    ) -> Result<(), Error> {
        // Check if asset exists
        let _ = self.get_asset(asset_uuid).await?;

        // In a real implementation, this would update the blinder information
        // For the mock, we just verify the asset exists and return success
        Ok(())
    }

    /// Changes the password for a specific manager.
    ///
    /// # Arguments
    /// * `manager_id` - The ID of the manager
    /// * `password` - The new password
    ///
    /// # Returns
    /// Returns new credentials including username, password, and token
    ///
    /// # Doc Test Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::MockApiClient;
    /// # use amp_rs::model::Manager;
    /// # use secrecy::Secret;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = Manager {
    ///     username: "test_manager".to_string(),
    ///     id: 1,
    ///     is_locked: false,
    ///     assets: vec![],
    /// };
    ///
    /// let client = MockApiClient::new().with_manager(manager);
    ///
    /// let new_password = Secret::new("new_password".to_string());
    /// let response = client.change_manager_password(1, new_password).await?;
    /// assert_eq!(response.username, "test_manager");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn change_manager_password(
        &self,
        manager_id: i64,
        password: secrecy::Secret<String>,
    ) -> Result<crate::model::ChangePasswordResponse, Error> {
        use crate::model::{ChangePasswordResponse, Password};

        // Check if manager exists
        let managers = self.inner.managers.lock().unwrap();
        let manager = managers
            .get(&manager_id)
            .ok_or_else(|| Error::RequestFailed(format!("Manager not found: {}", manager_id)))?;

        // Return new credentials
        Ok(ChangePasswordResponse {
            username: manager.username.clone(),
            password: secrecy::Secret::new(Password(password.expose_secret().clone())),
            token: secrecy::Secret::new("mock-token-12345".to_string()),
        })
    }

    /// Builder method to add a manager for testing.
    ///
    /// # Arguments
    /// * `manager` - The manager to add
    ///
    /// # Returns
    /// Self for method chaining
    #[must_use]
    pub fn with_manager(self, manager: crate::model::Manager) -> Self {
        self.inner
            .managers
            .lock()
            .unwrap()
            .insert(manager.id, manager);
        self
    }

    // Burn methods

    /// Creates a burn request
    pub async fn burn_request(
        &self,
        asset_uuid: &str,
        amount: i64,
    ) -> Result<crate::model::BurnCreate, AmpError> {
        use crate::model::{BurnCreate, Outpoint};

        if amount <= 0 {
            return Err(AmpError::validation("Amount to burn must be positive"));
        }

        let asset = self
            .get_asset(asset_uuid)
            .await
            .map_err(|e| AmpError::api(format!("Asset not found: {}", e)))?;

        Ok(BurnCreate {
            command: "destroyamount".to_string(),
            min_supported_client_script_version: 1,
            base_url: "https://amp-test.blockstream.com/api".to_string(),
            asset_uuid: asset_uuid.to_string(),
            asset_id: asset.asset_id.clone(),
            amount: amount as f64,
            utxos: vec![Outpoint {
                txid: format!("{:064x}", 67890),
                vout: 0,
            }],
        })
    }

    /// Confirms a burn
    pub async fn burn_confirm(
        &self,
        asset_uuid: &str,
        _request: &crate::model::BurnConfirmRequest,
    ) -> Result<crate::model::BurnResponse, AmpError> {
        // Update asset summary
        let mut summaries = self.inner.asset_summaries.lock().unwrap();
        if let Some(summary) = summaries.get_mut(asset_uuid) {
            summary.burned += 100_000; // Add some burned amount
        }

        Ok(crate::model::BurnResponse {
            success: true,
            message: Some("Burn confirmed successfully".to_string()),
        })
    }

    // Broadcast methods

    /// Broadcasts a transaction
    pub async fn broadcast_transaction(&self, tx_hex: &str) -> Result<BroadcastResponse, Error> {
        Ok(BroadcastResponse {
            txid: format!("{:064x}", tx_hex.len() as u64),
            hex: tx_hex.to_string(),
        })
    }

    /// Gets broadcast status
    pub async fn get_broadcast_status(&self, txid: &str) -> Result<BroadcastResponse, Error> {
        Ok(BroadcastResponse {
            txid: txid.to_string(),
            hex: format!("mock_hex_for_{}", txid),
        })
    }

    // Other methods that might be needed

    /// Gets changelog
    pub async fn get_changelog(&self) -> Result<serde_json::Value, Error> {
        Ok(serde_json::json!({
            "0.1.0": {
                "added": ["Initial release"]
            }
        }))
    }

    /// Gets registered user summary
    pub async fn get_registered_user_summary(
        &self,
        _user_id: i64,
        _asset_uuid: &str,
    ) -> Result<crate::model::RegisteredUserSummary, Error> {
        Err(Error::RequestFailed("Not yet implemented".to_string()))
    }

    /// Registers asset as authorized
    pub async fn register_asset_authorized(&self, asset_uuid: &str) -> Result<Asset, Error> {
        let mut assets = self.inner.assets.lock().unwrap();
        let asset = assets
            .get_mut(asset_uuid)
            .ok_or_else(|| Error::RequestFailed(format!("Asset not found: {}", asset_uuid)))?;

        asset.is_authorized = true;
        asset.is_registered = true;
        Ok(asset.clone())
    }

    /// Sets default GAID for registered user
    pub async fn set_default_gaid_for_registered_user(
        &self,
        _user_id: i64,
        _request: &crate::model::GaidRequest,
    ) -> Result<(), Error> {
        Ok(())
    }
}

// ============================================================================
// AmpClient Trait Implementation
// ============================================================================

use crate::client_trait::AmpClient;

#[async_trait::async_trait]
impl AmpClient for MockApiClient {
    async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        self.get_assets().await
    }
    
    async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        self.get_asset(asset_uuid).await
    }
    
    async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error> {
        self.get_asset_ownerships(asset_uuid, height).await
    }
    
    async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error> {
        self.get_asset_activities(asset_uuid, params).await
    }
    
    async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        self.get_asset_summary(asset_uuid).await
    }
    
    async fn get_asset_reissuances(&self, asset_uuid: &str) -> Result<Vec<Reissuance>, Error> {
        self.get_asset_reissuances(asset_uuid).await
            .map_err(|e| Error::RequestFailed(e.to_string()))
    }
    
    async fn get_registered_users(&self) -> Result<Vec<RegisteredUserResponse>, Error> {
        self.get_registered_users().await
    }
    
    async fn get_registered_user(&self, registered_id: i64) -> Result<RegisteredUserResponse, Error> {
        self.get_registered_user(registered_id).await
    }
    
    async fn get_registered_user_gaids(&self, registered_id: i64) -> Result<Vec<String>, Error> {
        self.get_registered_user_gaids(registered_id).await
    }
    
    async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        self.get_categories().await
    }
    
    async fn get_category(&self, registered_id: i64) -> Result<CategoryResponse, Error> {
        self.get_category(registered_id).await
    }
    
    async fn get_gaid_address(&self, gaid: &str) -> Result<AddressGaidResponse, Error> {
        self.get_gaid_address(gaid).await
    }
    
    async fn get_gaid_balance(&self, gaid: &str) -> Result<Vec<GaidBalanceEntry>, Error> {
        self.get_gaid_balance(gaid).await
    }
    
    async fn validate_gaid(&self, gaid: &str) -> Result<ValidateGaidResponse, Error> {
        self.validate_gaid(gaid).await
    }
}
