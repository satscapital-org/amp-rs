# Implementation Plan

- [ ] 1. Remove deprecated models from model.rs
  - Remove AssetGroup, CreateAssetGroup, UpdateAssetGroup, and AddAssetToGroup struct definitions
  - Remove AssetPermission, CreateAssetPermission, and UpdateAssetPermission struct definitions  
  - Remove Audit, CreateAudit, and UpdateAudit struct definitions
  - Remove any asset_group fields from remaining models
  - Verify the file compiles after model removal
  - _Requirements: 1.1, 2.1, 2.5, 3.1_

- [ ] 2. Update client.rs import statements
  - Remove deprecated model imports from the use statement in client.rs
  - Remove AssetGroup, AssetPermission, Audit and related models from imports
  - Remove CreateAssetGroup, CreateAssetPermission, CreateAudit and related models from imports
  - Remove UpdateAssetGroup, UpdateAssetPermission, UpdateAudit and related models from imports
  - Remove AddAssetToGroup from imports
  - Verify the file compiles after import cleanup
  - _Requirements: 4.5_

- [ ] 3. Remove asset group client methods
  - Remove list_asset_groups method from ApiClient impl
  - Remove create_asset_group method from ApiClient impl
  - Remove get_asset_group method from ApiClient impl
  - Remove update_asset_group method from ApiClient impl
  - Remove delete_asset_group method from ApiClient impl
  - Remove add_asset_to_group method from ApiClient impl
  - Verify the file compiles after method removal
  - _Requirements: 1.2_

- [ ] 4. Remove asset permission client methods
  - Remove list_asset_permissions method from ApiClient impl
  - Remove create_asset_permission method from ApiClient impl
  - Remove get_asset_permission method from ApiClient impl
  - Remove update_asset_permission method from ApiClient impl
  - Remove delete_asset_permission method from ApiClient impl
  - Verify the file compiles after method removal
  - _Requirements: 2.2_

- [ ] 5. Remove audit client methods
  - Remove list_audits method from ApiClient impl
  - Remove create_audit method from ApiClient impl
  - Remove get_audit method from ApiClient impl
  - Remove update_audit method from ApiClient impl
  - Remove delete_audit method from ApiClient impl
  - Verify the file compiles after method removal
  - _Requirements: 3.2_

- [ ] 6. Remove deprecated mock functions from mocks.rs
  - Remove mock_list_asset_groups function
  - Remove mock_create_asset_group function
  - Remove mock_get_asset_group function
  - Remove mock_update_asset_group function
  - Remove mock_delete_asset_group function
  - Remove mock_add_asset_to_group function
  - Remove mock_list_audits function
  - Remove mock_update_audit function
  - Remove mock_delete_audit function
  - Verify the file compiles after mock removal
  - _Requirements: 1.3, 2.3, 3.3_

- [ ] 7. Remove deprecated tests from tests/api.rs
  - Remove test_list_asset_groups_live test function
  - Remove test_create_and_delete_asset_group_live test function
  - Remove test_get_and_update_asset_group_live test function
  - Remove test_list_asset_groups_mock test function
  - Remove test_create_asset_group_mock test function
  - Remove test_get_asset_group_mock test function
  - Remove test_update_asset_group_mock test function
  - Remove test_delete_asset_group_mock test function
  - Remove test_add_asset_to_group_mock test function
  - Remove test_create_and_delete_audit_live test function
  - Remove test_get_and_update_audit_live test function
  - Remove test_update_audit_mock test function
  - Remove test_delete_audit_mock test function
  - Verify the file compiles after test removal
  - _Requirements: 1.4, 2.4, 3.4, 4.3_

- [ ] 8. Update README.md examples
  - Remove asset group example from README.md
  - Update any remaining examples to only reference supported functionality
  - Ensure all code examples in README compile and reference valid methods
  - _Requirements: 4.4_

- [ ] 9. Update product.md steering file
  - Remove references to "Asset groups and permissions" from the feature list
  - Remove references to "Audit functionality" from the feature list
  - Update the product overview to reflect only supported functionality
  - _Requirements: 5.1, 5.2_

- [ ] 10. Final verification and testing
  - Run cargo build to ensure project compiles successfully
  - Run cargo test to verify remaining functionality works
  - Confirm that approximately 10 test failures exist for remaining functionality
  - Verify no tests exist for removed deprecated functionality
  - _Requirements: 1.5, 2.5, 3.5, 4.1, 4.2, 4.3_