# Product Overview

AMP Client is a Rust client library for the Blockstream AMP (Asset Management Platform) API. It provides a comprehensive interface for interacting with AMP endpoints including:

- Asset issuance and management
- User registration and authentication
- Category operations
- Balance and ownership tracking

The library is designed to be async-first using tokio and provides both mocked and live testing capabilities. It handles authentication via JWT tokens with automatic token refresh and includes proper error handling throughout.

## Key Features

- Full async/await support with tokio
- Automatic JWT token management
- Comprehensive error handling with custom error types
- Mock testing support for development
- Live API testing capabilities
- Secure credential handling with the `secrecy` crate