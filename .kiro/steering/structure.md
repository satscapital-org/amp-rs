# Project Structure

## Root Level
- `Cargo.toml`: Package configuration and dependencies
- `Cargo.lock`: Dependency lock file
- `README.md`: Main documentation with usage examples
- `WARP.md`: Development guidance for WARP IDE
- `.env`: Environment variables (contains test credentials)
- `.gitignore`: Git ignore patterns

## Source Code (`src/`)
- `lib.rs`: Library entry point, re-exports main types
- `client.rs`: Main API client implementation with authentication
- `model.rs`: Data models and request/response structures
- `mocks.rs`: Mock implementations for testing

## Examples (`examples/`)
- `changelog.rs`: Example demonstrating API usage

## Tests (`tests/`)
- `api.rs`: Integration tests for API functionality

## Key Architectural Decisions

### Module Organization
- **client**: Contains the main `ApiClient` struct and HTTP logic
- **model**: All data structures, requests, and responses
- **mocks**: Testing utilities and mock implementations

### Naming Conventions
- Library name: `amp_rs` (internal), `amp-rs` (package name)
- Public API uses `amp_client` namespace
- Struct names use PascalCase (e.g., `ApiClient`, `TokenRequest`)
- Function names use snake_case (e.g., `get_registered_users`)

### Error Handling
- Custom `Error` enum in client module
- All public functions return `Result<T, Error>`
- Comprehensive error variants for different failure modes

### Testing Strategy
- Mock tests by default (no external dependencies)
- Live tests when `AMP_TESTS=live` environment variable is set
- State-changing tests marked with `#[ignore]` attribute
- Credentials required via environment variables for live tests