// Integration tests for ElementsRpc wallet-specific methods
use amp_rs::{ElementsRpc, TxInput};
use httpmock::prelude::*;
use serde_json::json;
use std::collections::HashMap;

// Tests for create_raw_transaction() method
#[tokio::test]
async fn test_create_raw_transaction_single_input_output() {
    let server = MockServer::start();

    // Mock createrawtransaction
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("createrawtransaction");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "0200000000010123456789abcdef1234567890abcdef1234567890abcdef1234567890abcdef00000000000000000001"
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    
    let inputs = vec![TxInput {
        txid: "test_txid".to_string(),
        vout: 0,
        sequence: None,
    }];
    
    let mut outputs = HashMap::new();
    outputs.insert("recipient_address".to_string(), 100.0);
    
    let mut assets = HashMap::new();
    assets.insert("recipient_address".to_string(), "asset_id".to_string());
    
    let result = rpc.create_raw_transaction(inputs, outputs, assets).await;
    
    assert!(result.is_ok());
    let hex = result.unwrap();
    assert!(!hex.is_empty());
}

#[tokio::test]
async fn test_create_raw_transaction_multiple_inputs_outputs() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("createrawtransaction");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "02000000000102..."
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    
    let inputs = vec![
        TxInput {
            txid: "txid1".to_string(),
            vout: 0,
            sequence: None,
        },
        TxInput {
            txid: "txid2".to_string(),
            vout: 1,
            sequence: None,
        },
    ];
    
    let mut outputs = HashMap::new();
    outputs.insert("address1".to_string(), 50.0);
    outputs.insert("address2".to_string(), 30.0);
    
    let mut assets = HashMap::new();
    assets.insert("address1".to_string(), "asset1".to_string());
    assets.insert("address2".to_string(), "asset2".to_string());
    
    let result = rpc.create_raw_transaction(inputs, outputs, assets).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_raw_transaction_empty_inputs_error() {
    let rpc = ElementsRpc::new(
        "http://localhost:18884".to_string(),
        "user".to_string(),
        "pass".to_string(),
    );
    
    let inputs = vec![];
    let outputs = HashMap::new();
    let assets = HashMap::new();
    
    let result = rpc.create_raw_transaction(inputs, outputs, assets).await;
    
    assert!(result.is_err());
}

// Tests for blind_raw_transaction() method
#[tokio::test]
async fn test_blind_raw_transaction_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";

    // Mock loadwallet
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "loadwallet",
                "params": [wallet_name]
            }));
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name, "warning": ""}
            }));
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("blindrawtransaction");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "blinded_transaction_hex"
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    
    let hex = "0200000000010123456789abcdef";
    
    let result = rpc.blind_raw_transaction(wallet_name, hex).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "blinded_transaction_hex");
}

// Tests for wallet management operations
#[tokio::test]
async fn test_create_wallet_standard() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("createwallet");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {
                    "name": wallet_name,
                    "warning": ""
                }
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.create_wallet(wallet_name, false).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_wallet_watch_only() {
    let server = MockServer::start();
    let wallet_name = "watch_only_wallet";

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("createwallet");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {
                    "name": wallet_name,
                    "warning": ""
                }
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.create_wallet(wallet_name, true).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_load_wallet_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "loadwallet",
                "params": [wallet_name]
            }));
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name, "warning": ""}
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.load_wallet(wallet_name).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_unload_wallet_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("unloadwallet");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": null
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.unload_wallet(wallet_name).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_list_wallets_multiple() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("listwallets");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": ["wallet1", "wallet2", "wallet3"]
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.list_wallets().await;
    
    assert!(result.is_ok());
    let wallets = result.unwrap();
    assert_eq!(wallets.len(), 3);
    assert!(wallets.contains(&"wallet1".to_string()));
}

#[tokio::test]
async fn test_list_wallets_empty() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("listwallets");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": []
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.list_wallets().await;
    
    assert!(result.is_ok());
    let wallets = result.unwrap();
    assert_eq!(wallets.len(), 0);
}

#[tokio::test]
async fn test_create_elements_wallet_success() {
    let server = MockServer::start();
    let wallet_name = "elements_wallet";

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("createwallet");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name}
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.create_elements_wallet(wallet_name).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_create_descriptor_wallet_success() {
    let server = MockServer::start();
    let wallet_name = "descriptor_wallet";

    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("createwallet");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name}
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.create_descriptor_wallet(wallet_name).await;
    
    assert!(result.is_ok());
}

// Tests for descriptor and key management
#[tokio::test]
async fn test_import_descriptor_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";
    let descriptor = "wpkh([d34db33f/84h/0h/0h]xpub.../0/*)";

    server.mock(|when, then| {
        when.method(POST)
            .path(&format!("/wallet/{}", wallet_name))
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("importdescriptors");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": [{"success": true}]
            }));
    });

    let rpc = ElementsRpc::new(server.url("/").trim_end_matches('/').to_string(), "user".to_string(), "pass".to_string());
    let result = rpc.import_descriptor(wallet_name, descriptor).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_dump_private_key_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";
    let address = "lq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f9lq";

    // Mock loadwallet
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "loadwallet",
                "params": [wallet_name]
            }));
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name, "warning": ""}
            }));
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("dumpprivkey");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "cPrivKeyHex123456789abcdef"
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.dump_private_key(wallet_name, address).await;
    
    assert!(result.is_ok());
    let private_key = result.unwrap();
    assert!(!private_key.is_empty());
}

#[tokio::test]
async fn test_import_private_key_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";
    let private_key = "cPrivKeyHex123456789abcdef";

    // Mock loadwallet
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "loadwallet",
                "params": [wallet_name]
            }));
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name, "warning": ""}
            }));
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("importprivkey");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": null
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.import_private_key(wallet_name, private_key, None, None).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_confidential_address_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";
    let unconfidential_address = "ert1qxxx";

    // Mock loadwallet
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "loadwallet",
                "params": [wallet_name]
            }));
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name, "warning": ""}
            }));
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("getaddressinfo");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {
                    "confidential": "lq1qqxxx"
                }
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.get_confidential_address(wallet_name, unconfidential_address).await;
    
    assert!(result.is_ok());
    let confidential = result.unwrap();
    assert_eq!(confidential, "lq1qqxxx");
}

#[tokio::test]
async fn test_get_unconfidential_address_success() {
    let server = MockServer::start();
    let wallet_name = "test_wallet";
    let confidential_address = "lq1qqxxx";

    // Mock loadwallet
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "method": "loadwallet",
                "params": [wallet_name]
            }));
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": {"name": wallet_name, "warning": ""}
            }));
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/wallet/test_wallet")
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body_contains("getunconfidentialaddress");
        then.status(200)
            .json_body(json!({
                "jsonrpc": "1.0",
                "id": "amp-client",
                "result": "ert1qxxx"
            }));
    });

    let rpc = ElementsRpc::new(server.url("/"), "user".to_string(), "pass".to_string());
    let result = rpc.get_unconfidential_address(wallet_name, confidential_address).await;
    
    assert!(result.is_ok());
    let unconfidential = result.unwrap();
    assert_eq!(unconfidential, "ert1qxxx");
}

