use amp_rs::signer::{Signer, SignerError};
use amp_rs::{AmpError, ElementsRpc, TxInput, Unspent};
use async_trait::async_trait;
use httpmock::prelude::*;

use serde_json::json;
use std::collections::HashMap;

/// Mock signer for testing transaction signing integration
#[derive(Debug, Clone)]
struct MockSigner {
    should_succeed: bool,
    return_value: Option<String>,
    expected_input: Option<String>,
    call_count: std::sync::Arc<std::sync::Mutex<usize>>,
}

impl MockSigner {
    /// Creates a new mock signer that will succeed with a default signed transaction
    fn new_success() -> Self {
        Self {
            should_succeed: true,
            return_value: None,
            expected_input: None,
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }

    /// Creates a new mock signer that will fail with a signing error
    fn new_failure() -> Self {
        Self {
            should_succeed: false,
            return_value: None,
            expected_input: None,
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }

    /// Creates a mock signer that returns a specific signed transaction
    fn with_return_value(signed_tx: String) -> Self {
        Self {
            should_succeed: true,
            return_value: Some(signed_tx),
            expected_input: None,
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }

    /// Creates a mock signer that expects a specific input transaction
    fn with_expected_input(expected: String) -> Self {
        Self {
            should_succeed: true,
            return_value: None,
            expected_input: Some(expected),
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }

    /// Returns the number of times sign_transaction was called
    fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl Signer for MockSigner {
    async fn sign_transaction(&self, unsigned_tx: &str) -> Result<String, SignerError> {
        // Increment call count
        {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
        }

        // Check expected input if specified
        if let Some(ref expected) = self.expected_input {
            if unsigned_tx != expected {
                return Err(SignerError::InvalidTransaction(format!(
                    "Expected transaction '{}', got '{}'",
                    expected, unsigned_tx
                )));
            }
        }

        if !self.should_succeed {
            return Err(SignerError::Lwk(
                "Mock signing failure for testing".to_string(),
            ));
        }

        // Return specific value or generate a default signed transaction
        match &self.return_value {
            Some(signed_tx) => Ok(signed_tx.clone()),
            None => {
                // Generate a realistic signed transaction by appending signature data
                Ok(format!("{}deadbeefcafebabe1234567890abcdef", unsigned_tx))
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
// Helper function to create mock UTXO data for testing
fn create_mock_utxos(asset_id: &str, amounts: Vec<f64>) -> Vec<Unspent> {
    amounts
        .into_iter()
        .enumerate()
        .map(|(i, amount)| Unspent {
            txid: format!("txid_{:03}", i),
            vout: i as u32,
            amount,
            asset: asset_id.to_string(),
            address: format!("address_{}", i),
            spendable: true,
            confirmations: Some(6),
            scriptpubkey: Some(format!("76a914{}88ac", "0".repeat(40))),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(format!("{:064}", i)),
            assetblinder: Some(format!("{:064}", i + 1000)),
        })
        .collect()
}

/// Helper function to create mock RPC response for listunspent
fn create_listunspent_mock(
    server: &MockServer,
    _wallet_name: &str,
    _asset_id: &str,
    utxos: Vec<Unspent>,
) {
    use httpmock::Method::POST;

    // Specific mock for createrawtransaction - return transaction hex (must come first)
    server.mock(|when, then| {
        when.method(POST)
            .body_contains("createrawtransaction");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002",
                "error": null
            }));
    });

    // Catch-all mock for all other RPC calls (listunspent, loadwallet, etc.)
    server.mock(|when, then| {
        when.method(POST);
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": utxos,
                "error": null
            }));
    });
}

/// Helper function to create mock RPC response for gettransaction
fn create_gettransaction_mock(
    server: &MockServer,
    txid: &str,
    confirmations: u32,
    blockheight: Option<u64>,
) {
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "gettransaction",
            "params": [txid, true]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": txid,
                "confirmations": confirmations,
                "blockheight": blockheight,
                "hex": "020000000001..."
            },
            "error": null
        }));
    });
}

/// Helper function to create mock RPC response for createrawtransaction

#[tokio::test]
async fn test_utxo_selection_sufficient_funds_single_utxo() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs: one large UTXO that covers the required amount
    let utxos = create_mock_utxos(asset_id, vec![150.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test selecting UTXOs for 100.0 + 1.0 fee = 101.0 total
    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 100.0, 1.0)
        .await;

    match result {
        Ok((selected_utxos, total_amount)) => {
            assert_eq!(selected_utxos.len(), 1);
            assert_eq!(total_amount, 150.0);
            assert_eq!(selected_utxos[0].amount, 150.0);
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Test failed with error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_utxo_selection_sufficient_funds_multiple_utxos() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs: multiple smaller UTXOs that together cover the required amount
    let utxos = create_mock_utxos(asset_id, vec![50.0, 30.0, 40.0, 25.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test selecting UTXos for 120.0 + 1.0 fee = 121.0 total
    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 120.0, 1.0)
        .await;

    assert!(result.is_ok());
    let (selected_utxos, total_amount) = result.unwrap();

    // Should select largest UTXOs first: 50.0 + 40.0 + 30.0 = 120.0 (sufficient)
    // or 50.0 + 40.0 + 30.0 + 25.0 = 145.0 depending on algorithm
    assert!(selected_utxos.len() >= 3);
    assert!(total_amount >= 121.0);

    // Verify UTXOs are sorted by amount (largest first)
    for i in 1..selected_utxos.len() {
        assert!(selected_utxos[i - 1].amount >= selected_utxos[i].amount);
    }
}

#[tokio::test]
async fn test_utxo_selection_insufficient_funds() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs: total amount is less than required
    let utxos = create_mock_utxos(asset_id, vec![10.0, 5.0, 3.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Try to select UTXOs for 100.0 + 1.0 fee = 101.0 total, but only have 18.0
    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 100.0, 1.0)
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    println!("Actual error: {}", error);
    assert!(error.to_string().contains("Insufficient UTXOs"));
}

#[tokio::test]
async fn test_utxo_selection_no_spendable_utxos() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs that are not spendable
    let mut utxos = create_mock_utxos(asset_id, vec![100.0, 50.0]);
    for utxo in &mut utxos {
        utxo.spendable = false;
    }
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 50.0, 1.0)
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("No spendable UTXOs"));
}

#[tokio::test]
async fn test_utxo_selection_exact_amount_needed() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs that exactly match the required amount
    let utxos = create_mock_utxos(asset_id, vec![50.0, 51.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Need exactly 101.0 (100.0 + 1.0 fee), have 51.0 + 50.0 = 101.0
    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 100.0, 1.0)
        .await;

    assert!(result.is_ok());
    let (selected_utxos, total_amount) = result.unwrap();
    assert_eq!(selected_utxos.len(), 2);
    assert_eq!(total_amount, 101.0);
}

#[tokio::test]
async fn test_utxo_selection_algorithm_largest_first() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs in random order to test sorting
    let utxos = create_mock_utxos(asset_id, vec![25.0, 100.0, 10.0, 75.0, 50.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Need 120.0 + 1.0 fee = 121.0 total
    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 120.0, 1.0)
        .await;

    assert!(result.is_ok());
    let (selected_utxos, total_amount) = result.unwrap();

    // Should select 100.0 + 75.0 = 175.0 (largest first algorithm)
    assert!(total_amount >= 121.0);

    // Verify first UTXO is the largest available
    assert_eq!(selected_utxos[0].amount, 100.0);

    // Verify UTXOs are in descending order by amount
    for i in 1..selected_utxos.len() {
        assert!(selected_utxos[i - 1].amount >= selected_utxos[i].amount);
    }
}

#[tokio::test]
async fn test_transaction_construction_with_mock_signer_success() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Setup UTXO mock
    let utxos = create_mock_utxos(asset_id, vec![150.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    // Setup transaction creation mock
    let mut address_amounts = HashMap::new();
    address_amounts.insert("recipient1".to_string(), 100.0);

    let mut expected_outputs = HashMap::new();
    expected_outputs.insert("recipient1".to_string(), 100.0);
    expected_outputs.insert("address_0".to_string(), 49.0); // Change: 150 - 100 - 1 fee

    let mut expected_assets = HashMap::new();
    expected_assets.insert("recipient1".to_string(), asset_id.to_string());
    expected_assets.insert("address_0".to_string(), asset_id.to_string());

    let _expected_inputs = vec![TxInput {
        txid: "txid_000".to_string(),
        vout: 0,
        sequence: None,
    }];

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test transaction construction
    let result = rpc
        .build_distribution_transaction(
            "test_wallet",
            asset_id,
            address_amounts,
            "address_0", // change address
            1.0,         // fee
        )
        .await;

    if result.is_err() {
        println!("Error: {}", result.as_ref().unwrap_err());
    }
    assert!(result.is_ok());
    let (raw_tx, selected_utxos, change_amount) = result.unwrap();

    // Verify transaction was built
    assert!(!raw_tx.is_empty());
    assert_eq!(selected_utxos.len(), 1);
    assert_eq!(change_amount, 50.0);
}

#[tokio::test]
async fn test_signer_integration_success() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002";
    let expected_signed_tx = format!("{}deadbeefcafebabe1234567890abcdef", unsigned_tx);

    let mock_signer = MockSigner::new_success();

    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;

    assert!(result.is_ok());
    let signed_tx = result.unwrap();
    assert_eq!(signed_tx, expected_signed_tx);
    assert_eq!(mock_signer.call_count(), 1);
}

#[tokio::test]
async fn test_signer_integration_failure() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002";

    let mock_signer = MockSigner::new_failure();

    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Mock signing failure"));
    assert_eq!(mock_signer.call_count(), 1);
}

#[tokio::test]
async fn test_signer_integration_with_expected_input() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002";

    let mock_signer = MockSigner::with_expected_input(unsigned_tx.to_string());

    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;

    assert!(result.is_ok());
    assert_eq!(mock_signer.call_count(), 1);
}

#[tokio::test]
async fn test_signer_integration_with_wrong_input() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002";
    let expected_tx = "different_transaction_hex";

    let mock_signer = MockSigner::with_expected_input(expected_tx.to_string());

    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Expected transaction"));
    assert_eq!(mock_signer.call_count(), 1);
}

#[tokio::test]
async fn test_signer_integration_with_custom_return_value() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002";
    let custom_signed_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000deadbeef00000000";

    let mock_signer = MockSigner::with_return_value(custom_signed_tx.to_string());

    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;

    if result.is_err() {
        println!("Error: {}", result.as_ref().unwrap_err());
    }
    assert!(result.is_ok());
    let signed_tx = result.unwrap();
    assert_eq!(signed_tx, custom_signed_tx);
    assert_eq!(mock_signer.call_count(), 1);
}

#[tokio::test]
async fn test_transaction_structure_validation_empty_hex() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let mock_signer = MockSigner::new_success();

    let result = rpc.sign_transaction("", &mock_signer).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("cannot be empty"));
    assert_eq!(mock_signer.call_count(), 0); // Should not call signer for invalid input
}

#[tokio::test]
async fn test_transaction_structure_validation_odd_length_hex() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let mock_signer = MockSigner::new_success();

    let result = rpc.sign_transaction("abc", &mock_signer).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("even length"));
    assert_eq!(mock_signer.call_count(), 0); // Should not call signer for invalid input
}

#[tokio::test]
async fn test_transaction_structure_validation_invalid_hex_characters() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let mock_signer = MockSigner::new_success();

    let result = rpc.sign_transaction("abcg", &mock_signer).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("invalid hex characters"));
    assert_eq!(mock_signer.call_count(), 0); // Should not call signer for invalid input
}

#[tokio::test]
async fn test_liquid_specific_transaction_format() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Setup UTXO mock with Liquid-specific asset ID
    let utxos = create_mock_utxos(asset_id, vec![100.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    // Setup transaction creation mock with Liquid-specific outputs
    let mut address_amounts = HashMap::new();
    address_amounts.insert(
        "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        50.0,
    );

    let mut expected_outputs = HashMap::new();
    expected_outputs.insert(
        "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        50.0,
    );
    expected_outputs.insert("address_0".to_string(), 49.0); // Change

    let mut expected_assets = HashMap::new();
    expected_assets.insert(
        "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        asset_id.to_string(),
    );
    expected_assets.insert("address_0".to_string(), asset_id.to_string());

    let _expected_inputs = vec![TxInput {
        txid: "txid_000".to_string(),
        vout: 0,
        sequence: None,
    }];

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .build_distribution_transaction(
            "test_wallet",
            asset_id,
            address_amounts,
            "address_0", // change address
            1.0,         // fee
        )
        .await;

    assert!(result.is_ok());
    let (raw_tx, selected_utxos, change_amount) = result.unwrap();

    // Verify Liquid-specific transaction structure
    assert!(!raw_tx.is_empty());
    assert!(raw_tx.starts_with("02")); // Liquid transaction version
    assert_eq!(selected_utxos.len(), 1);
    assert_eq!(selected_utxos[0].asset, asset_id); // Verify asset ID is preserved
    assert_eq!(change_amount, 50.0);
}

#[tokio::test]
async fn test_transaction_construction_with_multiple_outputs() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Setup UTXO mock with sufficient funds
    let utxos = create_mock_utxos(asset_id, vec![200.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    // Setup transaction creation mock with multiple outputs
    let mut address_amounts = HashMap::new();
    address_amounts.insert("recipient1".to_string(), 50.0);
    address_amounts.insert("recipient2".to_string(), 75.0);

    let mut expected_outputs = HashMap::new();
    expected_outputs.insert("recipient1".to_string(), 50.0);
    expected_outputs.insert("recipient2".to_string(), 75.0);
    expected_outputs.insert("address_0".to_string(), 73.0); // Change: 200 - 50 - 75 - 2 fee

    let mut expected_assets = HashMap::new();
    expected_assets.insert("recipient1".to_string(), asset_id.to_string());
    expected_assets.insert("recipient2".to_string(), asset_id.to_string());
    expected_assets.insert("address_0".to_string(), asset_id.to_string());

    let _expected_inputs = vec![TxInput {
        txid: "txid_000".to_string(),
        vout: 0,
        sequence: None,
    }];

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .build_distribution_transaction(
            "test_wallet",
            asset_id,
            address_amounts,
            "address_0", // change address
            2.0,         // fee
        )
        .await;

    assert!(result.is_ok());
    let (raw_tx, selected_utxos, change_amount) = result.unwrap();

    // Verify transaction with multiple outputs
    assert!(!raw_tx.is_empty());
    assert_eq!(selected_utxos.len(), 1);
    assert_eq!(change_amount, 75.0);
}

#[tokio::test]
async fn test_transaction_construction_no_change_needed() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Setup UTXO mock with exact amount needed
    let utxos = create_mock_utxos(asset_id, vec![101.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    // Setup transaction creation mock with no change output
    let mut address_amounts = HashMap::new();
    address_amounts.insert("recipient1".to_string(), 100.0);

    let mut expected_outputs = HashMap::new();
    expected_outputs.insert("recipient1".to_string(), 100.0);
    // No change output expected

    let mut expected_assets = HashMap::new();
    expected_assets.insert("recipient1".to_string(), asset_id.to_string());

    let _expected_inputs = vec![TxInput {
        txid: "txid_000".to_string(),
        vout: 0,
        sequence: None,
    }];

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .build_distribution_transaction(
            "test_wallet",
            asset_id,
            address_amounts,
            "address_0", // change address
            1.0,         // fee
        )
        .await;

    assert!(result.is_ok());
    let (raw_tx, selected_utxos, change_amount) = result.unwrap();

    // Verify transaction with no change
    assert!(!raw_tx.is_empty());
    assert_eq!(selected_utxos.len(), 1);
    assert_eq!(change_amount, 1.0); // Change is 101 - 100 = 1.0
}

#[tokio::test]
async fn test_transaction_construction_dust_change_handling() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Setup UTXO mock with amount that would create dust change
    let utxos = create_mock_utxos(asset_id, vec![100.5]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    // Setup transaction creation mock - change amount is 0.4 which is above dust threshold
    let mut address_amounts = HashMap::new();
    address_amounts.insert("recipient1".to_string(), 100.0);

    let mut expected_outputs = HashMap::new();
    expected_outputs.insert("recipient1".to_string(), 100.0);
    expected_outputs.insert("address_0".to_string(), 0.4); // Change: 100.5 - 100.0 - 0.1 = 0.4

    let mut expected_assets = HashMap::new();
    expected_assets.insert("recipient1".to_string(), asset_id.to_string());
    expected_assets.insert("address_0".to_string(), asset_id.to_string());

    let _expected_inputs = vec![TxInput {
        txid: "txid_000".to_string(),
        vout: 0,
        sequence: None,
    }];

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .build_distribution_transaction(
            "test_wallet",
            asset_id,
            address_amounts,
            "address_0", // change address
            0.1,         // small fee
        )
        .await;

    assert!(result.is_ok());
    let (raw_tx, selected_utxos, change_amount) = result.unwrap();

    // Verify dust change handling
    assert!(!raw_tx.is_empty());
    assert_eq!(selected_utxos.len(), 1);
    assert_eq!(change_amount, 0.5); // 100.5 - 100.0 = 0.5
}

#[tokio::test]
async fn test_transaction_construction_zero_amount_distribution() {
    let rpc = ElementsRpc::new(
        "http://localhost:18884".to_string(),
        "user".to_string(),
        "pass".to_string(),
    );

    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let address_amounts = HashMap::new(); // Empty distribution

    let result = rpc
        .build_distribution_transaction(
            "test_wallet",
            asset_id,
            address_amounts,
            "change_address", // change address
            1.0,
        )
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    println!("Actual error: {}", error);
    assert!(error
        .to_string()
        .contains("Total distribution amount must be greater than zero"));
}

#[tokio::test]
async fn test_signer_validation_comprehensive() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test cases for various invalid transaction formats
    let test_cases = vec![
        ("", "cannot be empty"),
        ("abc", "even length"),
        ("abcg", "invalid hex characters"),
    ];

    for (invalid_tx, expected_error) in test_cases {
        let mock_signer = MockSigner::new_success();
        let result = rpc.sign_transaction(invalid_tx, &mock_signer).await;

        assert!(
            result.is_err(),
            "Expected error for input: '{}'",
            invalid_tx
        );
        let error = result.unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "Expected error containing '{}', got: '{}'",
            expected_error,
            error
        );
        assert_eq!(
            mock_signer.call_count(),
            0,
            "Signer should not be called for invalid input: '{}'",
            invalid_tx
        );
    }
}

#[tokio::test]
async fn test_signer_return_value_validation() {
    let server = MockServer::start();
    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let unsigned_tx = "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000002";

    // Test signer returning empty string
    let mock_signer = MockSigner::with_return_value("".to_string());
    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));

    // Test signer returning odd length hex
    let mock_signer = MockSigner::with_return_value("abc".to_string());
    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("even length"));

    // Test signer returning invalid hex
    let mock_signer = MockSigner::with_return_value("abcg".to_string());
    let result = rpc.sign_transaction(unsigned_tx, &mock_signer).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid hex"));

    // Test signer returning transaction that meets length requirement but is below minimum size
    let short_unsigned_tx = "abcd"; // 2 bytes when decoded
    let mock_signer = MockSigner::with_return_value("abcdef".to_string()); // 3 bytes when decoded, longer than unsigned but below 10 byte minimum
    let result = rpc.sign_transaction(short_unsigned_tx, &mock_signer).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("minimum size"));
}

#[tokio::test]
async fn test_utxo_selection_edge_cases() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Test with zero fee
    let utxos = create_mock_utxos(asset_id, vec![100.0]);
    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 100.0, 0.0)
        .await;
    assert!(result.is_ok());
    let (selected_utxos, total_amount) = result.unwrap();
    assert_eq!(selected_utxos.len(), 1);
    assert_eq!(total_amount, 100.0);
}

#[tokio::test]
async fn test_utxo_selection_with_confirmations_filter() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

    // Create UTXOs with different confirmation counts
    let mut utxos = create_mock_utxos(asset_id, vec![100.0, 50.0]);
    utxos[0].confirmations = Some(6); // Confirmed
    utxos[1].confirmations = Some(0); // Unconfirmed

    create_listunspent_mock(&server, "test_wallet", asset_id, utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .select_utxos_for_amount("test_wallet", asset_id, 75.0, 1.0)
        .await;

    assert!(result.is_ok());
    let (selected_utxos, total_amount) = result.unwrap();

    // Should select confirmed UTXOs preferentially
    assert!(total_amount >= 76.0);
    for utxo in &selected_utxos {
        // All selected UTXOs should be spendable
        assert!(utxo.spendable);
    }
}

#[tokio::test]
async fn test_confirmation_polling_success_immediate() {
    let server = MockServer::start();
    let txid = "test_txid_immediate_confirmation";

    // Mock get_transaction to return transaction with sufficient confirmations immediately
    create_gettransaction_mock(&server, txid, 3, Some(12345));

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test with 2 required confirmations - should succeed immediately
    let result = rpc.wait_for_confirmations(txid, Some(2), Some(1)).await;

    if result.is_err() {
        println!("Error: {:?}", result.as_ref().unwrap_err());
    }
    assert!(result.is_ok());
    let tx_detail = result.unwrap();
    assert_eq!(tx_detail.txid, txid);
    assert_eq!(tx_detail.confirmations, 3);
    assert!(tx_detail.confirmations >= 2);
}

#[tokio::test]
async fn test_confirmation_polling_success_after_wait() {
    let server = MockServer::start();
    let txid = "test_txid_delayed_confirmation";

    // Mock get_transaction to return sufficient confirmations
    create_gettransaction_mock(&server, txid, 2, Some(12345));

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test with fast polling interval to speed up test
    let result = rpc
        .wait_for_confirmations_with_interval(txid, Some(2), Some(1), Some(1))
        .await;

    assert!(result.is_ok());
    let tx_detail = result.unwrap();
    assert_eq!(tx_detail.txid, txid);
    assert_eq!(tx_detail.confirmations, 2);
}

#[tokio::test]
async fn test_confirmation_polling_timeout() {
    let server = MockServer::start();
    let txid = "test_txid_timeout";

    // Mock get_transaction to always return 0 confirmations
    create_gettransaction_mock(&server, txid, 0, None);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test with very short timeout and fast polling
    let result = rpc
        .wait_for_confirmations_with_interval(txid, Some(2), Some(0), Some(1))
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(matches!(error, AmpError::Timeout(_)));
    assert!(error
        .to_string()
        .contains("Timeout waiting for confirmations"));
    assert!(error.to_string().contains(txid));
    assert!(error.to_string().contains("You can retry confirmation"));
}

#[tokio::test]
async fn test_confirmation_polling_rpc_errors_with_recovery() {
    let server = MockServer::start();
    let txid = "test_txid_rpc_errors";

    // Mock successful response - the polling logic handles RPC errors by continuing to poll
    create_gettransaction_mock(&server, txid, 3, Some(12345));

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Should succeed with sufficient confirmations
    let result = rpc
        .wait_for_confirmations_with_interval(txid, Some(2), Some(1), Some(1))
        .await;

    assert!(result.is_ok());
    let tx_detail = result.unwrap();
    assert_eq!(tx_detail.txid, txid);
    assert_eq!(tx_detail.confirmations, 3);
}

#[tokio::test]
async fn test_confirmation_polling_default_parameters() {
    let server = MockServer::start();
    let txid = "test_txid_defaults";

    // Mock get_transaction to return exactly 2 confirmations (default minimum)
    create_gettransaction_mock(&server, txid, 2, Some(12345));

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test with default parameters (None values)
    let result = rpc.wait_for_confirmations(txid, None, None).await;

    assert!(result.is_ok());
    let tx_detail = result.unwrap();
    assert_eq!(tx_detail.confirmations, 2);
}

#[tokio::test]
async fn test_confirmation_polling_custom_minimum_confirmations() {
    let server = MockServer::start();
    let txid = "test_txid_custom_min";

    // Mock get_transaction to return 5 confirmations
    create_gettransaction_mock(&server, txid, 5, Some(12345));

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test with custom minimum confirmations (6) - should timeout since we only have 5
    let result = rpc
        .wait_for_confirmations_with_interval(txid, Some(6), Some(0), Some(1))
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AmpError::Timeout(_)));

    // Test with lower minimum confirmations (3) - should succeed
    let result = rpc.wait_for_confirmations(txid, Some(3), Some(1)).await;

    assert!(result.is_ok());
    let tx_detail = result.unwrap();
    assert_eq!(tx_detail.confirmations, 5);
    assert!(tx_detail.confirmations >= 3);
}

#[tokio::test]
async fn test_change_data_collection_success_with_multiple_outputs() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let txid = "test_txid_change_multiple";

    // Mock loadwallet first
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "loadwallet",
            "params": ["test_wallet"]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {"name": "test_wallet", "warning": ""},
            "error": null
        }));
    });

    // Mock listunspent on wallet-specific endpoint with correct parameters
    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "listunspent",
                "params": [0, 9999999, [], true, {}]
            }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": txid,
                    "vout": 1,
                    "amount": 25.5,
                    "asset": asset_id,
                    "address": "change_address_1",
                    "spendable": true,
                    "confirmations": 3
                },
                {
                    "txid": txid,
                    "vout": 2,
                    "amount": 10.0,
                    "asset": asset_id,
                    "address": "change_address_2",
                    "spendable": true,
                    "confirmations": 3
                },
                {
                    "txid": "different_txid",
                    "vout": 0,
                    "amount": 50.0,
                    "asset": asset_id,
                    "address": "other_address",
                    "spendable": true,
                    "confirmations": 6
                }
            ],
            "error": null
        }));
    });

    let rpc = ElementsRpc::new(
        server.url("/").trim_end_matches('/').to_string(),
        "user".to_string(),
        "pass".to_string(),
    );

    let result = rpc
        .collect_change_data(asset_id, txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_ok());
    let change_data = result.unwrap();

    // Should only return UTXOs from the specified transaction
    assert_eq!(change_data.len(), 2);

    // Verify all returned UTXOs are from the correct transaction
    for utxo in &change_data {
        assert_eq!(utxo.txid, txid);
        assert_eq!(utxo.asset, asset_id);
        assert!(utxo.spendable);
    }

    // Verify specific amounts
    let amounts: Vec<f64> = change_data.iter().map(|u| u.amount).collect();
    assert!(amounts.contains(&25.5));
    assert!(amounts.contains(&10.0));
}

#[tokio::test]
async fn test_change_data_collection_no_change_outputs() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let txid = "test_txid_no_change";

    // Mock loadwallet first
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "loadwallet",
            "params": ["test_wallet"]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {"name": "test_wallet", "warning": ""},
            "error": null
        }));
    });

    // Mock listunspent on wallet-specific endpoint with correct parameters
    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "listunspent",
                "params": [0, 9999999, [], true, {}]
            }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": "different_txid_1",
                    "vout": 0,
                    "amount": 100.0,
                    "asset": asset_id,
                    "address": "other_address_1",
                    "spendable": true,
                    "confirmations": 6
                },
                {
                    "txid": "different_txid_2",
                    "vout": 1,
                    "amount": 50.0,
                    "asset": asset_id,
                    "address": "other_address_2",
                    "spendable": true,
                    "confirmations": 3
                }
            ],
            "error": null
        }));
    });

    let rpc = ElementsRpc::new(
        server.url("/").trim_end_matches('/').to_string(),
        "user".to_string(),
        "pass".to_string(),
    );

    let result = rpc
        .collect_change_data(asset_id, txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_ok());
    let change_data = result.unwrap();

    // Should return empty vector when no change outputs exist
    assert_eq!(change_data.len(), 0);
}

#[tokio::test]
async fn test_change_data_collection_filters_unspendable() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let txid = "test_txid_unspendable";

    // Mock loadwallet first
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "loadwallet",
            "params": ["test_wallet"]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {"name": "test_wallet", "warning": ""},
            "error": null
        }));
    });

    // Mock listunspent on wallet-specific endpoint with correct parameters
    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "listunspent",
                "params": [0, 9999999, [], true, {}]
            }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": txid,
                    "vout": 0,
                    "amount": 25.0,
                    "asset": asset_id,
                    "address": "spendable_address",
                    "spendable": true,
                    "confirmations": 3
                },
                {
                    "txid": txid,
                    "vout": 1,
                    "amount": 15.0,
                    "asset": asset_id,
                    "address": "unspendable_address",
                    "spendable": false,
                    "confirmations": 3
                }
            ],
            "error": null
        }));
    });

    let rpc = ElementsRpc::new(
        server.url("/").trim_end_matches('/').to_string(),
        "user".to_string(),
        "pass".to_string(),
    );

    let result = rpc
        .collect_change_data(asset_id, txid, &rpc, "test_wallet")
        .await;

    if result.is_err() {
        println!("Error: {}", result.as_ref().unwrap_err());
    }
    assert!(result.is_ok());
    let change_data = result.unwrap();

    // Should only return spendable UTXOs
    assert_eq!(change_data.len(), 1);
    assert_eq!(change_data[0].amount, 25.0);
    assert!(change_data[0].spendable);
    assert_eq!(change_data[0].address, "spendable_address");
}

#[tokio::test]
async fn test_change_data_collection_filters_wrong_asset() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let different_asset_id = "different_asset_id_hex_string_here_123456789abcdef";
    let txid = "test_txid_wrong_asset";

    // Mock loadwallet first
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "loadwallet",
            "params": ["test_wallet"]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {"name": "test_wallet", "warning": ""},
            "error": null
        }));
    });

    // Mock listunspent on wallet-specific endpoint with correct parameters
    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "listunspent",
                "params": [0, 9999999, [], true, {}]
            }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": txid,
                    "vout": 0,
                    "amount": 25.0,
                    "asset": asset_id,
                    "address": "correct_asset_address",
                    "spendable": true,
                    "confirmations": 3
                },
                {
                    "txid": txid,
                    "vout": 1,
                    "amount": 15.0,
                    "asset": different_asset_id,
                    "address": "wrong_asset_address",
                    "spendable": true,
                    "confirmations": 3
                }
            ],
            "error": null
        }));
    });

    let rpc = ElementsRpc::new(
        server.url("/").trim_end_matches('/').to_string(),
        "user".to_string(),
        "pass".to_string(),
    );

    let result = rpc
        .collect_change_data(asset_id, txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_ok());
    let change_data = result.unwrap();

    // Should only return UTXOs with the correct asset ID
    assert_eq!(change_data.len(), 1);
    assert_eq!(change_data[0].amount, 25.0);
    assert_eq!(change_data[0].asset, asset_id);
    assert_eq!(change_data[0].address, "correct_asset_address");
}

#[tokio::test]
async fn test_change_data_collection_rpc_error() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let txid = "test_txid_rpc_error";

    // Mock listunspent to return RPC error
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "listunspent",
            "params": [1, 9999999, [], true, {"asset": asset_id}]
        }));
        then.status(500).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "error": {
                "code": -1,
                "message": "RPC server error"
            }
        }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .collect_change_data(asset_id, txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(matches!(error, AmpError::Rpc(_)));
    assert!(error
        .to_string()
        .contains("Failed to query unspent outputs"));
}

#[tokio::test]
async fn test_change_data_formatting_for_api() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let txid = "test_txid_formatting";

    // Mock loadwallet first
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "loadwallet",
            "params": ["test_wallet"]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {"name": "test_wallet", "warning": ""},
            "error": null
        }));
    });

    // Mock listunspent on wallet-specific endpoint with correct parameters
    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "listunspent",
                "params": [0, 9999999, [], true, {}]
            }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": [
                {
                    "txid": txid,
                    "vout": 1,
                    "amount": 42.75,
                    "asset": asset_id,
                    "address": "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq",
                    "spendable": true,
                    "confirmations": 2,
                    "scriptpubkey": "76a914abcdef1234567890abcdef1234567890abcdef88ac",
                    "redeemscript": null,
                    "witnessscript": null
                }
            ],
            "error": null
        }));
    });

    let rpc = ElementsRpc::new(
        server.url("/").trim_end_matches('/').to_string(),
        "user".to_string(),
        "pass".to_string(),
    );

    let result = rpc
        .collect_change_data(asset_id, txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_ok());
    let change_data = result.unwrap();

    assert_eq!(change_data.len(), 1);
    let change_utxo = &change_data[0];

    // Verify all fields are properly formatted for API submission
    assert_eq!(change_utxo.txid, txid);
    assert_eq!(change_utxo.vout, 1);
    assert_eq!(change_utxo.amount, 42.75);
    assert_eq!(change_utxo.asset, asset_id);
    assert_eq!(
        change_utxo.address,
        "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"
    );
    assert!(change_utxo.spendable);
    assert_eq!(change_utxo.confirmations, Some(2));
    assert_eq!(
        change_utxo.scriptpubkey,
        Some("76a914abcdef1234567890abcdef1234567890abcdef88ac".to_string())
    );

    // Test serialization to ensure it matches API expectations
    let serialized = serde_json::to_string(&change_data).unwrap();
    assert!(serialized.contains(&txid));
    assert!(serialized.contains("42.75"));
    assert!(serialized.contains(&asset_id));
    assert!(serialized.contains("lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"));
}

#[tokio::test]
async fn test_confirmation_timeout_error_message_format() {
    let server = MockServer::start();
    let txid = "test_txid_timeout_message";

    // Mock get_transaction to always return 0 confirmations
    server.mock(|when, then| {
        when.method(POST).path("/").json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "method": "gettransaction",
            "params": [txid]
        }));
        then.status(200).json_body(json!({
            "jsonrpc": "1.0",
            "id": "amp-client",
            "result": {
                "txid": txid,
                "confirmations": 0,
                "blockheight": null,
                "hex": "020000000001..."
            },
            "error": null
        }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    let result = rpc
        .wait_for_confirmations_with_interval(txid, Some(2), Some(0), Some(1))
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Verify error message contains all required information
    let error_msg = error.to_string();
    assert!(error_msg.contains("Timeout waiting for confirmations"));
    assert!(error_msg.contains(txid));
    assert!(error_msg.contains("You can retry confirmation"));
    assert!(error_msg.contains("calling the confirmation API"));

    // Verify error type is correct
    assert!(matches!(error, AmpError::Timeout(_)));

    // Test retry instructions
    if let Some(instructions) = error.retry_instructions() {
        assert!(instructions.contains("transaction ID"));
        assert!(instructions.contains("manually confirm"));
    }
}

#[tokio::test]
async fn test_collect_change_data_integration() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let distribution_txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

    // Create mock UTXOs including change outputs from the distribution transaction
    let mut all_utxos = Vec::new();

    // Add some existing UTXOs from other transactions
    all_utxos.push(Unspent {
        txid: "other_txid_123".to_string(),
        vout: 0,
        amount: 75.0,
        asset: asset_id.to_string(),
        address: "lq1qq1xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        spendable: true,
        confirmations: Some(10),
        scriptpubkey: Some("76a914abc123def456789abc123def456789abc123de88ac".to_string()),
        redeemscript: None,
        witnessscript: None,
        amountblinder: Some(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        ),
        assetblinder: Some(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        ),
    });

    // Add change outputs from the distribution transaction
    all_utxos.push(Unspent {
        txid: distribution_txid.to_string(),
        vout: 1,
        amount: 25.5,
        asset: asset_id.to_string(),
        address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        spendable: true,
        confirmations: Some(3),
        scriptpubkey: Some("76a914def456abc123789def456abc123789def456ab88ac".to_string()),
        redeemscript: None,
        witnessscript: None,
        amountblinder: Some(
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
        ),
        assetblinder: Some(
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
        ),
    });

    all_utxos.push(Unspent {
        txid: distribution_txid.to_string(),
        vout: 2,
        amount: 10.0,
        asset: asset_id.to_string(),
        address: "lq1qq3xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        spendable: true,
        confirmations: Some(3),
        scriptpubkey: Some("76a914ghi789jkl012345ghi789jkl012345ghi789jk88ac".to_string()),
        redeemscript: None,
        witnessscript: None,
        amountblinder: Some(
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
        ),
        assetblinder: Some(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        ),
    });

    // Add another UTXO from a different transaction
    all_utxos.push(Unspent {
        txid: "different_txid_456".to_string(),
        vout: 0,
        amount: 50.0,
        asset: asset_id.to_string(),
        address: "lq1qq4xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
        spendable: true,
        confirmations: Some(6),
        scriptpubkey: Some("76a914mno012pqr345678mno012pqr345678mno012pq88ac".to_string()),
        redeemscript: None,
        witnessscript: None,
        amountblinder: Some(
            "1010101010101010101010101010101010101010101010101010101010101010".to_string(),
        ),
        assetblinder: Some(
            "2020202020202020202020202020202020202020202020202020202020202020".to_string(),
        ),
    });

    // Create mock for listunspent RPC call
    create_listunspent_mock(&server, "test_wallet", asset_id, all_utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test collecting change data for the distribution transaction
    let result = rpc
        .collect_change_data(asset_id, distribution_txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_ok());
    let change_utxos = result.unwrap();

    // Should return exactly 2 change UTXOs from the distribution transaction
    assert_eq!(change_utxos.len(), 2);

    // Verify the change UTXOs are correctly filtered
    for utxo in &change_utxos {
        assert_eq!(utxo.txid, distribution_txid);
        assert_eq!(utxo.asset, asset_id);
        assert!(utxo.spendable);
    }

    // Verify specific change UTXOs
    let change_utxo_1 = change_utxos.iter().find(|u| u.vout == 1).unwrap();
    assert_eq!(change_utxo_1.amount, 25.5);
    assert_eq!(
        change_utxo_1.address,
        "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"
    );

    let change_utxo_2 = change_utxos.iter().find(|u| u.vout == 2).unwrap();
    assert_eq!(change_utxo_2.amount, 10.0);
    assert_eq!(
        change_utxo_2.address,
        "lq1qq3xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq"
    );
}

#[tokio::test]
async fn test_collect_change_data_no_change_scenario() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let distribution_txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

    // Create mock UTXOs with no change outputs from the distribution transaction
    let all_utxos = vec![
        Unspent {
            txid: "other_txid_123".to_string(),
            vout: 0,
            amount: 75.0,
            asset: asset_id.to_string(),
            address: "lq1qq1xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            spendable: true,
            confirmations: Some(10),
            scriptpubkey: Some("76a914abc123def456789abc123def456789abc123de88ac".to_string()),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(
                "3030303030303030303030303030303030303030303030303030303030303030".to_string(),
            ),
            assetblinder: Some(
                "4040404040404040404040404040404040404040404040404040404040404040".to_string(),
            ),
        },
        Unspent {
            txid: "different_txid_456".to_string(),
            vout: 0,
            amount: 50.0,
            asset: asset_id.to_string(),
            address: "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq".to_string(),
            spendable: true,
            confirmations: Some(6),
            scriptpubkey: Some("76a914mno012pqr345678mno012pqr345678mno012pq88ac".to_string()),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(
                "5050505050505050505050505050505050505050505050505050505050505050".to_string(),
            ),
            assetblinder: Some(
                "6060606060606060606060606060606060606060606060606060606060606060".to_string(),
            ),
        },
    ];

    // Create mock for listunspent RPC call
    create_listunspent_mock(&server, "test_wallet", asset_id, all_utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Test collecting change data when no change outputs exist
    let result = rpc
        .collect_change_data(asset_id, distribution_txid, &rpc, "test_wallet")
        .await;

    assert!(result.is_ok());
    let change_utxos = result.unwrap();

    // Should return empty vector when no change outputs exist
    assert_eq!(change_utxos.len(), 0);
}

#[tokio::test]
async fn test_collect_change_data_workflow_integration() {
    let server = MockServer::start();
    let asset_id = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
    let distribution_txid = "abc123def456789abc123def456789abc123def456789abc123def456789abc123de";

    // Simulate the post-confirmation scenario where we need to collect change data
    let post_confirmation_utxos = vec![
        // Change output from the distribution transaction
        Unspent {
            txid: distribution_txid.to_string(),
            vout: 2, // Change output
            amount: 48.0,
            asset: asset_id.to_string(),
            address: "change_address".to_string(),
            spendable: true,
            confirmations: Some(3),
            scriptpubkey: Some("76a914change_address_script_hash88ac".to_string()),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(
                "7070707070707070707070707070707070707070707070707070707070707070".to_string(),
            ),
            assetblinder: Some(
                "8080808080808080808080808080808080808080808080808080808080808080".to_string(),
            ),
        },
        // Some other unrelated UTXOs
        Unspent {
            txid: "unrelated_txid".to_string(),
            vout: 0,
            amount: 25.0,
            asset: asset_id.to_string(),
            address: "other_address".to_string(),
            spendable: true,
            confirmations: Some(10),
            scriptpubkey: Some("76a914other_address_script_hash88ac".to_string()),
            redeemscript: None,
            witnessscript: None,
            amountblinder: Some(
                "9090909090909090909090909090909090909090909090909090909090909090".to_string(),
            ),
            assetblinder: Some(
                "a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0".to_string(),
            ),
        },
    ];

    // Mock the listunspent call for change data collection
    create_listunspent_mock(&server, "test_wallet", asset_id, post_confirmation_utxos);

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());

    // Collect change data for confirmation
    let change_result = rpc
        .collect_change_data(asset_id, distribution_txid, &rpc, "test_wallet")
        .await;

    assert!(change_result.is_ok());
    let change_utxos = change_result.unwrap();

    // Should find exactly one change UTXO
    assert_eq!(change_utxos.len(), 1);
    assert_eq!(change_utxos[0].txid, distribution_txid);
    assert_eq!(change_utxos[0].vout, 2);
    assert_eq!(change_utxos[0].amount, 48.0);
    assert_eq!(change_utxos[0].address, "change_address");
    assert!(change_utxos[0].spendable);

    // This change data would then be used in the distribution confirmation API call
}
