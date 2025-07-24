#!/usr/bin/env bash
# Script to run test coverage for Claudius

set -e

echo "Running test coverage for Claudius..."
echo "====================================="

# Clean previous coverage data
cargo llvm-cov clean --workspace

# Run tests with coverage
echo "Running tests with coverage..."
CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov \
    --all-features \
    --workspace \
    --lcov \
    --output-path lcov.info

# Generate HTML report
echo "Generating HTML coverage report..."
CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov \
    --all-features \
    --workspace \
    --html

# Generate summary report
echo ""
echo "Coverage Summary:"
echo "================="
CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov \
    --all-features \
    --workspace \
    --summary-only

echo ""
echo "HTML report generated at: target/llvm-cov/html/index.html"
echo "LCOV report generated at: lcov.info"

# Open the HTML report if possible
if command -v xdg-open &> /dev/null; then
    echo "Opening HTML report in browser..."
    xdg-open target/llvm-cov/html/index.html
elif command -v open &> /dev/null; then
    echo "Opening HTML report in browser..."
    open target/llvm-cov/html/index.html
fi
