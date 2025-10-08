#!/bin/bash

echo "🔐 AMP Client Token Persistence Demo"
echo "===================================="
echo

# Check if token.json exists
if [ -f "token.json" ]; then
    echo "📄 Existing token.json found:"
    echo "$(cat token.json | jq '.' 2>/dev/null || cat token.json)"
    echo
    echo "🗑️  Removing existing token file for clean demo..."
    rm token.json
    echo
fi

echo "🚀 Running token persistence example..."
echo "Note: This requires valid AMP_USERNAME and AMP_PASSWORD environment variables"
echo

# Set token persistence environment variable
export AMP_TOKEN_PERSISTENCE=true

# Run the example (this will fail without valid credentials, but shows the structure)
cargo run --example token_persistence 2>/dev/null || echo "⚠️  Example requires valid AMP credentials to run fully"

echo
if [ -f "token.json" ]; then
    echo "✅ Token file created successfully!"
    echo "📄 Contents of token.json:"
    cat token.json | jq '.' 2>/dev/null || cat token.json
else
    echo "ℹ️  No token file created (requires valid credentials)"
fi

echo
echo "🧪 Running token persistence tests..."
cargo test --test token_persistence --quiet

echo
echo "✅ Token persistence demo completed!"
echo
echo "Key features demonstrated:"
echo "• Automatic token persistence to token.json"
echo "• Secure token storage with expiration tracking"
echo "• Thread-safe token management"
echo "• Automatic token refresh before expiry"
echo "• Environment-based configuration"
