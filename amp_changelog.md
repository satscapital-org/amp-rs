# AMP API Changelog

```json
{
  "0.1.0": "Reset API version to \"0.0.1\", which was previously set incorrectly.\n\nDeprecate endpoint /assets/{assetUuid}/assignments/{assignmentId}/edit.\nAdd new endpoints /assets/{assetUuid}/assignments/{assignmentId}/[,un]lock as a partial replacement.\n\nBump minimum supported client script version to \"0.0.2\".\nRemove \"distribution_status\" field from assignments details, add boolean field \"is_distributed\" as a replacement.\n\nAdd chance to specify \"vesting_timestamp\" when creating an assignment, add fields \"vesting_datetime\" and \"has_vested\" to assignments details.\n\nAdd new endpoint /changelog.\n\nDeprecate /investors/validate-gaid.\nAdd new endpoint (GET) /gaids/{gaid}/validate as a replacement.\nMove all /investors/categories* endpoints to /categories.\nAccept an array rather than an array wrapped in a dict in endpoints used to associate investors and assets to categories.\nAdd missing endpoints to (de)associate categories from assets or investors: /categories/{id}/[investors,assets]/[add,delete], /[investors,assets]/{id}/categories/[add,delete].\n\nAdd endpoints /asset/{assetUuid}/memo and /asset/{assetUuid}/memo/set to getand set a per asset memo.\n",
  "0.1.1": "Rename asset field \"authorizer_endpoint\" to \"issuer_authorization_endpoint\".\nAdditionally, such endpoint will be queried when authorizing transactions and can be used to change the authorization result.",
  "0.1.10": "Add new endpoint /assets/{assetUuid}/txs for list of transactions involved with the asset.\nAdd new endpoint /assets/{assetUuid}/txs/{txid} for a single transaction involved with the asset.\n",
  "0.1.11": "/assets/{assetUuid}/txs[,/{txid}], add \"asset_id\", fix date format, add reissuance token data.\n",
  "0.1.12": "/assets/{assetUuid}/txs[,/{txid}], add \"unblinded_url\".\nAdd new endpoint /assets/{assetUuid}/update-blinders.\n",
  "0.1.13": "Add field \"GAID\" for /assets/{assetUuid}/assignments[,/{assignmentId}].\n",
  "0.1.2": "Allow to specify asset registry precision at issuance. Add precision to issuance and asset details.",
  "0.1.3": "Added blacklist token counter in /assets/{assetUuid}/summary.",
  "0.1.4": "Add \"is_locked\" field to asset details.\nAdd new endpoints /assets/{assetUuid}/[,un]lock to change \"is_locked\".\nAdd new endpoint /gaids/{gaid}/address to obtain address for gaid.",
  "0.1.5": "Add new endpoint /assets/{assetUuid}/lost-outputs returns the lists of outputs that the server is now unable to track.\nAdd new endpoint /user/refresh-token to renew the authentication token.\nAdd new endpoint /user/change-password to change the password for the current user.\nRemove deprecated endpoint /assets/{assetUuid}/assignments/{assignmentId}/edit.\nAdd new manager endpoints. See specification file for details.",
  "0.1.6": "Deprecate endpoints to associate categories to assets and investors. Add new endpoints to replace them.\nAdd new endpoint /gaids/{gaid}/investor to get investor from gaid.",
  "0.1.7": "Removed /investors/validate-gaid.\nRemoved /investors/{investorId}/categories/add and /investors/{investorId}/categories/delete\nRemoved /categories/{categoryId}/investors/add and /categories/{categoryId}/investors/delete\nRemoved /categories/{categoryId}/assets/add and /categories/{categoryId}/assets/delete\nRemoved /assets/{assetUuid}/categories/add and /assets/{assetUuid}/categories/delete\n/assets/{assetUuid}/assignment/create does not accept multiple assignments.\nRenamed all occurrences of \"investor\" to \"registered_user\". Users do not need to upgrade immediately, as the old endpoints as well as the values returned have been duplicated. However they are encouraged to upgrade as soon as they can, as the duplicate will be maintained only for a short period.\n",
  "0.1.8": "Added \"vesting_timestamp\" to assignment details.\nAdded \"reissuance_token_id\" to asset details.\n",
  "0.1.9": "Added \"amount_blinder\" and \"asset_blinder\" to activities list.\n"
}
```
