#!/usr/bin/env bash
# Format all Rust code in the project

set -e

echo "Formatting Claudius codebase..."
echo "==============================="
echo ""

# Run rustfmt
if ! command -v rustfmt &> /dev/null; then
    echo "Error: rustfmt not found. Installing..."
    rustup component add rustfmt
fi

echo "Running rustfmt..."
cargo fmt --all

echo "✓ Code formatting completed!"
echo ""
echo "Changes made:"
git diff --stat --color 2>/dev/null || echo "No changes needed - code was already properly formatted!"
