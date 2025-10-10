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

/// # Panics
/// Panics if the request body cannot be parsed as JSON
pub fn mock_create_asset_assignments(server: &MockServer) {
    use serde_json::Value;

    server.mock(|when, then| {
        when.method(POST)
            .path("/assets/mock_asset_uuid/assignments/create")
            .header("content-type", "application/json")
            // Custom matcher to validate the request structure and data types
            .matches(|req| {
                // Parse the request body
                let body: Result<Value, _> = serde_json::from_slice(req.body.as_ref().unwrap());
                match body {
                    Ok(json) => {
                        // Validate that the request is wrapped in an "assignments" array
                        if let Some(assignments) = json.get("assignments") {
                            if let Some(assignments_array) = assignments.as_array() {
                                // Allow any number of assignments (1 or more)
                                if !assignments_array.is_empty() {
                                    // Validate each assignment
                                    for assignment in assignments_array {
                                        let has_registered_user = assignment
                                            .get("registered_user")
                                            .and_then(serde_json::Value::as_i64)
                                            .is_some();
                                        let has_amount = assignment
                                            .get("amount")
                                            .and_then(serde_json::Value::as_i64)
                                            .is_some();
                                        let vesting_timestamp_valid = assignment
                                            .get("vesting_timestamp")
                                            .is_none_or(|v| v.is_null() || v.is_i64()); // Optional field
                                        let ready_for_distribution_valid = assignment
                                            .get("ready_for_distribution")
                                            .is_none_or(serde_json::Value::is_boolean); // Optional field with default

                                        if !(has_registered_user
                                            && has_amount
                                            && vesting_timestamp_valid
                                            && ready_for_distribution_valid)
                                        {
                                            return false;
                                        }
                                    }
                                    return true;
                                }
                            }
                        }
                        false
                    }
                    Err(_) => false,
                }
            });
        then.status(200)
            .header("content-type", "application/json")
            // Response with single assignment for basic testing
            .json_body(json!([{
              "id": 10,
              "registered_user": 13,
              "amount": 100,
              "receiving_address": null,
              "distribution_uuid": null,
              "ready_for_distribution": true,
              "vesting_datetime": null,
              "vesting_timestamp": null,
              "has_vested": true,
              "is_distributed": false,
              "creator": 1,
              "GAID": "GA3DS3emT12zDF4RGywBvJqZfhefNp",
              "investor": 13
            }]));
    });
}

/// # Panics
/// Panics if the request body cannot be parsed as JSON
pub fn mock_create_asset_assignments_multiple(server: &MockServer) {
    use serde_json::Value;

    // First assignment request (amount: 100, user: 13)
    server.mock(|when, then| {
        when.method(POST)
            .path("/assets/mock_asset_uuid/assignments/create")
            .header("content-type", "application/json")
            .matches(|req| {
                let body: Result<Value, _> = serde_json::from_slice(req.body.as_ref().unwrap());
                body.is_ok_and(|json| {
                    json.get("assignments")
                        .and_then(|v| v.as_array())
                        .is_some_and(|assignments| {
                            assignments.len() == 1
                                && assignments[0].get("amount").and_then(serde_json::Value::as_i64)
                                    == Some(100)
                        })
                })
            });
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([
                {
                    "id": 10,
                    "registered_user": 13,
                    "amount": 100,
                    "receiving_address": null,
                    "distribution_uuid": null,
                    "ready_for_distribution": true,
                    "vesting_datetime": null,
                    "vesting_timestamp": null,
                    "has_vested": true,
                    "is_distributed": false,
                    "creator": 1,
                    "GAID": "GA3DS3emT12zDF4RGywBvJqZfhefNp",
                    "investor": 13
                }
            ]));
    });

    // Second assignment request (amount: 200, user: 14)
    server.mock(|when, then| {
        when.method(POST)
            .path("/assets/mock_asset_uuid/assignments/create")
            .header("content-type", "application/json")
            .matches(|req| {
                let body: Result<Value, _> = serde_json::from_slice(req.body.as_ref().unwrap());
                body.is_ok_and(|json| {
                    json.get("assignments")
                        .and_then(|v| v.as_array())
                        .is_some_and(|assignments| {
                            assignments.len() == 1
                                && assignments[0].get("amount").and_then(serde_json::Value::as_i64)
                                    == Some(200)
                        })
                })
            });
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([
                {
                    "id": 11,
                    "registered_user": 14,
                    "amount": 200,
                    "receiving_address": null,
                    "distribution_uuid": null,
                    "ready_for_distribution": true,
                    "vesting_datetime": null,
                    "vesting_timestamp": null,
                    "has_vested": true,
                    "is_distributed": false,
                    "creator": 1,
                    "GAID": "GA4DS3emT12zDF4RGywBvJqZfhefNp",
                    "investor": 14
                }
            ]));
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
        when.method(POST).path("/user/obtain_token");
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
        when.method(POST).path("/user/refresh_token");
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
                "GAID": "mock_gaid",
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
                "pubkey": "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
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
        when.method(POST)
            .path("/managers/1/assets/asset_uuid_1/remove");
        then.status(200);
    });

    server.mock(|when, then| {
        when.method(POST)
            .path("/managers/1/assets/asset_uuid_2/remove");
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

pub fn mock_lock_manager(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/managers/1/lock");
        then.status(200);
    });
}

pub fn mock_lock_manager_invalid_id(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/managers/999_999/lock");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Manager not found"
            }));
    });
}

pub fn mock_lock_manager_server_error(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/managers/1/lock");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Internal server error"
            }));
    });
}

pub fn mock_add_asset_to_manager(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/managers/1/assets/mock_asset_uuid/add");
        then.status(200);
    });
}

pub fn mock_add_asset_to_manager_invalid_manager_id(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/managers/999_999/assets/mock_asset_uuid/add");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Manager not found"
            }));
    });
}

pub fn mock_add_asset_to_manager_invalid_asset_uuid(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/managers/1/assets/invalid_asset_uuid/add");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Asset not found"
            }));
    });
}

pub fn mock_add_asset_to_manager_server_error(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/managers/1/assets/mock_asset_uuid/add");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Internal server error"
            }));
    });
}

pub fn mock_get_asset_assignment(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/assets/mock_asset_uuid/assignments/10");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 10,
                "registered_user": 13,
                "amount": 100,
                "receiving_address": null,
                "distribution_uuid": null,
                "ready_for_distribution": true,
                "vesting_datetime": null,
                "vesting_timestamp": null,
                "has_vested": true,
                "is_distributed": false,
                "creator": 1,
                "GAID": "GA3DS3emT12zDF4RGywBvJqZfhefNp",
                "investor": 13
            }));
    });
}

pub fn mock_unlock_manager(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/managers/1/unlock");
        then.status(200);
    });
}

pub fn mock_add_asset_treasury_addresses(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/assets/mock_asset_uuid/treasury-addresses/add")
            .header("content-type", "application/json");
        then.status(200);
    });
}

pub fn mock_get_asset_treasury_addresses(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/assets/mock_asset_uuid/treasury-addresses");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([
                "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw26",
                "vjU2i2EM2viGEzSywpStMPkTX9U9QSDsLSN63kJJYVpxKJZuxaph8v5r5Jf11aqnfBVdjSbrvcJ2pw27"
            ]));
    });
}

pub fn mock_delete_asset_assignment(server: &MockServer) {
    server.mock(|when, then| {
        when.method(DELETE)
            .path("/assets/mock_asset_uuid/assignments/10/delete");
        then.status(200);
    });
}

pub fn mock_lock_asset_assignment(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/assets/mock_asset_uuid/assignments/10/lock");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 10,
                "registered_user": 13,
                "amount": 100,
                "receiving_address": null,
                "distribution_uuid": null,
                "ready_for_distribution": true,
                "vesting_datetime": null,
                "vesting_timestamp": null,
                "has_vested": true,
                "is_distributed": false,
                "creator": 1,
                "GAID": "GA3DS3emT12zDF4RGywBvJqZfhefNp",
                "investor": 13
            }));
    });
}

pub fn mock_unlock_asset_assignment(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/assets/mock_asset_uuid/assignments/10/unlock");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 10,
                "registered_user": 13,
                "amount": 100,
                "receiving_address": null,
                "distribution_uuid": null,
                "ready_for_distribution": true,
                "vesting_datetime": null,
                "vesting_timestamp": null,
                "has_vested": true,
                "is_distributed": false,
                "creator": 1,
                "GAID": "GA3DS3emT12zDF4RGywBvJqZfhefNp",
                "investor": 13
            }));
    });
}
pub fn mock_lock_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/assets/mock_asset_uuid/lock");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "asset_uuid": "mock_asset_uuid",
                "name": "Mock Asset",
                "issuer": 1,
                "asset_id": "mock_asset_id",
                "reissuance_token_id": "mock_reissuance_token_id",
                "requirements": [],
                "ticker": "MOCK",
                "precision": 8,
                "domain": "example.com",
                "pubkey": "mock_pubkey",
                "is_registered": true,
                "is_authorized": true,
                "is_locked": true,
                "issuer_authorization_endpoint": "https://example.com/authorize",
                "transfer_restricted": true
            }));
    });
}

pub fn mock_unlock_asset(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT).path("/assets/mock_asset_uuid/unlock");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "asset_uuid": "mock_asset_uuid",
                "name": "Mock Asset",
                "issuer": 1,
                "asset_id": "mock_asset_id",
                "reissuance_token_id": "mock_reissuance_token_id",
                "requirements": [],
                "ticker": "MOCK",
                "precision": 8,
                "domain": "example.com",
                "pubkey": "mock_pubkey",
                "is_registered": true,
                "is_authorized": true,
                "is_locked": false,
                "issuer_authorization_endpoint": "https://example.com/authorize",
                "transfer_restricted": true
            }));
    });
}

pub fn mock_edit_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/registered_users/1/edit")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "name": "Updated User Name",
                "GAID": "mock_gaid",
                "is_company": false,
                "categories": [],
                "creator": 1
            }));
    });
}

pub fn mock_get_registered_user_summary(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/registered_users/1/summary");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "asset_uuid": "mock_asset_uuid",
                "asset_id": "mock_asset_id",
                "assignments": [{
                    "id": 1,
                    "registered_user": 1,
                    "amount": 1000,
                    "receiving_address": null,
                    "distribution_uuid": null,
                    "ready_for_distribution": true,
                    "vesting_datetime": null,
                    "vesting_timestamp": null,
                    "has_vested": true,
                    "is_distributed": false,
                    "creator": 1,
                    "GAID": "mock_gaid",
                    "investor": 1
                }],
                "assignments_sum": 1000,
                "distributions": [],
                "distributions_sum": 0,
                "balance": 1000
            }));
    });
}

pub fn mock_get_registered_user_gaids(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/registered_users/1/gaids");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([
                "GA44YYwPM8vuRMmjFL8i5kSqXhoTW2",
                "GAbYScu6jkWUND2jo3L4KJxyvo55d"
            ]));
    });
}

pub fn mock_add_gaid_to_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/registered_users/1/gaids/add")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });
}

pub fn mock_set_default_gaid_for_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/registered_users/1/gaids/set-default")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });
}

pub fn mock_get_gaid_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/gaids/GA44YYwPM8vuRMmjFL8i5kSqXhoTW2/registered_user");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "name": "Mock User",
                "GAID": "GA44YYwPM8vuRMmjFL8i5kSqXhoTW2",
                "is_company": false,
                "categories": [],
                "creator": 1
            }));
    });
}

pub fn mock_get_gaid_balance(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/gaids/GA44YYwPM8vuRMmjFL8i5kSqXhoTW2/balance");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!([
                {
                    "asset_uuid": "716cb816-6cc7-469d-a41f-f4ed1c0d2dce",
                    "asset_id": "5b72739ee4097c32e9eb2fa5f43fd51b35e13323e58c511d6da91adbc4ac24ca",
                    "balance": 0
                },
                {
                    "asset_uuid": "5fd36bad-f0af-4b13-a0b5-fb1a91b751a4",
                    "asset_id": "ae4bfd3b5dc9d6d1dc77e1c8840fa06b4e9abeabec024cf1d9efb96935757be0",
                    "balance": 0
                }
            ]));
    });
}

pub fn mock_get_gaid_asset_balance(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/gaids/GA44YYwPM8vuRMmjFL8i5kSqXhoTW2/balance/mock_asset_uuid");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "asset_uuid": "mock_asset_uuid",
                "asset_id": "mock_asset_id",
                "balance": 100_000
            }));
    });
}

pub fn mock_add_categories_to_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/registered_users/1/categories/add")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });
}

pub fn mock_remove_categories_from_registered_user(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/registered_users/1/categories/delete")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });
}

pub fn mock_get_asset_memo(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/assets/mock_asset_uuid/memo");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!("Sample memo for mock asset"));
    });
}

pub fn mock_set_asset_memo(server: &MockServer) {
    server.mock(|when, then| {
        when.method(POST)
            .path("/assets/mock_asset_uuid/memo/set")
            .header("content-type", "application/json");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
    });
}

pub fn mock_add_asset_to_category(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/categories/1/assets/mock_asset_uuid/add");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "name": "Mock Category",
                "description": "A mock category",
                "registered_users": [],
                "assets": ["mock_asset_uuid"]
            }));
    });
}

pub fn mock_remove_asset_from_category(server: &MockServer) {
    server.mock(|when, then| {
        when.method(PUT)
            .path("/categories/1/assets/mock_asset_uuid/remove");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "id": 1,
                "name": "Mock Category",
                "description": "A mock category",
                "registered_users": [],
                "assets": []
            }));
    });
}
pub fn mock_get_asset_assignment_invalid_asset_uuid(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/assets/invalid_asset_uuid/assignments/10");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Asset not found"
            }));
    });
}

pub fn mock_get_asset_assignment_invalid_assignment_id(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/assets/mock_asset_uuid/assignments/999_999");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Assignment not found"
            }));
    });
}

pub fn mock_get_asset_assignment_non_existent(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/assets/non_existent_asset/assignments/non_existent_assignment");
        then.status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Assignment not found"
            }));
    });
}

pub fn mock_get_asset_assignment_server_error(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET)
            .path("/assets/mock_asset_uuid/assignments/10");
        then.status(500)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": "Internal server error"
            }));
    });
}
