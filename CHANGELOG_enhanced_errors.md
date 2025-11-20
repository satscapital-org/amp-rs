# Enhanced Error Reporting - Changelog Entry

## Added

### Comprehensive Deserialization Error Diagnostics

- **New Error Variant**: Added `Error::ResponseDeserializationFailed` with detailed context fields:
  - `method`: HTTP method (GET, POST, etc.)
  - `endpoint`: Full URL that was called
  - `expected_type`: Rust type name that was expected
  - `serde_error`: Original serde deserialization error
  - `raw_response`: Complete raw response body text

- **Enhanced `request_json()` Method**: 
  - Now captures full request context before making API calls
  - Reads response body as text to preserve for error reporting
  - Provides comprehensive diagnostics when deserialization fails

- **Documentation**: Added `docs/enhanced-error-reporting.md` with:
  - Overview of the feature
  - Example error output
  - Benefits and use cases
  - Testing instructions

- **Example Code**: Added `examples/test_enhanced_errors.rs` demonstrating the enhanced error format

- **Tests**: Added `tests/enhanced_error_test.rs` with comprehensive test coverage:
  - Verification that all error fields are populated
  - Testing of Display and Debug formatting
  - Validation of error message structure

## Changed

- Modified `src/client.rs`:
  - Updated `Error` enum with new `ResponseDeserializationFailed` variant
  - Refactored `request_json()` to capture and report detailed error context
  - All API methods now benefit from enhanced error reporting

## Benefits

1. **Faster Debugging**: Developers can immediately see malformed response data
2. **Better Diagnostics**: Full context helps identify which API call failed and why
3. **Downstream Flexibility**: Applications can decide how to handle/log errors
4. **API Issue Detection**: Helps identify schema changes, missing fields, and type mismatches

## Migration Notes

- Existing code continues to work without changes
- Error handling code can optionally be updated to take advantage of the new detailed error information
- The `ResponseParsingFailed` variant is still available for backwards compatibility
