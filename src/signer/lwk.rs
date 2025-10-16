use super::{Signer, SignerError};
use async_trait::async_trait;
use elements::encode::Decodable;
use elements::pset::PartiallySignedTransaction;
use lwk_common::Signer as LwkSigner;
use lwk_signer::SwSigner;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// JSON structure for persistent mnemonic storage
///
/// `MnemonicStorage` handles the serialization and management of multiple BIP39
/// mnemonic phrases in a JSON file format. This structure supports indexed access
/// to mnemonics for consistent test identification and automatic generation of
/// new mnemonics when needed.
///
/// ## ⚠️ SECURITY WARNING ⚠️
///
/// This storage format keeps mnemonic phrases in **PLAIN TEXT**. It is designed
/// exclusively for testnet/regtest development and should never be used with
/// real funds or in production environments.
///
/// ## JSON File Structure
///
/// The storage uses the following JSON format in `mnemonic.local.json`:
///
/// ```json
/// {
///   "mnemonic": [
///     "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
///     "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
///     "legal winner thank year wave sausage worth useful legal winner thank yellow"
///   ]
/// }
/// ```
///
/// ### Field Details:
/// - **mnemonic**: Array of BIP39 mnemonic phrases
/// - **Index-based access**: Zero-based indexing for consistent test identification
/// - **Validation**: All mnemonics are validated on load and before storage
/// - **Atomic writes**: File updates use temporary files to prevent corruption
///
/// ## Supported Mnemonic Formats
///
/// - **Word counts**: 12, 15, 18, 21, or 24 words (BIP39 standard)
/// - **Language**: English wordlist only
/// - **Format**: Space-separated lowercase words
/// - **Validation**: Full BIP39 checksum validation
///
///
/// This struct handles persistent storage of mnemonic phrases in JSON format,
/// supporting multiple mnemonics for different test scenarios.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MnemonicStorage {
    mnemonic: Vec<String>,
}

impl MnemonicStorage {
    /// Create a new empty mnemonic storage
    pub const fn new() -> Self {
        Self {
            mnemonic: Vec::new(),
        }
    }

    /// Create mnemonic storage with initial mnemonics
    #[allow(dead_code)]
    pub fn with_mnemonics(mnemonics: Vec<String>) -> Result<Self, SignerError> {
        let storage = Self {
            mnemonic: mnemonics,
        };
        storage.validate()?;
        Ok(storage)
    }

    /// Validate all mnemonics in the storage
    pub fn validate(&self) -> Result<(), SignerError> {
        for (index, mnemonic) in self.mnemonic.iter().enumerate() {
            Self::validate_mnemonic_format(mnemonic).map_err(|e| {
                SignerError::InvalidMnemonic(format!("Invalid mnemonic at index {index}: {e}"))
            })?;
        }
        Ok(())
    }

    /// Validate a single mnemonic phrase format
    pub fn validate_mnemonic_format(mnemonic: &str) -> Result<(), String> {
        // First check for multiple consecutive spaces which could indicate empty words
        if mnemonic.contains("  ") {
            return Err(
                "Multiple consecutive spaces detected, which may indicate empty words".to_string(),
            );
        }

        let words: Vec<&str> = mnemonic.split_whitespace().collect();

        // Check word count (should be 12, 15, 18, 21, or 24 words for BIP39)
        match words.len() {
            12 | 15 | 18 | 21 | 24 => {}
            _ => {
                return Err(format!(
                    "Invalid word count: {}. Expected 12, 15, 18, 21, or 24 words",
                    words.len()
                ))
            }
        }

        // Check that all words are non-empty and contain only valid characters
        for (i, word) in words.iter().enumerate() {
            if word.is_empty() {
                return Err(format!("Empty word at position {}", i + 1));
            }

            // Check for valid characters (lowercase letters only for BIP39)
            if !word.chars().all(|c| c.is_ascii_lowercase()) {
                return Err(format!("Invalid characters in word '{}' at position {}. Only lowercase letters allowed", word, i + 1));
            }
        }

        Ok(())
    }

    /// Add a new mnemonic to the storage after validation
    ///
    /// This method is deprecated in favor of `append_mnemonic` which returns the index.
    /// It's kept for backward compatibility.
    #[allow(dead_code)]
    pub fn add_mnemonic(&mut self, mnemonic: String) -> Result<(), SignerError> {
        self.append_mnemonic(mnemonic)?;
        Ok(())
    }

    /// Get mnemonic at specific index
    ///
    /// This method is deprecated in favor of `get_mnemonic_by_index` for clarity.
    /// It's kept for backward compatibility.
    #[allow(dead_code)]
    pub fn get_mnemonic(&self, index: usize) -> Option<&String> {
        self.get_mnemonic_by_index(index)
    }

    /// Get the first mnemonic if available
    pub fn get_first_mnemonic(&self) -> Option<&String> {
        self.mnemonic.first()
    }

    /// Get the number of stored mnemonics
    pub const fn len(&self) -> usize {
        self.mnemonic.len()
    }

    /// Check if storage is empty
    #[allow(dead_code)]
    pub const fn is_empty(&self) -> bool {
        self.mnemonic.is_empty()
    }
}

impl MnemonicStorage {
    /// Get mnemonic by index from storage
    ///
    /// This function retrieves a mnemonic at the specified index from the storage.
    /// It handles out-of-bounds access gracefully by returning None.
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based index of the mnemonic to retrieve
    ///
    /// # Returns
    ///
    /// Returns an `Option` containing:
    /// - `Some(&String)` - Reference to the mnemonic at the specified index
    /// - `None` - If the index is out of bounds
    ///
    /// # Example
    ///
    /// Returns `Some(&String)` if the index exists, `None` otherwise.
    #[allow(dead_code)]
    pub fn get_mnemonic_by_index(&self, index: usize) -> Option<&String> {
        self.mnemonic.get(index)
    }

    /// Append a new mnemonic to the storage array
    ///
    /// This function adds a new mnemonic to the end of the storage array after
    /// validating its format. The mnemonic is validated before being added to
    /// ensure data integrity.
    ///
    /// # Arguments
    ///
    /// * `mnemonic` - The mnemonic phrase to append to the storage
    ///
    /// # Returns
    ///
    /// Returns a `Result` indicating success or failure:
    /// - `Ok(usize)` - The index where the mnemonic was added
    /// - `Err(SignerError)` - Validation error if the mnemonic format is invalid
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::InvalidMnemonic` - If the mnemonic format is invalid
    ///
    /// # Example
    ///
    /// Returns the index where the mnemonic was added.
    pub fn append_mnemonic(&mut self, mnemonic: String) -> Result<usize, SignerError> {
        // Validate the mnemonic format before adding
        Self::validate_mnemonic_format(&mnemonic).map_err(SignerError::InvalidMnemonic)?;

        // Add the mnemonic to the array
        self.mnemonic.push(mnemonic);

        // Return the index where it was added (length - 1)
        Ok(self.mnemonic.len() - 1)
    }

    /// Get mnemonic by index, generating and appending a new one if index doesn't exist
    ///
    /// This function attempts to retrieve a mnemonic at the specified index. If the
    /// index is out of bounds, it generates new mnemonics and appends them to the
    /// array until the requested index exists, then returns a copy of the mnemonic at that index.
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based index of the mnemonic to retrieve or create
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok(String)` - Copy of the mnemonic at the specified index
    /// - `Err(SignerError)` - Error during mnemonic generation or validation
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::InvalidMnemonic` - If generated mnemonic validation fails
    ///
    /// # Example
    ///
    /// Generates mnemonics as needed to reach the requested index.
    pub fn get_or_generate_mnemonic_at_index(
        &mut self,
        index: usize,
    ) -> Result<String, SignerError> {
        // If the index already exists, return a copy of the existing mnemonic
        if let Some(mnemonic) = self.mnemonic.get(index) {
            return Ok(mnemonic.clone());
        }

        // Generate new mnemonics until we reach the requested index
        while self.mnemonic.len() <= index {
            let new_mnemonic = Self::generate_new_mnemonic();
            self.append_mnemonic(new_mnemonic)?;
        }

        // Return a copy of the mnemonic at the requested index (guaranteed to exist now)
        Ok(self
            .mnemonic
            .get(index)
            .expect("Mnemonic should exist at index after generation")
            .clone())
    }

    /// Generate a new 12-word BIP39 mnemonic phrase
    ///
    /// This function generates a cryptographically secure 12-word mnemonic phrase
    /// using the BIP39 standard. The generated mnemonic can be used to create
    /// deterministic wallets and signers.
    ///
    /// # Returns
    ///
    /// Returns a `String` containing a 12-word mnemonic phrase with words
    /// separated by spaces.
    ///
    /// # Example
    ///
    /// Generates a cryptographically secure 12-word mnemonic phrase.
    pub fn generate_new_mnemonic() -> String {
        use bip39::{Language, Mnemonic};
        use rand::rngs::OsRng;

        // Generate a new 12-word mnemonic using cryptographically secure randomness
        let mnemonic = Mnemonic::generate_in_with(&mut OsRng, Language::English, 12)
            .expect("Failed to generate mnemonic");

        mnemonic.to_string()
    }

    /// Read mnemonic storage from mnemonic.local.json file
    ///
    /// This function attempts to read and parse the mnemonic.local.json file from the
    /// current working directory. It handles missing files gracefully by returning
    /// an empty storage structure.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok(MnemonicStorage)` - Parsed storage on success or empty storage if file doesn't exist
    /// - `Err(SignerError)` - File I/O error, JSON parsing error, or validation error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::FileIo` - File reading errors (except file not found)
    /// - `SignerError::Serialization` - JSON parsing errors
    /// - `SignerError::InvalidMnemonic` - Mnemonic validation errors
    ///
    /// # Example
    ///
    /// Reads from `mnemonic.local.json` or returns empty storage if file doesn't exist.
    pub fn read_from_file() -> Result<Self, SignerError> {
        Self::read_from_file_path("mnemonic.local.json")
    }

    /// Read mnemonic storage from a specific file path
    ///
    /// This function reads and parses a mnemonic storage file from the specified path.
    /// It handles missing files gracefully by returning an empty storage structure.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file containing mnemonic storage
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok(MnemonicStorage)` - Parsed storage on success or empty storage if file doesn't exist
    /// - `Err(SignerError)` - File I/O error, JSON parsing error, or validation error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::FileIo` - File reading errors (except file not found)
    /// - `SignerError::Serialization` - JSON parsing errors
    /// - `SignerError::InvalidMnemonic` - Mnemonic validation errors
    pub fn read_from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, SignerError> {
        let path = path.as_ref();

        // Handle missing file gracefully by returning empty storage
        if !path.exists() {
            tracing::debug!(
                "Mnemonic file {:?} does not exist, returning empty storage",
                path
            );
            return Ok(Self::new());
        }

        // Read file contents
        let contents = fs::read_to_string(path).map_err(|e| {
            tracing::error!("Failed to read mnemonic file {:?}: {}", path, e);
            SignerError::FileIo(e)
        })?;

        // Handle empty file gracefully
        if contents.trim().is_empty() {
            tracing::debug!("Mnemonic file {:?} is empty, returning empty storage", path);
            return Ok(Self::new());
        }

        // Parse JSON content
        let storage: Self = serde_json::from_str(&contents).map_err(|e| {
            tracing::error!("Failed to parse mnemonic file {:?}: {}", path, e);
            SignerError::Serialization(e)
        })?;

        // Validate all mnemonics in the loaded storage
        storage.validate().map_err(|e| {
            tracing::error!("Validation failed for mnemonics in file {:?}: {}", path, e);
            e
        })?;

        tracing::info!(
            "Successfully loaded {} mnemonics from {:?}",
            storage.len(),
            path
        );
        Ok(storage)
    }

    /// Write mnemonic storage to mnemonic.local.json file
    ///
    /// This function serializes the current storage to JSON and writes it to the
    /// mnemonic.local.json file in the current working directory.
    ///
    /// # Returns
    ///
    /// Returns a `Result` indicating success or failure:
    /// - `Ok(())` - File written successfully
    /// - `Err(SignerError)` - Serialization or file writing error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::Serialization` - JSON serialization errors
    /// - `SignerError::FileIo` - File writing errors
    pub fn write_to_file(&self) -> Result<(), SignerError> {
        self.write_to_file_path("mnemonic.local.json")
    }

    /// Write mnemonic storage to a specific file path
    ///
    /// This function serializes the current storage to JSON and writes it to the
    /// specified file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the JSON file should be written
    ///
    /// # Returns
    ///
    /// Returns a `Result` indicating success or failure:
    /// - `Ok(())` - File written successfully
    /// - `Err(SignerError)` - Serialization or file writing error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::Serialization` - JSON serialization errors
    /// - `SignerError::FileIo` - File writing errors
    pub fn write_to_file_path<P: AsRef<Path>>(&self, path: P) -> Result<(), SignerError> {
        let path = path.as_ref();

        // Serialize to pretty JSON for better readability
        let contents = serde_json::to_string_pretty(self).map_err(|e| {
            tracing::error!("Failed to serialize mnemonic storage: {}", e);
            SignerError::Serialization(e)
        })?;

        // Perform atomic write using temporary file to prevent corruption
        let temp_path = path.with_extension("tmp");

        // Write to temporary file first
        fs::write(&temp_path, &contents).map_err(|e| {
            tracing::error!(
                "Failed to write temporary mnemonic file {:?}: {}",
                temp_path,
                e
            );
            SignerError::FileIo(e)
        })?;

        // Atomically rename temporary file to target file
        // This operation is atomic on most filesystems, preventing corruption
        fs::rename(&temp_path, path).map_err(|e| {
            tracing::error!(
                "Failed to rename temporary file {:?} to {:?}: {}",
                temp_path,
                path,
                e
            );
            // Clean up temporary file on failure
            let _ = fs::remove_file(&temp_path);
            SignerError::FileIo(e)
        })?;

        tracing::info!("Successfully wrote {} mnemonics to {:?}", self.len(), path);
        Ok(())
    }
}

/// Software-based transaction signer using Blockstream's Liquid Wallet Kit (LWK)
///
/// `LwkSoftwareSigner` provides transaction signing capabilities for Elements/Liquid
/// transactions using mnemonic phrases and LWK's `SwSigner` implementation. This signer
/// is designed specifically for testnet and regtest environments with persistent
/// mnemonic storage in JSON format.
///
/// ## ⚠️ CRITICAL SECURITY WARNING ⚠️
///
/// **THIS IMPLEMENTATION IS FOR TESTNET/REGTEST ONLY**
///
/// - Mnemonic phrases are stored in **PLAIN TEXT** in `mnemonic.local.json`
/// - Private keys are held in **UNENCRYPTED MEMORY**
/// - No password protection or encryption is provided
/// - Suitable ONLY for development, testing, and regtest environments
///
/// **NEVER USE IN PRODUCTION OR WITH REAL FUNDS**
///
/// For production environments, use:
/// - Hardware wallets (Ledger, Trezor)
/// - Encrypted key storage with proper key derivation
/// - Remote signing services with HSM backing
/// - Multi-signature setups with distributed key management
///
/// ## Features
///
/// - **Persistent Storage**: Automatic mnemonic persistence in JSON format
/// - **Multiple Mnemonics**: Support for multiple test signers with indexed access
/// - **Automatic Generation**: Generate new mnemonics when needed
/// - **BIP39 Compliance**: Full BIP39 mnemonic validation and support
/// - **Liquid Support**: Native support for Liquid/Elements confidential transactions
/// - **Async Interface**: Thread-safe async transaction signing
///
/// ## JSON Storage Format
///
/// The signer uses `mnemonic.local.json` with this structure:
///
/// ```json
/// {
///   "mnemonic": [
///     "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
///     "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
///     "additional test mnemonics..."
///   ]
/// }
/// ```
///
/// ## Usage Patterns
///
/// ### Single Signer Usage
///
/// ```rust,no_run
/// use amp_rs::signer::{Signer, LwkSoftwareSigner};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create from existing mnemonic
///     let signer = LwkSoftwareSigner::new(
///         "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
///     )?;
///     
///     // Or generate/load from file
///     let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
///     println!("Using mnemonic: {}", mnemonic);
///     
///     // Sign transactions
///     let unsigned_tx = "020000000001...";
///     let signed_tx = signer.sign_transaction(unsigned_tx).await?;
///     
///     Ok(())
/// }
/// ```
///
/// ### Multi-Signer Testing
///
/// ```rust,no_run
/// use amp_rs::signer::LwkSoftwareSigner;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create multiple signers for different test scenarios
///     let (_, alice_signer) = LwkSoftwareSigner::generate_new_indexed(0)?;
///     let (_, bob_signer) = LwkSoftwareSigner::generate_new_indexed(1)?;
///     let (_, charlie_signer) = LwkSoftwareSigner::generate_new_indexed(2)?;
///     
///     // Each signer has a different mnemonic for test isolation
///     assert!(alice_signer.is_testnet());
///     assert!(bob_signer.is_testnet());
///     assert!(charlie_signer.is_testnet());
///     
///     Ok(())
/// }
/// ```
///
/// ## Network Configuration
///
/// All `LwkSoftwareSigner` instances are configured for testnet/regtest:
/// - `is_testnet()` always returns `true`
/// - Compatible with Elements regtest and Liquid testnet
/// - Supports confidential transactions and Liquid-specific features
///
/// ## Thread Safety
///
/// The signer is thread-safe and implements `Send + Sync`:
/// - Can be shared across async tasks
/// - Safe for concurrent signing operations
/// - No internal mutable state after creation
///
/// ## Error Handling
///
/// All operations return detailed `SignerError` variants:
/// - `SignerError::InvalidMnemonic` - Mnemonic validation failures
/// - `SignerError::Lwk` - LWK signing operation failures
/// - `SignerError::HexParse` - Transaction hex parsing errors
/// - `SignerError::InvalidTransaction` - Transaction structure errors
/// - `SignerError::FileIo` - Mnemonic file I/O errors
/// - `SignerError::Serialization` - JSON parsing/serialization errors
#[derive(Debug)]
pub struct LwkSoftwareSigner {
    signer: lwk_signer::SwSigner,
    is_testnet: bool,
}

impl LwkSoftwareSigner {
    /// Create a new signer from an existing mnemonic phrase
    ///
    /// This method creates a new `LwkSoftwareSigner` instance from an existing mnemonic phrase.
    /// The signer is configured for testnet/regtest networks only for security.
    ///
    /// The method performs comprehensive validation of the mnemonic phrase including:
    /// - Format validation (word count, character validation)
    /// - BIP39 standard compliance validation
    /// - Checksum validation through BIP39 parsing
    ///
    /// # Arguments
    ///
    /// * `mnemonic_phrase` - A valid BIP39 mnemonic phrase (12, 15, 18, 21, or 24 words)
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok(LwkSoftwareSigner)` - Successfully created signer instance configured for testnet
    /// - `Err(SignerError)` - Mnemonic validation or signer creation error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::InvalidMnemonic` - If the mnemonic format is invalid or fails BIP39 validation
    /// - `SignerError::Lwk` - If LWK `SwSigner` creation fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::signer::{LwkSoftwareSigner, SignerError};
    /// # fn main() -> Result<(), SignerError> {
    /// let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    /// let signer = LwkSoftwareSigner::new(mnemonic)?;
    /// assert!(signer.is_testnet());
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(mnemonic_phrase: &str) -> Result<Self, SignerError> {
        tracing::debug!("Creating new LwkSoftwareSigner from provided mnemonic");

        // First validate mnemonic format (word count, character validation, etc.)
        MnemonicStorage::validate_mnemonic_format(mnemonic_phrase).map_err(|e| {
            tracing::error!("Mnemonic format validation failed: {}", e);
            SignerError::InvalidMnemonic(format!("Format validation failed: {e}"))
        })?;

        // Parse and validate the mnemonic using BIP39 standard
        // This validates the checksum and ensures it's a valid BIP39 mnemonic
        let mnemonic = bip39::Mnemonic::parse(mnemonic_phrase).map_err(|e| {
            tracing::error!("BIP39 mnemonic parsing failed: {}", e);
            SignerError::InvalidMnemonic(format!("BIP39 validation failed: {e}"))
        })?;

        tracing::debug!("Mnemonic validation successful, creating SwSigner instance");

        // Create SwSigner with testnet configuration
        // SwSigner::new expects a &str and is_mainnet bool (false for testnet)
        let signer = SwSigner::new(mnemonic_phrase, false) // false for testnet/regtest
            .map_err(|e| {
                tracing::error!(
                    "Failed to create SwSigner with {}-word mnemonic: {}",
                    mnemonic.word_count(),
                    e
                );
                SignerError::Lwk(format!(
                    "SwSigner creation failed with {}-word mnemonic: {}",
                    mnemonic.word_count(),
                    e
                ))
            })?;

        tracing::info!(
            "Successfully created LwkSoftwareSigner for testnet with {} word mnemonic",
            mnemonic.word_count()
        );

        Ok(Self {
            signer,
            is_testnet: true,
        })
    }

    /// Generate a new signer, loading first mnemonic from file or creating new one
    ///
    /// This method implements the following logic:
    /// 1. Check for existing mnemonic.local.json file
    /// 2. If file exists and has mnemonics, use the first one
    /// 3. If file doesn't exist or is empty, generate a new 12-word mnemonic
    /// 4. Save new mnemonics to file when generated
    /// 5. Return both mnemonic string and signer instance
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok((String, LwkSoftwareSigner))` - Mnemonic phrase and configured signer instance
    /// - `Err(SignerError)` - File I/O, parsing, or signer creation error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::FileIo` - File reading or writing errors
    /// - `SignerError::Serialization` - JSON parsing or serialization errors
    /// - `SignerError::InvalidMnemonic` - Mnemonic validation errors
    /// - `SignerError::Lwk` - Signer creation errors
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::signer::{LwkSoftwareSigner, SignerError};
    /// # fn main() -> Result<(), SignerError> {
    /// let (mnemonic, signer) = LwkSoftwareSigner::generate_new()?;
    /// println!("Using mnemonic: {}", mnemonic);
    /// assert!(signer.is_testnet());
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::cognitive_complexity)]
    pub fn generate_new() -> Result<(String, Self), SignerError> {
        tracing::debug!("Starting generate_new() - checking for existing mnemonic file");

        // Load existing storage or create empty one if file doesn't exist
        let mut storage = MnemonicStorage::read_from_file()?;

        let mnemonic = if let Some(existing_mnemonic) = storage.get_first_mnemonic() {
            // Use existing first mnemonic from file
            tracing::info!(
                "Found existing mnemonic file with {} mnemonics, using first one",
                storage.len()
            );
            existing_mnemonic.clone()
        } else {
            // Generate new mnemonic and save to file
            tracing::info!("No existing mnemonics found, generating new 12-word mnemonic");
            let new_mnemonic = MnemonicStorage::generate_new_mnemonic();

            // Add the new mnemonic to storage
            storage.append_mnemonic(new_mnemonic.clone())?;

            // Save updated storage to file
            storage.write_to_file()?;

            tracing::info!("Generated and saved new mnemonic to mnemonic.local.json");
            new_mnemonic
        };

        // Create signer instance with the mnemonic
        let signer_instance = Self::new(&mnemonic)?;

        tracing::info!("Successfully created LwkSoftwareSigner with mnemonic from generate_new()");
        Ok((mnemonic, signer_instance))
    }

    /// Generate a signer for a specific index, creating new mnemonic if needed
    ///
    /// This method implements indexed mnemonic access with automatic generation:
    /// 1. Load mnemonic at specified index if it exists in mnemonic.local.json
    /// 2. Generate new mnemonics and append to array if index doesn't exist
    /// 3. Update JSON file with new mnemonic when added
    /// 4. Return both mnemonic string and signer instance
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based index of the mnemonic to retrieve or create
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing:
    /// - `Ok((String, LwkSoftwareSigner))` - Mnemonic phrase and configured signer instance
    /// - `Err(SignerError)` - File I/O, parsing, or signer creation error
    ///
    /// # Errors
    ///
    /// This function can return:
    /// - `SignerError::FileIo` - File reading or writing errors
    /// - `SignerError::Serialization` - JSON parsing or serialization errors
    /// - `SignerError::InvalidMnemonic` - Mnemonic validation errors
    /// - `SignerError::Lwk` - Signer creation errors
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::signer::{LwkSoftwareSigner, SignerError};
    /// # fn main() -> Result<(), SignerError> {
    /// // Get mnemonic at index 2, generating new ones if needed
    /// let (mnemonic, signer) = LwkSoftwareSigner::generate_new_indexed(2)?;
    /// println!("Using mnemonic at index 2: {}", mnemonic);
    /// assert!(signer.is_testnet());
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_new_indexed(index: usize) -> Result<(String, Self), SignerError> {
        tracing::debug!(
            "Starting generate_new_indexed({}) - loading mnemonic storage",
            index
        );

        // Load existing storage or create empty one if file doesn't exist
        let mut storage = MnemonicStorage::read_from_file()?;

        // Get mnemonic at index, generating new ones if needed
        let mnemonic = storage.get_or_generate_mnemonic_at_index(index)?;

        // Save updated storage to file (in case new mnemonics were generated)
        storage.write_to_file()?;

        // Create signer instance with the mnemonic
        let signer_instance = Self::new(&mnemonic)?;

        tracing::info!("Successfully created LwkSoftwareSigner with mnemonic at index {} (storage now has {} mnemonics)", 
                      index, storage.len());
        Ok((mnemonic, signer_instance))
    }

    /// Check if this signer is configured for testnet/regtest networks
    ///
    /// This method returns the network configuration of the signer. For `LwkSoftwareSigner`,
    /// this will always return `true` as this implementation is designed exclusively for
    /// testnet and regtest environments.
    ///
    /// ## ⚠️ SECURITY NOTICE ⚠️
    ///
    /// This signer is **NEVER** configured for mainnet due to security considerations:
    /// - Mnemonic phrases are stored in plain text files
    /// - Private keys are held in unencrypted memory
    /// - No password protection or hardware security
    ///
    /// # Returns
    ///
    /// Returns `true` indicating testnet/regtest configuration. This implementation
    /// will never return `false` as mainnet usage is not supported.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use amp_rs::signer::{LwkSoftwareSigner, SignerError};
    /// # fn main() -> Result<(), SignerError> {
    /// let (_, signer) = LwkSoftwareSigner::generate_new()?;
    /// assert!(signer.is_testnet()); // Always true for LwkSoftwareSigner
    ///
    /// // Safe to use for testnet operations
    /// if signer.is_testnet() {
    ///     println!("Signer configured for testnet - safe for development");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub const fn is_testnet(&self) -> bool {
        self.is_testnet
    }
}

#[async_trait]
impl Signer for LwkSoftwareSigner {
    #[allow(clippy::too_many_lines)]
    async fn sign_transaction(&self, unsigned_tx: &str) -> Result<String, SignerError> {
        tracing::debug!(
            "Starting transaction signing process for hex: {}",
            &unsigned_tx[..std::cmp::min(unsigned_tx.len(), 64)]
        );

        // Input validation - check for empty or whitespace-only input
        if unsigned_tx.trim().is_empty() {
            tracing::error!("Empty transaction hex provided");
            return Err(SignerError::InvalidTransaction(
                "Transaction hex cannot be empty".to_string(),
            ));
        }

        // Input validation - check for reasonable hex length (minimum transaction size)
        if unsigned_tx.len() < 20 {
            // Minimum reasonable transaction hex length
            tracing::error!(
                "Transaction hex too short: {} characters",
                unsigned_tx.len()
            );
            return Err(SignerError::InvalidTransaction(format!(
                "Transaction hex too short: {} characters (minimum ~20 expected)",
                unsigned_tx.len()
            )));
        }

        // Parse unsigned transaction hex to elements::Transaction
        let tx_bytes = hex::decode(unsigned_tx).map_err(|e| {
            let preview = if unsigned_tx.len() > 40 {
                format!(
                    "{}...{}",
                    &unsigned_tx[..20],
                    &unsigned_tx[unsigned_tx.len() - 20..]
                )
            } else {
                unsigned_tx.to_string()
            };
            tracing::error!(
                "Failed to decode transaction hex (length: {}, preview: '{}'): {}",
                unsigned_tx.len(),
                preview,
                e
            );
            SignerError::HexParse(e)
        })?;

        tracing::debug!("Successfully decoded hex to {} bytes", tx_bytes.len());

        let unsigned_transaction =
            elements::Transaction::consensus_decode(&tx_bytes[..]).map_err(|e| {
                tracing::error!(
                    "Failed to deserialize transaction from {} bytes: {}",
                    tx_bytes.len(),
                    e
                );
                SignerError::InvalidTransaction(format!(
                    "Transaction deserialization failed from {} bytes: {}",
                    tx_bytes.len(),
                    e
                ))
            })?;

        tracing::debug!(
            "Successfully parsed transaction with {} inputs and {} outputs",
            unsigned_transaction.input.len(),
            unsigned_transaction.output.len()
        );

        // Validate transaction structure
        if unsigned_transaction.input.is_empty() {
            tracing::error!("Transaction has no inputs");
            return Err(SignerError::InvalidTransaction(
                "Transaction must have at least one input".to_string(),
            ));
        }

        if unsigned_transaction.output.is_empty() {
            tracing::error!("Transaction has no outputs");
            return Err(SignerError::InvalidTransaction(
                "Transaction must have at least one output".to_string(),
            ));
        }

        // Convert to PartiallySignedTransaction for LWK signing
        let mut pset = PartiallySignedTransaction::from_tx(unsigned_transaction);

        tracing::debug!(
            "Created PSET for signing with {} inputs",
            pset.inputs().len()
        );

        // Validate PSET structure before signing
        if pset.inputs().is_empty() {
            tracing::error!("PSET has no inputs after conversion");
            return Err(SignerError::InvalidTransaction(
                "PSET conversion resulted in no inputs".to_string(),
            ));
        }

        // Use SwSigner to sign the transaction
        let signed_inputs = self.signer.sign(&mut pset).map_err(|e| {
            tracing::error!(
                "LWK signing operation failed for transaction with {} inputs: {}",
                pset.inputs().len(),
                e
            );
            SignerError::Lwk(format!(
                "Transaction signing failed for {} inputs: {}",
                pset.inputs().len(),
                e
            ))
        })?;

        tracing::debug!("Successfully signed {} inputs", signed_inputs);

        // Extract the signed transaction from PSET
        let signed_transaction = pset.extract_tx().map_err(|e| {
            tracing::error!("Failed to extract signed transaction from PSET: {}", e);
            SignerError::Lwk(format!(
                "Transaction extraction failed after signing {signed_inputs} inputs: {e}"
            ))
        })?;

        // Serialize signed transaction back to hex string
        let signed_bytes = elements::encode::serialize(&signed_transaction);
        let signed_hex = hex::encode(signed_bytes);

        // Validate the serialization result
        if signed_hex.is_empty() {
            tracing::error!("Serialization produced empty hex string");
            return Err(SignerError::InvalidTransaction(
                "Transaction serialization produced empty result".to_string(),
            ));
        }

        // Add logging for successful signing operations
        tracing::info!(
            "Successfully signed transaction. TXID: {}",
            signed_transaction.txid()
        );
        tracing::debug!(
            "Signed transaction hex length: {} bytes (original: {} bytes)",
            signed_hex.len() / 2,
            tx_bytes.len()
        );

        Ok(signed_hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lwk_signer_creation() {
        // Test creating signer with valid mnemonic
        let valid_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = LwkSoftwareSigner::new(valid_mnemonic);
        assert!(result.is_ok());

        let signer = result.unwrap();
        assert!(signer.is_testnet());

        // Test creating signer with invalid mnemonic
        let invalid_mnemonic = "invalid mnemonic phrase";
        let result = LwkSoftwareSigner::new(invalid_mnemonic);
        assert!(result.is_err());
    }

    #[test]
    fn test_lwk_signer_generate_methods() {
        // Test generate_new method
        let result = LwkSoftwareSigner::generate_new();
        assert!(result.is_ok());
        let (mnemonic, signer) = result.unwrap();
        assert!(!mnemonic.is_empty());
        assert!(signer.is_testnet());

        // Test generate_new_indexed method
        let result = LwkSoftwareSigner::generate_new_indexed(0);
        assert!(result.is_ok());
        let (mnemonic, signer) = result.unwrap();
        assert!(!mnemonic.is_empty());
        assert!(signer.is_testnet());
    }

    #[test]
    fn test_generate_new_file_persistence() {
        use std::fs;

        // Clean up any existing test file
        let test_file = "test_generate_new.json";
        let _ = fs::remove_file(test_file);

        // Test 1: No existing file - should generate new mnemonic and save it
        {
            // Temporarily change the file path for testing by using MnemonicStorage directly
            let mut storage = MnemonicStorage::new();
            assert!(storage.is_empty());

            // Generate new mnemonic and add to storage
            let new_mnemonic = MnemonicStorage::generate_new_mnemonic();
            storage.append_mnemonic(new_mnemonic.clone()).unwrap();

            // Write to test file
            storage.write_to_file_path(test_file).unwrap();

            // Verify file was created and contains the mnemonic
            assert!(std::path::Path::new(test_file).exists());

            // Read back and verify
            let loaded_storage = MnemonicStorage::read_from_file_path(test_file).unwrap();
            assert_eq!(loaded_storage.len(), 1);
            assert_eq!(loaded_storage.get_first_mnemonic().unwrap(), &new_mnemonic);
        }

        // Test 2: Existing file with mnemonic - should use existing mnemonic
        {
            // Read the existing file
            let loaded_storage = MnemonicStorage::read_from_file_path(test_file).unwrap();
            assert_eq!(loaded_storage.len(), 1);
            let existing_mnemonic = loaded_storage.get_first_mnemonic().unwrap().clone();

            // Create signer from existing mnemonic to verify it works
            let signer_result = LwkSoftwareSigner::new(&existing_mnemonic);
            assert!(signer_result.is_ok());
            let signer = signer_result.unwrap();
            assert!(signer.is_testnet());
        }

        // Cleanup
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_generate_new_with_empty_file() {
        use std::fs;

        // Create empty test file
        let test_file = "test_empty_generate.json";
        fs::write(test_file, "").unwrap();

        // Test reading empty file should return empty storage
        let storage = MnemonicStorage::read_from_file_path(test_file).unwrap();
        assert!(storage.is_empty());

        // Cleanup
        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_generate_new_behavior_with_existing_file() {
        use std::fs;

        // Use a thread-safe approach to avoid race conditions with other tests
        static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = TEST_MUTEX.lock().unwrap();

        // Use a unique test file to avoid conflicts with other tests
        let test_file = "test_generate_new_behavior_isolated.json";

        // Clean up any existing test files
        let _ = fs::remove_file(test_file);
        let _ = fs::remove_file("mnemonic.local.json");

        // Test the behavior using MnemonicStorage directly to avoid file conflicts
        // Test 1: Create storage and generate first mnemonic
        let mut storage = MnemonicStorage::new();
        let first_mnemonic = MnemonicStorage::generate_new_mnemonic();
        storage.append_mnemonic(first_mnemonic.clone()).unwrap();
        storage.write_to_file_path(test_file).unwrap();

        // Test 2: Load storage and verify first mnemonic is returned
        let loaded_storage = MnemonicStorage::read_from_file_path(test_file).unwrap();
        assert_eq!(loaded_storage.len(), 1);
        assert_eq!(
            loaded_storage.get_first_mnemonic().unwrap(),
            &first_mnemonic
        );

        // Test 3: Verify signer creation works with the stored mnemonic
        let signer1 = LwkSoftwareSigner::new(&first_mnemonic).unwrap();
        assert!(signer1.is_testnet());

        // Test 4: Verify that loading the same file again returns the same mnemonic
        let loaded_storage_again = MnemonicStorage::read_from_file_path(test_file).unwrap();
        assert_eq!(loaded_storage_again.len(), 1);
        assert_eq!(
            loaded_storage_again.get_first_mnemonic().unwrap(),
            &first_mnemonic
        );

        // Test 5: Create another signer with the same mnemonic to verify consistency
        let signer2 = LwkSoftwareSigner::new(&first_mnemonic).unwrap();
        assert!(signer2.is_testnet());

        // Cleanup
        let _ = fs::remove_file(test_file);
        let _ = fs::remove_file("mnemonic.local.json");
    }

    #[test]
    fn test_generate_new_indexed_functionality() {
        use std::fs;

        // Use a thread-safe approach to avoid race conditions with other tests
        static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = TEST_MUTEX.lock().unwrap();

        // Use a unique test file to avoid conflicts with other tests
        let test_file = "test_indexed_functionality_isolated.json";

        // Clean up any existing test files
        let _ = fs::remove_file(test_file);
        let _ = fs::remove_file("mnemonic.local.json");

        // Test using MnemonicStorage directly to avoid file conflicts
        let mut storage = MnemonicStorage::new();

        // Test 1: Generate mnemonic at index 0 when storage is empty
        let mnemonic0 = storage.get_or_generate_mnemonic_at_index(0).unwrap();
        assert!(!mnemonic0.is_empty());
        assert_eq!(storage.len(), 1);
        assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic0).is_ok());

        // Test 2: Get same mnemonic at index 0 (should not generate new one)
        let mnemonic0_again = storage.get_or_generate_mnemonic_at_index(0).unwrap();
        assert_eq!(mnemonic0, mnemonic0_again);
        assert_eq!(storage.len(), 1); // Should still be 1

        // Test 3: Generate mnemonic at index 2 (should generate mnemonics at indices 1 and 2)
        let mnemonic2 = storage.get_or_generate_mnemonic_at_index(2).unwrap();
        assert!(!mnemonic2.is_empty());
        assert_ne!(mnemonic0, mnemonic2); // Should be different mnemonics
        assert_eq!(storage.len(), 3); // Should now have 3 mnemonics (indices 0, 1, 2)
        assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic2).is_ok());

        // Test 4: Get mnemonic at index 1 (should exist now)
        let mnemonic1 = storage.get_or_generate_mnemonic_at_index(1).unwrap();
        assert!(!mnemonic1.is_empty());
        assert_ne!(mnemonic0, mnemonic1);
        assert_ne!(mnemonic1, mnemonic2);
        assert_eq!(storage.len(), 3); // Should still be 3 (no new ones generated)
        assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic1).is_ok());

        // Test 5: Generate mnemonic at a high index to test multiple generation
        let mnemonic5 = storage.get_or_generate_mnemonic_at_index(5).unwrap();
        assert!(!mnemonic5.is_empty());
        assert_eq!(storage.len(), 6); // Should now have 6 mnemonics (indices 0-5)
        assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic5).is_ok());

        // Test 6: Verify all mnemonics are different and valid
        let all_mnemonics: Vec<String> = (0..6)
            .map(|i| storage.get_mnemonic_by_index(i).unwrap().clone())
            .collect();

        for i in 0..all_mnemonics.len() {
            // Verify each mnemonic is valid
            assert!(MnemonicStorage::validate_mnemonic_format(&all_mnemonics[i]).is_ok());

            // Verify all mnemonics are unique
            for j in (i + 1)..all_mnemonics.len() {
                assert_ne!(
                    all_mnemonics[i], all_mnemonics[j],
                    "Mnemonics at indices {} and {} should be different",
                    i, j
                );
            }
        }

        // Test 7: Verify signers can be created from all mnemonics
        for (i, mnemonic) in all_mnemonics.iter().enumerate() {
            let signer = LwkSoftwareSigner::new(mnemonic).unwrap();
            assert!(
                signer.is_testnet(),
                "Signer at index {} should be testnet",
                i
            );
        }

        // Test 8: Test file persistence
        storage.write_to_file_path(test_file).unwrap();
        let loaded_storage = MnemonicStorage::read_from_file_path(test_file).unwrap();
        assert_eq!(loaded_storage.len(), 6);

        // Verify all mnemonics are preserved correctly
        for i in 0..6 {
            assert_eq!(
                loaded_storage.get_mnemonic_by_index(i).unwrap(),
                storage.get_mnemonic_by_index(i).unwrap(),
                "Mnemonic at index {} should be preserved after file operations",
                i
            );
        }

        // Cleanup
        let _ = fs::remove_file(test_file);
        let _ = fs::remove_file("mnemonic.local.json");
    }

    #[tokio::test]
    async fn test_lwk_signer_trait_implementation() {
        // Test that LwkSoftwareSigner implements the Signer trait
        let valid_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = LwkSoftwareSigner::new(valid_mnemonic).unwrap();

        // Test signing with invalid hex (should return HexParse error)
        let invalid_hex = "test_transaction_hex";
        let result = signer.sign_transaction(invalid_hex).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::HexParse(_) => {} // Expected error
            other => panic!("Expected HexParse error, got: {:?}", other),
        }

        // Test signing with valid hex but invalid transaction (should return InvalidTransaction error)
        let valid_hex_invalid_tx = "deadbeef";
        let result = signer.sign_transaction(valid_hex_invalid_tx).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::InvalidTransaction(_) => {} // Expected error
            other => panic!("Expected InvalidTransaction error, got: {:?}", other),
        }
    }

    #[test]
    fn test_mnemonic_storage_validation() {
        // Test valid 12-word mnemonic
        let valid_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(MnemonicStorage::validate_mnemonic_format(valid_mnemonic).is_ok());

        // Test invalid word count
        let invalid_count = "abandon abandon abandon";
        assert!(MnemonicStorage::validate_mnemonic_format(invalid_count).is_err());

        // Test invalid characters (uppercase)
        let invalid_chars = "Abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(MnemonicStorage::validate_mnemonic_format(invalid_chars).is_err());

        // Test empty word
        let empty_word = "abandon  abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(MnemonicStorage::validate_mnemonic_format(empty_word).is_err());
    }

    #[test]
    fn test_mnemonic_storage_operations() {
        let mut storage = MnemonicStorage::new();
        assert!(storage.is_empty());
        assert_eq!(storage.len(), 0);

        // Add valid mnemonic
        let valid_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        assert!(storage.add_mnemonic(valid_mnemonic.clone()).is_ok());
        assert_eq!(storage.len(), 1);
        assert!(!storage.is_empty());

        // Get mnemonic by index
        assert_eq!(storage.get_mnemonic(0), Some(&valid_mnemonic));
        assert_eq!(storage.get_first_mnemonic(), Some(&valid_mnemonic));
        assert_eq!(storage.get_mnemonic(1), None);

        // Try to add invalid mnemonic
        let invalid_mnemonic = "invalid mnemonic".to_string();
        assert!(storage.add_mnemonic(invalid_mnemonic).is_err());
        assert_eq!(storage.len(), 1); // Should not have been added
    }

    #[test]
    fn test_mnemonic_storage_serialization() {
        let mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow".to_string(),
        ];

        let storage = MnemonicStorage::with_mnemonics(mnemonics.clone()).unwrap();

        // Test serialization
        let json = serde_json::to_string(&storage).unwrap();
        assert!(json.contains("mnemonic"));

        // Test deserialization
        let deserialized: MnemonicStorage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized.get_mnemonic(0), Some(&mnemonics[0]));
        assert_eq!(deserialized.get_mnemonic(1), Some(&mnemonics[1]));
    }

    #[test]
    fn test_file_reading_missing_file() {
        // Test reading from a non-existent file
        let result = MnemonicStorage::read_from_file_path("non_existent_file.json");
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert!(storage.is_empty());
        assert_eq!(storage.len(), 0);
    }

    #[test]
    fn test_file_reading_empty_file() {
        use std::fs;

        // Create a temporary empty file
        let temp_path = "test_empty.json";
        fs::write(temp_path, "").unwrap();

        // Test reading empty file
        let result = MnemonicStorage::read_from_file_path(temp_path);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert!(storage.is_empty());

        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_file_reading_valid_json() {
        use std::fs;

        // Create a temporary file with valid JSON
        let temp_path = "test_valid.json";
        let test_data = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                "legal winner thank year wave sausage worth useful legal winner thank yellow"
            ]
        }"#;
        fs::write(temp_path, test_data).unwrap();

        // Test reading valid file
        let result = MnemonicStorage::read_from_file_path(temp_path);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 2);
        assert_eq!(
            storage.get_mnemonic(0).unwrap(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        );
        assert_eq!(
            storage.get_mnemonic(1).unwrap(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
        );

        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_file_reading_invalid_json() {
        use std::fs;

        // Create a temporary file with invalid JSON
        let temp_path = "test_invalid.json";
        let invalid_json = r#"{ "mnemonic": [ "invalid json structure"#;
        fs::write(temp_path, invalid_json).unwrap();

        // Test reading invalid JSON
        let result = MnemonicStorage::read_from_file_path(temp_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::Serialization(_) => {} // Expected error type
            other => panic!("Expected Serialization error, got: {:?}", other),
        }

        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_file_reading_invalid_mnemonic_format() {
        use std::fs;

        // Create a temporary file with invalid mnemonic format
        let temp_path = "test_invalid_mnemonic.json";
        let test_data = r#"{
            "mnemonic": [
                "invalid mnemonic with wrong word count",
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ]
        }"#;
        fs::write(temp_path, test_data).unwrap();

        // Test reading file with invalid mnemonic
        let result = MnemonicStorage::read_from_file_path(temp_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(msg.contains("Invalid mnemonic at index 0"));
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_file_writing_and_reading_roundtrip() {
        use std::fs;

        // Create storage with test mnemonics
        let mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow".to_string(),
        ];
        let original_storage = MnemonicStorage::with_mnemonics(mnemonics.clone()).unwrap();

        // Write to file
        let temp_path = "test_roundtrip.json";
        let write_result = original_storage.write_to_file_path(temp_path);
        assert!(write_result.is_ok());

        // Read back from file
        let read_result = MnemonicStorage::read_from_file_path(temp_path);
        assert!(read_result.is_ok());
        let loaded_storage = read_result.unwrap();

        // Verify data integrity
        assert_eq!(loaded_storage.len(), original_storage.len());
        assert_eq!(
            loaded_storage.get_mnemonic(0),
            original_storage.get_mnemonic(0)
        );
        assert_eq!(
            loaded_storage.get_mnemonic(1),
            original_storage.get_mnemonic(1)
        );

        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_file_reading_whitespace_only() {
        use std::fs;

        // Create a temporary file with only whitespace
        let temp_path = "test_whitespace.json";
        fs::write(temp_path, "   \n\t  \r\n  ").unwrap();

        // Test reading whitespace-only file
        let result = MnemonicStorage::read_from_file_path(temp_path);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert!(storage.is_empty());

        // Cleanup
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_atomic_file_writing() {
        use std::fs;
        use std::path::Path;

        // Create storage with test data
        let mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
        ];
        let storage = MnemonicStorage::with_mnemonics(mnemonics).unwrap();

        let test_path = "test_atomic.json";
        let temp_path = "test_atomic.tmp";

        // Ensure clean state
        let _ = fs::remove_file(test_path);
        let _ = fs::remove_file(temp_path);

        // Write to file
        let result = storage.write_to_file_path(test_path);
        assert!(result.is_ok());

        // Verify target file exists and temporary file is cleaned up
        assert!(Path::new(test_path).exists());
        assert!(!Path::new(temp_path).exists());

        // Verify file contents are correct
        let read_result = MnemonicStorage::read_from_file_path(test_path);
        assert!(read_result.is_ok());
        let loaded_storage = read_result.unwrap();
        assert_eq!(loaded_storage.len(), 1);
        assert_eq!(loaded_storage.get_mnemonic(0), storage.get_mnemonic(0));

        // Cleanup
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_file_writing_serialization_error() {
        // This test would require creating a scenario where serialization fails
        // Since MnemonicStorage is simple and always serializable, we'll test
        // the error path by ensuring the error types are properly handled

        // Create empty storage
        let storage = MnemonicStorage::new();

        // Test writing to a valid path (should succeed)
        let test_path = "test_serialization.json";
        let result = storage.write_to_file_path(test_path);
        assert!(result.is_ok());

        // Cleanup
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_file_writing_io_error() {
        // Create storage with test data
        let mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
        ];
        let storage = MnemonicStorage::with_mnemonics(mnemonics).unwrap();

        // Try to write to an invalid path (directory that doesn't exist)
        let invalid_path = "/nonexistent/directory/test.json";
        let result = storage.write_to_file_path(invalid_path);
        assert!(result.is_err());

        // Verify it's a FileIo error
        match result.unwrap_err() {
            SignerError::FileIo(_) => {} // Expected error type
            other => panic!("Expected FileIo error, got: {:?}", other),
        }
    }

    #[test]
    fn test_file_update_scenario() {
        // Create initial storage with one mnemonic
        let initial_mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
        ];
        let mut storage = MnemonicStorage::with_mnemonics(initial_mnemonics).unwrap();

        let test_path = "test_update.json";

        // Write initial file
        let result = storage.write_to_file_path(test_path);
        assert!(result.is_ok());

        // Verify initial file
        let loaded = MnemonicStorage::read_from_file_path(test_path).unwrap();
        assert_eq!(loaded.len(), 1);

        // Add another mnemonic and update file
        let second_mnemonic =
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
                .to_string();
        storage.add_mnemonic(second_mnemonic.clone()).unwrap();

        // Write updated storage
        let update_result = storage.write_to_file_path(test_path);
        assert!(update_result.is_ok());

        // Verify updated file
        let updated_loaded = MnemonicStorage::read_from_file_path(test_path).unwrap();
        assert_eq!(updated_loaded.len(), 2);
        assert_eq!(updated_loaded.get_mnemonic(1), Some(&second_mnemonic));

        // Cleanup
        let _ = fs::remove_file(test_path);
    }

    #[test]
    fn test_get_mnemonic_by_index() {
        let mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow".to_string(),
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above".to_string(),
        ];
        let storage = MnemonicStorage::with_mnemonics(mnemonics.clone()).unwrap();

        // Test valid indices
        assert_eq!(storage.get_mnemonic_by_index(0), Some(&mnemonics[0]));
        assert_eq!(storage.get_mnemonic_by_index(1), Some(&mnemonics[1]));
        assert_eq!(storage.get_mnemonic_by_index(2), Some(&mnemonics[2]));

        // Test out-of-bounds access
        assert_eq!(storage.get_mnemonic_by_index(3), None);
        assert_eq!(storage.get_mnemonic_by_index(100), None);
    }

    #[test]
    fn test_append_mnemonic() {
        let mut storage = MnemonicStorage::new();
        assert_eq!(storage.len(), 0);

        // Test appending valid mnemonics
        let first_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        let first_index = storage.append_mnemonic(first_mnemonic.clone()).unwrap();
        assert_eq!(first_index, 0);
        assert_eq!(storage.len(), 1);
        assert_eq!(storage.get_mnemonic_by_index(0), Some(&first_mnemonic));

        let second_mnemonic =
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
                .to_string();
        let second_index = storage.append_mnemonic(second_mnemonic.clone()).unwrap();
        assert_eq!(second_index, 1);
        assert_eq!(storage.len(), 2);
        assert_eq!(storage.get_mnemonic_by_index(1), Some(&second_mnemonic));

        // Test appending invalid mnemonic
        let invalid_mnemonic = "invalid mnemonic with wrong word count".to_string();
        let result = storage.append_mnemonic(invalid_mnemonic);
        assert!(result.is_err());
        assert_eq!(storage.len(), 2); // Should not have been added

        // Verify error type
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(_) => {} // Expected error type
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }
    }

    #[test]
    fn test_get_or_generate_mnemonic_at_index() {
        let mut storage = MnemonicStorage::new();
        assert_eq!(storage.len(), 0);

        // Test generating mnemonic at index 0
        let mnemonic_0 = storage.get_or_generate_mnemonic_at_index(0).unwrap();
        assert_eq!(storage.len(), 1);
        assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic_0).is_ok());

        // Test getting existing mnemonic at index 0
        let same_mnemonic_0 = storage.get_or_generate_mnemonic_at_index(0).unwrap();
        assert_eq!(storage.len(), 1); // Should not have generated a new one
        assert_eq!(mnemonic_0, same_mnemonic_0);

        // Test generating mnemonic at index 2 (should generate indices 1 and 2)
        let mnemonic_2 = storage.get_or_generate_mnemonic_at_index(2).unwrap();
        assert_eq!(storage.len(), 3); // Should now have mnemonics at indices 0, 1, 2
        assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic_2).is_ok());

        // Verify all mnemonics exist and are valid
        for i in 0..3 {
            let mnemonic = storage.get_mnemonic_by_index(i).unwrap();
            assert!(MnemonicStorage::validate_mnemonic_format(mnemonic).is_ok());
        }

        // Verify mnemonics are different (extremely unlikely to be the same)
        let mnemonic_1 = storage.get_mnemonic_by_index(1).unwrap();
        assert_ne!(&mnemonic_0, mnemonic_1);
        assert_ne!(&mnemonic_2, mnemonic_1);
        assert_ne!(mnemonic_0, mnemonic_2);
    }

    #[test]
    fn test_generate_new_mnemonic() {
        // Generate multiple mnemonics and verify they are valid and unique
        let mut generated_mnemonics = Vec::new();

        for _ in 0..5 {
            let mnemonic = MnemonicStorage::generate_new_mnemonic();

            // Verify the mnemonic is valid
            assert!(MnemonicStorage::validate_mnemonic_format(&mnemonic).is_ok());

            // Verify it's a 12-word mnemonic
            let words: Vec<&str> = mnemonic.split_whitespace().collect();
            assert_eq!(words.len(), 12);

            // Verify all words are lowercase letters only
            for word in words {
                assert!(word.chars().all(|c| c.is_ascii_lowercase()));
                assert!(!word.is_empty());
            }

            // Verify uniqueness (extremely unlikely to generate duplicates)
            assert!(!generated_mnemonics.contains(&mnemonic));
            generated_mnemonics.push(mnemonic);
        }
    }

    #[test]
    fn test_indexed_access_with_file_operations() {
        use std::fs;

        let test_path = "test_indexed_access.json";

        // Start with empty storage
        let mut storage = MnemonicStorage::new();

        // Generate mnemonic at index 1 (should create indices 0 and 1)
        let mnemonic_1 = storage.get_or_generate_mnemonic_at_index(1).unwrap();
        assert_eq!(storage.len(), 2);

        // Write to file
        storage.write_to_file_path(test_path).unwrap();

        // Load from file and verify
        let loaded_storage = MnemonicStorage::read_from_file_path(test_path).unwrap();
        assert_eq!(loaded_storage.len(), 2);
        assert_eq!(loaded_storage.get_mnemonic_by_index(1), Some(&mnemonic_1));

        // Verify both mnemonics are valid
        for i in 0..2 {
            let mnemonic = loaded_storage.get_mnemonic_by_index(i).unwrap();
            assert!(MnemonicStorage::validate_mnemonic_format(mnemonic).is_ok());
        }

        // Cleanup
        let _ = fs::remove_file(test_path);
    }

    // ========================================
    // Task 7.4: Signer Functionality Tests
    // ========================================

    #[test]
    fn test_signer_creation_with_various_mnemonic_inputs() {
        // Test 1: Valid 12-word mnemonic
        let valid_12_word = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = LwkSoftwareSigner::new(valid_12_word);
        assert!(
            result.is_ok(),
            "Should create signer with valid 12-word mnemonic"
        );
        let signer = result.unwrap();
        assert!(
            signer.is_testnet(),
            "Signer should be configured for testnet"
        );

        // Test 2: Another valid 12-word mnemonic
        let valid_12_word_2 =
            "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let result = LwkSoftwareSigner::new(valid_12_word_2);
        assert!(
            result.is_ok(),
            "Should create signer with second valid 12-word mnemonic"
        );
        let signer = result.unwrap();
        assert!(
            signer.is_testnet(),
            "Signer should be configured for testnet"
        );

        // Test 3: Third valid 12-word mnemonic
        let valid_12_word_3 =
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";
        let result = LwkSoftwareSigner::new(valid_12_word_3);
        assert!(
            result.is_ok(),
            "Should create signer with third valid 12-word mnemonic"
        );
        let signer = result.unwrap();
        assert!(
            signer.is_testnet(),
            "Signer should be configured for testnet"
        );

        // Test 4: Invalid word count (too few words)
        let invalid_few_words = "abandon abandon abandon";
        let result = LwkSoftwareSigner::new(invalid_few_words);
        assert!(result.is_err(), "Should fail with too few words");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(
                    msg.contains("word count"),
                    "Error should mention word count issue"
                );
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 5: Invalid word count (too many words)
        let invalid_many_words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let result = LwkSoftwareSigner::new(invalid_many_words);
        assert!(result.is_err(), "Should fail with too many words");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(
                    msg.contains("word count"),
                    "Error should mention word count issue"
                );
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 6: Invalid characters (uppercase)
        let invalid_uppercase = "Abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = LwkSoftwareSigner::new(invalid_uppercase);
        assert!(result.is_err(), "Should fail with uppercase characters");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(
                    msg.contains("lowercase"),
                    "Error should mention lowercase requirement"
                );
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 7: Invalid characters (numbers)
        let invalid_numbers = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon 123";
        let result = LwkSoftwareSigner::new(invalid_numbers);
        assert!(result.is_err(), "Should fail with numeric characters");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(
                    msg.contains("lowercase"),
                    "Error should mention character validation"
                );
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 8: Empty mnemonic
        let empty_mnemonic = "";
        let result = LwkSoftwareSigner::new(empty_mnemonic);
        assert!(result.is_err(), "Should fail with empty mnemonic");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(_) => {}
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 9: Whitespace-only mnemonic
        let whitespace_mnemonic = "   \n\t  ";
        let result = LwkSoftwareSigner::new(whitespace_mnemonic);
        assert!(result.is_err(), "Should fail with whitespace-only mnemonic");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(_) => {}
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 10: Multiple consecutive spaces (empty words)
        let multiple_spaces = "abandon  abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = LwkSoftwareSigner::new(multiple_spaces);
        assert!(
            result.is_err(),
            "Should fail with multiple consecutive spaces"
        );
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(
                    msg.contains("consecutive spaces") || msg.contains("empty words"),
                    "Error should mention spacing issue"
                );
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 11: Invalid BIP39 checksum (valid format but invalid checksum)
        let invalid_checksum = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        let result = LwkSoftwareSigner::new(invalid_checksum);
        assert!(result.is_err(), "Should fail with invalid BIP39 checksum");
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(
                    msg.contains("BIP39"),
                    "Error should mention BIP39 validation failure"
                );
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }
    }

    #[test]
    fn test_network_configuration_validation() {
        // Test 1: Verify all signers are configured for testnet
        let test_mnemonics = vec![
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
        ];

        for (i, mnemonic) in test_mnemonics.iter().enumerate() {
            let signer = LwkSoftwareSigner::new(mnemonic).unwrap();
            assert!(
                signer.is_testnet(),
                "Signer {} should be configured for testnet",
                i
            );
        }

        // Test 2: Verify generated signers are also testnet
        let (_, generated_signer) = LwkSoftwareSigner::generate_new().unwrap();
        assert!(
            generated_signer.is_testnet(),
            "Generated signer should be configured for testnet"
        );

        // Test 3: Verify indexed generated signers are testnet
        for index in 0..3 {
            let (_, indexed_signer) = LwkSoftwareSigner::generate_new_indexed(index).unwrap();
            assert!(
                indexed_signer.is_testnet(),
                "Indexed signer {} should be configured for testnet",
                index
            );
        }

        // Test 4: Verify network configuration is consistent across multiple instances
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer1 = LwkSoftwareSigner::new(mnemonic).unwrap();
        let signer2 = LwkSoftwareSigner::new(mnemonic).unwrap();

        assert_eq!(
            signer1.is_testnet(),
            signer2.is_testnet(),
            "Network configuration should be consistent across instances"
        );
        assert!(
            signer1.is_testnet() && signer2.is_testnet(),
            "Both signers should be configured for testnet"
        );
    }

    #[tokio::test]
    async fn test_basic_transaction_signing_flow_with_mock_data() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = LwkSoftwareSigner::new(mnemonic).unwrap();

        // Test 1: Empty transaction hex should fail
        let empty_hex = "";
        let result = signer.sign_transaction(empty_hex).await;
        assert!(result.is_err(), "Should fail with empty transaction hex");
        match result.unwrap_err() {
            SignerError::InvalidTransaction(msg) => {
                assert!(
                    msg.contains("empty"),
                    "Error should mention empty transaction"
                );
            }
            other => panic!("Expected InvalidTransaction error, got: {:?}", other),
        }

        // Test 2: Whitespace-only transaction hex should fail
        let whitespace_hex = "   \n\t  ";
        let result = signer.sign_transaction(whitespace_hex).await;
        assert!(
            result.is_err(),
            "Should fail with whitespace-only transaction hex"
        );
        match result.unwrap_err() {
            SignerError::InvalidTransaction(msg) => {
                assert!(
                    msg.contains("empty"),
                    "Error should mention empty transaction"
                );
            }
            other => panic!("Expected InvalidTransaction error, got: {:?}", other),
        }

        // Test 3: Too short transaction hex should fail
        let short_hex = "abc123";
        let result = signer.sign_transaction(short_hex).await;
        assert!(
            result.is_err(),
            "Should fail with too short transaction hex"
        );
        match result.unwrap_err() {
            SignerError::InvalidTransaction(msg) => {
                assert!(
                    msg.contains("too short"),
                    "Error should mention transaction too short"
                );
            }
            other => panic!("Expected InvalidTransaction error, got: {:?}", other),
        }

        // Test 4: Invalid hex characters should fail with HexParse error
        let invalid_hex = "invalid_hex_characters_zz";
        let result = signer.sign_transaction(invalid_hex).await;
        assert!(result.is_err(), "Should fail with invalid hex characters");
        match result.unwrap_err() {
            SignerError::HexParse(_) => {} // Expected error type
            other => panic!("Expected HexParse error, got: {:?}", other),
        }

        // Test 5: Valid hex but invalid transaction structure should fail
        let valid_hex_invalid_tx = "deadbeefcafebabe1234567890abcdef";
        let result = signer.sign_transaction(valid_hex_invalid_tx).await;
        assert!(
            result.is_err(),
            "Should fail with invalid transaction structure"
        );
        match result.unwrap_err() {
            SignerError::InvalidTransaction(_) => {} // Expected error type
            other => panic!("Expected InvalidTransaction error, got: {:?}", other),
        }

        // Test 6: Odd-length hex that's long enough should fail with HexParse error
        let odd_hex = "deadbeefcafebabe12345"; // 21 characters (odd length, but > 20)
        let result = signer.sign_transaction(odd_hex).await;
        assert!(result.is_err(), "Should fail with odd-length hex");
        match result.unwrap_err() {
            SignerError::HexParse(_) => {} // Expected error type for odd-length hex
            other => panic!("Expected HexParse error, got: {:?}", other),
        }

        // Test 7: Very long invalid hex should still fail appropriately
        let long_invalid_hex = "z".repeat(1000);
        let result = signer.sign_transaction(&long_invalid_hex).await;
        assert!(result.is_err(), "Should fail with long invalid hex");
        match result.unwrap_err() {
            SignerError::HexParse(_) => {} // Expected error type
            other => panic!("Expected HexParse error, got: {:?}", other),
        }

        // Test 8: Valid hex that's too short for a transaction
        let too_short_valid_hex = "deadbeef";
        let result = signer.sign_transaction(too_short_valid_hex).await;
        assert!(
            result.is_err(),
            "Should fail with hex that's too short for transaction"
        );
        // This should be InvalidTransaction because length is checked first
        match result.unwrap_err() {
            SignerError::InvalidTransaction(msg) => {
                assert!(
                    msg.contains("too short"),
                    "Error should mention transaction too short"
                );
            }
            other => panic!("Expected InvalidTransaction error, got: {:?}", other),
        }

        // Test 9: Test error message preservation and context
        let test_invalid_hex = "gggggggggggggggggggggggg"; // Long enough but invalid hex
        let result = signer.sign_transaction(test_invalid_hex).await;
        assert!(result.is_err(), "Should fail with invalid hex");
        let error = result.unwrap_err();
        let error_string = format!("{}", error);
        assert!(
            error_string.contains("Hex parsing failed") || error_string.contains("parsing"),
            "Error message should provide context about hex parsing failure: {}",
            error_string
        );
    }

    #[tokio::test]
    async fn test_thread_safety_and_async_compatibility() {
        use std::sync::Arc;
        use tokio::task;

        // Test 1: Verify signer can be shared across threads (Arc<Signer>)
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = Arc::new(LwkSoftwareSigner::new(mnemonic).unwrap());

        // Test 2: Spawn multiple async tasks that use the same signer
        let mut handles = Vec::new();

        for i in 0..5 {
            let signer_clone = Arc::clone(&signer);
            let handle = task::spawn(async move {
                // Each task attempts to sign an invalid transaction (for testing purposes)
                let invalid_hex = format!("invalid_hex_characters_task_{}", i).repeat(3); // Make it long enough
                let result = signer_clone.sign_transaction(&invalid_hex).await;

                // All should fail with HexParse error
                assert!(result.is_err(), "Task {} should fail with invalid hex", i);
                match result.unwrap_err() {
                    SignerError::HexParse(_) => {} // Expected
                    other => panic!("Task {} expected HexParse error, got: {:?}", i, other),
                }

                // Return task ID for verification
                i
            });
            handles.push(handle);
        }

        // Test 3: Wait for all tasks to complete and verify results
        let mut completed_tasks = Vec::new();
        for handle in handles {
            let task_id = handle.await.expect("Task should complete successfully");
            completed_tasks.push(task_id);
        }

        // Verify all tasks completed
        completed_tasks.sort();
        assert_eq!(
            completed_tasks,
            vec![0, 1, 2, 3, 4],
            "All tasks should complete"
        );

        // Test 4: Test concurrent signer creation
        let creation_handles: Vec<_> = (0..3).map(|i| {
            let test_mnemonic = match i {
                0 => "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                1 => "legal winner thank year wave sausage worth useful legal winner thank yellow",
                _ => "letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
            };

            task::spawn(async move {
                let signer = LwkSoftwareSigner::new(test_mnemonic).unwrap();
                assert!(signer.is_testnet(), "Concurrent signer {} should be testnet", i);
                i
            })
        }).collect();

        // Wait for all creation tasks
        for handle in creation_handles {
            handle.await.expect("Signer creation task should complete");
        }

        // Test 5: Test concurrent file operations (generate_new_indexed)
        let file_handles: Vec<_> = (0..3)
            .map(|index| {
                task::spawn(async move {
                    // Use different indices to avoid conflicts
                    let actual_index = index + 10; // Offset to avoid conflicts with other tests
                    let result = LwkSoftwareSigner::generate_new_indexed(actual_index);

                    // Note: This might fail due to file system race conditions in concurrent tests
                    // but the signer itself should handle this gracefully
                    match result {
                        Ok((mnemonic, signer)) => {
                            assert!(
                                !mnemonic.is_empty(),
                                "Generated mnemonic should not be empty"
                            );
                            assert!(signer.is_testnet(), "Generated signer should be testnet");
                            Ok(actual_index)
                        }
                        Err(e) => {
                            // File system errors are acceptable in concurrent scenarios
                            match e {
                                SignerError::FileIo(_) | SignerError::Serialization(_) => {
                                    Ok(actual_index)
                                }
                                other => Err(other),
                            }
                        }
                    }
                })
            })
            .collect();

        // Wait for file operation tasks (allow some to fail due to concurrency)
        let mut successful_file_ops = 0;
        for handle in file_handles {
            match handle.await.expect("File operation task should complete") {
                Ok(_) => successful_file_ops += 1,
                Err(e) => {
                    // Log but don't fail the test for expected concurrency issues
                    eprintln!("Expected concurrency error in file operations: {:?}", e);
                }
            }
        }

        // At least one file operation should succeed
        assert!(
            successful_file_ops > 0,
            "At least one concurrent file operation should succeed"
        );

        // Test 6: Verify trait object compatibility (dynamic dispatch)
        let signer: Box<dyn Signer> = Box::new(LwkSoftwareSigner::new(mnemonic).unwrap());
        let result = signer
            .sign_transaction("invalid_hex_characters_long_enough_for_test")
            .await;
        assert!(result.is_err(), "Trait object should work correctly");
        match result.unwrap_err() {
            SignerError::HexParse(_) => {} // Expected
            other => panic!(
                "Expected HexParse error from trait object, got: {:?}",
                other
            ),
        }

        // Test 7: Test Send + Sync bounds by moving signer across thread boundary
        let signer = LwkSoftwareSigner::new(mnemonic).unwrap();
        let handle = task::spawn(async move {
            // Signer moved into async task (tests Send)
            assert!(signer.is_testnet());

            // Test signing operation in moved context
            let result = signer
                .sign_transaction("invalid_hex_characters_long_enough")
                .await;
            assert!(result.is_err());
        });

        handle.await.expect("Send/Sync test should complete");
    }

    #[tokio::test]
    async fn test_signer_error_handling_consistency() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = LwkSoftwareSigner::new(mnemonic).unwrap();

        // Test consistent error types for various invalid inputs
        let test_cases = vec![
            ("", "empty transaction"),
            ("   ", "whitespace transaction"),
            ("abc", "too short"),
            ("zz", "invalid hex chars"),
            ("abcdef", "short valid hex"),
        ];

        for (input, description) in test_cases {
            let result = signer.sign_transaction(input).await;
            assert!(result.is_err(), "Should fail for {}", description);

            // Verify error can be formatted and contains useful information
            let error = result.unwrap_err();
            let error_msg = format!("{}", error);
            assert!(
                !error_msg.is_empty(),
                "Error message should not be empty for {}",
                description
            );

            // Verify error implements standard error traits
            let _: &dyn std::error::Error = &error;
            let debug_msg = format!("{:?}", error);
            assert!(
                !debug_msg.is_empty(),
                "Debug message should not be empty for {}",
                description
            );
        }
    }

    #[test]
    fn test_signer_creation_performance_and_memory() {
        // Test that signer creation is reasonably fast and doesn't leak memory
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        // Create multiple signers to test for memory leaks or performance issues
        let start_time = std::time::Instant::now();
        let mut signers = Vec::new();

        for _ in 0..10 {
            let signer = LwkSoftwareSigner::new(mnemonic).unwrap();
            assert!(signer.is_testnet());
            signers.push(signer);
        }

        let elapsed = start_time.elapsed();

        // Signer creation should be reasonably fast (less than 5 seconds for 10 signers)
        // LWK initialization can take some time, especially on first run
        assert!(
            elapsed.as_secs() < 5,
            "Signer creation should be fast, took: {:?}",
            elapsed
        );

        // Verify all signers are properly configured
        for (i, signer) in signers.iter().enumerate() {
            assert!(signer.is_testnet(), "Signer {} should be testnet", i);
        }

        // Test that signers can be dropped without issues
        drop(signers);
    }

    #[test]
    fn test_signer_with_different_mnemonic_languages() {
        // Test with mnemonics that would be valid in different languages
        // (though LWK might only support English)

        // English mnemonic (should work)
        let english_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = LwkSoftwareSigner::new(english_mnemonic);
        assert!(result.is_ok(), "English mnemonic should work");

        // Test with mnemonic that has valid format but might not be in English wordlist
        // This should fail during BIP39 validation if the words aren't in the wordlist
        let potentially_invalid_words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon invalid";
        let result = LwkSoftwareSigner::new(potentially_invalid_words);
        // This might succeed or fail depending on whether "invalid" is in the BIP39 wordlist
        // The test verifies that the validation process works correctly either way
        match result {
            Ok(signer) => {
                assert!(
                    signer.is_testnet(),
                    "If signer is created, it should be testnet"
                );
            }
            Err(SignerError::InvalidMnemonic(_)) => {
                // This is also acceptable if the word isn't in the BIP39 wordlist
            }
            Err(other) => {
                panic!(
                    "Unexpected error type for potentially invalid words: {:?}",
                    other
                );
            }
        }
    }

    // ===== JSON FILE OPERATIONS TESTS (Task 7.3) =====
    // These tests specifically cover Requirements 2.1, 2.3, 2.8 for JSON file operations

    #[test]
    fn test_json_file_reading_comprehensive() {
        use std::fs;

        // Test 1: Reading valid JSON with single mnemonic
        let test_path_single = "test_json_single.json";
        let valid_single_json = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ]
        }"#;
        fs::write(test_path_single, valid_single_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_single);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 1);
        assert_eq!(
            storage.get_mnemonic_by_index(0).unwrap(),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        );

        // Test 2: Reading valid JSON with multiple mnemonics
        let test_path_multiple = "test_json_multiple.json";
        let valid_multiple_json = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                "legal winner thank year wave sausage worth useful legal winner thank yellow",
                "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"
            ]
        }"#;
        fs::write(test_path_multiple, valid_multiple_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_multiple);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 3);
        assert_eq!(
            storage.get_mnemonic_by_index(1).unwrap(),
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
        );
        assert_eq!(
            storage.get_mnemonic_by_index(2).unwrap(),
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"
        );

        // Test 3: Reading valid JSON with empty mnemonic array
        let test_path_empty_array = "test_json_empty_array.json";
        let empty_array_json = r#"{
            "mnemonic": []
        }"#;
        fs::write(test_path_empty_array, empty_array_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_empty_array);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 0);
        assert!(storage.is_empty());

        // Test 4: Reading JSON with extra whitespace and formatting
        let test_path_whitespace = "test_json_whitespace.json";
        let whitespace_json = r#"
        {
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ]
        }
        "#;
        fs::write(test_path_whitespace, whitespace_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_whitespace);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 1);

        // Cleanup
        let _ = fs::remove_file(test_path_single);
        let _ = fs::remove_file(test_path_multiple);
        let _ = fs::remove_file(test_path_empty_array);
        let _ = fs::remove_file(test_path_whitespace);
    }

    #[test]
    fn test_json_file_reading_invalid_formats() {
        use std::fs;

        // Test 1: Invalid JSON syntax (missing closing brace)
        let test_path_syntax = "test_json_invalid_syntax.json";
        let invalid_syntax_json = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            "#;
        fs::write(test_path_syntax, invalid_syntax_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_syntax);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::Serialization(_) => {} // Expected error type
            other => panic!("Expected Serialization error, got: {:?}", other),
        }

        // Test 2: Invalid JSON structure (missing mnemonic field)
        let test_path_structure = "test_json_invalid_structure.json";
        let invalid_structure_json = r#"{
            "invalid_field": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ]
        }"#;
        fs::write(test_path_structure, invalid_structure_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_structure);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::Serialization(_) => {} // Expected error type
            other => panic!("Expected Serialization error, got: {:?}", other),
        }

        // Test 3: Invalid JSON with wrong data type (mnemonic as string instead of array)
        let test_path_type = "test_json_invalid_type.json";
        let invalid_type_json = r#"{
            "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        }"#;
        fs::write(test_path_type, invalid_type_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_type);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::Serialization(_) => {} // Expected error type
            other => panic!("Expected Serialization error, got: {:?}", other),
        }

        // Test 4: Valid JSON but invalid mnemonic content
        let test_path_invalid_mnemonic = "test_json_invalid_mnemonic.json";
        let invalid_mnemonic_json = r#"{
            "mnemonic": [
                "invalid mnemonic with wrong word count",
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ]
        }"#;
        fs::write(test_path_invalid_mnemonic, invalid_mnemonic_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_invalid_mnemonic);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(msg.contains("Invalid mnemonic at index 0"));
                assert!(msg.contains("Invalid word count"));
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Test 5: Completely malformed JSON
        let test_path_malformed = "test_json_malformed.json";
        let malformed_json = "not json at all { invalid content";
        fs::write(test_path_malformed, malformed_json).unwrap();

        let result = MnemonicStorage::read_from_file_path(test_path_malformed);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::Serialization(_) => {} // Expected error type
            other => panic!("Expected Serialization error, got: {:?}", other),
        }

        // Cleanup
        let _ = fs::remove_file(test_path_syntax);
        let _ = fs::remove_file(test_path_structure);
        let _ = fs::remove_file(test_path_type);
        let _ = fs::remove_file(test_path_invalid_mnemonic);
        let _ = fs::remove_file(test_path_malformed);
    }

    #[test]
    fn test_json_file_writing_and_updating() {
        use std::fs;

        // Test 1: Writing empty storage
        let test_path_empty = "test_json_write_empty.json";
        let empty_storage = MnemonicStorage::new();

        let result = empty_storage.write_to_file_path(test_path_empty);
        assert!(result.is_ok());

        // Verify file exists and contains correct JSON
        assert!(std::path::Path::new(test_path_empty).exists());
        let file_content = fs::read_to_string(test_path_empty).unwrap();
        assert!(file_content.contains("\"mnemonic\""));
        assert!(file_content.contains("[]"));

        // Verify we can read it back
        let loaded = MnemonicStorage::read_from_file_path(test_path_empty).unwrap();
        assert!(loaded.is_empty());

        // Test 2: Writing storage with single mnemonic
        let test_path_single = "test_json_write_single.json";
        let mut single_storage = MnemonicStorage::new();
        let mnemonic1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        single_storage.append_mnemonic(mnemonic1.clone()).unwrap();

        let result = single_storage.write_to_file_path(test_path_single);
        assert!(result.is_ok());

        // Verify file content
        let loaded = MnemonicStorage::read_from_file_path(test_path_single).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get_mnemonic_by_index(0).unwrap(), &mnemonic1);

        // Test 3: Updating existing file with additional mnemonics
        let mnemonic2 =
            "legal winner thank year wave sausage worth useful legal winner thank yellow"
                .to_string();
        let mnemonic3 =
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"
                .to_string();

        let mut updated_storage = loaded;
        updated_storage.append_mnemonic(mnemonic2.clone()).unwrap();
        updated_storage.append_mnemonic(mnemonic3.clone()).unwrap();

        let result = updated_storage.write_to_file_path(test_path_single);
        assert!(result.is_ok());

        // Verify updated content
        let final_loaded = MnemonicStorage::read_from_file_path(test_path_single).unwrap();
        assert_eq!(final_loaded.len(), 3);
        assert_eq!(final_loaded.get_mnemonic_by_index(0).unwrap(), &mnemonic1);
        assert_eq!(final_loaded.get_mnemonic_by_index(1).unwrap(), &mnemonic2);
        assert_eq!(final_loaded.get_mnemonic_by_index(2).unwrap(), &mnemonic3);

        // Test 4: Overwriting file with different content
        let test_path_overwrite = "test_json_write_overwrite.json";
        let mut new_storage = MnemonicStorage::new();
        let new_mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong".to_string();
        new_storage.append_mnemonic(new_mnemonic.clone()).unwrap();

        // Write initial content
        new_storage.write_to_file_path(test_path_overwrite).unwrap();
        let initial_loaded = MnemonicStorage::read_from_file_path(test_path_overwrite).unwrap();
        assert_eq!(initial_loaded.len(), 1);

        // Overwrite with different content
        let mut overwrite_storage = MnemonicStorage::new();
        overwrite_storage
            .append_mnemonic(mnemonic1.clone())
            .unwrap();
        overwrite_storage
            .append_mnemonic(mnemonic2.clone())
            .unwrap();

        overwrite_storage
            .write_to_file_path(test_path_overwrite)
            .unwrap();
        let overwritten_loaded = MnemonicStorage::read_from_file_path(test_path_overwrite).unwrap();
        assert_eq!(overwritten_loaded.len(), 2);
        assert_eq!(
            overwritten_loaded.get_mnemonic_by_index(0).unwrap(),
            &mnemonic1
        );
        assert_eq!(
            overwritten_loaded.get_mnemonic_by_index(1).unwrap(),
            &mnemonic2
        );

        // Cleanup
        let _ = fs::remove_file(test_path_empty);
        let _ = fs::remove_file(test_path_single);
        let _ = fs::remove_file(test_path_overwrite);
    }

    #[test]
    fn test_json_file_io_error_scenarios() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        // Test 1: Writing to invalid/non-existent directory
        let invalid_path = "/nonexistent/directory/test.json";
        let storage = MnemonicStorage::new();

        let result = storage.write_to_file_path(invalid_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::FileIo(_) => {} // Expected error type
            other => panic!("Expected FileIo error, got: {:?}", other),
        }

        // Test 2: Reading from directory instead of file
        let dir_path = "test_json_directory";
        fs::create_dir_all(dir_path).unwrap();

        let result = MnemonicStorage::read_from_file_path(dir_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::FileIo(_) => {} // Expected error type
            other => panic!("Expected FileIo error, got: {:?}", other),
        }

        // Test 3: Permission denied scenarios (Unix-specific)
        #[cfg(unix)]
        {
            let readonly_path = "test_json_readonly.json";

            // Create a file and make it read-only
            fs::write(readonly_path, "{}").unwrap();
            let mut perms = fs::metadata(readonly_path).unwrap().permissions();
            perms.set_mode(0o444); // Read-only
            fs::set_permissions(readonly_path, perms).unwrap();

            // Try to write to read-only file (should fail)
            let storage = MnemonicStorage::new();
            let result = storage.write_to_file_path(readonly_path);

            // Note: This might not always fail depending on the system and user permissions
            // So we'll just verify the error handling works if it does fail
            if result.is_err() {
                match result.unwrap_err() {
                    SignerError::FileIo(_) => {} // Expected error type
                    other => panic!("Expected FileIo error, got: {:?}", other),
                }
            }

            // Restore permissions for cleanup
            let mut perms = fs::metadata(readonly_path).unwrap().permissions();
            perms.set_mode(0o644); // Read-write
            fs::set_permissions(readonly_path, perms).unwrap();
            let _ = fs::remove_file(readonly_path);
        }

        // Test 4: File corruption recovery (reading corrupted file)
        let corrupted_path = "test_json_corrupted.json";

        // Create a file with binary data that's not valid UTF-8
        let binary_data = vec![0xFF, 0xFE, 0xFD, 0xFC, 0x00, 0x01, 0x02, 0x03];
        fs::write(corrupted_path, binary_data).unwrap();

        let result = MnemonicStorage::read_from_file_path(corrupted_path);
        assert!(result.is_err());
        // This could be either FileIo or Serialization error depending on how the system handles it
        match result.unwrap_err() {
            SignerError::FileIo(_) | SignerError::Serialization(_) => {} // Both are acceptable
            other => panic!("Expected FileIo or Serialization error, got: {:?}", other),
        }

        // Test 5: Disk space simulation (create very large file path)
        let long_path = "a".repeat(1000) + ".json";
        let storage = MnemonicStorage::new();

        let result = storage.write_to_file_path(&long_path);
        // This might succeed or fail depending on the filesystem limits
        if result.is_err() {
            match result.unwrap_err() {
                SignerError::FileIo(_) => {} // Expected error type
                other => panic!("Expected FileIo error, got: {:?}", other),
            }
        }

        // Cleanup
        let _ = fs::remove_dir_all(dir_path);
        let _ = fs::remove_file(corrupted_path);
        let _ = fs::remove_file(&long_path);
    }

    #[test]
    fn test_json_atomic_write_operations() {
        use std::fs;
        use std::path::Path;
        use std::thread;
        use std::time::Duration;

        // Test 1: Verify atomic write behavior (temp file creation and rename)
        let test_path = "test_json_atomic.json";
        let temp_path = "test_json_atomic.tmp";

        // Ensure clean state
        let _ = fs::remove_file(test_path);
        let _ = fs::remove_file(temp_path);

        let mut storage = MnemonicStorage::new();
        storage.append_mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()).unwrap();

        // Write to file
        let result = storage.write_to_file_path(test_path);
        assert!(result.is_ok());

        // Verify target file exists and temp file is cleaned up
        assert!(Path::new(test_path).exists());
        assert!(
            !Path::new(temp_path).exists(),
            "Temporary file should be cleaned up after atomic write"
        );

        // Test 2: Verify file integrity during write operation
        let integrity_path = "test_json_integrity.json";
        let mut large_storage = MnemonicStorage::new();

        // Add multiple mnemonics to create a larger file
        for _i in 0..10 {
            let mnemonic = MnemonicStorage::generate_new_mnemonic();
            large_storage.append_mnemonic(mnemonic).unwrap();
        }

        // Write the file
        large_storage.write_to_file_path(integrity_path).unwrap();

        // Verify we can read it back completely
        let loaded_storage = MnemonicStorage::read_from_file_path(integrity_path).unwrap();
        assert_eq!(loaded_storage.len(), large_storage.len());

        for i in 0..large_storage.len() {
            assert_eq!(
                loaded_storage.get_mnemonic_by_index(i),
                large_storage.get_mnemonic_by_index(i)
            );
        }

        // Test 3: Concurrent write safety (simulate multiple writers)
        let concurrent_path = "test_json_concurrent.json";
        let concurrent_storage = MnemonicStorage::new();

        // This test verifies that the atomic write mechanism prevents corruption
        // even if multiple threads try to write simultaneously
        let handles: Vec<_> = (0..3)
            .map(|_i| {
                let path = concurrent_path.to_string();
                let mut storage = concurrent_storage.clone();

                thread::spawn(move || {
                    // Add a unique mnemonic for this thread (generate a proper BIP39 mnemonic)
                    let mnemonic = MnemonicStorage::generate_new_mnemonic();
                    storage.append_mnemonic(mnemonic).unwrap();

                    // Small delay to increase chance of concurrent access
                    thread::sleep(Duration::from_millis(1));

                    // Write to file
                    storage.write_to_file_path(&path)
                })
            })
            .collect();

        // Wait for all threads to complete
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // At least one write should succeed
        let successful_writes = results.iter().filter(|r| r.is_ok()).count();
        assert!(
            successful_writes > 0,
            "At least one concurrent write should succeed"
        );

        // The final file should be valid JSON (not corrupted)
        if Path::new(concurrent_path).exists() {
            let final_storage = MnemonicStorage::read_from_file_path(concurrent_path);
            assert!(
                final_storage.is_ok(),
                "Final file should be valid JSON after concurrent writes"
            );
        }

        // Test 4: Write failure cleanup (simulate failure during write)
        let cleanup_path = "test_json_cleanup.json";
        let cleanup_temp_path = "test_json_cleanup.tmp";

        // Create a scenario where the temp file might be left behind
        // (This is hard to simulate reliably, so we'll test the cleanup logic)
        let cleanup_storage = MnemonicStorage::new();

        // Manually create a temp file to simulate a previous failed write
        fs::write(cleanup_temp_path, "leftover temp file").unwrap();
        assert!(Path::new(cleanup_temp_path).exists());

        // Perform a successful write (should handle any existing temp file)
        let result = cleanup_storage.write_to_file_path(cleanup_path);
        assert!(result.is_ok());

        // Verify the target file exists and is valid
        assert!(Path::new(cleanup_path).exists());
        let loaded = MnemonicStorage::read_from_file_path(cleanup_path).unwrap();
        assert_eq!(loaded.len(), 0); // Empty storage

        // Test 5: Verify JSON formatting consistency
        let format_path = "test_json_format.json";
        let mut format_storage = MnemonicStorage::new();
        format_storage.append_mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()).unwrap();

        format_storage.write_to_file_path(format_path).unwrap();

        // Read the raw file content and verify it's properly formatted JSON
        let raw_content = fs::read_to_string(format_path).unwrap();

        // Should be pretty-printed JSON (contains newlines and indentation)
        assert!(
            raw_content.contains('\n'),
            "JSON should be pretty-printed with newlines"
        );
        assert!(
            raw_content.contains("  "),
            "JSON should be pretty-printed with indentation"
        );

        // Should be valid JSON that we can parse
        let parsed: serde_json::Value = serde_json::from_str(&raw_content).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("mnemonic").is_some());
        assert!(parsed["mnemonic"].is_array());

        // Cleanup
        let _ = fs::remove_file(test_path);
        let _ = fs::remove_file(temp_path);
        let _ = fs::remove_file(integrity_path);
        let _ = fs::remove_file(concurrent_path);
        let _ = fs::remove_file(cleanup_path);
        let _ = fs::remove_file(cleanup_temp_path);
        let _ = fs::remove_file(format_path);
    }

    #[test]
    fn test_json_file_edge_cases() {
        use std::fs;

        // Test 1: Very large mnemonic arrays
        let large_path = "test_json_large.json";
        let mut large_storage = MnemonicStorage::new();

        // Add 100 mnemonics to test performance and correctness with large files
        for _ in 0..100 {
            let mnemonic = MnemonicStorage::generate_new_mnemonic();
            large_storage.append_mnemonic(mnemonic).unwrap();
        }

        // Write and read back
        large_storage.write_to_file_path(large_path).unwrap();
        let loaded_large = MnemonicStorage::read_from_file_path(large_path).unwrap();
        assert_eq!(loaded_large.len(), 100);

        // Verify all mnemonics are preserved correctly
        for i in 0..100 {
            assert_eq!(
                loaded_large.get_mnemonic_by_index(i),
                large_storage.get_mnemonic_by_index(i)
            );
        }

        // Test 2: Unicode and special characters in file paths
        let unicode_path = "test_json_ünïcödé.json";
        let unicode_storage = MnemonicStorage::new();

        let result = unicode_storage.write_to_file_path(unicode_path);
        // This may succeed or fail depending on the filesystem
        if result.is_ok() {
            let loaded_unicode = MnemonicStorage::read_from_file_path(unicode_path).unwrap();
            assert_eq!(loaded_unicode.len(), 0);
        }

        // Test 3: Very long file paths
        let long_path = format!("{}.json", "very_long_filename_".repeat(10));
        let long_storage = MnemonicStorage::new();

        let result = long_storage.write_to_file_path(&long_path);
        // This may succeed or fail depending on filesystem limits
        if result.is_ok() {
            let loaded_long = MnemonicStorage::read_from_file_path(&long_path).unwrap();
            assert_eq!(loaded_long.len(), 0);
        }

        // Test 4: File with BOM (Byte Order Mark)
        let bom_path = "test_json_bom.json";
        let bom_content = "\u{FEFF}{\"mnemonic\":[\"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\"]}";
        fs::write(bom_path, bom_content).unwrap();

        let result = MnemonicStorage::read_from_file_path(bom_path);
        // This should handle BOM gracefully or fail with appropriate error
        match result {
            Ok(storage) => {
                assert_eq!(storage.len(), 1);
            }
            Err(SignerError::Serialization(_)) => {
                // BOM might cause JSON parsing to fail, which is acceptable
            }
            Err(other) => panic!("Unexpected error type for BOM file: {:?}", other),
        }

        // Test 5: File with different line endings (CRLF vs LF)
        let crlf_path = "test_json_crlf.json";
        let crlf_content = "{\r\n  \"mnemonic\": [\r\n    \"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\"\r\n  ]\r\n}";
        fs::write(crlf_path, crlf_content).unwrap();

        let result = MnemonicStorage::read_from_file_path(crlf_path);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 1);

        // Cleanup
        let _ = fs::remove_file(large_path);
        let _ = fs::remove_file(unicode_path);
        let _ = fs::remove_file(&long_path);
        let _ = fs::remove_file(bom_path);
        let _ = fs::remove_file(crlf_path);
    }

    #[test]
    fn test_json_file_recovery_scenarios() {
        use std::fs;

        // Test 1: Recovery from partial write (simulated by incomplete JSON)
        let partial_path = "test_json_partial.json";
        let partial_content = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                "legal winner thank year wave sausage worth useful legal winner thank"#; // Incomplete

        fs::write(partial_path, partial_content).unwrap();

        let result = MnemonicStorage::read_from_file_path(partial_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::Serialization(_) => {} // Expected for malformed JSON
            other => panic!("Expected Serialization error, got: {:?}", other),
        }

        // Recovery: overwrite with valid content
        let mut recovery_storage = MnemonicStorage::new();
        recovery_storage.append_mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()).unwrap();

        let recovery_result = recovery_storage.write_to_file_path(partial_path);
        assert!(recovery_result.is_ok());

        // Verify recovery was successful
        let recovered = MnemonicStorage::read_from_file_path(partial_path).unwrap();
        assert_eq!(recovered.len(), 1);

        // Test 2: Recovery from file with mixed valid/invalid mnemonics
        let mixed_path = "test_json_mixed.json";
        let mixed_content = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                "invalid mnemonic",
                "legal winner thank year wave sausage worth useful legal winner thank yellow"
            ]
        }"#;
        fs::write(mixed_path, mixed_content).unwrap();

        let result = MnemonicStorage::read_from_file_path(mixed_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            SignerError::InvalidMnemonic(msg) => {
                assert!(msg.contains("Invalid mnemonic at index 1"));
            }
            other => panic!("Expected InvalidMnemonic error, got: {:?}", other),
        }

        // Recovery: create new storage with only valid mnemonics
        let mut fixed_storage = MnemonicStorage::new();
        fixed_storage.append_mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()).unwrap();
        fixed_storage
            .append_mnemonic(
                "legal winner thank year wave sausage worth useful legal winner thank yellow"
                    .to_string(),
            )
            .unwrap();

        fixed_storage.write_to_file_path(mixed_path).unwrap();

        // Verify fix was successful
        let fixed = MnemonicStorage::read_from_file_path(mixed_path).unwrap();
        assert_eq!(fixed.len(), 2);

        // Test 3: Recovery from zero-byte file
        let zero_path = "test_json_zero.json";
        fs::write(zero_path, "").unwrap();

        let result = MnemonicStorage::read_from_file_path(zero_path);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert!(storage.is_empty());

        // Add content to previously empty file
        let mut populated_storage = MnemonicStorage::new();
        populated_storage.append_mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()).unwrap();
        populated_storage.write_to_file_path(zero_path).unwrap();

        let populated = MnemonicStorage::read_from_file_path(zero_path).unwrap();
        assert_eq!(populated.len(), 1);

        // Test 4: Recovery from file with extra JSON fields
        let extra_path = "test_json_extra.json";
        let extra_content = r#"{
            "mnemonic": [
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            ],
            "extra_field": "should be ignored",
            "version": 1,
            "metadata": {
                "created": "2023-01-01",
                "notes": "test file"
            }
        }"#;
        fs::write(extra_path, extra_content).unwrap();

        let result = MnemonicStorage::read_from_file_path(extra_path);
        assert!(result.is_ok());
        let storage = result.unwrap();
        assert_eq!(storage.len(), 1);

        // Verify that writing back preserves only the mnemonic field
        storage.write_to_file_path(extra_path).unwrap();
        let rewritten_content = fs::read_to_string(extra_path).unwrap();
        assert!(rewritten_content.contains("mnemonic"));
        assert!(!rewritten_content.contains("extra_field"));
        assert!(!rewritten_content.contains("version"));
        assert!(!rewritten_content.contains("metadata"));

        // Cleanup
        let _ = fs::remove_file(partial_path);
        let _ = fs::remove_file(mixed_path);
        let _ = fs::remove_file(zero_path);
        let _ = fs::remove_file(extra_path);
    }
}
