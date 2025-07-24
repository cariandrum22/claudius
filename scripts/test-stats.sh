#!/usr/bin/env bash
# Simple test statistics script that works with any Rust version

set -e

echo "Running Claudius Test Statistics"
echo "================================"
echo ""

# Count test files
echo "Test Files:"
echo "-----------"
TEST_FILES=$(find tests -name "*.rs" -type f | sort)
UNIT_TEST_COUNT=$(echo "$TEST_FILES" | grep -c "tests/unit/" || true)
INTEGRATION_TEST_COUNT=$(echo "$TEST_FILES" | grep -c "tests/integration/" || true)

echo "Unit test files:        $UNIT_TEST_COUNT"
echo "Integration test files: $INTEGRATION_TEST_COUNT"
echo ""

# Count test functions (rough estimate)
echo "Test Functions (estimate):"
echo "-------------------------"
UNIT_TESTS=$(grep -r "#\[test\]" tests/unit/ 2>/dev/null | wc -l || echo "0")
INTEGRATION_TESTS=$(grep -r "#\[test\]" tests/integration/ 2>/dev/null | wc -l || echo "0")
TOTAL_TESTS=$((UNIT_TESTS + INTEGRATION_TESTS))

echo "Unit tests:        $UNIT_TESTS"
echo "Integration tests: $INTEGRATION_TESTS"
echo "Total tests:       $TOTAL_TESTS"
echo ""

# Count source files and lines
echo "Source Code Statistics:"
echo "----------------------"
SOURCE_FILES=$(find src -name "*.rs" -type f | wc -l)
SOURCE_LINES=$(find src -name "*.rs" -type f -exec cat {} \; | wc -l)
TEST_LINES=$(find tests -name "*.rs" -type f -exec cat {} \; | wc -l)

echo "Source files:      $SOURCE_FILES"
echo "Source lines:      $SOURCE_LINES"
echo "Test lines:        $TEST_LINES"
# Calculate ratio (handle missing bc)
if command -v bc &> /dev/null; then
    RATIO=$(echo "scale=2; $TEST_LINES * 100 / $SOURCE_LINES" | bc)
    echo "Test/Source ratio: ${RATIO}%"
else
    RATIO=$((TEST_LINES * 100 / SOURCE_LINES))
    echo "Test/Source ratio: ${RATIO}%"
fi
echo ""

# Run tests with JSON output for detailed stats
echo "Running Tests:"
echo "-------------"
TEMP_FILE=$(mktemp)

# First check if tests compile
if CLAUDIUS_TEST_MOCK_OP=1 cargo test --no-run > /dev/null 2>&1; then
    # Run tests and capture output
    if CLAUDIUS_TEST_MOCK_OP=1 cargo test -- --show-output 2>&1 | tee "$TEMP_FILE"; then
        echo ""
        echo "Test Summary:"
        echo "------------"
        # Extract test results
        PASSED=$(grep -E "test result: ok" "$TEMP_FILE" | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+" || echo "0")
        FAILED=$(grep -E "test result: FAILED" "$TEMP_FILE" | grep -oE "[0-9]+ failed" | grep -oE "[0-9]+" || echo "0")
        IGNORED=$(grep -E "ignored" "$TEMP_FILE" | grep -oE "[0-9]+ ignored" | grep -oE "[0-9]+" || echo "0")

        echo "✓ Passed:  $PASSED"
        [ "$FAILED" -gt 0 ] && echo "✗ Failed:  $FAILED"
        [ "$IGNORED" -gt 0 ] && echo "⚠ Ignored: $IGNORED"

        if [ "$FAILED" -eq 0 ]; then
            echo ""
            echo "✓ All tests passed!"
        else
            echo ""
            echo "✗ Some tests failed!"
            exit 1
        fi
    else
        echo "✗ Test execution failed!"
        exit 1
    fi
else
    echo "✗ Tests failed to compile!"
    echo "This might be due to dependency version conflicts"
    echo "See README_COVERAGE.md for solutions."
    exit 1
fi

rm -f "$TEMP_FILE"

echo ""
echo "For detailed coverage analysis, see README_COVERAGE.md"
