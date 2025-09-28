#!/bin/bash
# Setup development tools for git-perf project

set -euo pipefail

echo "Installing development tools for git-perf..."

# Install cargo-nextest for testing
if ! command -v cargo-nextest &> /dev/null; then
    echo "Installing cargo-nextest..."
    cargo install --locked cargo-nextest@0.9
else
    echo "cargo-nextest is already installed"
fi

echo "Development tools setup complete!"
echo "You can now run tests with: cargo nextest run -- --skip slow"