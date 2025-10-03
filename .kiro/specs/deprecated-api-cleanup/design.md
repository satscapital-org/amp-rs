# Design Document

## Overview

This design outlines the systematic removal of deprecated API functionality from the AMP Rust client library. The deprecated functionality includes asset groups, asset permissions, and audit-related endpoints. The removal will be performed in a structured manner to ensure the remaining functionality continues to work correctly while eliminating all traces of the deprecated code.

## Architecture

The AMP client library follows a modular architecture with the following key components:

- **client.rs**: Contains the main `ApiClient` struct with HTTP methods for API endpoints
- **model.rs**: Contains all data structures, request/response models, and serialization logic
- **mocks.rs**: Contains mock implementations for testing
- **lib.rs**: Library entry point that re-exports main types
- **tests/**: Integration tests for API functionality
- **examples/**: Usage examples
- **README.md**: Documentation with usage examples

The deprecated functionality is distributed across all these components and needs to be systematically removed.

## Components and Interfaces

### Deprecated Models to Remove

From `src/model.rs`:
- `AssetGroup` - Asset group response model
- `CreateAssetGroup` - Asset group creation request
- `UpdateAssetGroup` - Asset group update request  
- `AddAssetToGroup` - Request to add asset to group
- `AssetPermission` - Asset permission response model
- `CreateAssetPermission` - Asset permission creation request
- `UpdateAssetPermission` - Asset permission update request
- `Audit` - Audit response model
- `CreateAudit` - Audit creation request
- `UpdateAudit` - Audit update request

### Deprecated Client Methods to Remove

From `src/client.rs`:
- `list_asset_groups()` - List all asset groups
- `create_asset_group()` - Create new asset group
- `get_asset_group()` - Get specific asset group
- `update_asset_group()` - Update asset group
- `delete_asset_group()` - Delete asset group
- `add_asset_to_group()` - Add asset to group
- `list_asset_permissions()` - List asset permissions
- `create_asset_permission()` - Create asset permission
- `get_asset_permission()` - Get specific asset permission
- `update_asset_permission()` - Update asset permission
- `delete_asset_permission()` - Delete asset permission
- `list_audits()` - List audits
- `create_audit()` - Create audit
- `get_audit()` - Get specific audit
- `update_audit()` - Update audit
- `delete_audit()` - Delete audit

### Deprecated Mock Functions to Remove

From `src/mocks.rs`:
- `mock_list_asset_groups()`
- `mock_create_asset_group()`
- `mock_get_asset_group()`
- `mock_update_asset_group()`
- `mock_delete_asset_group()`
- `mock_add_asset_to_group()`
- `mock_list_audits()`
- `mock_update_audit()`
- `mock_delete_audit()`

### Deprecated Tests to Remove

From `tests/api.rs`:
- All asset group related tests (live and mock)
- All asset permission related tests
- All audit related tests

### Fields to Remove from Remaining Models

- Remove `asset_group` fields from `CreateAssetPermission` and `UpdateAssetPermission` models (if they remain in other contexts)

## Data Models

### Remaining Core Models
The following models will remain unchanged:
- `Asset` - Core asset model
- `RegisteredUserResponse` - User management
- `CategoryResponse` - Category management
- `Manager` - Manager functionality
- `Balance`, `Ownership`, `Activity` - Asset tracking
- `TokenRequest`, `TokenResponse` - Authentication

### Import Statement Updates
The import statements in `src/client.rs` need to be updated to remove references to deprecated models:

```rust
// Remove these from imports:
// AssetGroup, AssetPermission, Audit, 
// CreateAssetGroup, CreateAssetPermission, CreateAudit,
// UpdateAssetGroup, UpdateAssetPermission, UpdateAudit,
// AddAssetToGroup
```

## Error Handling

No changes to error handling are required as the existing `Error` enum covers all necessary error cases for the remaining functionality. The removal of deprecated functionality will not impact the error handling strategy.

## Testing Strategy

### Test Removal Strategy
1. Remove all tests related to deprecated functionality
2. Ensure remaining tests continue to pass
3. Verify that approximately 10 test failures remain for existing functionality (these are pre-existing issues unrelated to the deprecated code removal)

### Compilation Verification
After each major removal step, the project must compile successfully to ensure no broken dependencies remain.

### Mock Cleanup
All mock functions related to deprecated functionality will be removed, ensuring the mock server setup remains clean and only supports active endpoints.

## Implementation Approach

### Phase 1: Model Cleanup
- Remove deprecated model structs from `src/model.rs`
- Update import statements in `src/client.rs`

### Phase 2: Client Method Removal
- Remove deprecated client methods from `src/client.rs`
- Ensure remaining methods are unaffected

### Phase 3: Mock and Test Cleanup
- Remove deprecated mock functions from `src/mocks.rs`
- Remove deprecated tests from `tests/api.rs`

### Phase 4: Documentation Updates
- Update README.md to remove deprecated functionality examples
- Update product.md steering file
- Verify all references are removed

### Phase 5: Final Verification
- Ensure project compiles
- Run remaining tests to verify functionality
- Confirm approximately 10 test failures remain for existing functionality

## Dependencies

No external dependencies need to be modified. The cleanup only involves removing code, not adding new functionality or dependencies.

## Security Considerations

The removal of deprecated functionality does not introduce any security concerns. The remaining authentication and secure credential handling mechanisms remain unchanged.