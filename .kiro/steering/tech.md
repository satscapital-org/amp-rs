# Technology Stack

## Build System
- **Cargo**: Standard Rust package manager and build tool
- **Edition**: Rust 2021

## Core Dependencies
- **reqwest**: HTTP client with JSON support
- **serde**: Serialization/deserialization framework
- **tokio**: Async runtime with full features
- **chrono**: Date and time handling with serde support
- **thiserror**: Error handling and custom error types
- **secrecy**: Secure credential handling
- **zeroize**: Memory zeroing for security

## Development Dependencies
- **httpmock**: HTTP mocking for tests

## Common Commands

### Building
```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Check compilation without building
```

### Testing
```bash
cargo test                                                    # Run mocked tests only
AMP_USERNAME=... AMP_PASSWORD=... AMP_TESTS=live cargo test  # Run live API tests
cargo test -- --ignored                                      # Run state-changing tests
```

### Code Quality
```bash
cargo fmt                    # Format code
cargo clippy -- -D warnings # Run linter with warnings as errors
```

### Examples
```bash
cargo run --example changelog  # Run changelog example
```

## Environment Variables
- `AMP_USERNAME`: Username for AMP API authentication
- `AMP_PASSWORD`: Password for AMP API authentication  
- `AMP_TESTS`: Set to "live" to run tests against actual API

## Architecture Patterns
- Async/await throughout with tokio
- Error handling via custom Error enum with thiserror
- Singleton pattern for token management using OnceCell
- Builder pattern for request structures
- Secure credential handling with Secret types