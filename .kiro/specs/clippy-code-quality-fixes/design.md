# Design Document

## Overview

This design addresses the systematic resolution of clippy warnings and errors identified in the AMP Rust client library. The approach focuses on maintaining code functionality while improving code quality, readability, and adherence to Rust best practices. All changes will be made incrementally with test validation at each step.

## Architecture

The fixes are organized into logical groups based on the type of issue:

1. **Literal and Format Improvements**: Numeric literal formatting and format string modernization
2. **Complexity Reduction**: Breaking down high-complexity functions into smaller, manageable pieces
3. **Documentation Enhancement**: Improving API documentation with proper formatting
4. **Type System Optimization**: Using `Self` and proper type annotations
5. **Performance Optimization**: Eliminating redundant operations and unnecessary async
6. **API Safety**: Adding proper error documentation and must-use annotations

## Components and Interfaces

### Affected Files
- `src/client.rs`: Main API client with authentication and token management
- `src/mocks.rs`: Mock implementations for testing

### Key Areas of Change

#### 1. TokenManager Complexity Reduction
The `TokenManager` contains several high-complexity functions that need refactoring:
- `detect()` (complexity 38/25)
- `create_strategy()` (complexity 47/25) 
- `clear_token()` (complexity 34/25)
- `load_token_from_disk()` (complexity 43/25)

**Refactoring Strategy**: Extract logical blocks into private helper methods while maintaining the same public API.

#### 2. Documentation Standardization
Multiple documentation comments need backtick formatting for:
- Type names (`TokenManager`, `ApiClient`)
- Function names (`get_global_instance()`)
- Code elements in descriptions

#### 3. Format String Modernization
Update format! macros to use inline variable syntax:
- `format!("mock_token_{}", test_name)` → `format!("mock_token_{test_name}")`
- Multi-line format strings with variables

#### 4. Performance Optimizations
- Remove redundant clones where ownership can be transferred
- Replace redundant closures with method references
- Remove unnecessary `async` keywords from synchronous functions

## Data Models

No changes to existing data models are required. All modifications preserve existing interfaces and data structures.

## Error Handling

### Enhanced Error Documentation
Functions returning `Result` types will receive comprehensive error documentation:
- `force_cleanup_token_files()`: Document file system errors
- `reset_global_instance()`: Document initialization errors

### Error Preservation
All existing error handling patterns and error types remain unchanged to maintain API compatibility.

## Testing Strategy

### Test Preservation Approach
1. **Incremental Validation**: Run tests after each logical group of changes
2. **Regression Prevention**: Ensure all existing tests continue to pass
3. **Mock Test Validation**: Verify mock implementations remain functional
4. **Live Test Compatibility**: Ensure live API tests still work with credentials

### Test Categories to Validate
- **Unit Tests**: Individual function behavior
- **Integration Tests**: API client functionality
- **Mock Tests**: Isolated testing without external dependencies
- **Live Tests**: Actual API interaction (when credentials available)

### Validation Commands
```bash
cargo test                    # Run all tests
cargo clippy --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms  # Verify fixes
cargo build                   # Ensure compilation
```

## Implementation Phases

### Phase 1: Simple Fixes
- Numeric literal separators
- Format string inlining
- Single character patterns
- Redundant closure elimination

### Phase 2: Documentation Enhancement
- Add backticks to code identifiers
- Add error documentation sections
- Improve API documentation clarity

### Phase 3: Type System Improvements
- Replace explicit types with `Self`
- Add `#[must_use]` annotations
- Remove redundant clones

### Phase 4: Complexity Reduction
- Refactor high-complexity functions
- Extract helper methods
- Maintain public API compatibility

### Phase 5: Async Optimization
- Remove unnecessary `async` keywords
- Ensure function signatures match their implementation

## Risk Mitigation

### API Compatibility
All changes maintain existing public APIs to prevent breaking changes for library users.

### Test Coverage
Comprehensive test validation after each phase ensures no regressions are introduced.

### Incremental Approach
Small, focused changes reduce the risk of introducing bugs and make issues easier to identify and fix.

## Success Criteria

1. All clippy warnings and errors are resolved
2. All existing tests continue to pass
3. Code compilation succeeds without warnings
4. API functionality remains unchanged
5. Code readability and maintainability are improved