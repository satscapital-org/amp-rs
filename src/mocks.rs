use httpmock::prelude::*;
use serde_json::json;

pub fn mock_get_changelog(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/changelog");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "0.1.0": {
                    "added": [
                        "Initial release"
                    ]
                }
            }));
    });
}

pub fn mock_create_asset_assignment(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/assets/mock_asset_uuid/assignments");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
              "id": 10,
              "registered_user": 13,
              "amount": 100,
              "receiving_address": "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26",
              "distribution_uuid": null,
              "ready_for_distribution": true,
              "vesting_datetime": null,
              "vesting_timestamp": null,
              "has_vested": true,
              "is_distributed": false,
              "creator": 1
            }));
    });
}



pub fn mock_broadcast_transaction(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/tx/broadcast");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "txid": "mock_txid",
                "hex": "mock_tx_hex"
            }));
    });
}

pub fn mock_get_broadcast_status(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/tx/broadcast/mock_txid");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "txid": "mock_txid",
                "hex": "mock_tx_hex"
            }));
    });
}





pub fn mock_remove_asset_from_group(server: &MockServer) {
    server.mock(|when, then| {
        when.method(DELETE)
            .path("/asset_groups/1/assets/mock_asset_uuid");
        then.status(200);
    });
}

pub fn mock_get_managers(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/managers");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([{
                "id": 1,
                "username": "mock_manager",
                "is_locked": false,
                "assets": []
            }]));
    });
}

pub fn mock_create_manager(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/managers/create");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 2,
                "username": "test_manager",
                "is_locked": false,
                "assets": []
            }));
    });
}

pub fn mock_obtain_token(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "token": "mock_token"
            }));
    });
}

pub fn mock_refresh_token(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/refresh_token")
            .header("authorization", "token mock_token");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "token": "mock_refreshed_token"
            }));
    });
}

pub fn mock_obtain_token_with_rate_limiting(server: &MockServer, retry_after_seconds: u64) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(429)
            .header("retry-after", retry_after_seconds.to_string())
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Too Many Requests"
            }));
    });
}

pub fn mock_obtain_token_server_error(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/obtain_token")
            .header("content-type", "application/json");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Internal Server Error"
            }));
    });
}

pub fn mock_refresh_token_failure(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/user/refresh_token");
        then.status(401)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Invalid token"
            }));
    });
}

pub fn mock_get_gaid_address(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/gaids/GAbYScu6jkWUND2jo3L4KJxyvo55d/address");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "address": "mock_address"
            }));
    });
}

pub fn mock_validate_gaid(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/gaids/GAbYScu6jkWUND2jo3L4KJxyvo55d/validate");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "is_valid": true
            }));
    });
}

pub fn mock_get_categories(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/categories");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([{
                "id": 1,
                "name": "Mock Category",
                "description": "A mock category",
                "registered_users": [],
                "assets": []
            }]));
    });
}

pub fn mock_add_category(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/categories/add");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 2,
                "name": "Test Category",
                "description": "Test category description",
                "registered_users": [],
                "assets": []
            }));
    });
}

pub fn mock_add_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/registered_users/add");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 2,
                "name": "Test User",
                "gaid": null,
                "is_company": false,
                "authorization_url": "https://example.com/auth_new",
                "categories": [],
                "creator": 1
            }));
    });
}

pub fn mock_delete_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(DELETE)
            .path("/assets/new_mock_asset_uuid/delete");
        then.status(200);
    });
}

pub fn mock_get_registered_users(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/registered_users");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([{
                "id": 1,
                "name": "Mock User",
                "gaid": "mock_gaid",
                "is_company": false,
                "authorization_url": "https://example.com/auth",
                "categories": [],
                "creator": 1
            }]));
    });
}

pub fn mock_get_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/registered_users/1");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "name": "Mock User",
                "gaid": "mock_gaid",
                "is_company": false,
                "authorization_url": "https://example.com/auth",
                "categories": [],
                "creator": 1
            }));
    });
}

pub fn mock_edit_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/assets/mock_asset_uuid/edit")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "Mock Asset",
                "asset_uuid": "mock_asset_uuid",
                "issuer": 1,
                "asset_id": "mock_asset_id",
                "reissuance_token_id": null,
                "requirements": [],
                "ticker": "MOCK",
                "precision": 8,
                "domain": "mock.com",
                "pubkey": "mock_pubkey",
                "is_registered": true,
                "is_authorized": true,
                "is_locked": false,
                "issuer_authorization_endpoint": "https://example.com/authorize",
                "transfer_restricted": true
            }));
    });
}

pub fn mock_issue_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/assets/issue");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "Test Asset",
                "amount": 1000,
                "destination_address": "destination_address",
                "domain": "example.com",
                "ticker": "TSTA",
                "pubkey": "03...",
                "is_confidential": true,
                "is_reissuable": false,
                "reissuance_amount": 0,
                "reissuance_address": "reissuance_address",
                "asset_id": "mock_asset_id",
                "reissuance_token_id": null,
                "asset_uuid": "new_mock_asset_uuid",
                "txid": "mock_txid",
                "vin": 0,
                "asset_vout": 0,
                "reissuance_vout": null,
                "issuer_authorization_endpoint": null,
                "transfer_restricted": true,
                "issuance_assetblinder": "mock_blinder",
                "issuance_tokenblinder": null
            }));
    });
}

pub fn mock_get_assets(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/assets");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([{
                "name": "Mock Asset",
                "asset_uuid": "mock_asset_uuid",
                "issuer": 1,
                "asset_id": "mock_asset_id",
                "reissuance_token_id": null,
                "requirements": [],
                "ticker": "MOCK",
                "precision": 8,
                "domain": "mock.com",
                "pubkey": "mock_pubkey",
                "is_registered": true,
                "is_authorized": true,
                "is_locked": false,
                "issuer_authorization_endpoint": null,
                "transfer_restricted": true
            }]));
    });
}

pub fn mock_get_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/assets/mock_asset_uuid");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "Mock Asset",
                "asset_uuid": "mock_asset_uuid",
                "issuer": 1,
                "asset_id": "mock_asset_id",
                "reissuance_token_id": null,
                "requirements": [],
                "ticker": "MOCK",
                "precision": 8,
                "domain": "mock.com",
                "pubkey": "mock_pubkey",
                "is_registered": true,
                "is_authorized": true,
                "is_locked": false,
                "issuer_authorization_endpoint": "https://example.com/authorize",
                "transfer_restricted": true
            }));
    });
}

pub fn mock_get_manager(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/managers/1");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "username": "mock_manager",
                "is_locked": false,
                "assets": ["asset_uuid_1", "asset_uuid_2"]
            }));
    });
}

pub fn mock_manager_remove_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST).path("/managers/1/assets/asset_uuid_1/remove");
        then.status(200);
    });
    
    server.mock(|when, then| {
        when.method(POST).path("/managers/1/assets/asset_uuid_2/remove");
        then.status(200);
    });
}

pub fn mock_get_current_manager_raw(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/managers/me");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "username": "current_manager",
                "is_locked": false,
                "assets": ["asset_uuid_1"]
            }));
    });
}

pub fn mock_unlock_manager(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/managers/1/unlock");
        then.status(200);
    });
}
