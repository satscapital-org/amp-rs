# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Removed the `mocks` feature flag. Mock server support is now always available as part of the standard package.
  - `httpmock` is now a regular dependency instead of an optional one
  - The `mocks` module is always compiled and available
  - Tests no longer require `--features mocks` to use mock functionality

## [0.1.0] - Initial Release

### Added
- Initial release of the AMP Rust client
- Support for all major AMP API endpoints
- Mock server functionality for testing
