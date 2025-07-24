#!/usr/bin/env bash
# Detailed coverage script with multiple output formats and options

set -e

# Default values
OUTPUT_FORMAT="all"
OPEN_REPORT=true
MIN_COVERAGE=80

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --format)
            OUTPUT_FORMAT="$2"
            shift 2
            ;;
        --no-open)
            OPEN_REPORT=false
            shift
            ;;
        --min-coverage)
            MIN_COVERAGE="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --format FORMAT      Output format: html, lcov, json, cobertura, all (default: all)"
            echo "  --no-open           Don't open HTML report in browser"
            echo "  --min-coverage PCT  Minimum coverage percentage required (default: 80)"
            echo "  --help              Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "Running detailed test coverage for Claudius..."
echo "============================================="
echo "Minimum coverage threshold: $MIN_COVERAGE%"
echo ""

# Clean previous coverage data
cargo llvm-cov clean --workspace

# Function to run coverage with specific format
run_coverage() {
    local format=$1
    local output_file=$2
    local extra_args=$3

    echo "Generating $format report..."
    # shellcheck disable=SC2086  # $extra_args is meant to be word-split
    CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov \
        --all-features \
        --workspace \
        $extra_args \
        --output-path "$output_file" || {
            echo "Failed to generate $format report"
            return 1
        }
    echo "✓ $format report generated at: $output_file"
}

# Generate reports based on requested format
case $OUTPUT_FORMAT in
    html)
        CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov --all-features --workspace --html
        echo "✓ HTML report generated at: target/llvm-cov/html/index.html"
        ;;
    lcov)
        run_coverage "LCOV" "lcov.info" "--lcov"
        ;;
    json)
        run_coverage "JSON" "coverage.json" "--json"
        ;;
    cobertura)
        run_coverage "Cobertura XML" "cobertura.xml" "--cobertura"
        ;;
    all)
        # Generate all formats
        CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov --all-features --workspace --html
        echo "✓ HTML report generated at: target/llvm-cov/html/index.html"

        run_coverage "LCOV" "lcov.info" "--lcov"
        run_coverage "JSON" "coverage.json" "--json"
        run_coverage "Cobertura XML" "cobertura.xml" "--cobertura"
        ;;
    *)
        echo "Unknown format: $OUTPUT_FORMAT"
        exit 1
        ;;
esac

# Generate summary and check coverage threshold
echo ""
echo "Coverage Summary:"
echo "================="
COVERAGE_OUTPUT=$(CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov --all-features --workspace --summary-only 2>&1)
echo "$COVERAGE_OUTPUT"

# Extract coverage percentage
COVERAGE_PCT=$(echo "$COVERAGE_OUTPUT" | grep -oE '[0-9]+\.[0-9]+%' | head -1 | sed 's/%//')

if [ -n "$COVERAGE_PCT" ]; then
    echo ""
    echo "Total coverage: $COVERAGE_PCT%"

    # Check if coverage meets minimum threshold
    if (( $(echo "$COVERAGE_PCT >= $MIN_COVERAGE" | bc -l) )); then
        echo "✓ Coverage meets minimum threshold ($MIN_COVERAGE%)"
    else
        echo "✗ Coverage is below minimum threshold ($MIN_COVERAGE%)"
        exit 1
    fi
fi

# Open HTML report if requested and available
if [ "$OPEN_REPORT" = true ] && [ -f "target/llvm-cov/html/index.html" ]; then
    if command -v xdg-open &> /dev/null; then
        echo ""
        echo "Opening HTML report in browser..."
        xdg-open target/llvm-cov/html/index.html
    elif command -v open &> /dev/null; then
        echo ""
        echo "Opening HTML report in browser..."
        open target/llvm-cov/html/index.html
    fi
fi
