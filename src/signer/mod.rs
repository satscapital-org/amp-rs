//! # Signer Module
//!
//! This module provides transaction signing capabilities for Elements/Liquid transactions
//! using various signing backends. The primary implementation uses Blockstream's Liquid
//! Wallet Kit (LWK) for software-based signing with mnemonic phrases.
//!
//! ## ⚠️ SECURITY WARNING ⚠️
//!
//! **TESTNET/REGTEST ONLY**: This implementation is designed exclusively for testnet
//! and regtest environments. It stores mnemonic phrases in plain text JSON files
//! and should NEVER be used in production or mainnet environments.
//!
//! For production use cases, consider:
//! - Hardware wallets (Ledger, Trezor)
//! - Encrypted key storage solutions
//! - Remote signing services with proper security
//! - Hardware Security Modules (HSMs)
//!
//! ## JSON File Format
//!
//! The signer uses `mnemonic.local.json` for persistent storage with the following structure:
//!
//! ```json
//! {
//!   "mnemonic": [
//!     "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
//!     "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
//!     "additional mnemonics as needed for test isolation..."
//!   ]
//! }
//! ```
//!
//! ### File Structure Details:
//! - **Location**: `mnemonic.local.json` in the current working directory
//! - **Format**: JSON with a single `mnemonic` array field
//! - **Content**: Array of BIP39 mnemonic phrases (12, 15, 18, 21, or 24 words)
//! - **Indexing**: Zero-based array indexing for consistent test identification
//! - **Persistence**: Automatically created and updated when new mnemonics are generated
//!
//! ## Usage Examples
//!
//! ### Basic Signer Creation
//!
//! ```rust,no_run
//! use amp_rs::signer::{Signer, LwkSoftwareSigner, SignerError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), SignerError> {
//!     // Create signer from existing mnemonic
//!     let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//!     let signer = LwkSoftwareSigner::new(mnemonic)?;
//!     
//!     // Sign a transaction
//!     let unsigned_tx = "020000000001..."; // Your unsigned transaction hex
//!     let signed_tx = signer.sign_transaction(unsigned_tx).await?;
//!     println!("Signed transaction: {}", signed_tx);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Automatic Mnemonic Generation
//!
//! ```rust,no_run
//! use amp_rs::signer::{LwkSoftwareSigner, SignerError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), SignerError> {
//!     // Generate new signer (loads first mnemonic from file or creates new)
//!     let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
//!     println!("Using mnemonic: {}", mnemonic);
//!     
//!     // Signer is ready to use
//!     assert!(signer.is_testnet());
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Indexed Mnemonic Access for Testing
//!
//! ```rust,no_run
//! use amp_rs::signer::{LwkSoftwareSigner, SignerError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), SignerError> {
//!     // Get specific mnemonic by index (generates new ones if needed)
//!     let (mnemonic_0, signer_0) = LwkSoftwareSigner::generate_new_indexed(0)?;
//!     let (mnemonic_2, signer_2) = LwkSoftwareSigner::generate_new_indexed(2)?;
//!     
//!     // Each signer uses a different mnemonic for test isolation
//!     assert_ne!(mnemonic_0, mnemonic_2);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ### Error Handling
//!
//! ```rust,no_run
//! use amp_rs::signer::{Signer, LwkSoftwareSigner, SignerError};
//!
//! async fn sign_with_error_handling(unsigned_tx: &str) -> Result<String, SignerError> {
//!     let (_, signer) = LwkSoftwareSigner::generate_new()?;
//!     
//!     match signer.sign_transaction(unsigned_tx).await {
//!         Ok(signed_tx) => {
//!             println!("Transaction signed successfully");
//!             Ok(signed_tx)
//!         },
//!         Err(SignerError::HexParse(e)) => {
//!             eprintln!("Invalid hex format: {}", e);
//!             Err(SignerError::HexParse(e))
//!         },
//!         Err(SignerError::InvalidTransaction(msg)) => {
//!             eprintln!("Invalid transaction: {}", msg);
//!             Err(SignerError::InvalidTransaction(msg))
//!         },
//!         Err(SignerError::Lwk(msg)) => {
//!             eprintln!("LWK signing failed: {}", msg);
//!             Err(SignerError::Lwk(msg))
//!         },
//!         Err(e) => {
//!             eprintln!("Unexpected error: {}", e);
//!             Err(e)
//!         }
//!     }
//! }
//! ```

pub mod error;
pub mod lwk;

pub use error::SignerError;
pub use lwk::LwkSoftwareSigner;

use async_trait::async_trait;

/// Trait for transaction signing implementations
///
/// This trait provides a unified interface for signing Elements/Liquid transactions
/// using various signing backends (software signers, hardware wallets, etc.).
///
/// # Usage
///
/// Implementations of this trait should handle the complete signing pipeline:
/// 1. Parse the unsigned transaction hex string
/// 2. Sign the transaction using the appropriate private key(s)
/// 3. Return the signed transaction as a hex string
///
/// # Thread Safety
///
/// All implementations must be thread-safe (Send + Sync) to support concurrent
/// signing operations in async environments.
///
/// # Example
///
/// ```rust,no_run
/// use amp_rs::signer::{Signer, LwkSoftwareSigner};
///
/// async fn sign_example() -> Result<(), Box<dyn std::error::Error>> {
///     let signer = LwkSoftwareSigner::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")?;
///     let unsigned_tx = "020000000001..."; // Unsigned transaction hex
///     let signed_tx = signer.sign_transaction(unsigned_tx).await?;
///     println!("Signed transaction: {}", signed_tx);
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait Signer: Send + Sync {
    /// Sign an unsigned transaction hex string
    ///
    /// # Arguments
    ///
    /// * `unsigned_tx` - Hex-encoded unsigned Elements/Liquid transaction
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok(String)` - Hex-encoded signed transaction on success
    /// - `Err(SignerError)` - Specific error variant describing the failure
    ///
    /// # Errors
    ///
    /// This method can return various `SignerError` variants:
    /// - `SignerError::HexParse` - Invalid hex encoding in input
    /// - `SignerError::InvalidTransaction` - Malformed transaction structure
    /// - `SignerError::Lwk` - Signing operation failed (implementation-specific)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::signer::{Signer, SignerError};
    /// # async fn example(signer: &dyn Signer) -> Result<(), SignerError> {
    /// let unsigned_tx = "020000000001...";
    /// match signer.sign_transaction(unsigned_tx).await {
    ///     Ok(signed_tx) => println!("Transaction signed: {}", signed_tx),
    ///     Err(SignerError::HexParse(_)) => println!("Invalid hex format"),
    ///     Err(e) => println!("Signing failed: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn sign_transaction(&self, unsigned_tx: &str) -> Result<String, SignerError>;

    /// Returns self as Any for downcasting to concrete types
    ///
    /// This method enables downcasting from the trait object to concrete implementations,
    /// allowing access to implementation-specific methods when needed.
    ///
    /// # Returns
    /// Returns a reference to self as `&dyn std::any::Any`
    fn as_any(&self) -> &dyn std::any::Any;
}
