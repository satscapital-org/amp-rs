# amp-rs Error Inventory

This document provides a comprehensive inventory of all error types in the amp-rs crate and identifies opportunities for enhancement similar to the `ResponseDeserializationFailed` improvement.

## Summary

**Total Error Enums:** 4
**Total Error Variants:** 27

## Error Enums

### 1. `Error` (src/client.rs)
**Purpose:** Main API client errors  
**Variants:** 9

| Variant | Current State | Enhancement Opportunity |
|---------|--------------|------------------------|
| `MissingEnvVar(String)` | ✅ Simple - includes var name | None needed |
| `RequestFailed(String)` | ⚠️ Basic string message | Could add HTTP method, endpoint, status code |
| `ResponseParsingFailed(String)` | ⚠️ Legacy variant | Kept for backwards compatibility |
| `ResponseDeserializationFailed { ... }` | ✅ **ENHANCED** | Already includes method, endpoint, type, error, raw response |
| `TokenRequestFailed { status, error_text }` | ✅ Good - includes status and error | None needed |
| `UrlParse(url::ParseError)` | ✅ Wraps external error | None needed |
| `Reqwest(reqwest::Error)` | ✅ Wraps external error | None needed |
| `InvalidRetryConfig(String)` | ✅ Simple - includes reason | None needed |
| `Token(TokenError)` | ✅ Wraps TokenError | None needed |

**Enhancement Candidates:** 1
- `RequestFailed` could be enhanced to include more context similar to `ResponseDeserializationFailed`

---

### 2. `AmpError` (src/client.rs)
**Purpose:** Distribution operations and ElementsRpc errors  
**Variants:** 8

| Variant | Current State | Enhancement Opportunity |
|---------|--------------|------------------------|
| `Api(String)` | ⚠️ Generic string | Could add endpoint, request details |
| `Rpc(String)` | ⚠️ Generic string | Could add RPC method, params, raw response |
| `Signer(SignerError)` | ✅ Wraps SignerError | None needed |
| `Timeout(String)` | ✅ Includes message | Could add duration, operation type |
| `Validation(String)` | ✅ Includes message | Could add field name, expected vs actual |
| `Network(reqwest::Error)` | ✅ Wraps external error | None needed |
| `Serialization(serde_json::Error)` | ✅ Wraps external error | Could add context similar to ResponseDeserializationFailed |
| `Existing(Error)` | ✅ Wraps Error | None needed |

**Enhancement Candidates:** 4
- `Api` - Could include endpoint, HTTP details
- `Rpc` - Could include RPC method, parameters, raw response
- `Serialization` - Could include what was being serialized/deserialized
- `Timeout` - Could include more structured timing information

---

### 3. `TokenError` (src/client.rs)
**Purpose:** Token management operations  
**Variants:** 7

| Variant | Current State | Enhancement Opportunity |
|---------|--------------|------------------------|
| `RefreshFailed(String)` | ✅ Includes reason | None needed |
| `ObtainFailed { attempts, last_error }` | ✅ Good - includes attempts and error | None needed |
| `RateLimited { retry_after_seconds }` | ✅ Good - includes retry info | None needed |
| `Timeout { timeout_seconds }` | ✅ Good - includes duration | None needed |
| `Serialization(String)` | ✅ Includes error message | None needed |
| `Storage(String)` | ✅ Includes error message | Could add file path, operation type |
| `Validation(String)` | ✅ Includes error message | None needed |

**Enhancement Candidates:** 1
- `Storage` - Could include file path, operation type (read/write/delete)

---

### 4. `SignerError` (src/signer/error.rs)
**Purpose:** Transaction signing operations  
**Variants:** 6

| Variant | Current State | Enhancement Opportunity |
|---------|--------------|------------------------|
| `Lwk(String)` | ⚠️ Generic string | Could add operation type, transaction details |
| `InvalidMnemonic(String)` | ✅ Includes reason | Could add word count, invalid word index |
| `HexParse(hex::FromHexError)` | ✅ Wraps external error | Could add context about what was being parsed |
| `InvalidTransaction(String)` | ⚠️ Generic string | Could add transaction ID, input/output details |
| `Network(reqwest::Error)` | ✅ Wraps external error | None needed |
| `Serialization(serde_json::Error)` | ✅ Wraps external error | Could add context similar to ResponseDeserializationFailed |
| `FileIo(std::io::Error)` | ✅ Wraps external error | Could add file path, operation type |

**Enhancement Candidates:** 4
- `Lwk` - Could include operation type, transaction hex
- `InvalidTransaction` - Could include transaction ID, specific validation failure details
- `Serialization` - Could include what was being serialized
- `FileIo` - Could include file path and operation type

---

## Total Enhancement Opportunities

**High Priority (Network/API related):**
1. `Error::RequestFailed` - Add HTTP method, endpoint, status code
2. `AmpError::Rpc` - Add RPC method, params, raw response
3. `AmpError::Serialization` - Add serialization context

**Medium Priority (Operational context):**
4. `AmpError::Api` - Add endpoint and request details
5. `SignerError::Lwk` - Add operation type
6. `SignerError::InvalidTransaction` - Add transaction details

**Low Priority (File/Storage operations):**
7. `TokenError::Storage` - Add file path and operation
8. `SignerError::FileIo` - Add file path and operation
9. `SignerError::HexParse` - Add parsing context
10. `SignerError::Serialization` - Add serialization context

---

## Enhancement Pattern

Based on the successful `ResponseDeserializationFailed` implementation, the enhancement pattern should include:

### For Network/API Errors:
- HTTP method (GET, POST, etc.)
- Full endpoint URL
- Expected type or operation
- Original error message
- Raw response/request body

### For RPC Errors:
- RPC method name
- Parameters passed
- Expected return type
- Original error message
- Raw response

### For File/Storage Errors:
- File path
- Operation type (read/write/delete)
- Original error message

### For Validation Errors:
- Field name
- Expected value/type
- Actual value/type
- Validation rule that failed

---

## Implementation Priority

If implementing all enhancements, the recommended order would be:

1. **`AmpError::Rpc`** - Most impactful for debugging Elements node issues
2. **`Error::RequestFailed`** - Complements the enhanced ResponseDeserializationFailed
3. **`AmpError::Serialization`** - Follows same pattern as ResponseDeserializationFailed
4. **`SignerError::InvalidTransaction`** - Critical for transaction debugging
5. Others as needed based on actual error frequency in production

---

## Current State Summary

- ✅ **Well-designed:** 18 variants (67%)
- ⚠️ **Could be enhanced:** 9 variants (33%)
- 🎯 **Recently enhanced:** 1 variant (`ResponseDeserializationFailed`)

The crate already has good error handling practices. The enhancements would primarily add more diagnostic context for debugging, following the pattern established by `ResponseDeserializationFailed`.
