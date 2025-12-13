//! Trait for AMP API client implementations
//!
//! This trait defines the core methods needed by service layers.
//! Both production ApiClient and test MockApiClient implement this trait.

use async_trait::async_trait;
use crate::{Error, model::*};

/// Trait for AMP API client implementations
/// 
/// This trait defines the core methods needed by service layers.
/// Both production ApiClient and test MockApiClient implement this trait,
/// enabling tests to use MockApiClient directly with service methods.
#[async_trait]
pub trait AmpClient: Send + Sync {
    // Asset methods
    
    /// Get all assets from the AMP API
    async fn get_assets(&self) -> Result<Vec<Asset>, Error>;
    
    /// Get a specific asset by UUID
    async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error>;
    
    /// Get asset ownerships for a specific asset
    async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error>;
    
    /// Get asset activities for a specific asset
    async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error>;
    
    /// Get asset summary for a specific asset
    async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error>;
    
    /// Get asset reissuances for a specific asset
    async fn get_asset_reissuances(&self, asset_uuid: &str) -> Result<Vec<Reissuance>, Error>;
    
    // User methods
    
    /// Get all registered users
    async fn get_registered_users(&self) -> Result<Vec<RegisteredUserResponse>, Error>;
    
    /// Get a specific registered user by ID
    async fn get_registered_user(&self, registered_id: i64) -> Result<RegisteredUserResponse, Error>;
    
    /// Get GAIDs associated with a registered user
    async fn get_registered_user_gaids(&self, registered_id: i64) -> Result<Vec<String>, Error>;
    
    // Category methods
    
    /// Get all categories
    async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error>;
    
    /// Get a specific category by ID
    async fn get_category(&self, registered_id: i64) -> Result<CategoryResponse, Error>;
    
    // GAID methods
    
    /// Validate a GAID
    async fn validate_gaid(&self, gaid: &str) -> Result<ValidateGaidResponse, Error>;
    
    /// Get the address for a specific GAID
    async fn get_gaid_address(&self, gaid: &str) -> Result<AddressGaidResponse, Error>;
    
    /// Get the balance for a specific GAID
    async fn get_gaid_balance(&self, gaid: &str) -> Result<Vec<GaidBalanceEntry>, Error>;
}
