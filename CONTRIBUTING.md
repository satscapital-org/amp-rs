# Contributing

When contributing to this project, please follow these guidelines.

## API Specification

This client is based on the Blockstream AMP API Specification, which can be found at:
https://docs.liquid.net/docs/blockstream-amp-api-specification

## Documentation

Please ensure that you update the documentation as you add new functionality. This includes both the code documentation (doc comments) and any relevant changes to the README or other documentation files.

## Testing

This crate uses a combination of live and mocked tests to ensure correctness.

### Live vs. Mocked Tests

For every test that interacts with the AMP API, there should be two versions:

-   **A live test**, with the suffix `_live` (e.g., `test_get_changelog_live`). This test hits the actual AMP API and is intended to catch any breaking changes in the API.
-   **A mocked test**, with the suffix `_mock` (e.g., `test_get_changelog_mock`). This test uses a mock server to simulate the API and is intended to test the client's logic in isolation.

Live tests are skipped by default. To run them, you must set the `AMP_TESTS` environment variable to `live`:

```bash
AMP_TESTS=live cargo test
```

You will also need to set the `AMP_USERNAME` and `AMP_PASSWORD` environment variables with valid credentials to run the live tests.

### Adding New Tests

When adding a new test for an API endpoint, please add both a `_live` and a `_mock` version.

1.  **Create a mock function** in `src/mocks.rs`. This function should take a `&MockServer` and set up the expected response for the endpoint you are testing.
2.  **Create the `_mock` test** in `tests/api.rs`. This test should:
    -   Set dummy `AMP_USERNAME` and `AMP_PASSWORD` environment variables.
    -   Start a `MockServer`.
    -   Call the `mock_obtain_token` function from `src/mocks.rs`.
    -   Call your new mock function.
    -   Create an `ApiClient` using `with_base_url`, pointing to the mock server.
    -   Call the client method and assert the result.
3.  **Create the `_live` test** in `tests/api.rs`. This test should be a copy of the original test, with the `_live` suffix and the check to skip the test if `AMP_TESTS` is not set to `live`.
