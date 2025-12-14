//! Trait for AMP API client implementations
//!
//! This trait defines the core methods needed by service layers.
//! Both production ApiClient and test MockApiClient implement this trait.

use crate::{model::*, Error};
use async_trait::async_trait;

/// Trait for AMP API client implementations
///
/// This trait defines the core methods needed by service layers.
/// Both production ApiClient and test MockApiClient implement this trait,
/// enabling tests to use MockApiClient directly with service methods.
///
/// Blanket implementations are provided for `Box<T>` and `Arc<T>` where `T: AmpClient + ?Sized`,
/// allowing `Box<dyn AmpClient>` and `Arc<dyn AmpClient>` to be used wherever `AmpClient` is expected.
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
    async fn get_registered_user(
        &self,
        registered_id: i64,
    ) -> Result<RegisteredUserResponse, Error>;

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

    // Write methods (for create/update operations)

    /// Register an asset
    async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error>;

    /// Add a new registered user
    async fn add_registered_user(
        &self,
        new_user: &RegisteredUserAdd,
    ) -> Result<RegisteredUserResponse, Error>;

    /// Edit a registered user
    async fn edit_registered_user(
        &self,
        registered_user_id: i64,
        edit_data: &RegisteredUserEdit,
    ) -> Result<RegisteredUserResponse, Error>;

    /// Add a GAID to a registered user
    async fn add_gaid_to_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error>;

    /// Add a new category
    async fn add_category(&self, new_category: &CategoryAdd) -> Result<CategoryResponse, Error>;

    /// Add a registered user to a category
    async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error>;

    /// Remove a registered user from a category
    async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error>;

    /// Add an asset to a category
    async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error>;
}

/// Blanket implementation of AmpClient for Box<T>
///
/// This allows `Box<dyn AmpClient + Send + Sync>` to be used in generic contexts
/// that expect `&impl AmpClient`, enabling dependency injection of mock clients.
#[async_trait]
impl<T: AmpClient + ?Sized> AmpClient for Box<T> {
    async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        (**self).get_assets().await
    }

    async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        (**self).get_asset(asset_uuid).await
    }

    async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error> {
        (**self).get_asset_ownerships(asset_uuid, height).await
    }

    async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error> {
        (**self).get_asset_activities(asset_uuid, params).await
    }

    async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        (**self).get_asset_summary(asset_uuid).await
    }

    async fn get_asset_reissuances(&self, asset_uuid: &str) -> Result<Vec<Reissuance>, Error> {
        (**self).get_asset_reissuances(asset_uuid).await
    }

    async fn get_registered_users(&self) -> Result<Vec<RegisteredUserResponse>, Error> {
        (**self).get_registered_users().await
    }

    async fn get_registered_user(
        &self,
        registered_id: i64,
    ) -> Result<RegisteredUserResponse, Error> {
        (**self).get_registered_user(registered_id).await
    }

    async fn get_registered_user_gaids(&self, registered_id: i64) -> Result<Vec<String>, Error> {
        (**self).get_registered_user_gaids(registered_id).await
    }

    async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        (**self).get_categories().await
    }

    async fn get_category(&self, registered_id: i64) -> Result<CategoryResponse, Error> {
        (**self).get_category(registered_id).await
    }

    async fn validate_gaid(&self, gaid: &str) -> Result<ValidateGaidResponse, Error> {
        (**self).validate_gaid(gaid).await
    }

    async fn get_gaid_address(&self, gaid: &str) -> Result<AddressGaidResponse, Error> {
        (**self).get_gaid_address(gaid).await
    }

    async fn get_gaid_balance(&self, gaid: &str) -> Result<Vec<GaidBalanceEntry>, Error> {
        (**self).get_gaid_balance(gaid).await
    }

    async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error> {
        (**self).register_asset(asset_uuid).await
    }

    async fn add_registered_user(
        &self,
        new_user: &RegisteredUserAdd,
    ) -> Result<RegisteredUserResponse, Error> {
        (**self).add_registered_user(new_user).await
    }

    async fn edit_registered_user(
        &self,
        registered_user_id: i64,
        edit_data: &RegisteredUserEdit,
    ) -> Result<RegisteredUserResponse, Error> {
        (**self)
            .edit_registered_user(registered_user_id, edit_data)
            .await
    }

    async fn add_gaid_to_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error> {
        (**self)
            .add_gaid_to_registered_user(registered_user_id, gaid)
            .await
    }

    async fn add_category(&self, new_category: &CategoryAdd) -> Result<CategoryResponse, Error> {
        (**self).add_category(new_category).await
    }

    async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        (**self)
            .add_registered_user_to_category(category_id, user_id)
            .await
    }

    async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        (**self)
            .remove_registered_user_from_category(category_id, user_id)
            .await
    }

    async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        (**self)
            .add_asset_to_category(category_id, asset_uuid)
            .await
    }
}

/// Blanket implementation of AmpClient for Arc<T>
///
/// This allows `Arc<dyn AmpClient + Send + Sync>` to be used in generic contexts
/// that expect `&impl AmpClient`. The Arc wrapper enables cheap cloning for
/// spawning background tasks without requiring the underlying client to implement Clone.
#[async_trait]
impl<T: AmpClient + ?Sized> AmpClient for std::sync::Arc<T> {
    async fn get_assets(&self) -> Result<Vec<Asset>, Error> {
        (**self).get_assets().await
    }

    async fn get_asset(&self, asset_uuid: &str) -> Result<Asset, Error> {
        (**self).get_asset(asset_uuid).await
    }

    async fn get_asset_ownerships(
        &self,
        asset_uuid: &str,
        height: Option<i64>,
    ) -> Result<Vec<Ownership>, Error> {
        (**self).get_asset_ownerships(asset_uuid, height).await
    }

    async fn get_asset_activities(
        &self,
        asset_uuid: &str,
        params: &AssetActivityParams,
    ) -> Result<Vec<Activity>, Error> {
        (**self).get_asset_activities(asset_uuid, params).await
    }

    async fn get_asset_summary(&self, asset_uuid: &str) -> Result<AssetSummary, Error> {
        (**self).get_asset_summary(asset_uuid).await
    }

    async fn get_asset_reissuances(&self, asset_uuid: &str) -> Result<Vec<Reissuance>, Error> {
        (**self).get_asset_reissuances(asset_uuid).await
    }

    async fn get_registered_users(&self) -> Result<Vec<RegisteredUserResponse>, Error> {
        (**self).get_registered_users().await
    }

    async fn get_registered_user(
        &self,
        registered_id: i64,
    ) -> Result<RegisteredUserResponse, Error> {
        (**self).get_registered_user(registered_id).await
    }

    async fn get_registered_user_gaids(&self, registered_id: i64) -> Result<Vec<String>, Error> {
        (**self).get_registered_user_gaids(registered_id).await
    }

    async fn get_categories(&self) -> Result<Vec<CategoryResponse>, Error> {
        (**self).get_categories().await
    }

    async fn get_category(&self, registered_id: i64) -> Result<CategoryResponse, Error> {
        (**self).get_category(registered_id).await
    }

    async fn validate_gaid(&self, gaid: &str) -> Result<ValidateGaidResponse, Error> {
        (**self).validate_gaid(gaid).await
    }

    async fn get_gaid_address(&self, gaid: &str) -> Result<AddressGaidResponse, Error> {
        (**self).get_gaid_address(gaid).await
    }

    async fn get_gaid_balance(&self, gaid: &str) -> Result<Vec<GaidBalanceEntry>, Error> {
        (**self).get_gaid_balance(gaid).await
    }

    async fn register_asset(&self, asset_uuid: &str) -> Result<RegisterAssetResponse, Error> {
        (**self).register_asset(asset_uuid).await
    }

    async fn add_registered_user(
        &self,
        new_user: &RegisteredUserAdd,
    ) -> Result<RegisteredUserResponse, Error> {
        (**self).add_registered_user(new_user).await
    }

    async fn edit_registered_user(
        &self,
        registered_user_id: i64,
        edit_data: &RegisteredUserEdit,
    ) -> Result<RegisteredUserResponse, Error> {
        (**self)
            .edit_registered_user(registered_user_id, edit_data)
            .await
    }

    async fn add_gaid_to_registered_user(
        &self,
        registered_user_id: i64,
        gaid: &str,
    ) -> Result<(), Error> {
        (**self)
            .add_gaid_to_registered_user(registered_user_id, gaid)
            .await
    }

    async fn add_category(&self, new_category: &CategoryAdd) -> Result<CategoryResponse, Error> {
        (**self).add_category(new_category).await
    }

    async fn add_registered_user_to_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        (**self)
            .add_registered_user_to_category(category_id, user_id)
            .await
    }

    async fn remove_registered_user_from_category(
        &self,
        category_id: i64,
        user_id: i64,
    ) -> Result<CategoryResponse, Error> {
        (**self)
            .remove_registered_user_from_category(category_id, user_id)
            .await
    }

    async fn add_asset_to_category(
        &self,
        category_id: i64,
        asset_uuid: &str,
    ) -> Result<CategoryResponse, Error> {
        (**self)
            .add_asset_to_category(category_id, asset_uuid)
            .await
    }
}
